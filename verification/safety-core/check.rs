use noble_contracts::source::{Session, Stage};
use noble_contracts::{DiagnosticKind, Limits};
use noble_kernel::contracts::TEST_EMIT;
use noble_kernel::types::EffSet;
use noble_kernel::untrusted::{Constraint, Outcome};
use std::path::Path;

const LIMITS: Limits = Limits {
    bytes: 65_536,
    nodes: 16_384,
    depth: 64,
    work: 2_000_000,
};

fn source(path: &str) -> Result<Vec<u8>, String> {
    std::fs::read(Path::new(path)).map_err(|error| format!("{path}: {error}"))
}

fn static_refusal(bytes: &[u8], stage: Stage) -> Result<(), String> {
    let session = Session::without_test_hosts();
    let error = session.prepare(bytes, &[], LIMITS).expect_err("source unexpectedly accepted");
    if error.stage() != stage || error.diagnostic().kind != DiagnosticKind::Invalid {
        return Err(format!("wrong static refusal: {error:?}"));
    }
    Ok(())
}

fn effect_refusal(bytes: &[u8]) -> Result<(), String> {
    // The resource-free bootstrap test host supplies exactly Text -- ! test.emit
    // for this word; no runtime host implementation is constructed or invoked.
    let session = Session::new();
    let prepared = session
        .prepare(bytes, &[], LIMITS)
        .map_err(|error| format!("source could not prepare with its declared host: {error:?}"))?;
    let submission = prepared.submission().ok_or("source is not a submission")?;
    if submission.request.expected.allowed_effects.as_slice() != [TEST_EMIT] ||
        submission.body.texts.len() != 1 ||
        submission.body.texts[0].bytes.as_slice() != b"audit" {
        return Err("wrong host contract, inferred effect or source literal".into());
    }
    if !matches!(noble_kernel::acceptance::check(
        &submission.environment, &submission.request, &submission.body.candidate
    ), Outcome::Accepted(_)) {
        return Err("positive independently checked host-bound candidate was not accepted".into());
    }
    // Separately supplied claim: use the *same parsed and typed candidate*,
    // changing only the caller's allowed bound. This is the actual acceptance
    // checker, not a guest execution or an unbound-word substitute.
    let mut request = submission.request.clone();
    request.expected.allowed_effects = EffSet::empty();
    match noble_kernel::acceptance::check(
        &submission.environment, &request, &submission.body.candidate
    ) {
        Outcome::Invalid(diagnostic) if diagnostic.constraint == Constraint::EffectInclusion(TEST_EMIT) => Ok(()),
        outcome => Err(format!("independent empty-bound admission did not reject test.emit: {outcome:?}")),
    }
}

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() != 4 {
        return Err("usage: check CASE01_SOURCE CASE05_SOURCE CASE14_POINTER CASE14_TRANSMUTE".into());
    }
    static_refusal(&source(&args[0])?, Stage::Check)?;
    println!("{{\"id\":\"S-CASE-01\",\"stage\":\"check\",\"outcome\":\"type-reject\",\"guest_requests\":0,\"protected_operations\":0}}");
    effect_refusal(&source(&args[1])?)?;
    println!("{{\"id\":\"S-CASE-05\",\"stage\":\"check\",\"outcome\":\"effect-reject\",\"constraint\":\"EffectInclusion(test.emit)\",\"guest_requests\":0,\"protected_operations\":0}}");
    static_refusal(&source(&args[2])?, Stage::Resolve)?;
    static_refusal(&source(&args[3])?, Stage::Resolve)?;
    println!("{{\"id\":\"S-CASE-14\",\"stage\":\"resolve\",\"outcome\":\"unbound-word\",\"variants\":2,\"guest_requests\":0,\"protected_operations\":0}}");
    Ok(())
}
