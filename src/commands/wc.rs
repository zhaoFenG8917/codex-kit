use crate::cli::Format;
use crate::utils::encoding;
use crate::utils::output;
use crate::utils::Result;
use serde::Serialize;
use std::fs;
use std::path::Path;

#[derive(Serialize)]
struct Wc {
    file: String,
    lines: usize,
    /// Whitespace-separated tokens; for CJK text, count chars instead.
    words: usize,
    chars: usize,
    bytes: u64,
}

pub fn run(file: &Path, format: Format) -> Result<()> {
    let raw = fs::read(file).map_err(|e| format!("{}: {e}", file.display()))?;
    let decoded = encoding::decode_bytes(&raw);
    let wc = Wc {
        file: file.display().to_string(),
        lines: decoded.text.lines().count(),
        words: decoded.text.split_whitespace().count(),
        chars: decoded.text.chars().count(),
        bytes: raw.len() as u64,
    };
    match format {
        Format::Json => output::print_json(&wc),
        Format::Plain => println!(
            "{:>8} lines  {:>8} words  {:>8} chars  {:>8} bytes  {}",
            wc.lines, wc.words, wc.chars, wc.bytes, wc.file
        ),
    }
    Ok(())
}
