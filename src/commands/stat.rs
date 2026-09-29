use crate::cli::Format;
use crate::utils::output;
use crate::utils::Result;
use serde::Serialize;
use std::process::Command;

#[derive(Serialize)]
struct Stat {
    os: &'static str,
    arch: &'static str,
    hostname: Option<String>,
    cwd: Option<String>,
    path_dirs: Vec<String>,
    python_version: Option<String>,
    node_version: Option<String>,
    git_version: Option<String>,
}

pub fn run(format: Format) -> Result<()> {
    let stat = Stat {
        os: std::env::consts::OS,
        arch: std::env::consts::ARCH,
        hostname: hostname(),
        cwd: std::env::current_dir()
            .ok()
            .map(|p| p.display().to_string()),
        path_dirs: std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())
            .map(|p| p.display().to_string())
            .collect(),
        python_version: python_version(),
        node_version: probe("node", &["--version"]),
        git_version: probe("git", &["--version"]),
    };

    match format {
        Format::Json => output::print_json(&stat),
        Format::Plain => {
            println!("os:             {}", stat.os);
            println!("arch:           {}", stat.arch);
            println!("hostname:       {}", or_missing(&stat.hostname));
            println!("cwd:            {}", or_missing(&stat.cwd));
            println!("python_version: {}", or_missing(&stat.python_version));
            println!("node_version:   {}", or_missing(&stat.node_version));
            println!("git_version:    {}", or_missing(&stat.git_version));
            println!("path_dirs:");
            for d in &stat.path_dirs {
                println!("  - {d}");
            }
        }
    }
    Ok(())
}

fn or_missing(v: &Option<String>) -> String {
    v.clone().unwrap_or_else(|| "(not installed)".to_string())
}

fn hostname() -> Option<String> {
    std::env::var("COMPUTERNAME")
        .ok()
        .or_else(|| std::env::var("HOSTNAME").ok())
        .or_else(|| probe("hostname", &[]))
}

fn python_version() -> Option<String> {
    probe("python", &["--version"]).or_else(|| probe("py", &["--version"]))
}

/// Run `<cmd> --version`-style probe, returning the first non-empty output line.
fn probe(cmd: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(cmd).args(args).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&out.stdout);
    let text = if stdout.trim().is_empty() {
        String::from_utf8_lossy(&out.stderr).into_owned()
    } else {
        stdout.into_owned()
    };
    text.lines()
        .next()
        .map(|l| l.trim().to_string())
        .filter(|s| !s.is_empty())
}
