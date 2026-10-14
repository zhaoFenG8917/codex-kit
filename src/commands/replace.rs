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
    old: Option<&str>,
    new: Option<&str>,
    old_file: Option<&Path>,
    new_file: Option<&Path>,
    dry_run: bool,
    use_regex: bool,
    no_backup: bool,
    format: Format,
) -> Result<()> {
    let raw = fs::read(file).map_err(|e| format!("{}: {e}", file.display()))?;
    let decoded = encoding::decode_bytes(&raw);

    // Needles may come from files: multi-line / CJK blocks survive the trip
    // far better than through shell command-line quoting.
    let read_arg = |inline: Option<&str>, path: Option<&Path>, what: &str| -> Result<String> {
        match (inline, path) {
            (Some(s), None) => Ok(s.to_string()),
            (None, Some(p)) => {
                let bytes = fs::read(p).map_err(|e| format!("{}: {e}", p.display()))?;
                Ok(encoding::decode_bytes(&bytes).text)
            }
            (Some(_), Some(_)) => Err(format!("pass {what} either inline or via file, not both").into()),
            (None, None) => Err(format!("missing {what}: pass it inline or via --{what}-file").into()),
        }
    };
    let old = read_arg(old, old_file, "old")?;
    let new = read_arg(new, new_file, "new")?;

    let (replaced, count) = if use_regex {
        let re = Regex::new(&old).map_err(|e| format!("invalid regex '{old}': {e}"))?;
        let count = re.find_iter(&decoded.text).count();
        (re.replace_all(&decoded.text, new.as_str()).into_owned(), count)
    } else {
        if old.is_empty() {
            return Err("<old> must not be empty".into());
        }
        // CRLF-insensitive: match on LF-normalized text, restore the file's
        // dominant line ending afterwards (a uniform CRLF file stays CRLF).
        let crlf = raw.windows(2).any(|w| w == b"\r\n");
        let text_lf = decoded.text.replace("\r\n", "\n");
        let old_lf = old.replace("\r\n", "\n");
        let new_lf = new.replace("\r\n", "\n");
        let count = text_lf.matches(&old_lf).count();
        let replaced = text_lf.replace(&old_lf, &new_lf);
        let replaced = if crlf {
            replaced.replace('\n', "\r\n")
        } else {
            replaced
        };
        (replaced, count)
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
