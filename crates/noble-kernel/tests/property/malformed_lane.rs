//! Structurally corrupted candidates: bounded outcome, never acceptance.

/// Structurally corrupted candidates in the malformed lane.
const MALFORMED: u64 = 200;

// r[verify DX-PROPERTY-02]
// r[verify VT-M2-01]
#[test]
#[expect(
    tigerstyle::path_segment_repetition,
    reason = "Owner: noble-maintainers; the test name is the retained evidence name cited by verification/m3-fragment.md, so it keeps its subject rather than the module's spelling."
)]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; each of 200 seeded corruptions returns an error on acceptance and any checker panic fails the test directly; bounded nonaccepting outcomes remain deliberately permitted."
)]
fn malformed_candidates_never_panic_or_accept() -> Result<(), String> {
    let env = crate::gen::environment()?;
    let mut rng = crate::rng::Stream::new(0x0B1E_5EED_0000_0002);
    let mut rejected = 0;
    let mut bounded = 0;
    let mut unavailable = 0;
    let mut internal_failure_count = 0;
    let kinds = usize::try_from(crate::malform::KINDS)
        .map_err(|error| format!("corruption kinds: {error}"))?;
    let mut labels: Vec<(&str, u32)> = Vec::with_capacity(kinds);
    let mut index = 0;
    while index < MALFORMED {
        let case = crate::gen::case(&mut rng)?;
        let (broken, label) = crate::malform::corrupt(&mut rng, case)?;
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
