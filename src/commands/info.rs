use crate::cli::Format;
use crate::utils::output;
use crate::utils::Result;
use chrono::{DateTime, Local};
use serde::Serialize;
use std::fs;
use std::path::Path;

#[derive(Serialize)]
struct Info {
    path: String,
    exists: bool,
    #[serde(rename = "type")]
    kind: Option<String>,
    size_bytes: Option<u64>,
    created: Option<String>,
    modified: Option<String>,
    readonly: Option<bool>,
}

pub fn run(path: &Path, format: Format) -> Result<()> {
    let meta = fs::symlink_metadata(path);
    let info = match meta {
        Err(_) => Info {
            path: path.display().to_string(),
            exists: false,
            kind: None,
            size_bytes: None,
            created: None,
            modified: None,
            readonly: None,
        },
        Ok(meta) => {
            let file_type = meta.file_type();
            let kind = if file_type.is_symlink() {
                "symlink"
            } else if file_type.is_dir() {
                "dir"
            } else {
                "file"
            };
            Info {
                path: path.display().to_string(),
                exists: true,
                kind: Some(kind.to_string()),
                size_bytes: Some(meta.len()),
                created: meta.created().ok().map(fmt_time),
                modified: meta.modified().ok().map(fmt_time),
                readonly: Some(meta.permissions().readonly()),
            }
        }
    };

    match format {
        Format::Json => output::print_json(&info),
        Format::Plain => {
            println!("path:     {}", info.path);
            println!("exists:   {}", info.exists);
            if info.exists {
                println!("type:     {}", info.kind.as_deref().unwrap_or("-"));
                println!("size:     {} bytes", info.size_bytes.unwrap_or(0));
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
        }
    }
    Ok(())
}

fn fmt_time(t: std::time::SystemTime) -> String {
    let dt: DateTime<Local> = t.into();
    dt.format("%Y-%m-%d %H:%M:%S %z").to_string()
}
