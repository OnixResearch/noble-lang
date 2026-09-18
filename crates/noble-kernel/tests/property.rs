//! Bounded property harness for the M2 fragment checker (DX-PROPERTY-01/02).
//!
//! Lane one generates seeded candidates and requires the kernel checker and
//! a separately written acceptance oracle to agree on every one; a
//! disagreement triggers bounded shrinking that preserves the failure
//! predicate, and the minimized case fails the test with full detail.
//! Lane two corrupts fresh candidates structurally — truncated arenas,
//! foreign revisions, dangling references, oversized witnesses, over-deep
//! quotation chains, raw identifier bits — and requires the kernel to return
//! a bounded outcome, never acceptance and never a panic, under the debug
//! profile the workspace pins to `overflow-checks = true`.
//!
//! Determinism: fixed seeds, no time or environment input.

#[path = "property/fit.rs"]
mod fit;
#[path = "property/gen.rs"]
mod gen;
#[path = "property/knobs.rs"]
mod knobs;
#[path = "property/malform.rs"]
mod malform;
#[path = "property/oracle.rs"]
mod oracle;
#[path = "property/otypes.rs"]
mod otypes;
#[path = "property/rng.rs"]
mod rng;
#[path = "property/shrink.rs"]
mod shrink;
#[path = "property/table.rs"]
mod table;

use noble_kernel::untrusted::Outcome;
use otypes::Decision;
use rng::Rng;

/// Generated candidates in the agreement lane.
const GENERATED: u64 = 1000;
/// Structurally corrupted candidates in the malformed lane.
const MALFORMED: u64 = 200;

// r[verify DX-PROPERTY-01]
// r[verify VT-M2-01]
#[test]
fn generated_candidates_agree_with_independent_oracle() -> Result<(), String> {
    let env = gen::environment()?;
    let mut rng = Rng::new(0x0B1E_5EED_0000_0001);
    let mut both_accepted = 0;
    let mut both_rejected = 0;
    let mut index = 0;
    while index < GENERATED {
        let case = gen::case(&mut rng);
        let decision = oracle::decide(&case.request, &case.candidate);
        let outcome = noble_kernel::acceptance::check(&env, &case.request, &case.candidate);
        if matches!(
            (decision, &outcome),
            (Decision::Accept, Outcome::Accepted(_))
        ) || matches!(
            (decision, &outcome),
            (Decision::Reject, Outcome::Invalid(_))
        ) {
            match decision {
                Decision::Accept => both_accepted += 1,
                Decision::Reject => both_rejected += 1,
            }
        } else {
            return Err(disagreement_report(
                index,
                decision,
                &outcome,
                &env,
                &case.request,
                &case.candidate,
            ));
        }
        index += 1;
    }
    println!(
        "property/agreement: {GENERATED} generated candidates, {both_accepted} accepted by both sides, {both_rejected} rejected by both sides, 0 disagreements"
    );
    println!(
        "property/agreement: eliminator patterns emitted: case={} if={} list.case={}",
        gen::ELIMINATORS[0].load(std::sync::atomic::Ordering::Relaxed),
        gen::ELIMINATORS[1].load(std::sync::atomic::Ordering::Relaxed),
        gen::ELIMINATORS[2].load(std::sync::atomic::Ordering::Relaxed)
    );
    Ok(())
}

/// Shrink one disagreement while it persists, then report it.
fn disagreement_report(
    index: u64,
    decision: Decision,
    outcome: &Outcome,
    env: &noble_kernel::contracts::Env,
    request: &noble_kernel::untrusted::Request,
    candidate: &noble_kernel::untrusted::Candidate,
) -> String {
    let holds = |probe_request: &noble_kernel::untrusted::Request,
                 probe: &noble_kernel::untrusted::Candidate| {
        let probe_decision = oracle::decide(probe_request, probe);
        let probe_outcome = noble_kernel::acceptance::check(env, probe_request, probe);
        let agrees = matches!(
            (&probe_decision, &probe_outcome),
            (Decision::Accept, Outcome::Accepted(_)) | (Decision::Reject, Outcome::Invalid(_))
        );
        !agrees
    };
    let (small_request, small, steps) = shrink::shrink(request, candidate, &holds);
    format!(
        "kernel/oracle disagreement at generated case {index} (seed stream 0x0B1E_5EED_0000_0001)\n\
         oracle: {decision:?}\n\
         kernel: {outcome:?}\n\
         shrunk in {steps} steps (bound {}):\n\
         request: {small_request:?}\n\
         candidate: {small:?}",
        shrink::MAX_STEPS,
    )
}

// r[verify DX-PROPERTY-02]
// r[verify VT-M2-01]
#[test]
fn malformed_candidates_never_panic_or_accept() -> Result<(), String> {
    let env = gen::environment()?;
    let mut rng = Rng::new(0x0B1E_5EED_0000_0002);
    let mut rejected = 0;
    let mut bounded = 0;
    let mut foreign = 0;
    let mut labels: Vec<(&str, u32)> = Vec::new();
    let mut index = 0;
    while index < MALFORMED {
        let case = gen::case(&mut rng);
        let (broken, label) = malform::corrupt(&mut rng, case);
        let outcome = noble_kernel::acceptance::check(&env, &broken.request, &broken.candidate);
        match outcome {
            Outcome::Accepted(_) => {
                return Err(format!(
                    "malformed lane case {index} ({label}) was accepted; candidate: {:?}",
                    broken.candidate
                ));
            }
            Outcome::Invalid(_) => rejected += 1,
            Outcome::Exhausted(_) => bounded += 1,
            Outcome::Unsupported(_) | Outcome::InternalFailure => foreign += 1,
        }
        match labels.iter_mut().find(|entry| entry.0 == label) {
            Some(entry) => entry.1 += 1,
            None => labels.push((label, 1)),
        }
        index += 1;
    }
    let breakdown = labels
        .iter()
        .map(|(label, count)| format!("{label}={count}"))
        .collect::<Vec<_>>()
        .join(" ");
    println!(
        "property/malformed: {MALFORMED} corrupted candidates, 0 accepted, 0 panics; outcomes: invalid={rejected} exhausted={bounded} unsupported-or-failure={foreign}; kinds: {breakdown}"
    );
    Ok(())
}
