use crate::cli::Format;
use crate::utils::output;
use crate::utils::Result;
use encoding_rs::{Encoding, GB18030, GBK, UTF_8};
use serde::Serialize;
use std::fs::{self, OpenOptions};
use std::io::{Read, Write as IoWrite};
use std::path::Path;

#[derive(Serialize)]
struct WriteResult {
    file: String,
    encoding: String,
    bytes: usize,
    appended: bool,
}

/// Write stdin to a file with an explicit encoding. This is the reliable way
/// to create GBK-encoded files from an agent (PowerShell's redirection would
/// mangle them).
pub fn run(file: &Path, encoding: &str, append: bool, format: Format) -> Result<()> {
    let enc: &'static Encoding = match encoding.to_ascii_lowercase().as_str() {
        "utf8" | "utf-8" => UTF_8,
        "gbk" => GBK,
        "gb18030" => GB18030,
        other => {
            return Err(format!("unsupported encoding '{other}' (use utf8, gbk, gb18030)").into())
        }
    };

    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input)?;
    let (bytes, _, had_errors) = enc.encode(&input);
    if had_errors {
        return Err(format!("input contains characters that cannot be encoded as {encoding}").into());
    }

    if let Some(parent) = file.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }
    let mut opts = OpenOptions::new();
    opts.write(true).create(true);
    if append {
        opts.append(true);
    } else {
        opts.truncate(true);
    }
    let mut f = opts.open(file)?;
    f.write_all(&bytes)?;

    let result = WriteResult {
        file: file.display().to_string(),
        encoding: encoding.to_string(),
        bytes: bytes.len(),
        appended: append,
    };
    match format {
        Format::Json => output::print_json(&result),
        Format::Plain => println!(
            "{} {} bytes to {} ({})",
            if append { "Appended" } else { "Wrote" },
            bytes.len(),
            file.display(),
            encoding
        ),
    }
    Ok(())
}
