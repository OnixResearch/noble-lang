//! ADAPT-06: a layout-valid source submission whose public environment
//! advertises the resource-free `test.emit` host contract as effect-free.
//! The kernel accepts that locally consistent false premise; the Wasm compiler
//! must refuse it against its own fixed host contract before emitting a program.

use noble_kernel::contracts::{Behavior, Definition, TEST_EMIT};
use noble_kernel::execution::Submission;
use noble_kernel::shapes::EffectSlot;
use noble_kernel::types::EffSet;
use noble_kernel::untrusted::{Constraint, Node, Outcome};
use noble_wasm::source::Compiler;

/// The fixed bootstrap `Text -- ! {test.emit}` row.
const EMIT: Definition = Definition(22);

/// The actual typed `"audit" test.emit` submission, checked to be the attack
/// premise rather than a hand-built substitute.
fn authentic() -> Result<Submission, String> {
    let limits = noble_contracts::Limits::default();
    let prepared = noble_contracts::source::Session::new()
        .prepare(b"\"audit\" test.emit", &[], limits)
        .map_err(|error| error.diagnostic().message.clone())?;
    let submission = prepared
        .submission()
        .cloned()
        .ok_or("typed source produced no executable submission")?;
    let expected = &submission.request.expected;
    let invokes_emit = submission
        .body
        .candidate
        .nodes
        .iter()
        .any(|node| matches!(node, Node::Invocation { def, .. } if *def == EMIT));
    let is_premise = submission.definitions.is_empty()
        && expected.stack_in.is_empty()
        && expected.stack_out.is_empty()
        && expected.allowed_effects == EffSet::from_ids(&[TEST_EMIT])
        && submission.environment.kind(EMIT) == Some(Behavior::TestEmit)
        && submission
            .environment
            .scheme(EMIT)
            .is_some_and(|scheme| scheme.effects == [EffectSlot::Effect(TEST_EMIT)])
        && invokes_emit
        && matches!(submission.body.texts.as_slice(), [text] if text.bytes == b"audit");
    if !is_premise {
        return Err(format!("not the typed test.emit premise: {submission:?}"));
    }
    Ok(submission)
}

fn kernel(submission: &Submission) -> Outcome {
    noble_kernel::acceptance::check(
        &submission.environment,
        &submission.request,
        &submission.body.candidate,
    )
}

fn refused(submission: &Submission) -> bool {
    matches!(
        Compiler::new().prepare(submission),
        Err(noble_wasm::Diagnostic::Invalid)
    )
}

#[test]
fn forged_effect_free_emit_contract_is_refused_before_emission() -> Result<(), String> {
    let authentic = authentic()?;
    if Compiler::new().prepare(&authentic).is_err() {
        return Err("the authentic test.emit submission did not compile".into());
    }
    let mut forged = authentic.clone();
    forged.environment.defs[22].effects = Vec::new();
    forged.request.expected.allowed_effects = EffSet::empty();
    // Trusting the supplied environment, the kernel derives no effect. Only the
    // compiler's independent host contract can expose the forgery.
    match kernel(&forged) {
        Outcome::Accepted(checked) if checked.interface.effects == EffSet::empty() => {}
        other => return Err(format!("forged premise rejected by kernel: {other:?}")),
    }
    if !refused(&forged) {
        return Err("an effect-free test.emit contract was not refused as invalid".into());
    }
    Ok(())
}

#[test]
fn request_only_empty_bound_is_an_effect_inclusion_refusal() -> Result<(), String> {
    let mut bounded = authentic()?;
    bounded.request.expected.allowed_effects = EffSet::empty();
    match kernel(&bounded) {
        Outcome::Invalid(diagnostic)
            if diagnostic.constraint == Constraint::EffectInclusion(TEST_EMIT) => {}
        other => return Err(format!("empty bound did not reject test.emit: {other:?}")),
    }
    if !refused(&bounded) {
        return Err("an empty-bound test.emit submission was not refused as invalid".into());
    }
    Ok(())
}
