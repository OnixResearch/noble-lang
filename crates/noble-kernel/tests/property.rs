#![feature(register_tool)]
#![register_tool(octet, tigerstyle)]
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
#[path = "property/generator/gen.rs"]
mod gen;
#[path = "property/generator/knobs.rs"]
mod knobs;
#[path = "property/generator/malform.rs"]
mod malform;
#[path = "property/oracle.rs"]
mod oracle;
#[path = "property/otypes.rs"]
mod otypes;
#[path = "property/generator/random.rs"]
mod rng;
#[path = "property/shrink.rs"]
mod shrink;
#[path = "property/table.rs"]
mod table;

/// Generated candidates in the agreement lane.
const GENERATED: u64 = 1000;
/// Structurally corrupted candidates in the malformed lane.
const MALFORMED: u64 = 200;

/// The oracle's contract table must carry an arm for every pool word: a
/// missing arm makes the differential harness silently reject that word's
/// candidates instead of comparing them, so the agreement lane loses all
/// power over it. This control fails the suite the moment an arm goes
/// missing (it is the refusing check behind the removed-eliminator-oracle-
/// arm refusal in `verification/m3-proof-gate-refusals.sh`).
// r[verify DX-PROPERTY-01]
#[test]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; every pool word is checked for an independent oracle arm and a missing arm returns its word identity as an error; redundant assertions would not add coverage."
)]
fn oracle_table_covers_every_pool_word() -> Result<(), String> {
    let env = gen::environment()?;
    for word in fit::WORDS.iter().copied() {
        let index = usize::try_from(word).map_err(|error| format!("word {word}: {error}"))?;
        let kinds: Vec<noble_kernel::words::VariableKind> = if let Some(def) = env.defs.get(index) {
            def.var_kinds.clone()
        } else {
            // The resource-maker fixture is not an environment definition;
            // its documented contract is one stack variable.
            vec![noble_kernel::words::VariableKind::Stack]
        };
        let inst = noble_kernel::words::Inst {
            bindings: kinds
                .iter()
                .map(|kind| match kind {
                    noble_kernel::words::VariableKind::Stack => {
                        noble_kernel::words::Binding::Stack(vec![
                            noble_kernel::types::Ty::I64,
                            noble_kernel::types::Ty::Bool,
                        ])
                    }
                    noble_kernel::words::VariableKind::Value => {
                        noble_kernel::words::Binding::Value(noble_kernel::types::Ty::I64)
                    }
                    noble_kernel::words::VariableKind::Effect => {
                        noble_kernel::words::Binding::Effect(noble_kernel::types::EffSet::empty())
                    }
                })
                .collect(),
        };
        if table::word_face(word, &inst).is_none() {
            return Err(format!(
                "the oracle's contract table has no arm for pool word {word}"
            ));
        }
    }
    println!(
        "property/oracle-coverage: the contract table decides every pool word ({} arms)",
        fit::WORDS.len()
    );
    Ok(())
}

// r[verify DX-PROPERTY-01]
// r[verify VT-M2-01]
#[test]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; all 1000 seeded candidates must match the independent oracle or return a predicate-preserving minimized disagreement; assertion padding would lose that failure report."
)]
fn generated_candidates_agree_with_independent_oracle() -> Result<(), String> {
    let env = gen::environment()?;
    let mut rng = rng::Stream::new(0x0B1E_5EED_0000_0001);
    let mut both_accepted = 0;
    let mut both_rejected = 0;
    let mut index = 0;
    while index < GENERATED {
        let case = gen::case(&mut rng)?;
        let decision = oracle::decide(&case.request, &case.candidate);
        let outcome = noble_kernel::acceptance::check(&env, &case.request, &case.candidate);
        if matches!(
            (decision, &outcome),
            (
                otypes::Decision::Accept,
                noble_kernel::untrusted::Outcome::Accepted(_)
            )
        ) || matches!(
            (decision, &outcome),
            (
                otypes::Decision::Reject,
                noble_kernel::untrusted::Outcome::Invalid(_)
            )
        ) {
            #[expect(
                tigerstyle::fragile_exhaustive_enum_match,
                reason = "Owner: noble-maintainers; every oracle Decision must be classified explicitly in agreement totals; a new decision must fail compilation until the harness defines its meaning."
            )]
            match decision {
                otypes::Decision::Accept => both_accepted += 1,
                otypes::Decision::Reject => both_rejected += 1,
            }
        } else {
            return Err(disagreement_report(
                index,
                decision,
                &outcome,
                Disagreement {
                    env: &env,
                    request: &case.request,
                    candidate: &case.candidate,
                },
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

/// Inputs needed to replay and minimize a kernel/oracle disagreement.
struct Disagreement<'a> {
    env: &'a noble_kernel::contracts::Env,
    request: &'a noble_kernel::untrusted::Request,
    candidate: &'a noble_kernel::untrusted::Candidate,
}

/// Shrink one disagreement while it persists, then report it.
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; disagreement_report replays the exact acceptance/invalidity predicate during bounded shrinking and formats the surviving failure; it does not replace the caller's Result failure with a panic."
)]
fn disagreement_report(
    index: u64,
    decision: otypes::Decision,
    outcome: &noble_kernel::untrusted::Outcome,
    original: Disagreement<'_>,
) -> String {
    let holds = |probe_request: &noble_kernel::untrusted::Request,
                 probe: &noble_kernel::untrusted::Candidate| {
        let probe_decision = oracle::decide(probe_request, probe);
        let probe_outcome = noble_kernel::acceptance::check(original.env, probe_request, probe);
        let is_agreement = matches!(
            (&probe_decision, &probe_outcome),
            (
                otypes::Decision::Accept,
                noble_kernel::untrusted::Outcome::Accepted(_)
            ) | (
                otypes::Decision::Reject,
                noble_kernel::untrusted::Outcome::Invalid(_)
            )
        );
        !is_agreement
    };
    let (small_request, small, steps) =
        shrink::minimize(original.request, original.candidate, &holds);
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
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; each of 200 seeded corruptions returns an error on acceptance and any checker panic fails the test directly; bounded nonaccepting outcomes remain deliberately permitted."
)]
fn malformed_candidates_never_panic_or_accept() -> Result<(), String> {
    let env = gen::environment()?;
    let mut rng = rng::Stream::new(0x0B1E_5EED_0000_0002);
    let mut rejected = 0;
    let mut bounded = 0;
    let mut unavailable = 0;
    let mut internal_failure_count = 0;
    let kinds =
        usize::try_from(malform::KINDS).map_err(|error| format!("corruption kinds: {error}"))?;
    let mut labels: Vec<(&str, u32)> = Vec::with_capacity(kinds);
    let mut index = 0;
    while index < MALFORMED {
        let case = gen::case(&mut rng)?;
        let (broken, label) = malform::corrupt(&mut rng, case)?;
        let outcome = noble_kernel::acceptance::check(&env, &broken.request, &broken.candidate);
        match outcome {
            noble_kernel::untrusted::Outcome::Accepted(_) => {
                return Err(format!(
                    "malformed lane case {index} ({label}) was accepted; candidate: {:?}",
                    broken.candidate
                ));
            }
            noble_kernel::untrusted::Outcome::Invalid(_) => rejected += 1,
            noble_kernel::untrusted::Outcome::Exhausted(_) => bounded += 1,
            noble_kernel::untrusted::Outcome::Unsupported(_) => unavailable += 1,
            noble_kernel::untrusted::Outcome::InternalFailure => internal_failure_count += 1,
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
        "property/malformed: {MALFORMED} corrupted candidates, 0 accepted, 0 panics; outcomes: invalid={rejected} exhausted={bounded} unsupported={unavailable} internal-failure={internal_failure_count}; kinds: {breakdown}"
    );
    Ok(())
}
