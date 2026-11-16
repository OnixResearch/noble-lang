//! Persisted verification receipts are never trusted from bytes: load
//! re-establishes each one through the loading owner's verifier.

use super::*;

#[test]
fn reload_reestablishes_every_receipt_and_reproduces_all_views() -> Result<(), String> {
    let fixture = fixture()?;
    let bytes = fixture
        .library
        .encode()
        .map_err(|error| format!("encode: {error:?}"))?;
    let manifest = fixture
        .library
        .identity_manifest()
        .map_err(|error| format!("manifest: {error:?}"))?;
    let views = status_views(&fixture.library)?;
    let later = tested_record(24)?;
    for restart in 0..2 {
        let mut verifier = fixture.owner.replica();
        verifier.confirm(&later.evidence[0]);
        let calls = verifier.calls();
        let mut reloaded =
            ProcedureLibrary::load_with_verifier(&bytes, &fixture.namespace, verifier.boxed())
                .map_err(|error| format!("restart {restart}: {error:?}"))?;
        // Three distinct receipts, each re-established once although they
        // are retained in several generations.
        assert_eq!(calls.get(), 3);
        assert_eq!(
            reloaded
                .identity_manifest()
                .map_err(|error| format!("reloaded manifest: {error:?}"))?,
            manifest
        );
        assert_eq!(
            reloaded
                .encode()
                .map_err(|error| format!("reloaded encode: {error:?}"))?,
            bytes
        );
        assert_eq!(reloaded.records(), fixture.library.records());
        assert_eq!(status_views(&reloaded)?, views);
        assert_eq!(calls.get(), 3, "inspection after reload asks nothing");
        // The loading owner's verifier serves later admissions.
        reloaded
            .admit(&fixture.namespace, None, later.clone())
            .map_err(|error| format!("later admission: {error:?}"))?;
        assert_eq!(calls.get(), 4);
    }
    Ok(())
}

#[test]
fn persisted_receipts_are_not_trusted_without_the_same_owner_answer() -> Result<(), String> {
    let fixture = fixture()?;
    let bytes = fixture
        .library
        .encode()
        .map_err(|error| format!("encode: {error:?}"))?;
    let promoted = record(&fixture.library, fixture.promoted)?;
    let tested = record(&fixture.library, fixture.tested)?;
    let of_class = |class: EvidenceClass| {
        promoted
            .evidence
            .iter()
            .find(|claim| claim.class == class)
            .ok_or_else(|| format!("{class:?} claim missing"))
    };
    let (proof, test) = (
        of_class(EvidenceClass::Lean)?,
        of_class(EvidenceClass::Test)?,
    );
    // Checked the tested claim only.
    let mut partial = OwnerLedger::empty();
    partial.confirm(&tested.evidence[0]);
    // Checked everything, but observed another outcome for the proof.
    let mut dissenting = OwnerLedger::empty();
    dissenting.confirm(&tested.evidence[0]);
    dissenting.confirm(test);
    dissenting.observe(proof, EvidenceOutcome::Unknown);
    let refusals = [
        ProcedureLibrary::load(&bytes, &fixture.namespace),
        ProcedureLibrary::load_with_verifier(
            &bytes,
            &fixture.namespace,
            OwnerLedger::empty().boxed(),
        ),
        ProcedureLibrary::load_with_verifier(&bytes, &fixture.namespace, partial.boxed()),
        ProcedureLibrary::load_with_verifier(&bytes, &fixture.namespace, dissenting.boxed()),
    ];
    for (case, outcome) in refusals.into_iter().enumerate() {
        let refused = outcome.expect_err("an unconfirmed persisted receipt must refuse load");
        assert_eq!(refused.kind, RefusalKind::Invalid, "case {case}");
        assert_eq!(refused.location.field, "verified_evidence", "case {case}");
    }
    Ok(())
}

#[test]
fn a_store_from_a_lenient_verifier_cannot_launder_a_fabricated_proof() -> Result<(), String> {
    let namespace = Session::without_test_hosts();
    let fabricated = promoted_record(61)?;
    // A verifier controlled by the submitter vouches for anything it is shown.
    let mut lenient = OwnerLedger::empty();
    for vouched in &fabricated.evidence {
        lenient.confirm(vouched);
    }
    let mut forger = ProcedureLibrary::with_verifier(revision(7), lenient.boxed());
    forger
        .admit(&namespace, Some(&proof_policy()), fabricated)
        .map_err(|error| format!("lenient admission: {error:?}"))?;
    let bytes = forger
        .encode()
        .map_err(|error| format!("encode: {error:?}"))?;
    let refusals = [
        ProcedureLibrary::load(&bytes, &namespace),
        ProcedureLibrary::load_with_verifier(&bytes, &namespace, OwnerLedger::empty().boxed()),
    ];
    for outcome in refusals {
        let refused = outcome.expect_err("a forged persisted receipt must refuse load");
        assert_eq!(refused.kind, RefusalKind::Invalid);
        assert_eq!(refused.location.field, "verified_evidence");
        assert_eq!(refused.location.index, Some(0));
    }
    Ok(())
}

#[test]
fn a_claims_only_store_revision_is_refused_explicitly() -> Result<(), String> {
    let namespace = Session::without_test_hosts();
    let mut bytes = ProcedureLibrary::new(revision(7))
        .encode()
        .map_err(|error| format!("encode: {error:?}"))?;
    bytes
        .get_mut(..8)
        .ok_or("store magic missing")?
        .copy_from_slice(b"NPLIB001");
    let refused = ProcedureLibrary::load(&bytes, &namespace)
        .expect_err("revision-1 stores carried unverified status and must not load");
    assert_eq!(refused.kind, RefusalKind::Invalid);
    assert_eq!(refused.location.field, "magic");
    Ok(())
}

#[test]
fn rollback_requires_every_verified_toolchain_and_asks_nothing() -> Result<(), String> {
    let mut fixture = fixture()?;
    let claimed_only = [String::from("procedure-fixture-v1")];
    let refused = fixture
        .library
        .rollback(&fixture.namespace, 2, &claimed_only)
        .expect_err("an unavailable verifier toolchain must refuse rollback");
    assert_eq!(refused.kind, RefusalKind::Invalid);
    assert_eq!(refused.location.field, "verified_evidence.toolchain");
    assert_eq!(fixture.library.current_generation(), 3);
    fixture
        .library
        .rollback(
            &fixture.namespace,
            2,
            &[
                String::from("procedure-fixture-v1"),
                String::from(OWNER_TOOLCHAIN),
            ],
        )
        .map_err(|error| format!("rollback: {error:?}"))?;
    assert_eq!(fixture.library.current_generation(), 4);
    assert_eq!(
        record(&fixture.library, fixture.tested)?.status,
        Status::Tested
    );
    assert!(record(&fixture.library, fixture.promoted).is_err());
    assert_eq!(fixture.calls.get(), 4);
    Ok(())
}
