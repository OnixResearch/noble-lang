//! Issue #131: only evidence verified at admission by the library owner's
//! verifier establishes `tested` or `promoted`; submitter claims stay visible
//! as unverified provenance.

use super::super::super::procedures::support::verifier::{attestation, OWNER_TOOLCHAIN};
use super::*;
use noble_contracts::source::procedures::{ProcedureId, Promotion, Query, QueryBudget};
use std::cell::Cell;
use std::rc::Rc;

#[path = "reload.rs"]
mod reload;

const TEST_OBLIGATION: u32 = 5;

fn claim(
    record: &ProcedureRecord,
    obligation: u32,
    class: EvidenceClass,
    outcome: EvidenceOutcome,
) -> EvidenceRecord {
    let mut output = evidence(record, class, outcome);
    output.obligation = obligation;
    output
}

fn proof_policy() -> ConsumerPolicy {
    ConsumerPolicy {
        revision: 3,
        obligations: vec![ConsumerObligation {
            id: 7,
            allowed_classes: vec![EvidenceClass::Lean],
        }],
    }
}

fn tested_record(value: i64) -> Result<ProcedureRecord, String> {
    let mut record =
        literal_record(value, Producer::Chunk).map_err(|error| format!("tested: {error:?}"))?;
    record.evidence.push(claim(
        &record,
        TEST_OBLIGATION,
        EvidenceClass::Test,
        EvidenceOutcome::Passed,
    ));
    record.status = Status::Tested;
    record.refresh_retrieval_signature();
    Ok(record)
}

fn promoted_record(value: i64) -> Result<ProcedureRecord, String> {
    let mut record =
        literal_record(value, Producer::Chunk).map_err(|error| format!("promoted: {error:?}"))?;
    record.evidence.push(claim(
        &record,
        7,
        EvidenceClass::Lean,
        EvidenceOutcome::Proved,
    ));
    record.evidence.push(claim(
        &record,
        TEST_OBLIGATION,
        EvidenceClass::Test,
        EvidenceOutcome::Passed,
    ));
    record.status = Status::Promoted;
    record.promotion = Some(Promotion {
        policy_revision: 3,
        obligations: vec![7],
    });
    record.refresh_retrieval_signature();
    Ok(record)
}

/// One library holding a record that merely claims a proof, an owner-verified
/// `tested` record and an owner-verified `promoted` record.
struct Fixture {
    namespace: Session,
    library: ProcedureLibrary,
    /// The owner's observations, for replicas given to other libraries.
    owner: OwnerLedger,
    /// Verifier calls made by `library`.
    calls: Rc<Cell<u32>>,
    claimed: ProcedureId,
    tested: ProcedureId,
    promoted: ProcedureId,
}

fn fixture() -> Result<Fixture, String> {
    let namespace = Session::without_test_hosts();
    let mut claimed =
        literal_record(21, Producer::Chunk).map_err(|error| format!("claimed: {error:?}"))?;
    let unverified_proof = evidence(&claimed, EvidenceClass::Lean, EvidenceOutcome::Proved);
    claimed.evidence.push(unverified_proof);
    let tested = tested_record(22)?;
    let promoted = promoted_record(23)?;
    let mut owner = OwnerLedger::empty();
    for checked in tested.evidence.iter().chain(&promoted.evidence) {
        owner.confirm(checked);
    }
    let verifier = owner.replica();
    let calls = verifier.calls();
    let mut library = ProcedureLibrary::with_verifier(revision(7), verifier.boxed());
    let claimed_id = library
        .admit(&namespace, None, claimed)
        .map_err(|error| format!("claimed: {error:?}"))?;
    let tested_id = library
        .admit(&namespace, None, tested)
        .map_err(|error| format!("tested: {error:?}"))?;
    let promoted_id = library
        .admit(&namespace, Some(&proof_policy()), promoted)
        .map_err(|error| format!("promoted: {error:?}"))?;
    // One question per distinct claim, at the admission that introduced it.
    assert_eq!(calls.get(), 4);
    assert_eq!(library.current_generation(), 3);
    Ok(Fixture {
        namespace,
        library,
        owner,
        calls,
        claimed: claimed_id,
        tested: tested_id,
        promoted: promoted_id,
    })
}

fn record(library: &ProcedureLibrary, id: ProcedureId) -> Result<&ProcedureRecord, String> {
    library
        .records()
        .iter()
        .find(|record| record.procedure_id == id)
        .ok_or_else(|| String::from("admitted record is absent"))
}

fn status_query(status: Option<Status>) -> Query {
    Query {
        exact_interface: None,
        composition_context: None,
        dependency_root: None,
        status,
    }
}

/// Candidates under each status filter, in canonical order.
fn status_views(library: &ProcedureLibrary) -> Result<Vec<Vec<ProcedureId>>, String> {
    let mut budget = QueryBudget::new();
    let mut views = Vec::with_capacity(4);
    for status in [
        None,
        Some(Status::Promoted),
        Some(Status::Tested),
        Some(Status::Candidate),
    ] {
        let result = library
            .retrieve(&status_query(status), &mut budget)
            .map_err(|error| format!("{status:?} query: {error:?}"))?;
        views.push(result.candidates);
    }
    Ok(views)
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
fn owner_verified_claims_establish_status_with_bound_receipts() -> Result<(), String> {
    let fixture = fixture()?;
    let promoted = record(&fixture.library, fixture.promoted)?;
    assert_eq!(promoted.status, Status::Promoted);
    assert_eq!(promoted.evidence.len(), 2);
    let receipts = promoted.verified_evidence();
    assert_eq!(receipts.len(), 2);
    for receipt in receipts {
        assert_eq!(receipt.subject_binding(), &fixture.promoted);
        assert_eq!(receipt.verifier(), OWNER_VERIFIER);
        assert_eq!(receipt.toolchain(), [String::from(OWNER_TOOLCHAIN)]);
        assert_eq!(receipt.policy_revision(), 3);
        assert_eq!(receipt.input_digest(), &fixture.promoted);
        assert_ne!(receipt.command_digest(), &[0; 32]);
        let claim = promoted
            .evidence
            .iter()
            .find(|claim| claim.obligation == receipt.obligation())
            .ok_or("receipt answers no retained claim")?;
        assert_eq!(receipt.class(), claim.class);
        assert_eq!(receipt.outcome(), claim.outcome);
    }
    assert_ne!(receipts[0].claim_digest(), receipts[1].claim_digest());
    let tested = record(&fixture.library, fixture.tested)?;
    assert_eq!(tested.status, Status::Tested);
    assert_eq!(tested.verified_evidence().len(), 1);
    assert_eq!(tested.verified_evidence()[0].class(), EvidenceClass::Test);
    // The unverified proof claim stays visible but establishes nothing.
    let claimed = record(&fixture.library, fixture.claimed)?;
    assert_eq!(claimed.status, Status::Candidate);
    assert_eq!(claimed.evidence.len(), 1);
    assert!(claimed.verified_evidence().is_empty());
    Ok(())
}

#[test]
fn readmission_reuses_decisions_and_new_claims_are_asked_once() -> Result<(), String> {
    let mut fixture = fixture()?;
    let before = fixture
        .library
        .encode()
        .map_err(|error| format!("encode: {error:?}"))?;
    let readmitted = record(&fixture.library, fixture.promoted)?.clone();
    fixture
        .library
        .admit(
            &fixture.namespace,
            Some(&proof_policy()),
            readmitted.clone(),
        )
        .map_err(|error| format!("idempotent readmission: {error:?}"))?;
    assert_eq!(fixture.calls.get(), 4, "retained claims are not re-asked");
    assert_eq!(
        fixture
            .library
            .encode()
            .map_err(|error| format!("encode after: {error:?}"))?,
        before
    );

    // An appended claim is asked exactly once, and an unverified one does
    // not change the promoted state.
    let mut extended = readmitted;
    let review = claim(&extended, 8, EvidenceClass::Review, EvidenceOutcome::Passed);
    extended.evidence.push(review);
    fixture
        .library
        .admit(&fixture.namespace, Some(&proof_policy()), extended)
        .map_err(|error| format!("appended claim: {error:?}"))?;
    assert_eq!(fixture.calls.get(), 5);
    let promoted = record(&fixture.library, fixture.promoted)?;
    assert_eq!(promoted.evidence.len(), 3);
    assert_eq!(promoted.verified_evidence().len(), 2);
    assert_eq!(promoted.status, Status::Promoted);
    Ok(())
}

#[test]
fn receipts_do_not_travel_with_a_record() -> Result<(), String> {
    let fixture = fixture()?;
    let promoted = record(&fixture.library, fixture.promoted)?.clone();
    let tested = record(&fixture.library, fixture.tested)?.clone();
    assert_eq!(promoted.verified_evidence().len(), 2);
    // A record cloned out of a library that verified it carries receipts,
    // but another library discards them and asks its own owner.
    let strangers = [
        ProcedureLibrary::new(revision(7)),
        ProcedureLibrary::with_verifier(revision(7), OwnerLedger::empty().boxed()),
    ];
    for mut stranger in strangers {
        let refused = stranger
            .admit(&fixture.namespace, Some(&proof_policy()), promoted.clone())
            .expect_err("a transplanted receipt must not promote");
        assert_eq!(refused.kind, RefusalKind::ForgedStatus);
        assert_eq!(refused.location.field, "promotion.obligations");
        let refused = stranger
            .admit(&fixture.namespace, None, tested.clone())
            .expect_err("a transplanted receipt must not mark tested");
        assert_eq!(refused.kind, RefusalKind::ForgedStatus);
        assert_eq!(refused.location.field, "status");
        assert!(stranger.records().is_empty());
    }
    // The same owner, consulted by a second library, re-derives them.
    let mut peer = ProcedureLibrary::with_verifier(revision(7), fixture.owner.replica().boxed());
    peer.admit(&fixture.namespace, Some(&proof_policy()), promoted.clone())
        .map_err(|error| format!("same owner: {error:?}"))?;
    assert_eq!(
        peer.records()[0].verified_evidence(),
        promoted.verified_evidence()
    );
    Ok(())
}

#[test]
fn the_verified_outcome_not_the_claimed_outcome_decides() -> Result<(), String> {
    let namespace = Session::without_test_hosts();
    let tested = tested_record(41)?;
    let test_claim = tested.evidence[0].clone();
    let mut owner = OwnerLedger::empty();
    owner.observe(&test_claim, EvidenceOutcome::Failed);
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
    let manifest: serde_json::Value = serde_json::from_slice(
        &library
            .identity_manifest()
            .map_err(|error| format!("manifest: {error:?}"))?,
    )
    .map_err(|error| format!("manifest JSON: {error}"))?;
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
    for (report, kind, field) in [
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
    ] {
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

#[test]
fn inspection_and_status_filters_never_run_the_verifier() -> Result<(), String> {
    let mut fixture = fixture()?;
    let library = &fixture.library;
    let views = status_views(library)?;
    assert_eq!(views[0].len(), 3);
    assert_eq!(
        views[1..],
        [
            vec![fixture.promoted],
            vec![fixture.tested],
            vec![fixture.claimed]
        ]
    );
    let manifest = library
        .identity_manifest()
        .map_err(|error| format!("manifest: {error:?}"))?;
    let text = core::str::from_utf8(&manifest).map_err(|error| format!("utf8: {error}"))?;
    assert!(text.contains("\"verified_evidence\""));
    library
        .encode()
        .map_err(|error| format!("encode: {error:?}"))?;
    library
        .canonical_chunk_order()
        .map_err(|error| format!("chunk order: {error:?}"))?;
    let receipt_count: usize = library
        .records()
        .iter()
        .map(|record| record.verified_evidence().len())
        .sum();
    assert_eq!(receipt_count, 3);
    assert_eq!(library.generation_records().len(), 4);
    assert_eq!(
        fixture.calls.get(),
        4,
        "inspection must not consult the verifier"
    );

    // Demotion re-derives status from retained receipts only.
    fixture
        .library
        .demote(&fixture.namespace, fixture.promoted)
        .map_err(|error| format!("demote: {error:?}"))?;
    assert_eq!(
        record(&fixture.library, fixture.promoted)?.status,
        Status::Tested
    );
    assert_eq!(fixture.calls.get(), 4);
    Ok(())
}
