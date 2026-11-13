//! Size-change evidence over checked definitions (issue #101).
//!
//! Each case is one module in the signed declared-text profile; the assertion is the verdict the
//! front end reports, which the Lean lane re-checks by evaluating the generated certificate.

use noble_contracts::source::{
    Argument, Evidence, Measure, ModuleSession, RetainedSourceLimit, Unsupported, Verdict,
};

const LIMITS: noble_contracts::Limits = noble_contracts::Limits {
    bytes: 65_536,
    nodes: 16_384,
    depth: 64,
    work: 2_000_000,
};

fn retained() -> RetainedSourceLimit {
    RetainedSourceLimit::new(LIMITS.bytes).expect("size-change test source limit")
}

fn session() -> Result<ModuleSession, String> {
    ModuleSession::new_declared_text(&[], true, retained()).map_err(|error| format!("{error:?}"))
}

/// The evidence one module declaration installs.
fn analyze(source: &[u8]) -> Result<Vec<Evidence>, String> {
    let prepared = session()?
        .prepare(source, &[], LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    Ok(prepared.size_change_evidence())
}

fn only(evidence: Vec<Evidence>) -> Evidence {
    assert_eq!(evidence.len(), 1, "one recursive definition expected");
    evidence.into_iter().next().expect("one entry")
}

fn module(body: &str) -> Vec<u8> {
    std::format!("module m@1 [ {body} ]").into_bytes()
}

/// Structural list recursion: the cons branch passes the tail, a structural decrease.
#[test]
fn structural_list_recursion_is_accepted() -> Result<(), String> {
    let entry = only(analyze(&module(
        "signature go forall<S:stack> [ S List<I64> -- S I64 ! pure ] export go \
         def go [ [ 0 ] [ swap drop go ] list.case ]",
    ))?);
    assert_eq!(entry.definition, "go");
    assert_eq!(entry.measures, vec![Measure::Structural]);
    assert_eq!(entry.sites.len(), 1);
    assert_eq!(entry.sites[0].arguments, vec![Argument::Tail]);
    assert_eq!(entry.verdict, Verdict::Accepted { measure: 0 });
    Ok(())
}

/// A guarded countdown: `0 <` is the positivity guard the counter predecessor needs.
#[test]
fn guarded_countdown_is_accepted() -> Result<(), String> {
    let entry = only(analyze(&module(
        "signature down forall<S:stack> [ S I64 -- S I64 ! pure ] export down \
         def down [ dup 0 swap < [ 1 - down ] [ ] if ]",
    ))?);
    assert_eq!(entry.measures, vec![Measure::Counter]);
    assert_eq!(entry.sites.len(), 1);
    assert_eq!(entry.sites[0].arguments, vec![Argument::Predecessor]);
    assert_eq!(entry.sites[0].guarded, vec![true]);
    assert_eq!(entry.verdict, Verdict::Accepted { measure: 0 });
    Ok(())
}

/// REC-01's zero test is not a positivity guard: the branch with the predecessor has no `0 <`, so
/// the argument is opaque and the definition is refused.
#[test]
fn zero_test_is_not_a_positivity_guard() -> Result<(), String> {
    let entry = only(analyze(&module(
        "signature down forall<S:stack> [ S I64 -- S I64 ! pure ] export down \
         def down [ dup 0 = [ ] [ 1 - down ] if ]",
    ))?);
    assert_eq!(entry.sites.len(), 1);
    assert_eq!(entry.sites[0].arguments, vec![Argument::Opaque]);
    assert_eq!(entry.sites[0].guarded, vec![false]);
    assert!(matches!(entry.verdict, Verdict::Refused { .. }));
    Ok(())
}

/// A non-decreasing self-call: refused, and the witness names the site that fails the measure.
#[test]
fn non_decreasing_self_call_is_refused_with_its_site() -> Result<(), String> {
    let entry = only(analyze(&module(
        "signature same forall<S:stack> [ S I64 -- S I64 ! pure ] export same def same [ same ]",
    ))?);
    assert_eq!(entry.sites.len(), 1);
    assert_eq!(entry.sites[0].arguments, vec![Argument::Same]);
    match entry.verdict {
        Verdict::Refused { witnesses } => {
            assert_eq!(witnesses.len(), 1);
            assert_eq!(witnesses[0].measure, 0);
            assert_eq!(witnesses[0].site, 0);
            assert_eq!(witnesses[0].argument, Argument::Same);
        }
        other => return Err(std::format!("expected a refusal, got {other:?}")),
    }
    Ok(())
}

/// Wrapping `I64` arithmetic as a measure: unclassifiable, refused rather than assumed decreasing.
#[test]
fn wrapping_arithmetic_is_refused() -> Result<(), String> {
    let entry = only(analyze(&module(
        "signature wrap forall<S:stack> [ S I64 -- S I64 ! pure ] export wrap def wrap [ 1 + wrap ]",
    ))?);
    assert_eq!(entry.sites[0].arguments, vec![Argument::Opaque]);
    match entry.verdict {
        Verdict::Refused { witnesses } => {
            assert_eq!(witnesses[0].argument, Argument::Opaque);
        }
        other => return Err(std::format!("expected a refusal, got {other:?}")),
    }
    Ok(())
}

/// A guard on one measure does not justify another measure's predecessor: the guarded measure is
/// the shared measure, the unguarded one stays opaque.
#[test]
fn guard_justifies_only_the_measure_it_names() -> Result<(), String> {
    let entry = only(analyze(&module(
        "signature two forall<S:stack> [ S I64 I64 -- S I64 I64 ! pure ] export two \
         def two [ dup 0 swap < [ 1 - swap 1 - swap two ] [ ] if ]",
    ))?);
    assert_eq!(entry.measures, vec![Measure::Counter, Measure::Counter]);
    assert_eq!(entry.sites.len(), 1);
    assert_eq!(entry.sites[0].guarded, vec![false, true]);
    assert_eq!(
        entry.sites[0].arguments,
        vec![Argument::Opaque, Argument::Predecessor]
    );
    assert_eq!(entry.verdict, Verdict::Accepted { measure: 1 });
    Ok(())
}

/// Several sites all passing the same measure as its predecessor certify the family.
#[test]
fn two_sites_sharing_the_measure_are_accepted() -> Result<(), String> {
    let entry = only(analyze(&module(
        "signature down forall<S:stack> [ S I64 -- S I64 ! pure ] export down \
         def down [ dup 0 swap < [ 1 - dup 0 swap < [ down ] [ down ] if ] [ ] if ]",
    ))?);
    assert_eq!(entry.sites.len(), 2);
    assert_eq!(entry.sites[0].guarded, vec![true]);
    assert_eq!(entry.sites[1].guarded, vec![true]);
    assert_eq!(entry.verdict, Verdict::Accepted { measure: 0 });
    Ok(())
}

/// One decreasing branch cannot hide another self-call that passes the measure unchanged.
#[test]
fn one_non_decreasing_site_refuses_the_shared_measure() -> Result<(), String> {
    let entry = only(analyze(&module(
        "signature down forall<S:stack> [ S I64 -- S I64 ! pure ] export down \
         def down [ dup 0 swap < [ 1 - down ] [ down ] if ]",
    ))?);
    assert_eq!(entry.sites.len(), 2);
    assert_eq!(entry.sites[0].arguments, vec![Argument::Predecessor]);
    assert_eq!(entry.sites[1].arguments, vec![Argument::Same]);
    match entry.verdict {
        Verdict::Refused { witnesses } => {
            assert_eq!(witnesses.len(), 1);
            assert_eq!(witnesses[0].site, 1);
            assert_eq!(witnesses[0].argument, Argument::Same);
        }
        other => return Err(std::format!("expected a refusal, got {other:?}")),
    }
    Ok(())
}

/// A module with no self call installs no evidence.
#[test]
fn non_recursive_definitions_carry_no_evidence() -> Result<(), String> {
    let evidence = analyze(&module(
        "signature twice forall<S:stack> [ S I64 -- S I64 ! pure ] export twice \
         def twice [ 1 + 1 + ]",
    ))?;
    assert!(
        evidence.is_empty(),
        "expected no evidence, got {evidence:?}"
    );
    Ok(())
}

/// A signature outside the bounded profile reports `unsupported`, never a verdict.
#[test]
fn signature_outside_the_profile_is_unsupported() -> Result<(), String> {
    let entry = only(analyze(&module(
        "signature flag forall<S:stack> [ S Bool -- S Bool ! pure ] export flag def flag [ flag ]",
    ))?);
    assert!(
        matches!(
            &entry.verdict,
            Verdict::Unsupported {
                reason: Unsupported::SignatureOutsideProfile,
                ..
            }
        ),
        "expected an unsupported verdict, got {:?}",
        entry.verdict
    );
    Ok(())
}

/// A body whose programs the walk cannot model reports `unsupported` rather than guessing.
#[test]
fn unmodelled_program_construct_is_unsupported() -> Result<(), String> {
    let entry = only(analyze(&module(
        "signature spin forall<S:stack> [ S I64 -- S I64 ! pure ] export spin \
         def spin [ 1 quote run drop spin ]",
    ))?);
    assert!(
        matches!(
            entry.verdict,
            Verdict::Unsupported {
                reason: Unsupported::UnmodelledCall,
                ..
            }
        ),
        "expected an unmodelled-call verdict, got {:?}",
        entry.verdict
    );
    Ok(())
}

/// A named call may execute its own recursion, so a direct decreasing self-call cannot hide it.
#[test]
fn named_recursive_helper_is_not_assumed_terminating() -> Result<(), String> {
    let evidence = analyze(&module(
        "signature spin forall<S:stack> [ S I64 -- S I64 ! pure ] export spin \
         def spin [ spin ] \
         signature down forall<S:stack> [ S I64 -- S I64 ! pure ] export down \
         def down [ dup 0 swap < [ 1 - down ] [ ] if spin ]",
    ))?;
    let entry = evidence
        .iter()
        .find(|entry| entry.definition == "down")
        .ok_or_else(|| std::string::String::from("missing recursive caller"))?;
    assert!(
        matches!(
            &entry.verdict,
            Verdict::Unsupported {
                reason: Unsupported::UnmodelledCall,
                ..
            }
        ),
        "recursive helper must not be treated as returning: {:?}",
        entry.verdict
    );
    Ok(())
}

/// The certificate gives each accepted entry a generated numeric identifier and `decide` proofs,
/// and carries no theorem for a refusal.
#[test]
fn certificate_carries_the_accepted_verdict_only() -> Result<(), String> {
    let accepted = only(analyze(&module(
        "signature down forall<S:stack> [ S I64 -- S I64 ! pure ] export down \
         def down [ dup 0 swap < [ 1 - down ] [ ] if ]",
    ))?);
    let refused = only(analyze(&module(
        "signature same forall<S:stack> [ S I64 -- S I64 ! pure ] export same def same [ same ]",
    ))?);
    let certificate = noble_contracts::source::certificate(&[accepted, refused]);
    assert!(certificate.contains("import NobleM2.SizeChangeFixtures"));
    assert!(certificate.contains("def definition_0Args : Fin 1 → Args 1"));
    assert!(certificate.contains("def definition_0Measure : Fin 1 := ⟨0, by decide⟩"));
    assert!(certificate.contains("theorem definition_0Certified : certifiedAll"));
    assert!(certificate.contains("theorem definition_0Shared : ∀ site : Fin 1"));
    assert!(certificate.contains("decide"));
    assert!(
        !certificate.contains("definition_1Args"),
        "a refusal carries no certificate"
    );
    Ok(())
}
