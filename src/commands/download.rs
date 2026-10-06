use crate::cli::Format;
use crate::utils::output;
use crate::utils::Result;
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// Below this size, parallel segments aren't worth the overhead.
const MIN_PARALLEL_SIZE: u64 = 8 * 1024 * 1024;

#[derive(Serialize)]
struct DownloadResult {
    url: String,
    output: String,
    bytes: u64,
    threads: usize,
    resumed_bytes: u64,
    duration_ms: u128,
}

struct Outcome {
    bytes: u64,
    threads: usize,
    resumed_bytes: u64,
}

struct Probe {
    total: Option<u64>,
    ranges: bool,
}

#[derive(Serialize, Deserialize)]
struct PartsMeta {
    url: String,
    total: u64,
    threads: usize,
}

/// Download a URL to a file with multi-threaded segments and resume support:
///
/// - Probes the server for Range support (HEAD, then a `bytes=0-0` GET).
/// - `--threads N` splits the file into N segments downloaded in parallel
///   into `<output>.ckparts/` (each part file doubles as resume state).
/// - `--resume` continues an interrupted download; without it an existing
///   output file is an error (use --force to restart from scratch).
/// - Falls back to a single stream when ranges are unsupported or the file
///   is small. Non-2xx responses are errors; partial junk is never left as
///   the output file.
pub fn run(
    url: &str,
    output: Option<&Path>,
    timeout: u64,
    force: bool,
    resume: bool,
    threads: usize,
    format: Format,
) -> Result<()> {
    let out_path: PathBuf = match output {
        Some(p) => p.to_path_buf(),
        None => PathBuf::from(filename_from_url(url)),
    };
    if out_path.is_dir() {
        return Err(format!("output path is a directory: {}", out_path.display()).into());
    }
    if force {
        if out_path.is_file() {
            fs::remove_file(&out_path)?;
        }
        let dir = parts_dir(&out_path);
        if dir.exists() {
            fs::remove_dir_all(&dir)?;
        }
    }
    let existing = if out_path.is_file() {
        fs::metadata(&out_path)?.len()
    } else {
        0
    };
    if existing > 0 && !resume {
        return Err(format!(
            "output exists: {} (use --resume to continue or --force to restart)",
            out_path.display()
        )
        .into());
    }

    let started = Instant::now();
    let outcome = if existing > 0 {
        // Continue an interrupted single-stream download in place.
        single(url, &out_path, existing, timeout)?
    } else if threads > 1 {
        match probe(url, timeout) {
            Probe {
                total: Some(total),
                ranges: true,
            } if total >= MIN_PARALLEL_SIZE => parallel(url, &out_path, total, threads, timeout)?,
            _ => single(url, &out_path, 0, timeout)?,
        }
    } else {
        single(url, &out_path, 0, timeout)?
    };
    let duration_ms = started.elapsed().as_millis();

    let result = DownloadResult {
        url: url.to_string(),
        output: out_path.display().to_string(),
        bytes: outcome.bytes,
        threads: outcome.threads,
        resumed_bytes: outcome.resumed_bytes,
        duration_ms,
    };
    match format {
        Format::Json => output::print_json(&result),
        Format::Plain => {
            let extra = match (result.threads > 1, result.resumed_bytes > 0) {
                (true, true) => format!(
                    ", {} threads, resumed {}",
                    result.threads,
                    human_size(result.resumed_bytes)
                ),
                (true, false) => format!(", {} threads", result.threads),
                (false, true) => format!(", resumed {}", human_size(result.resumed_bytes)),
                _ => String::new(),
            };
            println!(
                "downloaded {} -> {} ({duration_ms} ms{extra})",
                human_size(result.bytes),
                out_path.display()
            );
        }
    }
    Ok(())
}

fn agent(timeout: u64) -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(timeout))
        .build()
}

/// HEAD first; if that doesn't prove range support, probe with a 1-byte GET.
fn probe(url: &str, timeout: u64) -> Probe {
    let agent = agent(timeout.clamp(5, 15));
    if let Ok(resp) = agent.head(url).call() {
        let total = resp.header("Content-Length").and_then(|v| v.parse().ok());
        let ranges = resp
            .header("Accept-Ranges")
            .map(|v| v.trim().eq_ignore_ascii_case("bytes"))
            .unwrap_or(false);
        if total.is_some() && ranges {
            return Probe { total, ranges };
        }
    }
    match agent.get(url).set("Range", "bytes=0-0").call() {
        Ok(resp) if resp.status() == 206 => {
            let total = resp
                .header("Content-Range")
                .and_then(|cr| cr.rsplit('/').next())
                .and_then(|v| v.trim().parse().ok());
            Probe {
                total,
                ranges: total.is_some(),
            }
        }
        _ => Probe {
            total: None,
            ranges: false,
        },
    }
}

fn single(url: &str, out_path: &Path, resume_from: u64, timeout: u64) -> Result<Outcome> {
    let mut req = agent(timeout).get(url);
    if resume_from > 0 {
        req = req.set("Range", &format!("bytes={resume_from}-"));
    }
    let resp = match req.call() {
        Ok(r) => r,
        Err(ureq::Error::Status(code, _)) => {
            return Err(format!("download failed: HTTP {code} for {url}").into())
        }
        Err(ureq::Error::Transport(t)) => return Err(format!("download failed: {t}").into()),
    };
    if resume_from > 0 && resp.status() != 206 {
        return Err(format!(
            "server does not support resume (expected 206, got {}); use --force to restart",
            resp.status()
        )
        .into());
    }
    if let Some(parent) = out_path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }
    let mut opts = OpenOptions::new();
    opts.write(true).create(true);
    if resume_from > 0 {
        opts.append(true);
    } else {
        opts.truncate(true);
    }
    let mut file = opts.open(out_path)?;
    let written = io::copy(&mut resp.into_reader(), &mut file)?;
    Ok(Outcome {
        bytes: resume_from + written,
        threads: 1,
        resumed_bytes: resume_from,
    })
}

fn parts_dir(out: &Path) -> PathBuf {
    PathBuf::from(format!("{}.ckparts", out.display()))
}

fn parallel(
    url: &str,
    out_path: &Path,
    total: u64,
    threads: usize,
    timeout: u64,
) -> Result<Outcome> {
    let dir = parts_dir(out_path);
    let meta_path = dir.join("meta.json");
    // A stale plan (different url/size/threads) makes old parts meaningless.
    if let Ok(text) = fs::read_to_string(&meta_path) {
        if let Ok(m) = serde_json::from_str::<PartsMeta>(&text) {
            if m.url != url || m.total != total || m.threads != threads {
                let _ = fs::remove_dir_all(&dir);
            }
        }
    }
    fs::create_dir_all(&dir)?;
    fs::write(
        &meta_path,
        serde_json::to_string(&PartsMeta {
            url: url.to_string(),
            total,
            threads,
        })?,
    )?;

    let seg = total.div_ceil(threads as u64);
    let mut handles = Vec::new();
    for i in 0..threads as u64 {
        let start = i * seg;
        if start >= total {
            break;
        }
        let end = ((i + 1) * seg).min(total) - 1;
        let part = dir.join(format!("part-{i}"));
        let url = url.to_string();
        handles.push(std::thread::spawn(move || segment(&url, &part, start, end, timeout)));
    }
    let nseg = handles.len();

    let mut resumed_bytes = 0u64;
    let mut failure: Option<String> = None;
    for h in handles {
        match h.join() {
            Ok(Ok(resumed)) => resumed_bytes += resumed,
            Ok(Err(e)) => failure = Some(e.to_string()),
            Err(_) => failure = Some("segment thread panicked".into()),
        }
    }
    if let Some(e) = failure {
        return Err(format!(
            "download incomplete ({e}); parts kept in {} — rerun with the same arguments to continue",
            dir.display()
        )
        .into());
    }

    if let Some(parent) = out_path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }
    let mut out = fs::File::create(out_path)?;
    let mut bytes = 0u64;
    for i in 0..nseg {
        let mut f = fs::File::open(dir.join(format!("part-{i}")))?;
        bytes += io::copy(&mut f, &mut out)?;
    }
    if bytes != total {
        return Err(format!("size mismatch after merge: {bytes} != {total}").into());
    }
    fs::remove_dir_all(&dir)?;
    Ok(Outcome {
        bytes,
        threads: nseg,
        resumed_bytes,
    })
}

/// Download one [start, end] segment into its part file; the part file's
/// current length is the resume position, so interrupted segments continue
/// exactly where they stopped.
fn segment(
    url: &str,
    part: &Path,
    start: u64,
    end: u64,
    timeout: u64,
) -> std::result::Result<u64, String> {
    let want = end - start + 1;
    let mut have = fs::metadata(part).map(|m| m.len()).unwrap_or(0);
    if have == want {
        return Ok(want);
    }
    if have > want {
        // Cannot happen with a consistent plan; treat as corrupt.
        fs::remove_file(part).map_err(|e| e.to_string())?;
        have = 0;
    }
    let resp = agent(timeout)
        .get(url)
        .set("Range", &format!("bytes={}-{}", start + have, end))
        .call()
        .map_err(|e| format!("segment {start}-{end}: {e}"))?;
    if resp.status() != 206 {
        return Err(format!(
            "segment {start}-{end}: server did not honor Range (HTTP {})",
            resp.status()
        ));
    }
    let mut f = OpenOptions::new()
        .create(true)
        .append(true)
        .open(part)
        .map_err(|e| e.to_string())?;
    let n = io::copy(&mut resp.into_reader(), &mut f).map_err(|e| e.to_string())?;
    if have + n != want {
        return Err(format!(
            "segment {start}-{end}: incomplete ({}/{want})",
            have + n
        ));
    }
    Ok(have)
}

fn filename_from_url(url: &str) -> String {
    let path = url.split(['?', '#']).next().unwrap_or(url);
    let name = path.rsplit('/').next().unwrap_or("");
    if name.is_empty() {
        "download.bin".to_string()
    } else {
        name.to_string()
    }
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
