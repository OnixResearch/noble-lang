//! Report streams the adapter refuses as protocol failures.

use super::{
    assert_class, body, edit, expect_failure, good_lines, run_with, runtime_line, static_line,
    Script, DEFINITION, ROOT, SPAWN,
};
use noble_ngraph_experiment::l1::{self, Opt};
use serde_json::json;
use std::sync::PoisonError;

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
