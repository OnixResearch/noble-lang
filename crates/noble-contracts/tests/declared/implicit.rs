//! Implicit-argument elaboration (`implicit-args/v1`) end-to-end controls.
//!
//! Every case goes through real module preparation, so the opt-in grammar,
//! the elaborator, the elaboration record and the untouched explicit checker
//! are exercised together. The elaborator is never treated as proof authority:
//! expanded terms are checked by `check_batch`, the same explicit checker that
//! refuses `(infer)` without elaboration.

use noble_contracts::intrinsic::{
    check_batch, check_batch_with_budgets, ElaborationRecord, Form, FormKind, PendingGoal,
    ProofBudgets, ProofObligation, ProofWork,
};

const POLY_REFL: &str = "proof 1 poly-refl : [ (Pi (A Type0) (Pi (x A) (Eq A x x))) ] [ (intro (A Type0) (intro (x A) (refl x))) ]";

/// Prepare `source` against a fresh source session.
fn prepare(
    source: &str,
) -> Result<noble_contracts::source::ModulePrepared, noble_contracts::source::Error> {
    let session = noble_contracts::source::ModuleSession::new(&[], super::retained())?;
    session.prepare(source.as_bytes(), &[], super::LIMITS)
}

fn refuse(source: &str) -> String {
    let (stage, message) = refusal(source);
    assert_eq!(stage, noble_contracts::source::Stage::Check, "{message}");
    message
}

fn refusal(source: &str) -> (noble_contracts::source::Stage, String) {
    let error = prepare(source).expect_err("elaboration admitted a refused proof");
    (error.stage(), error.diagnostic().message.clone())
}

fn batch(source: &str) -> noble_contracts::source::ModulePrepared {
    prepare(source)
        .map_err(|error| error.diagnostic().message.clone())
        .expect("elaboration admits the proof")
}

fn obligations(
    prepared: &noble_contracts::source::ModulePrepared,
) -> &noble_contracts::intrinsic::ProofBatch {
    prepared
        .proof_obligations()
        .expect("a proof declaration stages one obligation batch")
}

fn records(prepared: &noble_contracts::source::ModulePrepared) -> &[ElaborationRecord] {
    &obligations(prepared).elaborations
}

#[expect(
    tigerstyle::ambiguous_params,
    reason = "Owner: noble-contracts; the two declarations are positional in module order and named at every call site"
)]
fn module(first: &str, second: &str) -> String {
    format!("module logic@1 [ {first} {second} ]")
}

/// A tiny S-expression builder for hand-built proof terms.
#[expect(
    tigerstyle::allocation_in_loop,
    reason = "Owner: noble-contracts; the test-only builder opens one child list per open parenthesis of a short literal fixture"
)]
fn sexpr(text: &str) -> Form {
    let span = noble_contracts::Span {
        start: 0,
        end: u32::try_from(text.len()).expect("a test S-expression fits a u32 span"),
    };
    let mut stack: std::vec::Vec<std::vec::Vec<Form>> = std::vec![std::vec::Vec::new()];
    let mut atom = std::string::String::new();
    let flush = |atom: &mut std::string::String, stack: &mut std::vec::Vec<std::vec::Vec<Form>>| {
        if atom.is_empty() {
            return;
        }
        let value = Form {
            kind: FormKind::Atom(std::mem::take(atom)),
            span,
        };
        if let Some(top) = stack.last_mut() {
            top.push(value);
        }
    };
    for character in text.chars() {
        match character {
            '(' => {
                flush(&mut atom, &mut stack);
                stack.push(std::vec::Vec::new());
            }
            ')' => {
                flush(&mut atom, &mut stack);
                let items = stack.pop().unwrap_or_default();
                let value = Form {
                    kind: FormKind::List(items),
                    span,
                };
                if let Some(top) = stack.last_mut() {
                    top.push(value);
                }
            }
            ' ' => flush(&mut atom, &mut stack),
            other => atom.push(other),
        }
    }
    flush(&mut atom, &mut stack);
    let mut roots = stack.pop().unwrap_or_default();
    assert_eq!(roots.len(), 1, "one root form");
    roots.remove(0)
}

fn infer_unit() -> Form {
    Form {
        kind: FormKind::List(std::vec![Form {
            kind: FormKind::Atom("infer".into()),
            span: noble_contracts::Span { start: 0, end: 7 },
        }]),
        span: noble_contracts::Span { start: 0, end: 7 },
    }
}

/// The explicit checker still refuses the abbreviations when a caller hands
/// it raw terms directly; elaboration is a separate, opt-in pass.
#[test]
fn explicit_checker_still_refuses_abbreviations() {
    let prepared = batch(&module(
        POLY_REFL,
        "proof 1 refl-list infer : [ (Pi (x (List I64)) (Eq (List I64) x x)) ] [ (intro (x (List I64)) (app (use poly-refl) x)) ]",
    ));
    let mut raw = obligations(&prepared).clone();
    raw.obligations[1].term = infer_unit();
    raw.elaborations.clear();
    let error = check_batch(&raw, super::LIMITS).expect_err("explicit core rejects (infer)");
    assert_eq!(error.kind, noble_contracts::DiagnosticKind::Invalid);
    assert_eq!(error.message, "proof term has no inferable proposition");
}

/// IMPL-1/IMPL-3: `infer` after the name records the opt-in and a `(infer)`
/// expansion comes with a record naming the binder, head, solution and the
/// originating constraint spans.
#[test]
fn infer_opt_in_is_recorded_and_expanded() {
    let prepared = batch(&module(
        POLY_REFL,
        "proof 1 refl-list infer : [ (Pi (x (List I64)) (Eq (List I64) x x)) ] [ (intro (x (List I64)) (app (use poly-refl) x)) ]",
    ));
    let staged = obligations(&prepared);
    assert!(!staged.obligations[0].elaboration);
    assert!(staged.obligations[1].elaboration);
    let recorded = records(&prepared);
    assert_eq!(recorded.len(), 1);
    let record = &recorded[0];
    assert_eq!(record.proof, "refl-list");
    assert_eq!(record.revision, "implicit-args/v1");
    assert_eq!(
        record.explicit,
        "(intro (x (List I64)) (apply (apply (use poly-refl) (List I64)) x))"
    );
    assert_eq!(
        record.abbreviated,
        "(intro (x (List I64)) (app (use poly-refl) x))"
    );
    assert_eq!(record.inserted.len(), 1);
    let inserted = &record.inserted[0];
    assert_eq!(inserted.binder, "A");
    assert_eq!(inserted.position, 1);
    assert_eq!(inserted.head, "logic@1/poly-refl");
    assert_eq!(inserted.solution, "(List I64)");
    assert!(
        inserted.constraints.contains(&inserted.span),
        "the goal constraint names the abbreviation span: {:?}",
        inserted.constraints
    );
    let checked = check_batch(staged, super::LIMITS).expect("expanded proof checks");
    assert_eq!(checked[1].claim, "(∀ (v0 : El (.list .i64)), v0 = v0)");
}

/// IMPL-CASE-01 (P1): an imported proof is named by its alias-qualified
/// reference in `(infer)` mode exactly as in the explicit core, and its
/// identity is the exporting module's declaration.
#[test]
fn imported_proof_elaborates_under_its_alias() -> Result<(), String> {
    let session = noble_contracts::source::ModuleSession::new(&[], super::retained())
        .map_err(|error| format!("{error:?}"))?;
    let exporter = format!("module logic@1 [ {POLY_REFL} export poly-refl ]");
    let prepared = session
        .prepare(exporter.as_bytes(), &[], super::LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    let (published, result) = session.commit_verified(prepared, |batch| {
        let mut checked =
            check_batch(batch, super::LIMITS).map_err(|error| format!("{error:?}"))?;
        for proof in &mut checked {
            proof.model_revision = "a".repeat(64);
            proof.checker_revision = "b".repeat(64);
        }
        Ok(checked)
    });
    result.map_err(|error| format!("{error:?}"))?;
    let session = super::commit(published, b"import logic@1 as alias")?;
    let prepared = session
        .prepare(
            b"module imported@1 [ proof 1 imported-list infer : [ (Pi (x (List I64)) (Eq (List I64) x x)) ] [ (intro (x (List I64)) (app (use alias.poly-refl) x)) ] ]",
            &[],
            super::LIMITS,
        )
        .map_err(|error| error.diagnostic().message.clone())?;
    let recorded = records(&prepared);
    assert_eq!(recorded.len(), 1);
    assert_eq!(
        recorded[0].explicit,
        "(intro (x (List I64)) (apply (apply (use alias.poly-refl) (List I64)) x))"
    );
    assert_eq!(recorded[0].inserted.len(), 1);
    assert_eq!(recorded[0].inserted[0].head, "logic@1/poly-refl");
    assert_eq!(recorded[0].inserted[0].solution, "(List I64)");
    let checked =
        check_batch(obligations(&prepared), super::LIMITS).map_err(|error| format!("{error:?}"))?;
    assert_eq!(checked[0].claim, "(∀ (v0 : El (.list .i64)), v0 = v0)");
    Ok(())
}

/// IMPL-1: an opted-in proof with no abbreviation passes through unchanged and
/// records nothing.
#[test]
fn opt_in_without_abbreviation_passes_through() {
    let prepared = batch(&module(
        POLY_REFL,
        "proof 1 plain infer : [ (Pi (x I64) (Eq I64 x x)) ] [ (intro (x I64) (refl x)) ]",
    ));
    assert!(obligations(&prepared).obligations[1].elaboration);
    assert!(records(&prepared).is_empty());
    check_batch(obligations(&prepared), super::LIMITS).expect("plain proof checks");
}

/// IMPL-1: abbreviations without the opt-in are refused with the named
/// opt-in diagnostic before any checking.
#[test]
fn abbreviations_require_the_opt_in() {
    let message = refuse(&module(
        POLY_REFL,
        "proof 1 refl-list : [ (Pi (x (List I64)) (Eq (List I64) x x)) ] [ (intro (x (List I64)) (app (use poly-refl) x)) ]",
    ));
    assert_eq!(
        message,
        "unsupported: app requires the infer elaboration opt-in"
    );
    let message = refuse(&module(
        POLY_REFL,
        "proof 1 refl-list : [ (Pi (x (List I64)) (Eq (List I64) x x)) ] [ (intro (x (List I64)) (apply (use poly-refl) (infer))) ]",
    ));
    assert_eq!(
        message,
        "unsupported: infer requires the infer elaboration opt-in"
    );
}

/// IMPL-1: `(infer)` is admissible only at the three selected positions; every
/// other position is refused with a position-naming diagnostic.
#[test]
fn infer_positions_are_exactly_the_selected_ones() {
    let claim = "[ (Eq I64 0 0) ]";
    for (term, expected, at_check) in [
        (
            "[ (infer) ]",
            "unsupported: infer is not a whole proof term",
            true,
        ),
        (
            "[ (apply (use (infer)) 0) ]",
            // The source `use` resolver refuses a form reference first.
            "use requires a proof identifier",
            false,
        ),
        (
            "[ (apply (infer) 0) ]",
            "unsupported: infer cannot be an application head",
            true,
        ),
        (
            "[ (app (infer) 0) ]",
            "unsupported: infer cannot be an application head",
            true,
        ),
        (
            "[ (refl (infer)) ]",
            "unsupported: infer cannot be a refl operand",
            true,
        ),
        (
            "[ (intro (x (infer)) (refl x)) ]",
            "unsupported: infer cannot be an intro binder sort",
            true,
        ),
        (
            "[ (intro ((infer) I64) (refl (infer))) ]",
            "unsupported: infer cannot be an intro binder name",
            true,
        ),
        (
            "[ (subst (infer) (refl 0) (refl 0)) ]",
            "unsupported: infer cannot be a subst motive",
            true,
        ),
        (
            "[ (exec-literal (infer) (infer)) ]",
            "unsupported: infer is not admissible at this rule position",
            true,
        ),
    ] {
        let source = format!("module logic@1 [ proof 1 staged infer : {claim} {term} ]");
        let (stage, message) = refusal(&source);
        assert_eq!(message, expected, "term {term}");
        let expected_stage = if at_check {
            noble_contracts::source::Stage::Check
        } else {
            noble_contracts::source::Stage::Resolve
        };
        assert_eq!(stage, expected_stage, "term {term}");
    }
}

/// IMPL-1/IMPL-2: A1 at a `Type0` binder and at a value binder.
#[test]
fn apply_placeholders_elaborate_from_the_goal() {
    let prepared = batch(&format!(
        "module logic@1 [ {POLY_REFL} proof 1 refl-type infer : [ (Pi (x I64) (Eq I64 x x)) ] [ (intro (x I64) (apply (apply (use poly-refl) (infer)) (infer))) ] ]"
    ));
    let recorded = records(&prepared);
    assert_eq!(recorded.len(), 1);
    assert_eq!(
        recorded[0].explicit,
        "(intro (x I64) (apply (apply (use poly-refl) I64) x))"
    );
    assert_eq!(recorded[0].inserted.len(), 2);
    assert_eq!(recorded[0].inserted[0].solution, "I64");
    assert_eq!(recorded[0].inserted[1].solution, "x");
    let checked =
        check_batch(obligations(&prepared), super::LIMITS).expect("expanded proof checks");
    assert_eq!(checked[1].claim, "(∀ (v0 : El .i64), v0 = v0)");
}

/// IMPL-TRUST-01: elaboration and the explicit checker share one request
/// budget. The checker resumes from the work elaboration charged, so a work
/// limit that fits checking alone but not both together refuses as exhausted.
#[test]
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; a test entry point runs the non-const module preparation and checker, so the const suggestion does not apply"
)]
fn elaboration_and_checking_share_one_request_budget() {
    let prepared = batch(&format!(
        "module logic@1 [ {POLY_REFL} proof 1 refl-type infer : [ (Pi (x I64) (Eq I64 x x)) ] [ (intro (x I64) (apply (apply (use poly-refl) (infer)) (infer))) ] ]"
    ));
    let staged = obligations(&prepared);
    let elaboration = staged.elaboration_work;
    assert!(
        elaboration.total > 0,
        "the abbreviated proof charged the shared meter"
    );
    let mut alone = staged.clone();
    alone.elaboration_work = ProofWork {
        normalization: 0,
        substitution: 0,
        total: 0,
    };
    let budgets = ProofBudgets::from_limits(super::LIMITS);
    let (_, checker_only) =
        check_batch_with_budgets(&alone, super::LIMITS, budgets).expect("explicit term checks");
    let (_, shared) =
        check_batch_with_budgets(staged, super::LIMITS, budgets).expect("shared request fits");
    assert_eq!(shared.total, checker_only.total + elaboration.total);
    let exact = noble_contracts::Limits {
        work: shared.total,
        ..super::LIMITS
    };
    check_batch_with_budgets(staged, exact, ProofBudgets::from_limits(exact))
        .expect("exactly the combined work fits");
    let short = noble_contracts::Limits {
        work: shared.total - 1,
        ..super::LIMITS
    };
    let error = check_batch_with_budgets(staged, short, ProofBudgets::from_limits(short))
        .expect_err("one unit below the combined work exhausts");
    assert_eq!(error.kind, noble_contracts::DiagnosticKind::Exhausted);
    check_batch_with_budgets(&alone, short, ProofBudgets::from_limits(short))
        .expect("checking alone fits the same limit");
}

/// IMPL-2: P1 with a context code, whose solution must stay below its depth.
#[test]
fn context_code_solution_is_well_scoped() {
    let prepared = batch(&module(
        POLY_REFL,
        "proof 1 refl-context infer : [ (Pi (B Type0) (Pi (y (List B)) (Eq (List B) y y))) ] [ (intro (B Type0) (intro (y (List B)) (app (use poly-refl) y))) ]",
    ));
    let record = &records(&prepared)[0];
    assert_eq!(record.inserted[0].solution, "(List B)");
    assert_eq!(
        record.explicit,
        "(intro (B Type0) (intro (y (List B)) (apply (apply (use poly-refl) (List B)) y)))"
    );
    check_batch(obligations(&prepared), super::LIMITS).expect("well-scoped code checks");
}

const DEPENDENT_BASE: &str = "proof 1 base : [ (Pi (x I64) (Pi (h (Eq I64 x x)) (Pi (A Type0) (Pi (y (Pair (List A) I64)) (Eq (Pair (List A) I64) y y))))) ] [ (intro (x I64) (intro (h (Eq I64 x x)) (intro (A Type0) (intro (y (Pair (List A) I64)) (refl y))))) ]";

/// IMPL-2: P2, the dependent-rebasing case with an inferable explicit premise.
#[test]
fn dependent_rebasing_with_premise_elaborates() {
    let prepared = batch(&module(
        DEPENDENT_BASE,
        "proof 1 derived infer : [ (Pi (x I64) (Pi (z (Pair (List I64) I64)) (Eq (Pair (List I64) I64) z z))) ] [ (intro (x I64) (intro (z (Pair (List I64) I64)) (app (use base) x (refl x) z))) ]",
    ));
    let record = &records(&prepared)[0];
    assert_eq!(
        record.explicit,
        "(intro (x I64) (intro (z (Pair (List I64) I64)) (apply (apply (apply (apply (use base) x) (refl x)) I64) z)))"
    );
    assert_eq!(record.inserted.len(), 1);
    assert_eq!(record.inserted[0].binder, "A");
    assert_eq!(record.inserted[0].position, 3);
    assert_eq!(record.inserted[0].solution, "I64");
    let checked = check_batch(obligations(&prepared), super::LIMITS).expect("P2 expansion checks");
    assert_eq!(
        checked[1].claim,
        "(∀ (v0 : El .i64), (∀ (v1 : El (.pair (.list .i64) .i64)), v1 = v1))"
    );
}

/// IMPL-2: an unused `Type0` binder is never defaulted.
#[test]
fn underconstrained_binder_is_refused_never_defaulted() {
    let message = refuse(&module(
        "proof 1 const-refl : [ (Pi (A Type0) (Pi (B Type0) (Pi (x A) (Eq A x x)))) ] [ (intro (A Type0) (intro (B Type0) (intro (x A) (refl x)))) ]",
        "proof 1 bad infer : [ (Pi (x I64) (Eq I64 x x)) ] [ (intro (x I64) (app (use const-refl) x)) ]",
    ));
    assert!(message.starts_with("underconstrained:"), "{message}");
    assert!(message.contains("binder `B`"), "{message}");
    assert!(message.contains("position 2"), "{message}");
    assert!(message.contains("const-refl"), "{message}");
    assert!(message.contains("none is chosen"), "{message}");
}

// The source `use` resolver names only prior or imported proofs, so a premise
// binder is exercised through an inferable witness (`refl`), exactly as the
// accepted CONTRACT fixtures do.
const TWICE_REFL: &str = "proof 1 twice-refl : [ (Pi (x I64) (Pi (h (Eq I64 x x)) (Eq I64 x x))) ] [ (intro (x I64) (intro (h (Eq I64 x x)) (refl x))) ]";

/// IMPL-2: a premise placeholder is refused and candidates are listed, not
/// chosen, even though both candidates have the identical proposition.
#[test]
fn premise_placeholder_lists_candidates_without_choosing() {
    let source = module(
        TWICE_REFL,
        "proof 1 bad infer : [ (Pi (x I64) (Pi (h1 (Eq I64 x x)) (Pi (h2 (Eq I64 x x)) (Eq I64 x x)))) ] [ (intro (x I64) (intro (h1 (Eq I64 x x)) (intro (h2 (Eq I64 x x)) (app (use twice-refl) x (infer))))) ]",
    );
    let message = refuse(&source);
    assert!(
        message.starts_with(
            "unsupported: premise inference for binder `h` (position 2) of logic@1/twice-refl;"
        ),
        "{message}"
    );
    // Each candidate row points at its intro binder in the submitted source.
    for (name, next) in [("h1", "(intro"), ("h2", "(app")] {
        let binder = format!("({name} (Eq I64 x x))");
        let start = source
            .rfind(&format!("{binder} {next}"))
            .expect("intro binder in the term");
        let row = format!(
            "local premise `{name}` at {start}..{}",
            start + binder.len()
        );
        assert!(message.contains(&row), "{row}: {message}");
    }
}

/// IMPL-2: a value of the wrong code is a constraint clash, not a guess. The
/// refusal names the argument it was checking and the unresolved binder, head
/// identity and originating constraint spans (IMPL-ELAB-01).
#[test]
fn wrong_value_code_is_invalid_constraint() {
    let source = module(
        DEPENDENT_BASE,
        "proof 1 bad infer : [ (Pi (x I64) (Pi (w Bool) (Eq Bool w w))) ] [ (intro (x I64) (intro (w Bool) (app (use base) x (refl x) w))) ]",
    );
    let message = refuse(&source);
    let w = source.rfind(" w)))").expect("the clashing argument") + 1;
    assert_eq!(
        message,
        format!(
            "invalid-constraint: code (Pair (List ?0) I64) does not match code Bool; checking the argument for binder `y` (position 4) of logic@1/base; ?0 is the argument for binder `A` (position 3) of logic@1/base, constrained at {w}..{}",
            w + 1
        )
    );
}

/// IMPL-EXPOSE-01: one refusal lists at most sixteen originating constraint
/// spans and counts the rest; its category is unchanged.
#[test]
fn constraint_spans_are_capped_with_a_count() {
    let count = 20;
    let names: Vec<String> = (1..=count).map(|at| format!("x{at}")).collect();
    let mut claim = String::from("(Eq A x1 x1)");
    let mut term = String::from("(refl x1)");
    for name in names.iter().rev() {
        claim = format!("(Pi ({name} A) {claim})");
        term = format!("(intro ({name} A) {term})");
    }
    let many = format!("proof 1 many : [ (Pi (A Type0) {claim}) ] [ (intro (A Type0) {term}) ]");
    // Nineteen I64 arguments fix and re-constrain A; the twentieth is Bool.
    let values: Vec<String> = (1..count).map(|at| format!("y{at}")).collect();
    let mut goal = String::from("(Pi (w Bool) (Eq I64 y1 y1))");
    let mut proof = format!("(intro (w Bool) (app (use many) {} w))", values.join(" "));
    for name in values.iter().rev() {
        goal = format!("(Pi ({name} I64) {goal})");
        proof = format!("(intro ({name} I64) {proof})");
    }
    let message = refuse(&module(
        &many,
        &format!("proof 1 bad infer : [ {goal} ] [ {proof} ]"),
    ));
    assert!(
        message.starts_with("invalid-constraint: code I64 does not match code Bool;"),
        "{message}"
    );
    // Every listed span renders as `start..end`.
    assert_eq!(message.matches("..").count(), 16, "{message}");
    assert!(
        message.ends_with("; and 4 more constraint spans"),
        "{message}"
    );
}

/// IMPL-EXPOSE-01: a rendered term is cut at 1 KiB with the cut bytes counted,
/// so a refusal stays within 16 KiB whatever the size of its codes.
#[test]
fn rendered_terms_are_capped_with_a_count() {
    // A full Pair tree of depth 8 (256 I64 leaves), built level by level.
    let mut wide = String::from("I64");
    for _ in 0..8 {
        wide = format!("(Pair {wide} {wide})");
    }
    let (pair, sum) = (format!("(Pair {wide} I64)"), format!("(Sum {wide} I64)"));
    let message = refuse(&module(
        "proof 1 t2 : [ (Pi (A Type0) (Pi (x A) (Pi (y A) (Eq A x x)))) ] [ (intro (A Type0) (intro (x A) (intro (y A) (refl x)))) ]",
        &format!(
            "proof 1 bad infer : [ (Pi (u {pair}) (Pi (v {sum}) (Eq {pair} u u))) ] [ (intro (u {pair}) (intro (v {sum}) (app (use t2) u v))) ]"
        ),
    ));
    assert!(
        message.starts_with("invalid-constraint: code (Pair (Pair"),
        "{message}"
    );
    assert_eq!(message.matches(" more bytes]").count(), 2, "{message}");
    assert!(message.len() < 3 * 1024, "{}", message.len());
    assert!(message.contains("of logic@1/t2"), "{message}");
}

/// IMPL-2: app spines do not admit under- or over-application; the refusal
/// names the head and the binder or argument that does not fit.
#[test]
fn app_spines_refuse_under_and_over_application() {
    let under = refuse(&module(
        TWICE_REFL,
        "proof 1 bad infer : [ (Pi (x I64) (Eq I64 x x)) ] [ (intro (x I64) (app (use twice-refl) x)) ]",
    ));
    assert_eq!(
        under,
        "unsupported: app under-application: no argument for binder `h` (position 2) of logic@1/twice-refl"
    );
    let over = refuse(&module(
        TWICE_REFL,
        "proof 1 bad infer : [ (Pi (x I64) (Eq I64 x x)) ] [ (intro (x I64) (app (use twice-refl) x (refl x) x)) ]",
    ));
    assert_eq!(
        over,
        "unsupported: app over-application of logic@1/twice-refl: argument 3 has no binder"
    );
}

/// IMPL-TRUST-01: the host's requested proof budgets bound elaboration itself;
/// a budget below the elaborator's own need exhausts there, naming the stage
/// and the exceeded budget, and nothing is accepted or refuted.
#[test]
fn shared_proof_budgets_bound_elaboration() {
    let source = module(
        POLY_REFL,
        "proof 1 short infer : [ (Pi (x (List I64)) (Eq (List I64) x x)) ] [ (intro (x (List I64)) (app (use poly-refl) x)) ]",
    );
    for (budgets, expected) in [
        (
            ProofBudgets {
                normalization_work: 0,
                substitution_work: super::LIMITS.work,
            },
            "exhausted: elaboration unification step exceeded the normalization budget",
        ),
        (
            ProofBudgets {
                normalization_work: super::LIMITS.work,
                substitution_work: 0,
            },
            "exhausted: elaboration substitution step exceeded the substitution budget",
        ),
    ] {
        let mut session = noble_contracts::source::ModuleSession::new(&[], super::retained())
            .expect("module session");
        session.share_proof_budgets(budgets);
        let error = session
            .prepare(source.as_bytes(), &[], super::LIMITS)
            .expect_err("the shared budget bounds elaboration");
        assert_eq!(
            error.diagnostic().kind,
            noble_contracts::DiagnosticKind::Exhausted
        );
        assert_eq!(error.diagnostic().message, expected);
    }
    // Under the limits' own budgets the same source elaborates.
    assert_eq!(records(&batch(&source)).len(), 1);
}

/// IMPL-1/IMPL-2: A3 takes the structural literal from the exact subject.
#[test]
fn contract_literal_placeholder_takes_the_subject_literal() {
    let source = "module selected@1 [ def selected [ 1 + ] contract 1 law [ subject selected input [ x I64 ] output [ y I64 ] requires [ true ] ensures [ (eq (out y) (add (in x) 1)) ] ] proof 1 verified infer for law [ (export-unary-I64 (intro (tail Stack) (intro (x I64) (pc-sequence (pc-exact (exec-literal (infer))) (pc-exact (exec-add)) (bridge (by-exact-append-assoc)) (join (by-exact-result))))) (mc1-true-eq-wrap)) ] ]";
    let prepared = batch(source);
    let record = &records(&prepared)[0];
    assert_eq!(record.proof, "verified");
    assert_eq!(record.inserted.len(), 1);
    assert_eq!(record.inserted[0].binder, "exec-literal");
    assert_eq!(record.inserted[0].position, 1);
    assert_eq!(record.inserted[0].head, "selected@1/law");
    assert_eq!(record.inserted[0].solution, "1");
    assert!(
        record.explicit.contains("(exec-literal 1)"),
        "{}",
        record.explicit
    );
    check_batch(obligations(&prepared), super::LIMITS).expect("A3 expansion checks");
}

/// IMPL-2: elaboration is charged to a bounded request and names its stage.
#[test]
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; a test entry point runs the non-const module preparation and elaborator, so the const suggestion does not apply"
)]
fn elaboration_budget_exhaustion_names_the_stage() {
    let prepared = batch(&module(
        POLY_REFL,
        "proof 1 explicit-refl : [ (Pi (x (List I64)) (Eq (List I64) x x)) ] [ (intro (x (List I64)) (refl x)) ]",
    ));
    let staged = obligations(&prepared);
    let goal = match &staged.obligations[1].goal {
        PendingGoal::Pure { proposition } => Some(proposition.clone()),
        PendingGoal::Contract { .. } => None,
    }
    .expect("pure fixture");
    let abbreviated = sexpr("(intro (x (List I64)) (app (use poly-refl) x))");
    let span = abbreviated.span;
    let obligation = ProofObligation {
        name: "abbreviated".into(),
        revision: 1,
        goal: PendingGoal::Pure { proposition: goal },
        term: abbreviated,
        span,
        elaboration: true,
    };
    let mut all = staged.obligations.clone();
    all.push(obligation);
    let tiny = noble_contracts::intrinsic::ProofBudgets {
        normalization_work: 2,
        substitution_work: 2,
    };
    let error = noble_contracts::intrinsic::elaborate_obligations(
        "logic",
        1,
        &all,
        &[],
        super::LIMITS,
        tiny,
    )
    .err()
    .expect("tiny budget must exhaust elaboration");
    assert_eq!(error.kind, noble_contracts::DiagnosticKind::Exhausted);
    assert!(
        error.message.starts_with("exhausted: elaboration"),
        "{}",
        error.message
    );
}

/// IMPL-3: two fresh preparations of the same bytes record identical
/// expansions, in the same constraint order.
#[test]
fn elaboration_is_deterministic_across_sessions() {
    let source = module(
        DEPENDENT_BASE,
        "proof 1 derived infer : [ (Pi (x I64) (Pi (z (Pair (List I64) I64)) (Eq (Pair (List I64) I64) z z))) ] [ (intro (x I64) (intro (z (Pair (List I64) I64)) (app (use base) x (refl x) z))) ]",
    );
    let first = batch(&source);
    let second = batch(&source);
    assert_eq!(records(&first), records(&second));
    assert_eq!(records(&first)[0].inserted, records(&second)[0].inserted);
}
