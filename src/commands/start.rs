use crate::cli::Format;
use crate::utils::output;
use crate::utils::Result;
use serde::Serialize;
use std::fs::{self, OpenOptions};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[derive(Serialize)]
struct StartResult {
    pid: u32,
    name: String,
    cmd: String,
    log: String,
    detached: bool,
    wait_port: Option<u16>,
    /// None: no --wait-port. Some(true): listening. Some(false): still not
    /// listening after --wait-timeout (process keeps running; check the log).
    ready: Option<bool>,
}

/// Start a long-running service detached from this session. stdout/stderr go
/// to a log file; the process survives after codex-kit exits (no console
/// window on Windows, own process group on Unix).
pub fn run(
    cmd: &[String],
    name: Option<&str>,
    cwd: Option<&Path>,
    log: Option<&Path>,
    wait_port: Option<u16>,
    wait_timeout: u64,
    format: Format,
) -> Result<()> {
    let label = name.map(str::to_string).unwrap_or_else(|| {
        cmd[0]
            .rsplit(['\\', '/'])
            .next()
            .unwrap_or(&cmd[0])
            .trim_end_matches(".exe")
            .to_string()
    });

    let log_path: PathBuf = match log {
        Some(p) => p.to_path_buf(),
        None => {
            let dir = std::env::temp_dir().join("codex-kit-logs");
            fs::create_dir_all(&dir)?;
            let safe: String = label
                .chars()
                .map(|c| {
                    if c.is_alphanumeric() || c == '-' || c == '_' {
                        c
                    } else {
                        '-'
                    }
                })
                .collect();
            dir.join(format!("{safe}-{}.log", std::process::id()))
        }
    };
    if let Some(parent) = log_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let out = OpenOptions::new().create(true).append(true).open(&log_path)?;
    let err = out.try_clone()?;

    let mut command = Command::new(&cmd[0]);
    command
        .args(&cmd[1..])
        .stdin(Stdio::null())
        .stdout(Stdio::from(out))
        .stderr(Stdio::from(err));
    if let Some(dir) = cwd {
        command.current_dir(dir);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW);
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        // New process group: no SIGHUP when the calling terminal goes away.
        command.process_group(0);
    }

    // On Windows the child would otherwise inherit our own stdout/stderr pipe
    // handles; the caller reading our output via a pipe would then never see
    // EOF (the daemon holds the pipe open forever). Strip the inherit flag
    // from our std handles around spawn() so only the log/stdin handles we
    // explicitly pass reach the child.
    #[cfg(windows)]
    set_stdio_inheritable(false);
    let spawn_result = command.spawn();
    #[cfg(windows)]
    set_stdio_inheritable(true);
    let mut child = spawn_result.map_err(|e| format!("failed to start '{}': {e}", cmd[0]))?;
    let pid = child.id();

    let mut ready = None;
    if let Some(port) = wait_port {
        let deadline = Instant::now() + Duration::from_secs(wait_timeout);
        let addr: std::net::SocketAddr = format!("127.0.0.1:{port}").parse()?;
        loop {
            if let Some(status) = child.try_wait()? {
                return Err(format!(
                    "'{}' exited during startup (code {:?}); see log: {}",
                    cmd[0],
                    status.code(),
                    log_path.display()
                )
                .into());
            }
            if TcpStream::connect_timeout(&addr, Duration::from_millis(300)).is_ok() {
                ready = Some(true);
                break;
            }
            if Instant::now() >= deadline {
                ready = Some(false);
                break;
            }
            std::thread::sleep(Duration::from_millis(250));
        }
    }
    // The Child handle is dropped here without wait(): the detached service
    // keeps running independently of this process.

    let result = StartResult {
        pid,
        name: label,
        cmd: cmd.join(" "),
        log: log_path.display().to_string(),
        detached: true,
        wait_port,
        ready,
    };
    match format {
        Format::Json => output::print_json(&result),
        Format::Plain => {
            println!("started '{}' (pid {pid})", result.cmd);
            println!("log: {}", result.log);
            match (result.wait_port, result.ready) {
                (Some(port), Some(true)) => println!("port {port} is listening — service ready"),
                (Some(port), Some(false)) => println!(
                    "WARNING: port {port} not listening within {wait_timeout}s; \
                     process is still running, check the log"
                ),
                _ => {}
            }
        }
    }
    Ok(())
}

#[cfg(windows)]
fn set_stdio_inheritable(inheritable: bool) {
    use windows_sys::Win32::Foundation::{SetHandleInformation, HANDLE_FLAG_INHERIT, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Console::{
        GetStdHandle, STD_ERROR_HANDLE, STD_INPUT_HANDLE, STD_OUTPUT_HANDLE,
    };
    let (mask, flags) = if inheritable {
        (HANDLE_FLAG_INHERIT, HANDLE_FLAG_INHERIT)
    } else {
        (HANDLE_FLAG_INHERIT, 0)
    };
    unsafe {
        for id in [STD_INPUT_HANDLE, STD_OUTPUT_HANDLE, STD_ERROR_HANDLE] {
            let h = GetStdHandle(id);
            if !h.is_null() && h != INVALID_HANDLE_VALUE {
                let _ = SetHandleInformation(h, mask, flags);
            }
        }
    }
}
