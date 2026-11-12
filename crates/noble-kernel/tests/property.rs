#![feature(register_tool)]
#![register_tool(octet, tigerstyle)]
#![expect(
    tigerstyle::no_panic,
    reason = "property test crate: a failing generated property panics by design"
)]
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
#[path = "property/malformed_lane.rs"]
mod malformed_lane;
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
#[expect(
    tigerstyle::function_length,
    reason = "Owner: noble-maintainers; the agreement lane keeps generation, comparison, coverage assertions and the failure report in one reading order; the derivations and shrinking it delegates are already separate functions."
)]
fn generated_candidates_agree_with_independent_oracle() -> Result<(), String> {
    let env = gen::environment()?;
    let mut rng = rng::Stream::new(0x0B1E_5EED_0000_0001);
    let mut both_accepted = 0;
    let mut both_rejected = 0;
    // Per-word emission counts and the words that appear in accepted cases.
    // Without both, a generator whose words never fit would keep "agreeing"
    // with the oracle on rejections alone while its Data and branch rules
    // decided nothing.
    let mut emitted = [0_u32; fit::WORDS.len()];
    let mut accepted_words: Vec<u32> = Vec::new();
    let mut index = 0;
    while index < GENERATED {
        let case = gen::case(&mut rng)?;
        let decision = oracle::decide(&case.request, &case.candidate);
        let outcome = noble_kernel::acceptance::check(&env, &case.request, &case.candidate);
        for word in &case.words {
            if let Some(slot) = usize::try_from(*word)
                .ok()
                .and_then(|word_index| emitted.get_mut(word_index))
            {
                *slot = slot.saturating_add(1);
            }
        }
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
                otypes::Decision::Accept => {
                    both_accepted += 1;
                    accepted_words.extend(accepted_word_derivations(&case, &outcome)?);
                }
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
    if both_accepted == 0 {
        return Err("the agreement lane accepted no generated candidate".to_string());
    }
    if both_rejected == 0 {
        return Err("the agreement lane rejected no generated candidate".to_string());
    }
    // The primitive words this lane claims to exercise must be emitted *and*
    // decided by their own rules: a generator whose witnesses never fit keeps
    // "agreeing" with the oracle on rejections alone (the defect this control
    // pins for `dup`/`drop`'s Data rules).
    let accepted_once = [(0_u32, "dup"), (1, "drop"), (2, "swap")];
    // The eliminators are asserted as generated: their candidates are compared
    // for agreement like every other word, and this lane claims only that the
    // pool reaches them.
    let emitted_once = [(17_u32, "case"), (18, "if"), (21, "list.case")];
    let mut kind_report: Vec<String> =
        Vec::with_capacity(accepted_once.len().saturating_add(emitted_once.len()));
    for (word, name) in accepted_once {
        let emission_count = usize::try_from(word)
            .ok()
            .and_then(|word_index| emitted.get(word_index))
            .copied()
            .unwrap_or(0);
        if emission_count == 0 {
            return Err(format!(
                "the generator emitted no {name} ({word}) in {GENERATED} cases"
            ));
        }
        if !accepted_words.contains(&word) {
            return Err(format!(
                "no accepted candidate used {name} ({word}); its rule decided no case"
            ));
        }
        kind_report.push(format!("{name}={emission_count}"));
    }
    for (word, name) in emitted_once {
        let emission_count = usize::try_from(word)
            .ok()
            .and_then(|word_index| emitted.get(word_index))
            .copied()
            .unwrap_or(0);
        if emission_count == 0 {
            return Err(format!(
                "the generator emitted no {name} ({word}) in {GENERATED} cases"
            ));
        }
        kind_report.push(format!("{name}={emission_count}"));
    }
    println!(
        "property/agreement: {GENERATED} generated candidates, {both_accepted} accepted by both sides, {both_rejected} rejected by both sides, 0 disagreements"
    );
    println!(
        "property/agreement: {}; dup/drop/swap each decided an accepted case",
        kind_report.join(" ")
    );
    Ok(())
}

/// The word definitions the checker itself derived for one accepted candidate.
///
/// A knob-corrupted case may have had a node repaired or repointed, so only an
/// uncorrupted case's derivations attest the generated witness itself.
#[expect(
    tigerstyle::unbounded_collection_growth,
    reason = "Owner: noble-maintainers; one accepted candidate contributes at most its declared nodes limit (256) of derivations, each appended once; the vector is bounded by the request the generator declares."
)]
fn accepted_word_derivations(
    case: &gen::Case,
    outcome: &noble_kernel::untrusted::Outcome,
) -> Result<Vec<u32>, String> {
    let mut words = Vec::new();
    if case.is_corrupted {
        return Ok(words);
    }
    let noble_kernel::untrusted::Outcome::Accepted(checked) = outcome else {
        return Ok(words);
    };
    for derivation in &checked.derivations {
        let node_index = match usize::try_from(derivation.node.0) {
            Ok(node_index) => node_index,
            Err(problem) => {
                return Err(format!(
                    "derived node {} exceeds usize: {problem}",
                    derivation.node.0
                ))
            }
        };
        if let Some(noble_kernel::untrusted::Node::Invocation { def, .. }) =
            case.candidate.nodes.get(node_index)
        {
            words.push(def.0);
        }
    }
    Ok(words)
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
