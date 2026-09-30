use crate::cli::Format;
use crate::utils::output;
use crate::utils::Result;
use chrono::{DateTime, Local};
use colored::Colorize;
use serde::Serialize;
use std::fs;
use std::path::Path;

#[derive(Serialize)]
struct Entry {
    name: String,
    #[serde(rename = "type")]
    kind: String,
    size: u64,
    modified: String,
    hidden: bool,
}

#[derive(Serialize)]
struct LimitedListing {
    entries: Vec<Entry>,
    total: usize,
    returned: usize,
    truncated: bool,
}

pub fn run(
    path: Option<&Path>,
    all: bool,
    long: bool,
    limit: Option<usize>,
    format: Format,
) -> Result<()> {
    let dir = path.unwrap_or_else(|| Path::new("."));
    let read_dir = fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;

    let mut entries = Vec::new();
    for entry in read_dir {
        let entry = entry?;
        let hidden = is_hidden(&entry);
        if hidden && !all {
            continue;
        }
        let file_type = entry.file_type()?;
        let meta = entry.metadata()?;
        let modified: DateTime<Local> = meta.modified()?.into();
        let kind = if file_type.is_symlink() {
            "symlink"
        } else if file_type.is_dir() {
            "dir"
        } else {
            "file"
        };
        entries.push(Entry {
            name: entry.file_name().to_string_lossy().into_owned(),
            kind: kind.to_string(),
            size: meta.len(),
            modified: modified.format("%Y-%m-%d %H:%M:%S").to_string(),
            hidden,
        });
    }
    entries.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

    let total = entries.len();
    let truncated = match limit {
        Some(n) if n < total => {
            entries.truncate(n);
            true
        }
        _ => false,
    };

    match format {
        Format::Json => {
            if limit.is_some() {
                // Keep the JSON valid after truncation and tell the caller
                // how much was hidden.
                output::print_json(&LimitedListing {
                    returned: entries.len(),
                    total,
                    truncated,
                    entries,
                });
            } else {
                output::print_json(&entries);
            }
        }
        Format::Plain => {
            for e in &entries {
                let name = match e.kind.as_str() {
                    "dir" => e.name.blue().bold(),
                    "symlink" => e.name.cyan(),
                    _ => e.name.normal(),
                };
                if long {
                    println!("{:>10}  {}  {}", human_size(e.size), e.modified, name);
                } else {
                    println!("{name}");
                }
            }
            if truncated {
                println!("... and {} more (of {total} total)", total - entries.len());
            }
        }
    }
    Ok(())
}

fn human_size(size: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = size as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{size} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

fn is_hidden(entry: &fs::DirEntry) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if let Ok(meta) = entry.metadata() {
            // FILE_ATTRIBUTE_HIDDEN
            if meta.file_attributes() & 0x2 != 0 {
                return true;
            }
        }
    }
    entry.file_name().to_string_lossy().starts_with('.')
}
