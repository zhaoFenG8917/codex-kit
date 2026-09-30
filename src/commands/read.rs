use crate::cli::Format;
use crate::utils::encoding;
use crate::utils::output;
use crate::utils::Result;
use serde::Serialize;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

#[derive(Serialize)]
struct ReadResult {
    file: String,
    encoding: String,
    start: usize,
    end: usize,
    lines: Vec<String>,
}

/// Read a 1-based line range, streaming: lines before the range are skipped,
/// reading stops right after the range end. Raw bytes are decoded with the
/// same auto-detection as head/tail.
pub fn run(file: &Path, range: &str, format: Format) -> Result<()> {
    let (start, end) = parse_range(range)?;
    if start == 0 {
        return Err("range is 1-based; start must be >= 1".into());
    }
    if let Some(e) = end {
        if e < start {
            return Err(format!("invalid range '{range}': end < start").into());
        }
    }

    let f = File::open(file).map_err(|e| format!("{}: {e}", file.display()))?;
    let mut reader = BufReader::new(f);
    let mut raw = Vec::new();
    let mut buf = Vec::new();
    let mut line_no = 0usize;
    let mut last_included = 0usize;
    loop {
        buf.clear();
        if reader.read_until(b'\n', &mut buf)? == 0 {
            break;
        }
        line_no += 1;
        if line_no < start {
            continue;
        }
        if let Some(e) = end {
            if line_no > e {
                break;
            }
        }
        raw.extend_from_slice(&buf);
        last_included = line_no;
    }

    let decoded = encoding::decode_bytes(&raw);
    let lines: Vec<String> = decoded.text.lines().map(str::to_string).collect();

    match format {
        Format::Json => output::print_json(&ReadResult {
            file: file.display().to_string(),
            encoding: decoded.encoding.name().to_string(),
            start,
            end: last_included,
            lines,
        }),
        Format::Plain => {
            for (i, line) in lines.iter().enumerate() {
                println!("{:>6}  {line}", start + i);
            }
        }
    }
    Ok(())
}

/// "100:200" -> (100, Some(200)); "100:" -> (100, None); ":200" -> (1, Some(200));
/// "150" -> (150, Some(150)).
fn parse_range(range: &str) -> Result<(usize, Option<usize>)> {
    let bad = || format!("invalid range '{range}' (use 100:200, 100:, :200, or 150)");
    if let Some((a, b)) = range.split_once(':') {
        let start: usize = if a.is_empty() {
            1
        } else {
            a.parse().map_err(|_| bad())?
        };
        let end: Option<usize> = if b.is_empty() {
            None
        } else {
            Some(b.parse().map_err(|_| bad())?)
        };
        Ok((start, end))
    } else {
        let n: usize = range.parse().map_err(|_| bad())?;
        Ok((n, Some(n)))
    }
}
