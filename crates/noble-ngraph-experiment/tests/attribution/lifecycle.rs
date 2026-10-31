//! Child exit, signal, input delivery and reaping.

use super::{
    assert_class, body, edit, expect_failure, good_lines, run_with, runtime_line, static_line,
    Script, NORMAL, PROCESS, PROTOCOL, SPAWN, TRANSPORT, UNBOUND_WORD,
};
use noble_ngraph_experiment::l1::Opt;
use serde_json::json;
use std::sync::PoisonError;

#[test]
fn an_unexplained_exit_or_signal_fails_as_process() {
    let lines = good_lines(Opt::Off);
    let exited = expect_failure(run_with("nonzero-exit", body(&lines, true, "exit 3")));
    assert_class(&exited, PROCESS, "exit status Some(3)");
    assert_eq!(
        exited.validated, 5,
        "every numeric result printed, yet no completed run"
    );

    let crashed = expect_failure(run_with("crash", body(&lines, true, "kill -9 $$")));
    assert_class(&crashed, PROCESS, "signal 9");
    assert_eq!(crashed.signal, Some(9));

    let _guard = SPAWN.lock().unwrap_or_else(PoisonError::into_inner);
    let refused = [
        runtime_line(1, "5 4", Opt::Off, NORMAL, &[5, 4]),
        static_line(2, "resolve", UNBOUND_WORD),
    ];
    let script = Script::new("refusal-exit-0", body(&refused, true, "exit 0"));
    let failure = expect_failure(
        script
            .cli(Opt::Off)
            .session(&["5 4".to_owned(), "nope".to_owned()]),
    );
    assert_class(&failure, PROCESS, "outcomes give 2");
}

#[test]
fn undelivered_input_fails_as_transport() {
    let _guard = SPAWN.lock().unwrap_or_else(PoisonError::into_inner);
    // More than a pipe buffer, and the fixture never reads it.
    let large = "1 drop ".repeat(1 << 18);
    let lines = [
        runtime_line(1, "5 4", Opt::Off, NORMAL, &[5, 4]),
        runtime_line(2, &large, Opt::Off, NORMAL, &[5, 4]),
    ];
    let script = Script::new("undelivered", body(&lines, false, "exit 0"));
    let failure = expect_failure(
        script
            .cli(Opt::Off)
            .session(&["5 4".to_owned(), large.clone()]),
    );
    assert_class(&failure, TRANSPORT, "not all delivered");
    assert!(failure.write_error.is_some());
}

#[test]
fn a_running_child_with_an_invalid_stream_is_stopped_and_reaped() {
    let _guard = SPAWN.lock().unwrap_or_else(PoisonError::into_inner);
    let wrong = [edit(
        &runtime_line(1, "5 4", Opt::Off, NORMAL, &[5, 4]),
        |r| r["backend"] = json!("gc"),
    )];
    let script = Script::new("stopped", body(&wrong, false, "while :; do :; done"));
    let failure = expect_failure(script.cli(Opt::Off).session(&["5 4".to_owned()]));
    assert_class(&failure, PROTOCOL, "backend");
    assert_eq!(
        failure.signal,
        Some(9),
        "the adapter kills and reaps the child"
    );
}
