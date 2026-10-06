pub mod encoding;
pub mod output;

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

/// Move a file or directory, falling back to copy+delete when src and dst
/// live on different volumes (rename only works within one volume).
pub fn move_entry(src: &std::path::Path, dst: &std::path::Path) -> Result<()> {
    if let Some(p) = dst.parent() {
        std::fs::create_dir_all(p)?;
    }
    match std::fs::rename(src, dst) {
        Ok(()) => Ok(()),
        Err(_) => {
            if src.is_dir() {
                copy_dir(src, dst)?;
                std::fs::remove_dir_all(src)?;
            } else {
                std::fs::copy(src, dst)?;
                std::fs::remove_file(src)?;
            }
            Ok(())
        }
    }
}

fn copy_dir(src: &std::path::Path, dst: &std::path::Path) -> Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in walkdir::WalkDir::new(src) {
        let entry = entry.map_err(|e| e.to_string())?;
        let rel = entry.path().strip_prefix(src).map_err(|e| e.to_string())?;
        let target = dst.join(rel);
        if entry.file_type().is_dir() {
            std::fs::create_dir_all(&target)?;
        } else {
            std::fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

/// Apply --unset / --env KEY=VAL modifications to a child Command.
pub fn apply_env(
    command: &mut std::process::Command,
    unset: &[String],
    env: &[String],
) -> Result<()> {
    for key in unset {
        command.env_remove(key);
    }
    for pair in env {
        let (k, v) = pair
            .split_once('=')
            .ok_or_else(|| format!("invalid --env '{pair}' (use KEY=VAL)"))?;
        command.env(k, v);
    }
    Ok(())
}
