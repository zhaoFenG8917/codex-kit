use crate::cli::Format;
use crate::utils::encoding::{self, Decoded};
use crate::utils::output;
use crate::utils::Result;
use serde::Serialize;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::Path;

#[derive(Serialize)]
struct Lines {
    file: String,
    encoding: String,
    lines: Vec<String>,
}

/// First N lines. Reads raw bytes line by line and stops as soon as N lines
/// are collected; safe for huge files. Splitting raw bytes on b'\n' is safe
/// for UTF-8/GBK/GB18030 because 0x0A never appears inside a multibyte char.
pub fn head(file: &Path, n: usize, format: Format) -> Result<()> {
    if is_utf16(file)? {
        // UTF-16 line endings contain 0x00; fall back to a full decode.
        let decoded = encoding::read_file_auto(file)?;
        let lines = decoded.text.lines().take(n).map(str::to_string).collect();
        return finish(file, &decoded, lines, format);
    }
    let f = File::open(file).map_err(|e| format!("{}: {e}", file.display()))?;
    let mut reader = BufReader::new(f);
    let mut raw = Vec::new();
    let mut buf = Vec::new();
    let mut count = 0;
    while count < n {
        buf.clear();
        if reader.read_until(b'\n', &mut buf)? == 0 {
            break;
        }
        raw.extend_from_slice(&buf);
        count += 1;
    }
    let decoded = encoding::decode_bytes(&raw);
    let lines = decoded.text.lines().map(str::to_string).collect();
    finish(file, &decoded, lines, format)
}

/// Last N lines. Seeks from the end in growing blocks until enough newlines
/// are collected, so arbitrarily large files never get fully loaded.
pub fn tail(file: &Path, n: usize, format: Format) -> Result<()> {
    let mut f = File::open(file).map_err(|e| format!("{}: {e}", file.display()))?;
    let size = f.metadata()?.len();

    let mut end = size;
    let mut block = 8192u64;
    let mut raw: Vec<u8> = Vec::new();
    while end > 0 && raw.iter().filter(|&&b| b == b'\n').count() <= n {
        let start = end.saturating_sub(block);
        f.seek(SeekFrom::Start(start))?;
        let mut chunk = vec![0u8; (end - start) as usize];
        f.read_exact(&mut chunk)?;
        chunk.extend_from_slice(&raw);
        raw = chunk;
        end = start;
        block *= 2;
    }

    let decoded = encoding::decode_bytes(&raw);
    let all: Vec<&str> = decoded.text.lines().collect();
    // The first element may be a partial line cut mid-block; taking the last
    // N lines drops it because we collected at least N+1 newlines.
    let lines = all
        .iter()
        .skip(all.len().saturating_sub(n))
        .map(|s| s.to_string())
        .collect();
    finish(file, &decoded, lines, format)
}

fn finish(file: &Path, decoded: &Decoded, lines: Vec<String>, format: Format) -> Result<()> {
    match format {
        Format::Json => output::print_json(&Lines {
            file: file.display().to_string(),
            encoding: decoded.encoding.name().to_string(),
            lines,
        }),
        Format::Plain => {
            for line in &lines {
                println!("{line}");
            }
        }
    }
    Ok(())
}

fn is_utf16(file: &Path) -> Result<bool> {
    let mut f = File::open(file).map_err(|e| format!("{}: {e}", file.display()))?;
    let mut sig = [0u8; 2];
    let read = f.read(&mut sig).unwrap_or(0);
    Ok(read == 2 && (sig == [0xFF, 0xFE] || sig == [0xFE, 0xFF]))
}

// Keep fs import used by the UTF-16 fallback path.
#[allow(unused)]
fn _force_fs_used(p: &Path) -> std::io::Result<Vec<u8>> {
    fs::read(p)
}
