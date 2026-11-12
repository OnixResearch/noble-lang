//! Subject admission: the bootstrap rows extraction selects behaviour from,
//! and the bytes the subject identity hashes.

use noble_ngraph::extract::{self, source, Budget, Request};

use super::common::IDS;

fn prepare(root: &str, inputs: &[noble_kernel::types::Ty]) -> source::Prepared {
    source::prepare(
        noble_contracts::source::Session::new(),
        noble_contracts::Limits::default(),
        &[],
        root,
        inputs,
    )
    .expect("frontend accepts")
}

fn admit(p: &source::Prepared) -> Result<extract::Admitted<'_>, noble_ngraph::Diagnostic> {
    extract::admit(
        &IDS,
        &p.submission,
        &Request {
            profile: "ngraph-i64/0",
            claimed_closure: &p.claimed_closure,
            budget: Budget::default(),
        },
    )
}

fn subject(root: &str) -> noble_ngraph::ir::Subject {
    let p = prepare(root, &[]);
    admit(&p).expect("admitted").subject().clone()
}

#[test]
fn a_forged_bootstrap_row_is_refused() {
    let root = "dup swap";
    let mut p = prepare(root, &source::i64s(1));
    // Rows 0 and 2 of the table are `dup` and `swap`; their schemes stay in
    // place while their behaviours trade places, so every body still types
    // but extraction would pick the other operation for each invocation.
    p.submission.environment.kinds.swap(0, 2);
    let e = admit(&p).expect_err("the built-in rows are not the bootstrap table");
    assert_eq!(e.code_string(), "E-SUBJECT");
}

#[test]
fn the_environment_is_part_of_the_subject_identity() {
    let root = "dup swap";
    let p = prepare(root, &source::i64s(1));
    let canonical = admit(&p).expect("admitted").subject().clone();
    let mut q = prepare(root, &source::i64s(1));
    q.submission.environment.caller_module = Some(7);
    let from_module = admit(&q)
        .expect("the bootstrap rows are unchanged")
        .subject()
        .clone();
    assert_ne!(canonical.identity, from_module.identity);
}

#[test]
fn text_literal_bytes_are_part_of_the_subject_identity() {
    let a = subject("[ \"a\" ] drop 1");
    let b = subject("[ \"b\" ] drop 1");
    assert_eq!(a.kind, noble_ngraph::ir::SubjectKind::Extracted);
    assert_ne!(
        a.identity, b.identity,
        "the two programs differ only in a dropped text literal's bytes"
    );
}
