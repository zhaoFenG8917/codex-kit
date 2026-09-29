use crate::cli::Format;
use crate::utils::encoding;
use crate::utils::output;
use crate::utils::Result;
use regex::Regex;
use serde::Serialize;
use similar::TextDiff;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Serialize)]
struct ReplaceResult {
    file: String,
    replacements: usize,
    dry_run: bool,
    backup: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    preview: Option<String>,
}

pub fn run(
    file: &Path,
    old: &str,
    new: &str,
    dry_run: bool,
    use_regex: bool,
    no_backup: bool,
    format: Format,
) -> Result<()> {
    let raw = fs::read(file).map_err(|e| format!("{}: {e}", file.display()))?;
    let decoded = encoding::decode_bytes(&raw);

    let (replaced, count) = if use_regex {
        let re = Regex::new(old).map_err(|e| format!("invalid regex '{old}': {e}"))?;
        let count = re.find_iter(&decoded.text).count();
        (re.replace_all(&decoded.text, new).into_owned(), count)
    } else {
        if old.is_empty() {
            return Err("<old> must not be empty".into());
        }
        let count = decoded.text.matches(old).count();
        (decoded.text.replace(old, new), count)
    };

    let mut backup_path = None;
    let mut preview = None;

    if dry_run {
        let diff = TextDiff::from_lines(&decoded.text, &replaced);
        preview = Some(diff.unified_diff().header("before", "after").to_string());
    } else {
        if !no_backup {
            // Back up the original bytes untouched, encoding included.
            let bak = PathBuf::from(format!("{}.bak", file.display()));
            fs::write(&bak, &raw).map_err(|e| format!("{}: {e}", bak.display()))?;
            backup_path = Some(bak.display().to_string());
        }
        let out = encoding::encode_back(&decoded, &replaced);
        fs::write(file, out).map_err(|e| format!("{}: {e}", file.display()))?;
    }

    let result = ReplaceResult {
        file: file.display().to_string(),
        replacements: count,
        dry_run,
        backup: backup_path,
        preview,
    };
    match format {
        Format::Json => output::print_json(&result),
        Format::Plain => {
            if let Some(p) = &result.preview {
                print!("{p}");
            }
            println!("{} replacement(s) in {}", result.replacements, result.file);
            if let Some(b) = &result.backup {
                println!("backup written to {b}");
            }
        }
    }
    Ok(())
}
