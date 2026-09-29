use crate::cli::Format;
use crate::utils::output;
use crate::utils::Result;
use glob::Pattern;
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Serialize)]
struct Node {
    name: String,
    #[serde(rename = "type")]
    kind: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    children: Vec<Node>,
}

pub fn run(path: Option<&Path>, depth: usize, ignore: &[String], format: Format) -> Result<()> {
    let root = path.unwrap_or_else(|| Path::new("."));
    let patterns = compile_patterns(ignore)?;

    match format {
        Format::Json => {
            let name = root.display().to_string();
            let children = if depth > 0 {
                build_children(root, depth, &patterns)?
            } else {
                Vec::new()
            };
            output::print_json(&Node {
                name,
                kind: "dir".to_string(),
                children,
            });
        }
        Format::Plain => {
            println!("{}", root.display());
            if depth > 0 {
                print_children(root, "", depth, &patterns)?;
            }
        }
    }
    Ok(())
}

fn compile_patterns(patterns: &[String]) -> Result<Vec<Pattern>> {
    patterns
        .iter()
        .map(|p| {
            Pattern::new(p).map_err(|e| format!("invalid --ignore pattern '{p}': {e}").into())
        })
        .collect()
}

/// Sorted entries of a directory, dirs first, with ignored names filtered out.
fn sorted_entries(dir: &Path, patterns: &[Pattern]) -> Result<Vec<(PathBuf, bool)>> {
    let read_dir = fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut dirs = Vec::new();
    let mut files = Vec::new();
    for entry in read_dir {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if patterns.iter().any(|p| p.matches(&name)) {
            continue;
        }
        // Do not follow symlinks: loops in tree output are never useful.
        let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
        if is_dir {
            dirs.push((entry.path(), true));
        } else {
            files.push((entry.path(), false));
        }
    }
    let key = |p: &(PathBuf, bool)| {
        p.0.file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_lowercase()
    };
    dirs.sort_by_key(key);
    files.sort_by_key(key);
    dirs.extend(files);
    Ok(dirs)
}

fn print_children(dir: &Path, prefix: &str, depth: usize, patterns: &[Pattern]) -> Result<()> {
    if depth == 0 {
        return Ok(());
    }
    let entries = sorted_entries(dir, patterns)?;
    let last_index = entries.len().saturating_sub(1);
    for (i, (path, is_dir)) in entries.iter().enumerate() {
        let last = i == last_index;
        let connector = if last { "└── " } else { "├── " };
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        println!("{prefix}{connector}{name}");
        if *is_dir && depth > 1 {
            let child_prefix = format!("{prefix}{}", if last { "    " } else { "│   " });
            print_children(path, &child_prefix, depth - 1, patterns)?;
        }
    }
    Ok(())
}

fn build_children(dir: &Path, depth: usize, patterns: &[Pattern]) -> Result<Vec<Node>> {
    if depth == 0 {
        return Ok(Vec::new());
    }
    let mut nodes = Vec::new();
    for (path, is_dir) in sorted_entries(dir, patterns)? {
        let name = path.file_name().unwrap_or_default().to_string_lossy().into_owned();
        let children = if is_dir {
            build_children(&path, depth - 1, patterns)?
        } else {
            Vec::new()
        };
        nodes.push(Node {
            name,
            kind: if is_dir { "dir" } else { "file" }.to_string(),
            children,
        });
    }
    Ok(nodes)
}
