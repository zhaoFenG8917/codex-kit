use crate::cli::Format;
use crate::utils::output;
use crate::utils::Result;
use serde::Serialize;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Serialize)]
struct ArchiveResult {
    archive: String,
    files: usize,
}

#[derive(Serialize)]
struct ExtractResult {
    archive: String,
    dest: String,
    files: usize,
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Zip,
    TarGz,
}

fn detect(path: &Path) -> Result<Kind> {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    if name.ends_with(".zip") {
        Ok(Kind::Zip)
    } else if name.ends_with(".tar.gz") || name.ends_with(".tgz") {
        Ok(Kind::TarGz)
    } else {
        Err(format!("unknown archive format '{name}' (use .zip or .tar.gz)").into())
    }
}

/// Collect (disk path, archive-internal name) pairs for every input.
fn collect_entries(inputs: &[PathBuf]) -> Result<Vec<(PathBuf, String)>> {
    let mut entries = Vec::new();
    for input in inputs {
        if !input.exists() {
            return Err(format!("input not found: {}", input.display()).into());
        }
        let base = input
            .file_name()
            .ok_or_else(|| format!("invalid input path: {}", input.display()))?
            .to_string_lossy()
            .to_string();
        if input.is_dir() {
            for entry in WalkDir::new(input) {
                let entry = entry.map_err(|e| e.to_string())?;
                let rel = entry
                    .path()
                    .strip_prefix(input)
                    .map_err(|e| e.to_string())?;
                if rel.as_os_str().is_empty() {
                    continue;
                }
                let name = format!("{}/{}", base, rel.to_string_lossy().replace('\\', "/"));
                entries.push((entry.path().to_path_buf(), name));
            }
        } else {
            entries.push((input.clone(), base));
        }
    }
    Ok(entries)
}

pub fn archive(output: &Path, inputs: &[PathBuf], format: Format) -> Result<()> {
    let kind = detect(output)?;
    let entries = collect_entries(inputs)?;
    let count = match kind {
        Kind::Zip => write_zip(output, &entries)?,
        Kind::TarGz => write_targz(output, &entries)?,
    };
    let result = ArchiveResult {
        archive: output.display().to_string(),
        files: count,
    };
    match format {
        Format::Json => output::print_json(&result),
        Format::Plain => println!("Created {} ({} entries)", output.display(), count),
    }
    Ok(())
}

pub fn extract(archive: &Path, dest: Option<&Path>, format: Format) -> Result<()> {
    if !archive.is_file() {
        return Err(format!("archive not found: {}", archive.display()).into());
    }
    let kind = detect(archive)?;
    let dest = dest.map(Path::to_path_buf).unwrap_or_else(|| PathBuf::from("."));
    fs::create_dir_all(&dest)?;
    let count = match kind {
        Kind::Zip => extract_zip(archive, &dest)?,
        Kind::TarGz => extract_targz(archive, &dest)?,
    };
    let result = ExtractResult {
        archive: archive.display().to_string(),
        dest: dest.display().to_string(),
        files: count,
    };
    match format {
        Format::Json => output::print_json(&result),
        Format::Plain => println!("Extracted {} entries to {}", count, dest.display()),
    }
    Ok(())
}

fn write_zip(output: &Path, entries: &[(PathBuf, String)]) -> Result<usize> {
    use zip::write::SimpleFileOptions;
    let file = fs::File::create(output)?;
    let mut zip = zip::ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    let mut count = 0;
    for (disk, name) in entries {
        if disk.is_dir() {
            zip.add_directory(format!("{name}/"), options)?;
        } else {
            zip.start_file(name, options)?;
            zip.write_all(&fs::read(disk)?)?;
        }
        count += 1;
    }
    zip.finish()?;
    Ok(count)
}

fn extract_zip(archive: &Path, dest: &Path) -> Result<usize> {
    let file = fs::File::open(archive)?;
    let mut zip = zip::ZipArchive::new(file)?;
    let mut count = 0;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i)?;
        // enclosed_name() rejects entries that would escape `dest` (zip-slip).
        let Some(safe_name) = entry.enclosed_name() else {
            continue;
        };
        let outpath = dest.join(safe_name);
        if entry.name().ends_with('/') {
            fs::create_dir_all(&outpath)?;
        } else {
            if let Some(parent) = outpath.parent() {
                fs::create_dir_all(parent)?;
            }
            let mut outfile = fs::File::create(&outpath)?;
            io::copy(&mut entry, &mut outfile)?;
        }
        count += 1;
    }
    Ok(count)
}

fn write_targz(output: &Path, entries: &[(PathBuf, String)]) -> Result<usize> {
    let file = fs::File::create(output)?;
    let enc = flate2::write::GzEncoder::new(file, flate2::Compression::default());
    let mut tar = tar::Builder::new(enc);
    let mut count = 0;
    for (disk, name) in entries {
        if disk.is_dir() {
            tar.append_dir(name, disk)?;
        } else {
            let mut f = fs::File::open(disk)?;
            tar.append_file(name, &mut f)?;
        }
        count += 1;
    }
    let enc = tar.into_inner()?;
    enc.finish()?;
    Ok(count)
}

fn extract_targz(archive: &Path, dest: &Path) -> Result<usize> {
    let file = fs::File::open(archive)?;
    let dec = flate2::read::GzDecoder::new(file);
    let mut tar = tar::Archive::new(dec);
    let mut count = 0;
    for entry in tar.entries()? {
        let mut entry = entry?;
        // Reject paths that would escape `dest`.
        let path = entry.path()?;
        if path.components().any(|c| matches!(c, std::path::Component::ParentDir)) {
            continue;
        }
        let owned = path.into_owned();
        if let Some(parent) = dest.join(&owned).parent().map(|p| p.to_path_buf()) {
            fs::create_dir_all(parent)?;
        }
        entry.unpack(dest.join(&owned))?;
        count += 1;
    }
    Ok(count)
}
