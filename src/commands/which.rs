use crate::cli::Format;
use crate::utils::encoding;
use crate::utils::output;
use crate::utils::Result;
use serde::Serialize;
use std::env;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

#[derive(Serialize)]
struct WhichResult {
    command: String,
    found: bool,
    path: Option<String>,
    version: Option<String>,
}

/// Locate a command on PATH. On Windows, PATHEXT extensions are tried
/// automatically, so `which python` finds `python.exe`.
pub fn run(command: &str, format: Format) -> Result<()> {
    let path = find_on_path(command);
    if path.is_none() {
        // Let the shared error printer produce the single output line/object.
        return Err(format!("'{command}' not found on PATH").into());
    }
    let version = path.as_deref().and_then(probe_version);
    let result = WhichResult {
        command: command.to_string(),
        found: true,
        path: path.as_ref().map(|p| p.display().to_string()),
        version: version.clone(),
    };
    match format {
        Format::Json => output::print_json(&result),
        Format::Plain => {
            println!("{}", path.as_ref().unwrap().display());
            if let Some(v) = &version {
                println!("{v}");
            }
        }
    }
    Ok(())
}

fn find_on_path(command: &str) -> Option<PathBuf> {
    let p = Path::new(command);
    if (command.contains('\\') || command.contains('/')) && p.is_file() {
        return Some(p.to_path_buf());
    }
    let path_var = env::var_os("PATH")?;
    let exts: Vec<String> = if cfg!(windows) {
        env::var("PATHEXT")
            .unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".into())
            .split(';')
            .map(|s| s.to_ascii_lowercase())
            .collect()
    } else {
        Vec::new()
    };
    let has_ext = p.extension().is_some();
    for dir in env::split_paths(&path_var) {
        let direct = dir.join(command);
        if direct.is_file() {
            return Some(direct);
        }
        if cfg!(windows) && !has_ext {
            for ext in &exts {
                let candidate = dir.join(format!("{command}{ext}"));
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
        }
    }
    None
}

/// Probe `--version` with a 2s timeout so a hanging tool can't block us.
fn probe_version(path: &Path) -> Option<String> {
    let mut child = Command::new(path)
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .ok()?;
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        if matches!(child.try_wait(), Ok(Some(_))) {
            break;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return None;
        }
        thread::sleep(Duration::from_millis(10));
    }
    let mut buf = Vec::new();
    if let Some(mut out) = child.stdout.take() {
        let _ = out.read_to_end(&mut buf);
    }
    if let Some(mut err) = child.stderr.take() {
        let _ = err.read_to_end(&mut buf);
    }
    let text = encoding::decode_bytes(&buf).text;
    text.lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .map(|l| l.chars().take(120).collect())
}
