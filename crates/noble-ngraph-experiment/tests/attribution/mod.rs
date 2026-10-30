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
    let normal = outcome == "normal";
    json!({
        "schema": l1::SCHEMA,
        "profile": l1::PROFILE,
        "backend": l1::BACKEND,
        "submission": ordinal,
        "stage": "wasm",
        "outcome": outcome,
        "status": if normal { 0 } else { 5 },
        "stack": stack
            .iter()
            .map(|v| json!({ "handle": null, "type": "I64", "value": v.to_string() }))
            .collect::<Vec<_>>(),
        "request_trace": [],
        "session_state": if normal { "retained" } else { "terminated-after-runtime-failure" },
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

#[test]
fn complete_runs_keep_their_results_in_both_legs() {
    for opt in [Opt::Off, Opt::On] {
        let _guard = SPAWN.lock().unwrap_or_else(PoisonError::into_inner);
        let script = Script::new(
            &format!("complete-{}", opt.flag()),
            &body(&good_lines(opt), true, "exit 0"),
        );
        let run = script
            .cli(opt)
            .run_vectors(&[DEFINITION], ROOT, 1, &vectors(2))
            .expect("a complete, attributed run");
        let stacks: Vec<_> = run.results.iter().map(|r| r.stack.clone()).collect();
        assert_eq!(
            stacks,
            vec![vec![l1::Value::I64(21)]; 2],
            "repeated inputs keep repeated results"
        );
        assert_eq!(run.restarts, 0);
    }
}

#[test]
fn zero_output_roots_keep_empty_stacks() {
    let _guard = SPAWN.lock().unwrap_or_else(PoisonError::into_inner);
    let lines = [
        runtime_line(1, "5 4", Opt::Off, "normal", &[5, 4]),
        runtime_line(2, "drop drop", Opt::Off, "normal", &[]),
        runtime_line(3, "5 4", Opt::Off, "normal", &[5, 4]),
        runtime_line(4, "drop drop", Opt::Off, "normal", &[]),
    ];
    let script = Script::new("zero-output", &body(&lines, true, "exit 0"));
    let run = script
        .cli(Opt::Off)
        .run_vectors(&[], "drop drop", 0, &vectors(2))
        .expect("complete");
    assert!(run
        .results
        .iter()
        .all(|r| r.stack.is_empty() && r.outcome == "normal"));
    assert_eq!(run.results.len(), 2);
}

#[test]
fn a_quota_restart_starts_ordinals_again() {
    let _guard = SPAWN.lock().unwrap_or_else(PoisonError::into_inner);
    let first = [
        static_line(1, "check", "defined"),
        runtime_line(2, "5 4", Opt::Off, "normal", &[5, 4]),
        runtime_line(3, ROOT, Opt::Off, "normal", &[21]),
        runtime_line(4, "drop 5 4", Opt::Off, "normal", &[5, 4]),
        runtime_line(5, ROOT, Opt::Off, "runtime-exhausted", &[]),
    ];
    let later = [
        static_line(1, "check", "defined"),
        runtime_line(2, "5 4", Opt::Off, "normal", &[5, 4]),
        runtime_line(3, ROOT, Opt::Off, "normal", &[21]),
        runtime_line(4, "drop 5 4", Opt::Off, "normal", &[5, 4]),
        runtime_line(5, ROOT, Opt::Off, "normal", &[21]),
    ];
    let script = Script::new(
        "quota-restart",
        &two_processes(
            &body(&first, false, "exit 5"),
            &body(&later, true, "exit 0"),
        ),
    );
    let run = script
        .cli(Opt::Off)
        .run_vectors(&[DEFINITION], ROOT, 1, &vectors(3))
        .expect("restarted run");
    assert_eq!(run.restarts, 1);
    assert_eq!(run.results.len(), 3);
    assert!(run
        .results
        .iter()
        .all(|r| r.stack == vec![l1::Value::I64(21)]));
}

#[test]
fn a_chunk_boundary_starts_a_fresh_ordinal_sequence() {
    let _guard = SPAWN.lock().unwrap_or_else(PoisonError::into_inner);
    let per_chunk = l1::MAX_SUBMISSIONS / 2;
    let lines_for = |vectors: usize| -> Vec<String> {
        (0..vectors)
            .flat_map(|i| {
                let input = if i == 0 { "5 4" } else { "drop 5 4" };
                let at = u64::try_from(2 * i).expect("ordinal");
                [
                    runtime_line(at + 1, input, Opt::Off, "normal", &[5, 4]),
                    runtime_line(at + 2, "+", Opt::Off, "normal", &[9]),
                ]
            })
            .collect()
    };
    let script = Script::new(
        "chunk-boundary",
        &two_processes(
            &body(&lines_for(per_chunk), true, "exit 0"),
            &body(&lines_for(1), true, "exit 0"),
        ),
    );
    let run = script
        .cli(Opt::Off)
        .run_vectors(&[], "+", 1, &vectors(per_chunk + 1))
        .expect("two chunks");
    assert_eq!(run.results.len(), per_chunk + 1);
    assert_eq!(run.restarts, 0);
}

#[test]
fn legitimate_refusals_and_terminal_outcomes_are_observations() {
    let _guard = SPAWN.lock().unwrap_or_else(PoisonError::into_inner);
    let refused = [
        runtime_line(1, "5 4", Opt::Off, "normal", &[5, 4]),
        static_line(2, "resolve", "unbound-word"),
    ];
    let script = Script::new("refusal", &body(&refused, true, "exit 2"));
    let reports = script
        .cli(Opt::Off)
        .session(&["5 4".to_owned(), "nope".to_owned()])
        .expect("a refusal is an observation");
    assert_eq!(
        reports
            .iter()
            .map(|r| r.outcome.as_str())
            .collect::<Vec<_>>(),
        ["normal", "unbound-word"]
    );

    let trapped = [
        runtime_line(1, "1 0", Opt::Off, "normal", &[1, 0]),
        runtime_line(2, "/", Opt::Off, "trap", &[]),
    ];
    let early = Script::new("terminal", &body(&trapped, false, "exit 1"));
    let submissions = ["1 0".to_owned(), "/".to_owned(), "9".to_owned()];
    let reports = early
        .cli(Opt::Off)
        .session(&submissions)
        .expect("a terminal outcome may end the run early");
    assert_eq!(reports.len(), 2);
}

#[test]
fn wrong_or_missing_identity_fields_fail_as_protocol() {
    let lines = good_lines(Opt::Off);
    let cases: [(&str, String, &str); 4] = [
        (
            "wrong-schema",
            edit(&lines[2], |r| r["schema"] = json!("noble-core-report/v0")),
            "schema",
        ),
        (
            "missing-schema",
            edit(&lines[2], |r| {
                r.as_object_mut().expect("object").remove("schema");
            }),
            "schema None",
        ),
        (
            "wrong-profile",
            edit(&lines[2], |r| r["profile"] = json!("Calculator-v1")),
            "profile",
        ),
        (
            "missing-ordinal",
            edit(&lines[2], |r| {
                r.as_object_mut().expect("object").remove("submission");
            }),
            "names submission None",
        ),
    ];
    for (name, replaced, phrase) in cases {
        let mut changed = lines.clone();
        changed[2] = replaced;
        assert_class(
            &expect_failure(run_with(name, &changed, "exit 0")),
            l1::PROTOCOL,
            phrase,
        );
    }
}

#[test]
fn duplicated_reordered_extra_and_missing_lines_fail_as_protocol() {
    let lines = good_lines(Opt::Off);
    let mut duplicated = lines.clone();
    duplicated[3] = edit(&lines[3], |r| {
        r["submission"] = json!(3);
        r["module"]["submission"] = json!(3);
    });
    assert_class(
        &expect_failure(run_with("duplicate", &duplicated, "exit 0")),
        l1::PROTOCOL,
        "names submission Some(3)",
    );

    let mut reordered = lines.clone();
    reordered.swap(3, 4);
    assert_class(
        &expect_failure(run_with("reordered", &reordered, "exit 0")),
        l1::PROTOCOL,
        "names submission Some(5)",
    );

    let mut extra = lines.clone();
    extra.push(runtime_line(6, ROOT, Opt::Off, "normal", &[21]));
    assert_class(
        &expect_failure(run_with("extra", &extra, "exit 0")),
        l1::PROTOCOL,
        "extra report",
    );

    let truncated = &lines[..4];
    let failure = expect_failure(run_with("truncated", truncated, "exit 0"));
    assert_class(&failure, l1::PROTOCOL, "4 reports for 5 submissions");
    assert_eq!(failure.validated, 4, "the validated prefix is kept");
}

#[test]
fn malformed_runtime_reports_fail_as_protocol() {
    let lines = good_lines(Opt::Off);
    let cases: [(&str, String, &str); 3] = [
        (
            "no-stack",
            edit(&lines[2], |r| {
                r.as_object_mut().expect("object").remove("stack");
            }),
            "`stack`",
        ),
        (
            "bad-value",
            edit(&lines[2], |r| r["stack"][0]["value"] = json!("21x")),
            "bad I64",
        ),
        (
            "nonzero-status",
            edit(&lines[2], |r| r["status"] = json!(3)),
            "status 3",
        ),
    ];
    for (name, replaced, phrase) in cases {
        let mut changed = lines.clone();
        changed[2] = replaced;
        assert_class(
            &expect_failure(run_with(name, &changed, "exit 0")),
            l1::PROTOCOL,
            phrase,
        );
    }
}

#[test]
fn a_root_cannot_be_answered_by_another_submissions_report() {
    let lines = good_lines(Opt::Off);
    let cases: [(&str, usize, String, &str); 5] = [
        (
            "definition-as-root",
            2,
            static_line(3, "check", "defined"),
            "reported defined",
        ),
        (
            "input-replayed-as-root",
            2,
            lines[1].clone(),
            "names submission Some(2)",
        ),
        (
            "input-renumbered-as-root",
            2,
            runtime_line(3, "5 4", Opt::Off, "normal", &[21]),
            "source digest",
        ),
        (
            "root-replayed-on-repeated-input",
            4,
            lines[2].clone(),
            "names submission Some(3)",
        ),
        (
            "normal-definition",
            0,
            runtime_line(1, DEFINITION, Opt::Off, "normal", &[]),
            "definition submission reported normal",
        ),
    ];
    for (name, at, replaced, phrase) in cases {
        let mut changed = lines.clone();
        changed[at] = replaced;
        assert_class(
            &expect_failure(run_with(name, &changed, "exit 0")),
            l1::PROTOCOL,
            phrase,
        );
    }
}

/// The arithmetic control stays separate: a fresh, correctly attributed root
/// with a wrong value is not a protocol failure. It keeps its value, so the
/// differential comparison is what counts it as a numeric disagreement.
#[test]
fn a_fresh_report_with_a_wrong_value_is_left_to_the_numeric_comparison() {
    let mut lines = good_lines(Opt::Off);
    lines[2] = runtime_line(3, ROOT, Opt::Off, "normal", &[22]);
    let run = run_with("fresh-wrong-value", &lines, "exit 0")
        .expect("an attributed report is accepted whatever its value");
    let stacks: Vec<_> = run.results.iter().map(|r| r.stack.clone()).collect();
    assert_eq!(
        stacks,
        vec![vec![l1::Value::I64(22)], vec![l1::Value::I64(21)]]
    );
}

#[test]
fn mutated_module_identity_fails_as_protocol() {
    let lines = good_lines(Opt::Off);
    let mut digest_changed = lines.clone();
    digest_changed[4] = edit(&lines[4], |r| {
        r["module"]["source_sha256"] = json!("0".repeat(64))
    });
    assert_class(
        &expect_failure(run_with("digest", &digest_changed, "exit 0")),
        l1::PROTOCOL,
        "source digest",
    );

    let mut leg_changed = lines.clone();
    leg_changed[2] = edit(&lines[2], |r| r["module"]["optimized"] = json!(true));
    assert_class(
        &expect_failure(run_with("leg", &leg_changed, "exit 0")),
        l1::PROTOCOL,
        "optimization",
    );
}

#[test]
fn an_unexplained_exit_or_signal_fails_as_process() {
    let lines = good_lines(Opt::Off);
    let exited = expect_failure(run_with("nonzero-exit", &lines, "exit 3"));
    assert_class(&exited, l1::PROCESS, "exit status Some(3)");
    assert_eq!(
        exited.validated, 5,
        "every numeric result printed, yet no completed run"
    );

    let crashed = expect_failure(run_with("crash", &lines, "kill -9 $$"));
    assert_class(&crashed, l1::PROCESS, "signal 9");
    assert_eq!(crashed.signal, Some(9));

    let _guard = SPAWN.lock().unwrap_or_else(PoisonError::into_inner);
    let refused = [
        runtime_line(1, "5 4", Opt::Off, "normal", &[5, 4]),
        static_line(2, "resolve", "unbound-word"),
    ];
    let script = Script::new("refusal-exit-0", &body(&refused, true, "exit 0"));
    let failure = expect_failure(
        script
            .cli(Opt::Off)
            .session(&["5 4".to_owned(), "nope".to_owned()]),
    );
    assert_class(&failure, l1::PROCESS, "outcomes give 2");
}

#[test]
fn undelivered_input_fails_as_transport() {
    let _guard = SPAWN.lock().unwrap_or_else(PoisonError::into_inner);
    // More than a pipe buffer, and the fixture never reads it.
    let large = "1 drop ".repeat(1 << 18);
    let lines = [
        runtime_line(1, "5 4", Opt::Off, "normal", &[5, 4]),
        runtime_line(2, &large, Opt::Off, "normal", &[5, 4]),
    ];
    let script = Script::new("undelivered", &body(&lines, false, "exit 0"));
    let failure = expect_failure(
        script
            .cli(Opt::Off)
            .session(&["5 4".to_owned(), large.clone()]),
    );
    assert_class(&failure, l1::TRANSPORT, "not all delivered");
    assert!(failure.write_error.is_some());
}

#[test]
fn a_line_after_a_terminal_outcome_or_past_the_bound_fails_as_protocol() {
    let _guard = SPAWN.lock().unwrap_or_else(PoisonError::into_inner);
    let lines = [
        runtime_line(1, "1 0", Opt::Off, "normal", &[1, 0]),
        runtime_line(2, "/", Opt::Off, "trap", &[]),
        runtime_line(3, "9", Opt::Off, "normal", &[1, 0, 9]),
    ];
    let script = Script::new("after-terminal", &body(&lines, true, "exit 1"));
    let submissions = ["1 0".to_owned(), "/".to_owned(), "9".to_owned()];
    assert_class(
        &expect_failure(script.cli(Opt::Off).session(&submissions)),
        l1::PROTOCOL,
        "follows the terminal",
    );

    let oversized = [format!(
        "{{\"padding\":\"{}\"}}",
        "x".repeat(l1::MAX_REPORT_BYTES)
    )];
    let script = Script::new("oversized", &body(&oversized, true, "exit 0"));
    assert_class(
        &expect_failure(script.cli(Opt::Off).session(&["5 4".to_owned()])),
        l1::PROTOCOL,
        "exceeds",
    );
}

#[test]
fn a_running_child_with_an_invalid_stream_is_stopped_and_reaped() {
    let _guard = SPAWN.lock().unwrap_or_else(PoisonError::into_inner);
    let wrong = [edit(
        &runtime_line(1, "5 4", Opt::Off, "normal", &[5, 4]),
        |r| r["backend"] = json!("gc"),
    )];
    let script = Script::new("stopped", &body(&wrong, false, "while :; do :; done"));
    let failure = expect_failure(script.cli(Opt::Off).session(&["5 4".to_owned()]));
    assert_class(&failure, l1::PROTOCOL, "backend");
    assert_eq!(
        failure.signal,
        Some(9),
        "the adapter kills and reaps the child"
    );
}
