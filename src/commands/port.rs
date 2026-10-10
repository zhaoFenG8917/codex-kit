use crate::cli::Format;
use crate::utils::output;
use crate::utils::Result;
use serde::Serialize;
use std::net::{IpAddr, SocketAddr, TcpStream, ToSocketAddrs};
use std::process::Command;
use std::time::{Duration, Instant};
use sysinfo::{Pid, ProcessesToUpdate, System};

#[derive(Serialize)]
struct PortResult {
    host: String,
    port: u16,
    listening: bool,
    latency_ms: Option<u128>,
    pid: Option<u32>,
    process_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    kill: Option<crate::commands::ps::KillOutcome>,
}

pub fn run(
    port: u16,
    host: Option<&str>,
    timeout_ms: u64,
    kill: bool,
    tree: bool,
    format: Format,
) -> Result<()> {
    let host = host.unwrap_or("127.0.0.1");
    let local = matches!(host, "127.0.0.1" | "localhost" | "::1");
    let addr = resolve(host, port)?;

    let started = Instant::now();
    let listening =
        TcpStream::connect_timeout(&addr, Duration::from_millis(timeout_ms)).is_ok();
    let latency_ms = listening.then(|| started.elapsed().as_millis());

    // Owner detection only makes sense on the local machine.
    let pid = if local && listening {
        find_owner_pid(port)
    } else {
        None
    };
    let process_name = pid.and_then(process_name_of);

    let mut kill_outcome = None;
    if kill {
        if !local {
            return Err("--kill only works for local ports".into());
        }
        if !listening {
            return Err(format!("port {port}: not listening, nothing to kill").into());
        }
        let pid = pid.ok_or("could not determine the owner pid of this port")?;
        kill_outcome = Some(crate::commands::ps::kill_impl(pid, tree)?);
    }

    let result = PortResult {
        host: host.to_string(),
        port,
        listening,
        latency_ms,
        pid,
        process_name,
        kill: kill_outcome,
    };
    match format {
        Format::Json => output::print_json(&result),
        Format::Plain => {
            if !local {
                if result.listening {
                    println!(
                        "{host}:{port} reachable in {} ms",
                        latency_ms.unwrap_or_default()
                    );
                } else {
                    println!("{host}:{port} unreachable (timeout {timeout_ms} ms)");
                }
            } else if result.listening {
                match (result.pid, &result.process_name) {
                    (Some(pid), Some(name)) => {
                        println!("port {port}: LISTENING, owned by {name} (pid {pid})")
                    }
                    _ => println!("port {port}: LISTENING (owner unknown)"),
                }
            } else {
                println!("port {port}: not listening");
            }
            if let Some(k) = &result.kill {
                if k.killed {
                    println!("killed {} (pid {})", k.name, k.pid);
                } else {
                    println!("failed to kill {} (pid {})", k.name, k.pid);
                }
                if !k.tree_killed.is_empty() {
                    println!("also killed descendant(s): {:?}", k.tree_killed);
                }
                if !k.tree_failed.is_empty() {
                    println!("failed to kill descendant(s): {:?}", k.tree_failed);
                }
            }
        }
    }
    Ok(())
}

fn resolve(host: &str, port: u16) -> Result<SocketAddr> {
    if let Ok(ip) = host.parse::<IpAddr>() {
        return Ok(SocketAddr::new(ip, port));
    }
    (host, port)
        .to_socket_addrs()?
        .next()
        .ok_or_else(|| format!("could not resolve host '{host}'").into())
}

#[cfg(windows)]
fn find_owner_pid(port: u16) -> Option<u32> {
    let out = Command::new("netstat")
        .args(["-ano", "-p", "tcp"])
        .output()
        .ok()?;
    // netstat prints ASCII even on GBK systems; decode defensively anyway.
    let text = crate::utils::encoding::decode_bytes(&out.stdout).text;
    let needle = format!(":{port}");
    for line in text.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 5
            && parts[0] == "TCP"
            && parts[1].ends_with(&needle)
            && parts[3] == "LISTENING"
        {
            if let Ok(pid) = parts[4].parse() {
                return Some(pid);
            }
        }
    }
    None
}

#[cfg(not(windows))]
fn find_owner_pid(port: u16) -> Option<u32> {
    let out = Command::new("lsof")
        .args(["-nP", &format!("-iTCP:{port}"), "-sTCP:LISTEN", "-t"])
        .output()
        .ok()?;
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .next()?
        .trim()
        .parse()
        .ok()
}

fn process_name_of(pid: u32) -> Option<String> {
    let mut sys = System::new();
    // Light refresh: pid/parent/name only (see ps.rs light_refresh).
    sys.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        sysinfo::ProcessRefreshKind::nothing(),
    );
    sys.process(Pid::from_u32(pid))
        .map(|p| p.name().to_string_lossy().into_owned())
}
