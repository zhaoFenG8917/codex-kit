use crate::cli::Format;
use crate::utils::output;
use crate::utils::Result;
use chrono::{DateTime, Local};
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Serialize)]
struct Info {
    path: String,
    exists: bool,
    #[serde(rename = "type")]
    kind: Option<String>,
    /// Meaningful for files and symlinks only; a directory's length is a
    /// platform artifact (0 on Windows), never a size worth reporting.
    size_bytes: Option<u64>,
    /// Directory entry count (best effort); null for everything else.
    entries: Option<u64>,
    /// Symlink target path; null for everything else.
    target: Option<String>,
    /// Octal permission bits (Unix only); absent on Windows.
    #[cfg(unix)]
    mode: Option<String>,
    created: Option<String>,
    modified: Option<String>,
    readonly: Option<bool>,
}

/// Probe metadata for one or more paths:
///
/// - A single path prints one JSON object (the historical shape); several
///   paths print a JSON array, so one call can inspect a batch.
/// - Missing paths report `exists: false` with exit code 0 (probe
///   semantics), they never abort the batch.
/// - Directories get `entries` instead of a meaningless `size_bytes`;
///   symlinks get their `target`; Unix gets octal `mode`.
pub fn run(paths: &[PathBuf], format: Format) -> Result<()> {
    let infos: Vec<Info> = paths.iter().map(|p| probe(p)).collect();
    match format {
        Format::Json => {
            if infos.len() == 1 {
                output::print_json(&infos[0]);
            } else {
                output::print_json(&infos);
            }
        }
        Format::Plain => {
            for (i, info) in infos.iter().enumerate() {
                if i > 0 {
                    println!();
                }
                print_plain(info);
            }
        }
    }
    Ok(())
}

fn probe(path: &Path) -> Info {
    let meta = match fs::symlink_metadata(path) {
        Err(_) => {
            return Info {
                path: path.display().to_string(),
                exists: false,
                kind: None,
                size_bytes: None,
                entries: None,
                target: None,
                #[cfg(unix)]
                mode: None,
                created: None,
                modified: None,
                readonly: None,
            }
        }
        Ok(meta) => meta,
    };
    let file_type = meta.file_type();
    let is_symlink = file_type.is_symlink();
    let is_dir = file_type.is_dir();
    let kind = if is_symlink {
        "symlink"
    } else if is_dir {
        "dir"
    } else {
        "file"
    };
    Info {
        path: path.display().to_string(),
        exists: true,
        kind: Some(kind.to_string()),
        size_bytes: if is_dir { None } else { Some(meta.len()) },
        entries: if is_dir { count_entries(path) } else { None },
        target: if is_symlink {
            fs::read_link(path).ok().map(|t| t.display().to_string())
        } else {
            None
        },
        #[cfg(unix)]
        mode: Some(format!(
            "{:o}",
            std::os::unix::fs::MetadataExt::mode(&meta) & 0o7777
        )),
        created: meta.created().ok().map(fmt_time),
        modified: meta.modified().ok().map(fmt_time),
        readonly: Some(meta.permissions().readonly()),
    }
}

/// Best-effort directory entry count; None when the directory cannot be read.
fn count_entries(path: &Path) -> Option<u64> {
    let mut n = 0u64;
    for entry in fs::read_dir(path).ok()? {
        if entry.is_ok() {
            n += 1;
        }
    }
    Some(n)
}

fn print_plain(info: &Info) {
    println!("path:     {}", info.path);
    println!("exists:   {}", info.exists);
    if !info.exists {
        return;
    }
    println!("type:     {}", info.kind.as_deref().unwrap_or("-"));
    match info.size_bytes {
        Some(size) => println!("size:     {}", output::human_size(size)),
        None => println!("size:     -"),
    }
    if let Some(entries) = info.entries {
        println!("entries:  {entries}");
    }
    if let Some(target) = &info.target {
        println!("target:   {target}");
    }
    #[cfg(unix)]
    if let Some(mode) = &info.mode {
        println!("mode:     {mode}");
    }
    println!(
        "created:  {}",
        info.created.as_deref().unwrap_or("unsupported")
    );
    println!(
        "modified: {}",
        info.modified.as_deref().unwrap_or("unsupported")
    );
    println!("readonly: {}", info.readonly.unwrap_or(false));
}

fn fmt_time(t: std::time::SystemTime) -> String {
    let dt: DateTime<Local> = t.into();
    dt.format("%Y-%m-%d %H:%M:%S %z").to_string()
}
