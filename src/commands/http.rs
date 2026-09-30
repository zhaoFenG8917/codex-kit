use crate::cli::Format;
use crate::utils::output;
use crate::utils::Result;
use serde::Serialize;
use std::collections::BTreeMap;
use std::time::{Duration, Instant};

#[derive(Serialize)]
struct HttpResult {
    url: String,
    method: String,
    status: u16,
    duration_ms: u128,
    headers: BTreeMap<String, String>,
    body: String,
}

/// Cross-platform HTTP client for agents. Any HTTP response (including 4xx/5xx)
/// is reported as data with exit code 0; only transport-level failures
/// (DNS, connection refused, timeout) produce a non-zero exit. The response
/// body is decoded according to the server's charset header (GBK-safe).
pub fn run(
    url: &str,
    method: &str,
    headers: &[String],
    data: Option<&str>,
    timeout: u64,
    format: Format,
) -> Result<()> {
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(timeout))
        .build();
    let mut req = agent.request(&method.to_uppercase(), url);
    for h in headers {
        let (k, v) = h
            .split_once(':')
            .ok_or_else(|| format!("invalid header '{h}' (use 'Name: value')"))?;
        req = req.set(k.trim(), v.trim());
    }

    let started = Instant::now();
    let outcome = match data {
        Some(body) => req.send_string(body),
        None => req.call(),
    };
    let response = match outcome {
        Ok(r) => r,
        // An HTTP error status is still a valid response; report it as data.
        Err(ureq::Error::Status(_, r)) => r,
        Err(ureq::Error::Transport(t)) => return Err(format!("request to {url} failed: {t}").into()),
    };

    let status = response.status();
    let mut resp_headers = BTreeMap::new();
    for name in response.headers_names() {
        if let Some(v) = response.header(&name) {
            resp_headers.insert(name, v.to_string());
        }
    }
    let body = response
        .into_string()
        .unwrap_or_else(|_| "<binary or undecodable body>".to_string());
    let duration_ms = started.elapsed().as_millis();

    let result = HttpResult {
        url: url.to_string(),
        method: method.to_uppercase(),
        status,
        duration_ms,
        headers: resp_headers,
        body: body.clone(),
    };
    match format {
        Format::Json => output::print_json(&result),
        Format::Plain => {
            println!("HTTP {status} · {duration_ms} ms");
            print!("{body}");
            if !body.ends_with('\n') && !body.is_empty() {
                println!();
            }
        }
    }
    Ok(())
}
