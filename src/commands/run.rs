use crate::cli::Format;
use crate::utils::encoding;
use crate::utils::output;
use crate::utils::Result;
use serde::Serialize;
use std::io::Read;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

#[derive(Serialize)]
struct RunResult {
    cmd: String,
    exit_code: Option<i32>,
    timed_out: bool,
    duration_ms: u128,
    stdout: String,
    stderr: String,
}

/// Execute an external command with a timeout. stdout/stderr are captured on
/// reader threads (no pipe deadlock), output is decoded with encoding
/// auto-detection, and the child exit code is propagated (124 on timeout).
pub fn run(
    cmd: &[String],
    timeout: u64,
    unset: &[String],
    env: &[String],
    format: Format,
) -> Result<()> {
    let started = Instant::now();
    let mut command = Command::new(&cmd[0]);
    command.args(&cmd[1..])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    crate::utils::apply_env(&mut command, unset, env)?;
    let mut child = command
        .spawn()
        .map_err(|e| format!("failed to start '{}': {e}", cmd[0]))?;

    let mut out_pipe = child.stdout.take().unwrap();
    let mut err_pipe = child.stderr.take().unwrap();
    let out_thread = thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = out_pipe.read_to_end(&mut buf);
        buf
    });
    let err_thread = thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = err_pipe.read_to_end(&mut buf);
        buf
    });

    let deadline = started + Duration::from_secs(timeout);
    let mut status = None;
    loop {
        if let Some(s) = child.try_wait()? {
            status = Some(s);
            break;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            break;
        }
        thread::sleep(Duration::from_millis(15));
    }

    let stdout = encoding::decode_bytes(&out_thread.join().unwrap_or_default()).text;
    let stderr = encoding::decode_bytes(&err_thread.join().unwrap_or_default()).text;
    let duration_ms = started.elapsed().as_millis();
    let timed_out = status.is_none();
    let exit_code = status.and_then(|s| s.code());

    let result = RunResult {
        cmd: cmd.join(" "),
        exit_code,
        timed_out,
        duration_ms,
        stdout: stdout.clone(),
        stderr: stderr.clone(),
    };
    match format {
        Format::Json => output::print_json(&result),
        Format::Plain => {
            print!("{stdout}");
            if !stderr.is_empty() {
                eprint!("{stderr}");
            }
            eprintln!(
                "[exit {} · {} ms{}]",
                exit_code.map(|c| c.to_string()).unwrap_or_else(|| "none".into()),
                duration_ms,
                if timed_out { " · timed out" } else { "" }
            );
        }
    }
    std::process::exit(exit_code.unwrap_or(if timed_out { 124 } else { 1 }));
}
