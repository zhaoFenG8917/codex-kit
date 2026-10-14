use crate::cli::Format;
use crate::utils::encoding;
use crate::utils::output;
use crate::utils::Result;
use serde::{Deserialize, Serialize};
use similar::TextDiff;
use std::fs;
use std::path::{Path, PathBuf};

/// One guarded replacement inside a patch spec.
#[derive(Deserialize)]
struct Hunk {
    old: String,
    new: String,
    /// Expected number of matches in the file (default 1). If the actual
    /// count differs, the whole patch is aborted and nothing is written.
    #[serde(default = "default_count")]
    count: usize,
}

fn default_count() -> usize {
    1
}

#[derive(Serialize)]
struct HunkReport {
    index: usize,
    expected: usize,
    matched: usize,
    ok: bool,
}

#[derive(Serialize)]
struct PatchResult {
    file: String,
    spec: String,
    applied: bool,
    dry_run: bool,
    hunks: Vec<HunkReport>,
    backup: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    preview: Option<String>,
}

/// CRLF/LF-insensitive matching: normalize both haystack and hunks to LF,
/// then restore the file's dominant line ending on write. A uniform CRLF
/// file stays byte-identical outside the replaced hunks.
fn to_lf(s: &str) -> String {
    s.replace("\r\n", "\n")
}

/// Apply a multi-hunk, all-or-nothing patch described by a JSON spec file:
/// `[{"old": "...", "new": "...", "count": 1}, ...]`
///
/// This replaces the "write a Python patch script with per-hunk assertions"
/// workflow: every hunk must match its expected count or nothing is written.
pub fn run(file: &Path, spec: &Path, dry_run: bool, no_backup: bool, format: Format) -> Result<()> {
    let spec_raw = fs::read(spec).map_err(|e| format!("{}: {e}", spec.display()))?;
    let spec_text = encoding::decode_bytes(&spec_raw).text;
    let hunks: Vec<Hunk> = serde_json::from_str(&spec_text)
        .map_err(|e| format!("invalid patch spec {}: {e}", spec.display()))?;
    if hunks.is_empty() {
        return Err("patch spec contains no hunks".into());
    }

    let raw = fs::read(file).map_err(|e| format!("{}: {e}", file.display()))?;
    let decoded = encoding::decode_bytes(&raw);
    let crlf = raw.windows(2).any(|w| w == b"\r\n");

    let mut text = to_lf(&decoded.text);
    let mut reports = Vec::new();
    let mut failures = Vec::new();

    for (i, h) in hunks.iter().enumerate() {
        let old = to_lf(&h.old);
        if old.is_empty() {
            reports.push(HunkReport {
                index: i,
                expected: h.count,
                matched: 0,
                ok: false,
            });
            failures.push(format!("hunk {i}: `old` is empty"));
            continue;
        }
        let matched = text.matches(&old).count();
        let ok = matched == h.count;
        reports.push(HunkReport {
            index: i,
            expected: h.count,
            matched,
            ok,
        });
        if ok {
            text = text.replacen(&old, &to_lf(&h.new), matched);
        } else {
            failures.push(format!(
                "hunk {i}: expected {} match(es), found {matched}",
                h.count
            ));
        }
    }

    if !failures.is_empty() {
        return Err(format!(
            "patch aborted, nothing written to {}:\n  {}",
            file.display(),
            failures.join("\n  ")
        )
        .into());
    }

    let final_text = if crlf { text.replace('\n', "\r\n") } else { text };

    let mut backup_path = None;
    let mut preview = None;
    if dry_run {
        let diff = TextDiff::from_lines(decoded.text.as_str(), final_text.as_str());
        preview = Some(diff.unified_diff().header("before", "after").to_string());
    } else {
        if !no_backup {
            let bak = PathBuf::from(format!("{}.bak", file.display()));
            fs::write(&bak, &raw).map_err(|e| format!("{}: {e}", bak.display()))?;
            backup_path = Some(bak.display().to_string());
        }
        let out = encoding::encode_back(&decoded, &final_text);
        fs::write(file, out).map_err(|e| format!("{}: {e}", file.display()))?;
    }

    let result = PatchResult {
        file: file.display().to_string(),
        spec: spec.display().to_string(),
        applied: !dry_run,
        dry_run,
        hunks: reports,
        backup: backup_path,
        preview,
    };
    match format {
        Format::Json => output::print_json(&result),
        Format::Plain => {
            if let Some(p) = &result.preview {
                print!("{p}");
            }
            for h in &result.hunks {
                println!("hunk {}: {} match(es) ✓", h.index, h.matched);
            }
            println!(
                "{}{} hunk(s) in {}",
                if dry_run { "would apply " } else { "applied " },
                result.hunks.len(),
                result.file
            );
            if let Some(b) = &result.backup {
                println!("backup written to {b}");
            }
        }
    }
    Ok(())
}
