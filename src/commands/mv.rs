use crate::cli::Format;
use crate::utils::output;
use crate::utils::Result;
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Serialize)]
struct MvResult {
    src: String,
    dst: String,
    moved: bool,
}

/// Move/rename with the same safety philosophy as the rest of the toolkit:
/// refuses to clobber an existing destination unless --force is given.
pub fn run(src: &Path, dst: &Path, force: bool, format: Format) -> Result<()> {
    if !src.exists() {
        return Err(format!("source not found: {}", src.display()).into());
    }
    // `mv file existing-dir/` moves the file into the directory.
    let target: PathBuf = if dst.is_dir() {
        dst.join(
            src.file_name()
                .ok_or_else(|| format!("invalid source path: {}", src.display()))?,
        )
    } else {
        dst.to_path_buf()
    };
    if target.exists() {
        if !force {
            return Err(format!(
                "destination exists: {} (use --force to overwrite)",
                target.display()
            )
            .into());
        }
        if target.is_dir() {
            fs::remove_dir_all(&target)?;
        } else {
            fs::remove_file(&target)?;
        }
    }
    crate::utils::move_entry(src, &target)?;

    let result = MvResult {
        src: src.display().to_string(),
        dst: target.display().to_string(),
        moved: true,
    };
    match format {
        Format::Json => output::print_json(&result),
        Format::Plain => println!("{} -> {}", result.src, result.dst),
    }
    Ok(())
}
