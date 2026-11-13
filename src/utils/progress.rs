use crate::cli::Format;
use crate::utils::output::human_size;
use serde_json::json;
use std::io::{IsTerminal, Write};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Redraw interval when stderr is a terminal (animated bar).
const TICK: Duration = Duration::from_millis(200);
/// Line interval when stderr is piped (one line per tick, agent-friendly).
const LINE: Duration = Duration::from_millis(2_000);

/// Throttled progress reporter for long transfers.
///
/// All output goes to **stderr** so stdout stays a clean data stream:
/// - terminal stderr: animated single-line bar redrawn in place
/// - piped stderr (the agent case): one plain line per interval
/// - `--format json`: NDJSON `{"event":"progress",...}` per interval
///
/// Worker threads only touch the shared counter; the owning thread renders.
pub struct Progress {
    done: Arc<AtomicU64>,
    /// Bytes already present before this run (resume); excluded from speed.
    base: u64,
    total: Option<u64>,
    json: bool,
    tty: bool,
    enabled: bool,
    started: Instant,
    last_draw: Instant,
    drew_any: bool,
    finished: bool,
}

impl Progress {
    /// `already` is the byte count present before this run starts (resume).
    pub fn new(format: Format, already: u64, enabled: bool) -> Self {
        Progress {
            done: Arc::new(AtomicU64::new(already)),
            base: already,
            total: None,
            json: matches!(format, Format::Json),
            tty: std::io::stderr().is_terminal(),
            enabled,
            started: Instant::now(),
            last_draw: Instant::now(),
            drew_any: false,
            finished: false,
        }
    }

    /// Total size, once known (single streams learn it from Content-Length
    /// after the request starts). Ignored after the first draw.
    pub fn set_total(&mut self, total: Option<u64>) {
        if self.enabled && !self.drew_any {
            self.total = total;
        }
    }

    /// Shared counter handed to worker threads; they always count, the
    /// owning thread decides whether anything is rendered.
    pub fn counter(&self) -> Arc<AtomicU64> {
        Arc::clone(&self.done)
    }

    /// Count `n` freshly transferred bytes and draw if the interval elapsed.
    pub fn add(&mut self, n: u64) {
        if !self.enabled {
            return;
        }
        self.done.fetch_add(n, Ordering::Relaxed);
        self.maybe_draw();
    }

    /// Draw if the throttle interval has elapsed (parallel poll loop).
    pub fn maybe_draw(&mut self) {
        if !self.enabled || self.last_draw.elapsed() < self.interval() {
            return;
        }
        self.draw(false);
    }

    /// Final frame: always drawn once, terminates the line in terminal mode.
    pub fn finish(&mut self) {
        if self.enabled && !self.finished {
            self.draw(true);
            self.finished = true;
        }
    }

    fn interval(&self) -> Duration {
        if self.tty {
            TICK
        } else {
            LINE
        }
    }

    fn draw(&mut self, final_: bool) {
        let done = self.done.load(Ordering::Relaxed);
        let elapsed = self.started.elapsed().as_secs_f64().max(f64::EPSILON);
        let run_bytes = done.saturating_sub(self.base) as f64;
        let speed = run_bytes / elapsed;
        let percent = self.total.map(|t| {
            if t == 0 {
                100.0
            } else {
                done as f64 / t as f64 * 100.0
            }
        });
        let eta = match self.total {
            Some(t) if speed > 0.0 && done < t => Some((t - done) as f64 / speed),
            _ => None,
        };
        let line = if self.json {
            json!({
                "event": "progress",
                "downloaded": done,
                "total": self.total,
                "percent": percent.map(round1),
                "bytes_per_sec": speed as u64,
                "eta_seconds": eta.map(round1),
            })
            .to_string()
        } else {
            plain_line(done, percent, speed, eta)
        };
        let mut err = std::io::stderr();
        if self.tty {
            // Redraw in place; only the final frame ends the line so the
            // command's own result line starts fresh.
            let _ = write!(err, "\r{line}\x1b[K{}", if final_ { "\n" } else { "" });
        } else {
            let _ = writeln!(err, "{line}");
        }
        let _ = err.flush();
        self.last_draw = Instant::now();
        self.drew_any = true;
    }
}

impl Drop for Progress {
    fn drop(&mut self) {
        // An aborted run must not leave a half-drawn bar without a newline.
        if self.tty && self.drew_any && !self.finished {
            let _ = writeln!(std::io::stderr());
        }
    }
}

fn round1(v: f64) -> f64 {
    (v * 10.0).round() / 10.0
}

fn plain_line(done: u64, percent: Option<f64>, speed: f64, eta: Option<f64>) -> String {
    let mut s = String::from("downloading ");
    match percent {
        Some(p) => s.push_str(&format!("{p:.1}% {} {}", bar(p), human_size(done))),
        None => s.push_str(&human_size(done)),
    }
    s.push_str(&format!(" {}/s", human_size(speed as u64)));
    if let Some(e) = eta {
        s.push_str(&format!(" eta {}", human_duration(e)));
    }
    s
}

fn bar(percent: f64) -> String {
    const WIDTH: usize = 20;
    let filled = ((percent / 100.0) * WIDTH as f64)
        .round()
        .clamp(0.0, WIDTH as f64) as usize;
    format!("[{}{}]", "#".repeat(filled), "-".repeat(WIDTH - filled))
}

fn human_duration(secs: f64) -> String {
    let s = secs as u64;
    if s < 60 {
        format!("{s}s")
    } else {
        format!("{}m{}s", s / 60, s % 60)
    }
}
