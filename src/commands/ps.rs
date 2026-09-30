use crate::cli::Format;
use crate::utils::output;
use crate::utils::Result;
use chrono::{DateTime, Local};
use serde::Serialize;
use sysinfo::{Pid, ProcessesToUpdate, System};

#[derive(Serialize)]
struct Proc {
    pid: u32,
    name: String,
    exe: Option<String>,
    memory_bytes: u64,
    started: Option<String>,
}

pub fn ps(name: Option<&str>, format: Format) -> Result<()> {
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
        }
    }
    Ok(())
}

pub fn kill(pid: u32, format: Format) -> Result<()> {
    let mut sys = System::new();
    sys.refresh_processes(ProcessesToUpdate::All, true);
    let process = sys
        .process(Pid::from_u32(pid))
        .ok_or_else(|| format!("no process with pid {pid}"))?;
    let name = process.name().to_string_lossy().into_owned();
    let killed = process.kill();

    #[derive(Serialize)]
    struct KillResult {
        pid: u32,
        name: String,
        killed: bool,
    }
    let result = KillResult { pid, name, killed };
    match format {
        Format::Json => output::print_json(&result),
        Format::Plain => {
            if killed {
                println!("killed {} (pid {pid})", result.name);
            } else {
                println!("failed to kill {} (pid {pid})", result.name);
            }
        }
    }
    if killed {
        Ok(())
    } else {
        Err(format!("kill signal failed for pid {pid}").into())
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
