use crate::cli::Format;
use crate::utils::encoding;
use crate::utils::output;
use crate::utils::Result;
use colored::Colorize;
use serde::Serialize;
use similar::{ChangeTag, TextDiff};
use std::path::Path;

#[derive(Serialize)]
struct Change {
    op: String,
    old_line: Option<usize>,
    new_line: Option<usize>,
    text: String,
}

#[derive(Serialize)]
struct DiffResult {
    file1: String,
    file2: String,
    identical: bool,
    changes: Vec<Change>,
}

pub fn run(file1: &Path, file2: &Path, format: Format) -> Result<()> {
    let a = encoding::read_file_auto(file1).map_err(|e| format!("{}: {e}", file1.display()))?;
    let b = encoding::read_file_auto(file2).map_err(|e| format!("{}: {e}", file2.display()))?;
    let diff = TextDiff::from_lines(&a.text, &b.text);

    match format {
        Format::Json => {
            let changes = diff
                .iter_all_changes()
                .filter(|c| c.tag() != ChangeTag::Equal)
                .map(|c| Change {
                    op: match c.tag() {
                        ChangeTag::Delete => "delete",
                        ChangeTag::Insert => "insert",
                        ChangeTag::Equal => "equal",
                    }
                    .to_string(),
                    old_line: c.old_index().map(|i| i + 1),
                    new_line: c.new_index().map(|i| i + 1),
                    text: c.value().trim_end_matches('\n').to_string(),
                })
                .collect::<Vec<_>>();
            output::print_json(&DiffResult {
                file1: file1.display().to_string(),
                file2: file2.display().to_string(),
                identical: changes.is_empty(),
                changes,
            });
        }
        Format::Plain => {
            let unified = diff
                .unified_diff()
                .header(
                    &file1.display().to_string(),
                    &file2.display().to_string(),
                )
                .to_string();
            if unified.trim().is_empty() {
                println!("files are identical");
            } else {
                for line in unified.lines() {
                    if line.starts_with("+++") || line.starts_with("---") {
                        println!("{}", line.dimmed());
                    } else if let Some(rest) = line.strip_prefix('+') {
                        println!("{}", format!("+{rest}").green());
                    } else if let Some(rest) = line.strip_prefix('-') {
                        println!("{}", format!("-{rest}").red());
                    } else if line.starts_with("@@") {
                        println!("{}", line.cyan());
                    } else {
                        println!("{line}");
                    }
                }
            }
        }
    }
    Ok(())
}
