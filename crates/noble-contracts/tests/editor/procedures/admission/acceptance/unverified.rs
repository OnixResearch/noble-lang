//! Submitter claims, including the issue #131 reproducer, never establish a
//! status; the owner's verified outcome decides, and malformed attestations
//! are refused.

use super::super::super::support::verifier::attestation;
use super::*;

/// The issue #131 reproducer, plus a check that nothing was retained.
#[test]
fn fabricated_lean_proof_must_not_promote_procedure() -> Result<(), String> {
    let namespace = Session::without_test_hosts();
    let mut library = ProcedureLibrary::new(revision(7));
    let mut record = literal_record(6, Producer::Chunk).map_err(|e| format!("{e:?}"))?;
    record.evidence.push(EvidenceRecord {
        obligation: 7,
        class: EvidenceClass::Lean,
        outcome: EvidenceOutcome::Proved,
        toolchain: vec![String::from("unverified-caller-string")],
        claim: String::from("no proof artifact exists"),
        subject_binding: record.procedure_id,
        policy_revision: 3,
        capability: None,
    });
    record.status = Status::Promoted;
    record.promotion = Some(noble_contracts::source::procedures::Promotion {
        policy_revision: 3,
        obligations: vec![7],
    });
    record.refresh_retrieval_signature();
    let policy = ConsumerPolicy {
        revision: 3,
        obligations: vec![ConsumerObligation {
            id: 7,
            allowed_classes: vec![EvidenceClass::Lean],
        }],
    };

    // Required invariant: an unauthenticated string claiming Lean success
    // must not establish a verified/promoted procedure.
    assert!(library.admit(&namespace, Some(&policy), record).is_err());
    assert!(library.records().is_empty());
    Ok(())
}

/// The forged-`tested` control for the same root cause.
#[test]
fn unexecuted_test_claim_must_not_mark_procedure_tested() -> Result<(), String> {
    let namespace = Session::without_test_hosts();
    let mut library = ProcedureLibrary::new(revision(7));
    let mut record = literal_record(6, Producer::Chunk).map_err(|e| format!("{e:?}"))?;
    record.evidence.push(EvidenceRecord {
        obligation: 7,
        class: EvidenceClass::Test,
        outcome: EvidenceOutcome::Passed,
        toolchain: vec![String::from("unverified-caller-string")],
        claim: String::from("no test was executed"),
        subject_binding: record.procedure_id,
        policy_revision: 3,
        capability: None,
    });
    record.status = Status::Tested;
    record.refresh_retrieval_signature();

    // Same root cause: a caller-forged `test/passed` string is not an
    // executed test outcome and must not establish `tested`.
    assert!(library.admit(&namespace, None, record).is_err());
    assert!(library.records().is_empty());
    Ok(())
}

#[test]
fn fabricated_claims_are_refused_with_forged_status_and_leave_the_library_unchanged(
) -> Result<(), String> {
    let namespace = Session::without_test_hosts();
    let promoted = promoted_record(31)?;
    let tested = tested_record(32)?;
    // No verifier, and a verifier whose owner never checked these claims.
    for library in [
        ProcedureLibrary::new(revision(7)),
        ProcedureLibrary::with_verifier(revision(7), OwnerLedger::empty().boxed()),
    ] {
        let mut library = library;
        let refused = library
            .admit(&namespace, Some(&proof_policy()), promoted.clone())
            .expect_err("a fabricated proof claim must not promote");
        assert_eq!(refused.kind, RefusalKind::ForgedStatus);
        assert_eq!(refused.location.field, "promotion.obligations");
        assert_eq!(refused.location.index, Some(0));
        let refused = library
            .admit(&namespace, None, tested.clone())
            .expect_err("an unexecuted test claim must not mark tested");
        assert_eq!(refused.kind, RefusalKind::ForgedStatus);
        assert_eq!(refused.location.field, "status");
        assert!(library.records().is_empty());
        assert_eq!(library.current_generation(), 0);
    }
    Ok(())
}

#[test]
fn the_verified_outcome_not_the_claimed_outcome_decides() -> Result<(), String> {
    let namespace = Session::without_test_hosts();
    let tested = tested_record(41)?;
    let mut owner = OwnerLedger::empty();
    owner.observe(&tested.evidence[0], EvidenceOutcome::Failed);
    let mut library = ProcedureLibrary::with_verifier(revision(7), owner.boxed());
    let refused = library
        .admit(&namespace, None, tested.clone())
        .expect_err("a verified failure cannot support tested");
    assert_eq!(refused.kind, RefusalKind::ForgedStatus);
    assert_eq!(refused.location.field, "status");
    let mut candidate = tested;
    candidate.status = Status::Candidate;
    candidate.refresh_retrieval_signature();
    let id = library
        .admit(&namespace, None, candidate)
        .map_err(|error| format!("candidate: {error:?}"))?;
    let retained = record(&library, id)?;
    assert_eq!(retained.evidence[0].outcome, EvidenceOutcome::Passed);
    assert_eq!(
        retained.verified_evidence()[0].outcome(),
        EvidenceOutcome::Failed
    );
    let manifest = library
        .identity_manifest()
        .map_err(|error| format!("manifest: {error:?}"))?;
    let manifest: serde_json::Value =
        serde_json::from_slice(&manifest).map_err(|error| format!("manifest JSON: {error}"))?;
    let entry = &manifest["entries"][0];
    assert_eq!(entry["evidence"][0]["outcome"], "passed");
    assert_eq!(entry["verified_evidence"][0]["outcome"], "failed");
    Ok(())
}

#[test]
fn malformed_owner_attestations_are_refused() -> Result<(), String> {
    let namespace = Session::without_test_hosts();
    let tested = tested_record(51)?;
    let test_claim = tested.evidence[0].clone();
    let mut unbound = attestation(&test_claim, EvidenceOutcome::Passed);
    unbound.command_digest = [0; 32];
    let mut anonymous = attestation(&test_claim, EvidenceOutcome::Passed);
    anonymous.toolchain.clear();
    let cases = [
        (
            attestation(&test_claim, EvidenceOutcome::Proved),
            RefusalKind::ForgedStatus,
            "verified_evidence.outcome",
        ),
        (unbound, RefusalKind::Invalid, "verified_evidence.binding"),
        (
            anonymous,
            RefusalKind::Invalid,
            "verified_evidence.toolchain",
        ),
    ];
    for (report, kind, field) in cases {
        let mut owner = OwnerLedger::empty();
        owner.attest(&test_claim, report);
        let mut library = ProcedureLibrary::with_verifier(revision(7), owner.boxed());
        let refused = library
            .admit(&namespace, None, tested.clone())
            .expect_err("a malformed attestation must refuse admission");
        assert_eq!(refused.kind, kind, "{field}");
        assert_eq!(refused.location.field, field);
        assert!(library.records().is_empty());
    }
    Ok(())
}
