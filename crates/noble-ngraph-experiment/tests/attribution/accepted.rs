//! Sessions the adapter accepts, with their results unchanged.

use super::{
    body, good_lines, run_with, runtime_line, static_line, two_processes, vectors, Script,
    DEFINITION, ROOT, SPAWN,
};
use noble_ngraph_experiment::l1::{self, Opt};
use std::sync::PoisonError;

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
