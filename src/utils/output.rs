use crate::cli::Format;
use serde::Serialize;

/// Print a value as compact JSON on stdout.
pub fn print_json<T: Serialize>(value: &T) {
    match serde_json::to_string(value) {
        Ok(s) => println!("{s}"),
        Err(e) => print_error(Format::Json, &format!("failed to serialize output: {e}")),
    }
}

/// Errors go to stderr in plain mode; in JSON mode stdout carries {"error": ...}
/// so machine parsers never see mixed content on the wrong stream.
pub fn print_error(format: Format, message: &str) {
    match format {
        Format::Json => println!("{}", serde_json::json!({ "error": message })),
        Format::Plain => eprintln!("Error: {message}"),
    }
}

/// Human-readable byte size, e.g. "1.5 MB". Shared by download and its
/// progress reporter so both frames agree on units.
pub fn human_size(size: u64) -> String {
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
