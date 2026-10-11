use crate::cli::Format;
use crate::utils::output;
use crate::utils::Result;
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Serialize)]
struct CpResult {
    src: String,
    dst: String,
    copied: bool,
    /// Number of files written (1 for a single file copy).
    files: usize,
}

/// Copy a file or directory (recursive). Mirrors `mv` semantics: an existing
/// directory as destination means "copy into it", and an existing destination
/// is never clobbered unless --force is given.
pub fn run(src: &Path, dst: &Path, force: bool, format: Format) -> Result<()> {
    if !src.exists() {
        return Err(format!("source not found: {}", src.display()).into());
    }
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
    if let Some(p) = target.parent() {
        fs::create_dir_all(p)?;
    }

    let files = if src.is_dir() {
        crate::utils::copy_dir(src, &target)?;
        walkdir::WalkDir::new(&target)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .count()
    } else {
        fs::copy(src, &target)?;
        1
    };

    let result = CpResult {
        src: src.display().to_string(),
        dst: target.display().to_string(),
        copied: true,
        files,
    };
    match format {
        Format::Json => output::print_json(&result),
        Format::Plain => println!("{} -> {} ({} file(s))", result.src, result.dst, result.files),
    }
    Ok(())
}
