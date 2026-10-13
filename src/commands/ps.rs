use crate::cli::Format;
use crate::utils::output;
use crate::utils::Result;
use chrono::{DateTime, Local};
use serde::Serialize;
use std::collections::HashSet;
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

/// Kill/enumerate paths must not read PEB/exe/cmdline/environ of every
/// process: on Windows that opens handles and walks memory of processes that
/// may be suspended or protected, which can block for a long time. We only
/// need pid/parent/name, and those come from the fast Toolhelp snapshot.
fn light_refresh(sys: &mut System, which: ProcessesToUpdate<'_>) {
    sys.refresh_processes_specifics(which, true, ProcessRefreshKind::nothing());
}

#[derive(Serialize)]
struct Proc {
    pid: u32,
    name: String,
    exe: Option<String>,
    memory_bytes: u64,
    started: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cmdline: Option<String>,
}

pub fn ps(
    name: Option<&str>,
    pid: Option<u32>,
    cmd: bool,
    limit: Option<usize>,
    format: Format,
) -> Result<()> {
    let mut sys = System::new();
    // Mirror `refresh_processes` (memory/cpu/disk/exe), plus cmdline when
    // --cmd is passed: sysinfo never fills `cmd` unless explicitly asked.
    let kind = ProcessRefreshKind::nothing()
        .with_memory()
        .with_cpu()
        .with_disk_usage()
        .with_exe(UpdateKind::OnlyIfNotSet);
    let kind = if cmd {
        kind.with_cmd(UpdateKind::Always)
    } else {
        kind
    };
    sys.refresh_processes_specifics(ProcessesToUpdate::All, true, kind);
    let needle = name.map(|n| n.to_lowercase());
    let mut procs: Vec<Proc> = sys
        .processes()
        .values()
        .filter(|p| {
            if let Some(pid) = pid {
                return p.pid().as_u32() == pid;
            }
            let Some(n) = &needle else { return true };
            if p.name().to_string_lossy().to_lowercase().contains(n) {
                return true;
            }
            // With --cmd, also match against the command line so agents can
            // find "which process runs this script/jar" in one call.
            cmd && p
                .cmd()
                .iter()
                .any(|a| a.to_string_lossy().to_lowercase().contains(n))
        })
        .map(|p| Proc {
            pid: p.pid().as_u32(),
            name: p.name().to_string_lossy().into_owned(),
            exe: p.exe().map(|x| x.display().to_string()),
            memory_bytes: p.memory(),
            started: fmt_started(p.start_time()),
            cmdline: if cmd {
                let c = p
                    .cmd()
                    .iter()
                    .map(|s| s.to_string_lossy())
                    .collect::<Vec<_>>()
                    .join(" ");
                if c.is_empty() {
                    None
                } else {
                    Some(c)
                }
            } else {
                None
            },
        })
        .collect();
    procs.sort_by(|a, b| b.memory_bytes.cmp(&a.memory_bytes));

    let total = procs.len();
    let truncated = match limit {
        Some(n) if n < total => {
            procs.truncate(n);
            true
        }
        _ => false,
    };

    match format {
        Format::Json => output::print_json(&procs),
        Format::Plain => {
            println!("{:>7}  {:>9}  {:<30}  {}", "PID", "MEM", "NAME", "EXE");
            for p in &procs {
                println!(
                    "{:>7}  {:>9}  {:<30}  {}",
                    p.pid,
                    human_mb(p.memory_bytes),
                    truncate(&p.name, 30),
                    p.exe.as_deref().unwrap_or("-")
                );
                if cmd {
                    if let Some(c) = &p.cmdline {
                        println!("         cmd: {c}");
                    }
                }
            }
            if truncated {
                println!("... and {} more (of {total} total)", total - procs.len());
            }
        }
    }
    Ok(())
}

#[derive(Serialize)]
pub struct KillOutcome {
    pub pid: u32,
    pub name: String,
    pub killed: bool,
    /// Why the kill failed (e.g. access denied for an elevated process).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    pub tree_killed: Vec<u32>,
    pub tree_failed: Vec<u32>,
}

/// Refuse to touch anything that could take down the OS or ourselves:
/// the idle/system PIDs, this process, and any ancestor of this process
/// (killing those would take the calling shell/agent down with us).
/// The visited set guards against parent-pointer cycles caused by PID reuse
/// (a stale parent PID pointing at a descendant loops forever otherwise).
fn is_protected(sys: &System, pid: Pid) -> bool {
    let raw = pid.as_u32();
    if raw == 0 || raw == 4 {
        return true;
    }
    let mut visited = HashSet::new();
    let mut cur = sys.process(Pid::from_u32(std::process::id()));
    while let Some(p) = cur {
        if p.pid() == pid {
            return true;
        }
        if !visited.insert(p.pid()) {
            break;
        }
        cur = p.parent().and_then(|pp| sys.process(pp));
    }
    false
}

/// Descendants of `root`, post-order (deepest children first). The visited
/// set guards against parent-pointer cycles caused by PID reuse.
fn descendants(sys: &System, root: Pid) -> Vec<Pid> {
    fn visit(sys: &System, pid: Pid, order: &mut Vec<Pid>, visited: &mut HashSet<Pid>) {
        for (p, proc_) in sys.processes() {
            if proc_.parent() == Some(pid) && visited.insert(*p) {
                visit(sys, *p, order, visited);
                order.push(*p);
            }
        }
    }
    let mut order = Vec::new();
    let mut visited = HashSet::from([root]);
    visit(sys, root, &mut order, &mut visited);
    order
}

/// Hard cap so a bad PID can never snowball into killing half the machine.
const MAX_TREE: usize = 64;

/// Deliver a kill signal via the OS. Unlike sysinfo's boolean `kill()`,
/// this returns an actionable error message (e.g. access denied because
/// the target was started from an elevated terminal).
#[cfg(windows)]
fn native_kill(pid: Pid) -> std::result::Result<(), String> {
    use windows_sys::Win32::Foundation::{CloseHandle, GetLastError};
    use windows_sys::Win32::System::Threading::{
        OpenProcess, TerminateProcess, PROCESS_TERMINATE,
    };
    const ERROR_ACCESS_DENIED: u32 = 5;
    const ERROR_INVALID_PARAMETER: u32 = 87;
    let explain = |e: u32| match e {
        ERROR_ACCESS_DENIED => {
            "access denied: the process was likely started with administrator rights; re-run from an elevated terminal or use taskkill /F in one".to_string()
        }
        ERROR_INVALID_PARAMETER => "process does not exist".to_string(),
        _ => format!("os error {e}"),
    };
    unsafe {
        let handle = OpenProcess(PROCESS_TERMINATE, 0, pid.as_u32());
        if handle.is_null() {
            return Err(format!("OpenProcess: {}", explain(GetLastError())));
        }
        let ok = TerminateProcess(handle, 1);
        let err = if ok == 0 { Some(GetLastError()) } else { None };
        CloseHandle(handle);
        match err {
            None => Ok(()),
            Some(e) => Err(format!("TerminateProcess: {}", explain(e))),
        }
    }
}

#[cfg(not(windows))]
fn native_kill(sys: &mut System, pid: Pid) -> std::result::Result<(), String> {
    match sys.process(pid) {
        None => Ok(()),
        Some(p) if p.kill() => Ok(()),
        Some(_) => Err("kill signal not delivered (check permissions)".to_string()),
    }
}

/// Kill one process; "already exited" counts as success (a wrapper like
/// `cmd /c` often exits on its own the moment its child dies). Err carries
/// a human-readable reason when the process survives the kill attempt.
fn kill_one(sys: &mut System, pid: Pid) -> std::result::Result<(), String> {
    if sys.process(pid).is_none() {
        return Ok(());
    }
    #[cfg(windows)]
    let attempt = native_kill(pid);
    #[cfg(not(windows))]
    let attempt = native_kill(sys, pid);
    match attempt {
        Ok(()) => Ok(()),
        Err(reason) => {
            light_refresh(sys, ProcessesToUpdate::Some(&[pid]));
            if sys.process(pid).is_none() {
                Ok(())
            } else {
                Err(reason)
            }
        }
    }
}

pub fn kill_impl(pid: u32, tree: bool) -> Result<KillOutcome> {
    // Enumeration can stall on a machine with processes in a bad state; run
    // the work on a watchdog thread and give up with a clear error instead of
    // hanging forever (the stuck thread dies when the process exits).
    let (tx, rx) = std::sync::mpsc::channel::<std::result::Result<KillOutcome, String>>();
    std::thread::spawn(move || {
        let _ = tx.send(kill_impl_inner(pid, tree).map_err(|e| e.to_string()));
    });
    match rx.recv_timeout(std::time::Duration::from_secs(20)) {
        Ok(r) => r.map_err(Into::into),
        Err(_) => Err(
            "process enumeration timed out after 20s (system busy or a process is in a bad state)"
                .into(),
        ),
    }
}

fn kill_impl_inner(pid: u32, tree: bool) -> Result<KillOutcome> {
    let mut sys = System::new();
    light_refresh(&mut sys, ProcessesToUpdate::All);
    let target = Pid::from_u32(pid);
    if is_protected(&sys, target) {
        return Err(format!(
            "refusing to kill pid {pid}: it is a system process or an ancestor of this process"
        )
        .into());
    }
    let process = sys
        .process(target)
        .ok_or_else(|| format!("no process with pid {pid}"))?;
    let name = process.name().to_string_lossy().into_owned();

    // Parent first: wrapper processes (cmd/npm) may exit when their child
    // dies, so kill the target while it is definitely alive, then sweep
    // the (precomputed) descendants.
    let (killed, reason) = match kill_one(&mut sys, target) {
        Ok(()) => (true, None),
        Err(e) => (false, Some(e)),
    };

    let mut tree_killed = Vec::new();
    let mut tree_failed = Vec::new();
    if tree {
        let desc = descendants(&sys, target);
        if desc.len() > MAX_TREE {
            return Err(format!(
                "refusing to kill pid {pid}: its process tree has {} descendants (limit {MAX_TREE}); inspect with `codex-kit ps` first",
                desc.len()
            )
            .into());
        }
        for d in desc {
            if is_protected(&sys, d) {
                tree_failed.push(d.as_u32());
                continue;
            }
            match kill_one(&mut sys, d) {
                Ok(()) => tree_killed.push(d.as_u32()),
                Err(_) => tree_failed.push(d.as_u32()),
            }
        }
    }

    Ok(KillOutcome {
        pid,
        name,
        killed,
        reason,
        tree_killed,
        tree_failed,
    })
}

pub fn kill(pid: u32, tree: bool, format: Format) -> Result<()> {
    let result = kill_impl(pid, tree)?;
    match format {
        Format::Json => output::print_json(&result),
        Format::Plain => {
            if result.killed {
                println!("killed {} (pid {pid})", result.name);
            } else {
                let why = result.reason.as_deref().unwrap_or("unknown error");
                println!("failed to kill {} (pid {pid}): {why}", result.name);
            }
            if !result.tree_killed.is_empty() {
                println!(
                    "also killed {} descendant(s): {:?}",
                    result.tree_killed.len(),
                    result.tree_killed
                );
            }
            if !result.tree_failed.is_empty() {
                println!("failed to kill descendant(s): {:?}", result.tree_failed);
            }
        }
    }
    if result.killed && result.tree_failed.is_empty() {
        Ok(())
    } else {
        let why = result
            .reason
            .as_deref()
            .map(|r| format!(": {r}"))
            .unwrap_or_default();
        Err(format!("kill not fully successful for pid {pid}{why}").into())
    }
}

fn fmt_started(secs: u64) -> Option<String> {
    if secs == 0 {
        return None;
    }
    let dt: DateTime<Local> = DateTime::from_timestamp(secs as i64, 0)?.into();
    Some(dt.format("%Y-%m-%d %H:%M:%S").to_string())
}

fn human_mb(bytes: u64) -> String {
    format!("{:.0}M", bytes as f64 / 1024.0 / 1024.0)
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        format!("{s:<max$}")
    } else {
        let cut: String = s.chars().take(max - 1).collect();
        format!("{cut}…")
    }
}
