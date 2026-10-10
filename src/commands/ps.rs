use crate::cli::Format;
use crate::utils::output;
use crate::utils::Result;
use chrono::{DateTime, Local};
use serde::Serialize;
use std::collections::HashSet;
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};

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
}

pub fn ps(name: Option<&str>, limit: Option<usize>, format: Format) -> Result<()> {
    let mut sys = System::new();
    sys.refresh_processes(ProcessesToUpdate::All, true);
    let needle = name.map(|n| n.to_lowercase());
    let mut procs: Vec<Proc> = sys
        .processes()
        .values()
        .filter(|p| {
            needle
                .as_ref()
                .map(|n| p.name().to_string_lossy().to_lowercase().contains(n))
                .unwrap_or(true)
        })
        .map(|p| Proc {
            pid: p.pid().as_u32(),
            name: p.name().to_string_lossy().into_owned(),
            exe: p.exe().map(|x| x.display().to_string()),
            memory_bytes: p.memory(),
            started: fmt_started(p.start_time()),
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

/// Kill one process; "already exited" counts as success (a wrapper like
/// `cmd /c` often exits on its own the moment its child dies).
fn kill_one(sys: &mut System, pid: Pid) -> bool {
    if let Some(p) = sys.process(pid) {
        if p.kill() {
            return true;
        }
    }
    light_refresh(sys, ProcessesToUpdate::Some(&[pid]));
    sys.process(pid).is_none()
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
    let killed = kill_one(&mut sys, target);

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
            if kill_one(&mut sys, d) {
                tree_killed.push(d.as_u32());
            } else {
                tree_failed.push(d.as_u32());
            }
        }
    }

    Ok(KillOutcome {
        pid,
        name,
        killed,
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
                println!("failed to kill {} (pid {pid})", result.name);
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
        Err(format!("kill not fully successful for pid {pid}").into())
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
