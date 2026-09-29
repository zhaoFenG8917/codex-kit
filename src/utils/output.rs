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
