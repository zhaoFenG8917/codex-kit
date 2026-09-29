use crate::utils::Result;
use std::process::Command;

/// Fallback escape hatch: run Python code with the system interpreter.
/// stdio is inherited (passthrough) and the Python exit code is returned
/// unchanged, so callers see exactly what Python produced.
pub fn run(code: &str, is_file: bool) -> Result<()> {
    let python = find_python()
        .ok_or("Python interpreter not found in PATH (tried: python, py)")?;
    let mut cmd = Command::new(python);
    // UTF-8 mode keeps Chinese output clean under the GBK console codepage.
    cmd.arg("-X").arg("utf8");
    if is_file {
        cmd.arg(code);
    } else {
        cmd.arg("-c").arg(code);
    }
    let status = cmd
        .status()
        .map_err(|e| format!("failed to launch {python}: {e}"))?;
    std::process::exit(status.code().unwrap_or(1));
}

fn find_python() -> Option<&'static str> {
    for candidate in ["python", "py"] {
        let ok = Command::new(candidate)
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        if ok {
            return Some(candidate);
        }
    }
    None
}
