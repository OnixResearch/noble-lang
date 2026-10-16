//! Measurement helpers shared by the evidence harnesses: order statistics,
//! calibrated warm latency and host facts.

use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

use crate::json::Json;

/// Reads a host file, trimmed; `unsupported` when unreadable.
#[must_use]
pub fn read(path: &str) -> String {
    std::fs::read_to_string(path).map_or_else(|_| "unsupported".into(), |s| s.trim().to_owned())
}

/// `sha256sum` of a file; `unsupported` when it cannot be computed.
#[must_use]
pub fn sha256(path: &Path) -> String {
    Command::new("sha256sum")
        .arg(path)
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .and_then(|s| s.split_whitespace().next().map(str::to_owned))
        .unwrap_or_else(|| "unsupported".to_owned())
}

/// The first `model name` of `/proc/cpuinfo`.
#[must_use]
pub fn cpu_model() -> String {
    read("/proc/cpuinfo")
        .lines()
        .find(|l| l.starts_with("model name"))
        .and_then(|l| l.split(':').nth(1))
        .map_or_else(|| "unsupported".into(), |s| s.trim().to_owned())
}

/// Order statistics in nanoseconds (`n`, median, p25, p75, IQR, min, max)
/// and the median.
///
/// # Panics
/// On an empty sample.
#[must_use]
pub fn stats(mut v: Vec<f64>) -> (Json, f64) {
    v.sort_by(f64::total_cmp);
    let q = |p: f64| {
        #[allow(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            clippy::cast_precision_loss
        )]
        let k = ((v.len() - 1) as f64 * p).round() as usize;
        v[k]
    };
    let med = q(0.5);
    (
        Json::obj([
            ("n", Json::U(v.len() as u64)),
            ("median_ns", Json::F(med)),
            ("p25_ns", Json::F(q(0.25))),
            ("p75_ns", Json::F(q(0.75))),
            ("iqr_ns", Json::F(q(0.75) - q(0.25))),
            ("min_ns", Json::F(v[0])),
            ("max_ns", Json::F(v[v.len() - 1])),
        ]),
        med,
    )
}

/// A duration in nanoseconds.
#[must_use]
pub fn ns(d: Duration) -> f64 {
    d.as_secs_f64() * 1e9
}

/// Warm latency: `samples` samples of a calibrated batch (≥ 20 µs); returns
/// stats JSON (with `batch_per_sample`), median ns/eval and the batch size.
pub fn warm(samples: usize, mut f: impl FnMut()) -> (Json, f64, u64) {
    for _ in 0..50 {
        f();
    }
    let mut batch = 1u64;
    loop {
        let t = Instant::now();
        for _ in 0..batch {
            f();
        }
        if t.elapsed() >= Duration::from_micros(20) || batch >= 1 << 20 {
            break;
        }
        batch *= 2;
    }
    let mut v = Vec::with_capacity(samples);
    for _ in 0..samples {
        let t = Instant::now();
        for _ in 0..batch {
            f();
        }
        #[allow(clippy::cast_precision_loss)]
        v.push(ns(t.elapsed()) / batch as f64);
    }
    let (mut j, med) = stats(v);
    j.push("batch_per_sample", Json::U(batch));
    (j, med, batch)
}
