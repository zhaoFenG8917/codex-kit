use crate::cli::Format;
use crate::utils::output;
use crate::utils::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

/// One trashed item: a directory under `~/.codex-kit/trash/<id>/` holding
/// `meta.json` (this struct) and `payload` (the moved file/dir). Per-entry
/// metadata avoids a single manifest file getting corrupted.
#[derive(Serialize, Deserialize, Clone)]
struct TrashMeta {
    id: String,
    original_path: String,
    deleted_at: String,
    size_bytes: u64,
    is_dir: bool,
}

#[derive(Serialize)]
struct RmResult {
    path: String,
    action: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

#[derive(Serialize)]
struct RestoreResult {
    id: String,
    restored_to: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

#[derive(Serialize)]
struct EmptyResult {
    emptied: usize,
}

fn trash_root() -> Result<PathBuf> {
    let home = std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .ok_or("cannot determine home directory (USERPROFILE/HOME unset)")?;
    Ok(PathBuf::from(home).join(".codex-kit").join("trash"))
}

fn now_id() -> String {
    static SEQ: AtomicUsize = AtomicUsize::new(0);
    let ts = chrono::Local::now().format("%Y%m%d-%H%M%S");
    format!("{ts}-{}-{}", std::process::id(), SEQ.fetch_add(1, Ordering::SeqCst))
}

fn now_string() -> String {
    chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

fn dir_size(path: &Path) -> u64 {
    if path.is_file() {
        return path.metadata().map(|m| m.len()).unwrap_or(0);
    }
    walkdir::WalkDir::new(path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter_map(|e| e.metadata().ok())
        .map(|m| m.len())
        .sum()
}

/// Move across volumes if necessary (rename only works within one volume).
fn move_entry(src: &Path, dst: &Path) -> Result<()> {
    if let Some(p) = dst.parent() {
        fs::create_dir_all(p)?;
    }
    match fs::rename(src, dst) {
        Ok(()) => Ok(()),
        Err(_) => {
            if src.is_dir() {
                copy_dir(src, dst)?;
                fs::remove_dir_all(src)?;
            } else {
                fs::copy(src, dst)?;
                fs::remove_file(src)?;
            }
            Ok(())
        }
    }
}

fn copy_dir(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst)?;
    for entry in walkdir::WalkDir::new(src) {
        let entry = entry.map_err(|e| e.to_string())?;
        let rel = entry.path().strip_prefix(src).map_err(|e| e.to_string())?;
        let target = dst.join(rel);
        if entry.file_type().is_dir() {
            fs::create_dir_all(&target)?;
        } else {
            fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

fn read_meta(entry_dir: &Path) -> Option<TrashMeta> {
    let text = fs::read_to_string(entry_dir.join("meta.json")).ok()?;
    serde_json::from_str(&text).ok()
}

fn list_entries() -> Result<Vec<TrashMeta>> {
    let root = trash_root()?;
    let mut out = Vec::new();
    if root.is_dir() {
        for e in fs::read_dir(&root)? {
            let e = e?;
            if e.file_type()?.is_dir() {
                if let Some(m) = read_meta(&e.path()) {
                    out.push(m);
                }
            }
        }
    }
    out.sort_by(|a, b| b.id.cmp(&a.id)); // newest first
    Ok(out)
}

/// Safe delete: trash by default, permanent only with --force.
pub fn rm(paths: &[PathBuf], force: bool, dry_run: bool, format: Format) -> Result<()> {
    let root = trash_root()?;
    let mut results = Vec::new();
    let mut had_error = false;

    for path in paths {
        let display = path.display().to_string();
        if !path.exists() {
            had_error = true;
            results.push(RmResult {
                path: display,
                action: "error".into(),
                id: None,
                error: Some("not found".into()),
            });
            continue;
        }
        // Never trash the trash itself.
        if path
            .canonicalize()
            .map(|c| c.starts_with(&root))
            .unwrap_or(false)
        {
            had_error = true;
            results.push(RmResult {
                path: display,
                action: "error".into(),
                id: None,
                error: Some("refusing to remove the trash directory itself".into()),
            });
            continue;
        }
        if dry_run {
            results.push(RmResult {
                path: display,
                action: if force {
                    "would delete permanently".into()
                } else {
                    "would move to trash".into()
                },
                id: None,
                error: None,
            });
            continue;
        }
        if force {
            let r = if path.is_dir() {
                fs::remove_dir_all(path)
            } else {
                fs::remove_file(path)
            };
            match r {
                Ok(()) => results.push(RmResult {
                    path: display,
                    action: "deleted permanently".into(),
                    id: None,
                    error: None,
                }),
                Err(e) => {
                    had_error = true;
                    results.push(RmResult {
                        path: display,
                        action: "error".into(),
                        id: None,
                        error: Some(e.to_string()),
                    })
                }
            }
            continue;
        }
        // Default: move to trash.
        let id = now_id();
        let entry_dir = root.join(&id);
        let meta = TrashMeta {
            id: id.clone(),
            original_path: fs::canonicalize(path)
                .map(|p| {
                    let s = p.display().to_string();
                    // Strip the Windows verbatim prefix for readability.
                    s.strip_prefix(r"\\?\").map(str::to_string).unwrap_or(s)
                })
                .unwrap_or_else(|_| display.clone()),
            deleted_at: now_string(),
            size_bytes: dir_size(path),
            is_dir: path.is_dir(),
        };
        if let Err(e) = move_entry(path, &entry_dir.join("payload")) {
            had_error = true;
            results.push(RmResult {
                path: display,
                action: "error".into(),
                id: None,
                error: Some(e.to_string()),
            });
            continue;
        }
        fs::write(entry_dir.join("meta.json"), serde_json::to_string_pretty(&meta)?)?;
        results.push(RmResult {
            path: display,
            action: "moved to trash".into(),
            id: Some(id),
            error: None,
        });
    }

    match format {
        Format::Json => output::print_json(&results),
        Format::Plain => {
            for r in &results {
                match (&r.id, &r.error) {
                    (_, Some(e)) => println!("error  {}  ({e})", r.path),
                    (Some(id), _) => println!("trashed {}  (id {id})", r.path),
                    _ => println!("{}  {}", r.action, r.path),
                }
            }
        }
    }
    if had_error {
        return Err("some paths could not be removed".into());
    }
    Ok(())
}

pub fn list(empty: bool, format: Format) -> Result<()> {
    if empty {
        let n = list_entries()?.len();
        let root = trash_root()?;
        if root.is_dir() {
            fs::remove_dir_all(&root)?;
        }
        let result = EmptyResult { emptied: n };
        match format {
            Format::Json => output::print_json(&result),
            Format::Plain => println!("emptied {n} item(s) from trash"),
        }
        return Ok(());
    }
    let entries = list_entries()?;
    match format {
        Format::Json => output::print_json(&entries),
        Format::Plain => {
            if entries.is_empty() {
                println!("trash is empty");
            } else {
                println!("{:<22} {:>9}  {:<19}  {}", "ID", "SIZE", "DELETED AT", "ORIGINAL PATH");
                for m in &entries {
                    println!(
                        "{:<22} {:>9}  {:<19}  {}",
                        m.id,
                        human_size(m.size_bytes),
                        m.deleted_at,
                        m.original_path
                    );
                }
            }
        }
    }
    Ok(())
}

pub fn restore(ids: &[String], all: bool, overwrite: bool, format: Format) -> Result<()> {
    let root = trash_root()?;
    let entries = list_entries()?;
    let targets: Vec<TrashMeta> = if all {
        entries
    } else {
        let mut t = Vec::new();
        for id in ids {
            match entries.iter().find(|m| &m.id == id) {
                Some(m) => t.push(m.clone()),
                None => return Err(format!("no trash entry with id '{id}'").into()),
            }
        }
        t
    };
    if targets.is_empty() {
        match format {
            Format::Json => output::print_json(&Vec::<RestoreResult>::new()),
            Format::Plain => println!("nothing to restore"),
        }
        return Ok(());
    }

    let mut results = Vec::new();
    let mut had_error = false;
    for m in targets {
        let payload = root.join(&m.id).join("payload");
        let dest = PathBuf::from(&m.original_path);
        if dest.exists() {
            if !overwrite {
                had_error = true;
                results.push(RestoreResult {
                    id: m.id,
                    restored_to: None,
                    error: Some("original path exists (use --overwrite)".into()),
                });
                continue;
            }
            let r = if dest.is_dir() {
                fs::remove_dir_all(&dest)
            } else {
                fs::remove_file(&dest)
            };
            if let Err(e) = r {
                had_error = true;
                results.push(RestoreResult {
                    id: m.id,
                    restored_to: None,
                    error: Some(e.to_string()),
                });
                continue;
            }
        }
        match move_entry(&payload, &dest) {
            Ok(()) => {
                let _ = fs::remove_dir_all(root.join(&m.id));
                results.push(RestoreResult {
                    id: m.id,
                    restored_to: Some(m.original_path),
                    error: None,
                });
            }
            Err(e) => {
                had_error = true;
                results.push(RestoreResult {
                    id: m.id,
                    restored_to: None,
                    error: Some(e.to_string()),
                });
            }
        }
    }

    match format {
        Format::Json => output::print_json(&results),
        Format::Plain => {
            for r in &results {
                match (&r.restored_to, &r.error) {
                    (Some(p), _) => println!("restored {} -> {p}", r.id),
                    (_, Some(e)) => println!("error   {}  ({e})", r.id),
                    _ => {}
                }
            }
        }
    }
    if had_error {
        return Err("some items could not be restored".into());
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
