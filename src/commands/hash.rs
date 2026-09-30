use crate::cli::Format;
use crate::utils::output;
use crate::utils::Result;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Read;
use std::path::Path;

#[derive(Serialize)]
struct HashResult {
    file: String,
    algorithm: String,
    hash: String,
    bytes: u64,
}

/// Stream the file in 64KB chunks so large files don't blow up memory.
pub fn run(file: &Path, algorithm: &str, format: Format) -> Result<()> {
    let algo = algorithm.to_ascii_lowercase();
    if algo != "md5" && algo != "sha256" {
        return Err(format!("unsupported algorithm '{algorithm}' (use md5 or sha256)").into());
    }
    let mut f = fs::File::open(file)?;
    let size = f.metadata()?.len();
    let mut buf = [0u8; 65536];

    let hash = if algo == "md5" {
        let mut ctx = md5::Context::new();
        loop {
            let n = f.read(&mut buf)?;
            if n == 0 {
                break;
            }
            ctx.consume(&buf[..n]);
        }
        format!("{:x}", ctx.compute())
    } else {
        let mut h = Sha256::new();
        loop {
            let n = f.read(&mut buf)?;
            if n == 0 {
                break;
            }
            h.update(&buf[..n]);
        }
        format!("{:x}", h.finalize())
    };

    let result = HashResult {
        file: file.display().to_string(),
        algorithm: algo.clone(),
        hash: hash.clone(),
        bytes: size,
    };
    match format {
        Format::Json => output::print_json(&result),
        Format::Plain => println!("{hash}  {}", file.display()),
    }
    Ok(())
}
