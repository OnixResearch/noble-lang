//! The L1 adapter against fixture children (issue #75).
//!
//! Each fixture is a `/bin/sh` script standing in for `noble`. It prints
//! prepared `noble-core-report/v1` lines, optionally consumes its stdin, and
//! ends as the case requires. The lines follow the real CLI's protocol: the
//! profile, backend, 1-based `submission` ordinal and, on runtime reports, the
//! module record with the SHA-256 of the exact submitted text and the `--opt`
//! leg. Each case changes one fact and expects the named failure class.

use noble_ngraph_experiment::l1::{self, Cli, Error, Opt, SessionFailure};
use serde_json::json;
use std::path::PathBuf;
use std::sync::{Mutex, PoisonError};

/// Fixture children are written and spawned one case at a time, so no other
/// test thread forks while a script is still open for writing (`ETXTBSY`).
static SPAWN: Mutex<()> = Mutex::new(());

const DEFINITION: &str = "def sq [ dup * ]";
const ROOT: &str = "sq +";

/// A fixture `noble` in its own directory, removed afterwards.
struct Script {
    dir: PathBuf,
    path: PathBuf,
}

impl Script {
    fn new(name: &str, body: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("ngraph-l1-{}-{name}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("fixture directory");
        let path = dir.join("noble");
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).expect("fixture script");
        let mut permissions = std::fs::metadata(&path)
            .expect("fixture metadata")
            .permissions();
        std::os::unix::fs::PermissionsExt::set_mode(&mut permissions, 0o755);
        std::fs::set_permissions(&path, permissions).expect("fixture mode");
        Self { dir, path }
    }

    fn cli(&self, opt: Opt) -> Cli {
        Cli::new(self.path.clone(), opt)
    }
}

impl Drop for Script {
    fn drop(&mut self) {
        if let Err(e) = std::fs::remove_dir_all(&self.dir) {
            eprintln!("fixture cleanup {}: {e}", self.dir.display());
        }
    }
}

fn quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', "'\\''"))
}

/// A script body: optionally consume stdin, print `lines`, then `ending`.
fn body(lines: &[String], consume: bool, ending: &str) -> String {
    let read = if consume {
        "while read -r line; do :; done\n"
    } else {
        ""
    };
    let print = if lines.is_empty() {
        String::new()
    } else {
        format!(
            "printf '%s\\n' {}\n",
            lines.iter().map(|l| quote(l)).collect::<Vec<_>>().join(" ")
        )
    };
    format!("{read}{print}{ending}")
}

/// A body that answers its first invocation with `first` and every later one
/// with `later`, as a fresh process after a restart or a chunk boundary.
fn two_processes(first: &str, later: &str) -> String {
    format!("if [ -e \"$0.later\" ]; then\n{later}\nelse\n: > \"$0.later\"\n{first}\nfi")
}

fn digest(text: &str) -> String {
    noble_ngraph_experiment::bench::sha256_text(text).expect("sha256sum")
}

fn runtime_line(ordinal: u64, source: &str, opt: Opt, outcome: &str, stack: &[i64]) -> String {
    let is_normal = outcome == "normal";
    json!({
        "schema": l1::SCHEMA,
        "profile": l1::PROFILE,
        "backend": l1::BACKEND,
        "submission": ordinal,
        "stage": "wasm",
        "outcome": outcome,
        "status": if is_normal { 0 } else { 5 },
        "stack": stack
            .iter()
            .map(|v| json!({ "handle": null, "type": "I64", "value": v.to_string() }))
            .collect::<Vec<_>>(),
        "request_trace": [],
        "session_state": if is_normal { "retained" } else { "terminated-after-runtime-failure" },
        "module": { "submission": ordinal, "source_sha256": digest(source), "optimized": opt.optimized() },
    })
    .to_string()
}

fn static_line(ordinal: u64, stage: &str, outcome: &str) -> String {
    json!({
        "schema": l1::SCHEMA,
        "profile": l1::PROFILE,
        "backend": l1::BACKEND,
        "submission": ordinal,
        "stage": stage,
        "outcome": outcome,
        "diagnostic": "fixture",
    })
    .to_string()
}

fn vectors(count: usize) -> Vec<Vec<l1::Value>> {
    vec![vec![l1::Value::I64(5), l1::Value::I64(4)]; count]
}

/// The exact reports for `sq` and `sq +` over two `5 4` vectors.
fn good_lines(opt: Opt) -> Vec<String> {
    vec![
        static_line(1, "check", "defined"),
        runtime_line(2, "5 4", opt, "normal", &[5, 4]),
        runtime_line(3, ROOT, opt, "normal", &[21]),
        runtime_line(4, "drop 5 4", opt, "normal", &[5, 4]),
        runtime_line(5, ROOT, opt, "normal", &[21]),
    ]
}

fn edit(line: &str, change: impl FnOnce(&mut serde_json::Value)) -> String {
    let mut report: serde_json::Value = serde_json::from_str(line).expect("fixture JSON");
    change(&mut report);
    report.to_string()
}

fn expect_failure<T>(result: Result<T, Error>) -> SessionFailure {
    match result {
        Err(Error::Session(failure)) => *failure,
        Err(other) => panic!("expected a session failure, got {other}"),
        Ok(_) => panic!("expected a session failure, got a completed run"),
    }
}

fn run_with(name: &str, lines: &[String], ending: &str) -> Result<l1::Run<l1::Report>, Error> {
    let _guard = SPAWN.lock().unwrap_or_else(PoisonError::into_inner);
    let script = Script::new(name, &body(lines, true, ending));
    script
        .cli(Opt::Off)
        .run_vectors(&[DEFINITION], ROOT, 1, &vectors(2))
}

fn assert_class(failure: &SessionFailure, class: &str, phrase: &str) {
    assert_eq!(failure.class, class, "{failure:?}");
    assert!(
        failure.reason.contains(phrase),
        "{phrase:?} not in {:?}",
        failure.reason
    );
}

mod accepted;
mod lifecycle;
mod rejected;
