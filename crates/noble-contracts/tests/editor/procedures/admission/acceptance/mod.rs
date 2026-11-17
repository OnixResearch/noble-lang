//! Issue #131: only evidence verified at admission by the library owner's
//! verifier establishes `tested` or `promoted`; submitter claims stay visible
//! as unverified provenance.

use super::super::support::verifier::{OwnerLedger, OWNER_TOOLCHAIN, OWNER_VERIFIER};
use super::*;
use noble_contracts::source::procedures::{ProcedureId, Promotion, Query, QueryBudget};
use std::cell::Cell;
use std::rc::Rc;

#[path = "isolation.rs"]
mod isolation;
#[path = "reload.rs"]
mod reload;
#[path = "unverified.rs"]
mod unverified;

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
    let test = claim(
        &record,
        TEST_OBLIGATION,
        EvidenceClass::Test,
        EvidenceOutcome::Passed,
    );
    record.evidence.push(test);
    record.status = Status::Tested;
    record.refresh_retrieval_signature();
    Ok(record)
}

fn promoted_record(value: i64) -> Result<ProcedureRecord, String> {
    let mut record =
        literal_record(value, Producer::Chunk).map_err(|error| format!("promoted: {error:?}"))?;
    let proof = claim(&record, 7, EvidenceClass::Lean, EvidenceOutcome::Proved);
    let test = claim(
        &record,
        TEST_OBLIGATION,
        EvidenceClass::Test,
        EvidenceOutcome::Passed,
    );
    record.evidence = vec![proof, test];
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

/// Candidates of the unfiltered query, then of each status filter; every
/// filtered view is an ordered subsequence of the unfiltered one (LPL-05).
fn status_views(library: &ProcedureLibrary) -> Result<Vec<Vec<ProcedureId>>, String> {
    let mut budget = QueryBudget::new();
    let mut views = Vec::with_capacity(4);
    let filters = [
        None,
        Some(Status::Promoted),
        Some(Status::Tested),
        Some(Status::Candidate),
    ];
    for status in filters {
        let query = Query {
            exact_interface: None,
            composition_context: None,
            dependency_root: None,
            status,
        };
        let result = library
            .retrieve(&query, &mut budget)
            .map_err(|error| format!("{status:?} query: {error:?}"))?;
        views.push(result.candidates);
    }
    assert_eq!(budget.queries(), 4);
    for view in &views[1..] {
        let mut unfiltered = views[0].iter();
        assert!(view
            .iter()
            .all(|id| unfiltered.any(|candidate| candidate == id)));
    }
    Ok(views)
}

/// The issue #131 correction inside the laundering test: a claim with the
/// matching class, outcome, subject and policy revision cannot promote in
/// `unverified`, and the same record promotes in a library whose owner's
/// verifier independently checked exactly that claim. Returns that library.
pub(super) fn owner_decides_promotion(
    namespace: &Session,
    unverified: &mut ProcedureLibrary,
    policy: &ConsumerPolicy,
    promoted: &ProcedureRecord,
) -> Result<ProcedureLibrary, String> {
    let refused = unverified
        .admit(namespace, Some(policy), promoted.clone())
        .expect_err("an unverified proof claim must not promote");
    assert_eq!(refused.kind, RefusalKind::ForgedStatus);
    assert_eq!(refused.location.field, "promotion.obligations");
    assert!(unverified.records().is_empty());
    let mut ledger = OwnerLedger::empty();
    ledger.confirm(&promoted.evidence[0]);
    let mut verified = ProcedureLibrary::with_verifier(revision(7), ledger.boxed());
    verified
        .admit(namespace, Some(policy), promoted.clone())
        .map_err(|error| format!("owner-verified promotion: {error:?}"))?;
    let receipts = verified.records()[0].verified_evidence();
    assert_eq!(receipts.len(), 1);
    assert_eq!(receipts[0].verifier(), OWNER_VERIFIER);
    assert_eq!(receipts[0].outcome(), EvidenceOutcome::Proved);
    assert_eq!(verified.records()[0].evidence, promoted.evidence);
    Ok(verified)
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
        let answered = promoted
            .evidence
            .iter()
            .find(|claim| claim.obligation == receipt.obligation())
            .ok_or("receipt answers no retained claim")?;
        assert_eq!(receipt.class(), answered.class);
        assert_eq!(receipt.outcome(), answered.outcome);
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
    let policy = proof_policy();
    fixture
        .library
        .admit(&fixture.namespace, Some(&policy), readmitted.clone())
        .map_err(|error| format!("idempotent readmission: {error:?}"))?;
    assert_eq!(fixture.calls.get(), 4, "retained claims are not re-asked");
    let after = fixture
        .library
        .encode()
        .map_err(|error| format!("encode after: {error:?}"))?;
    assert_eq!(after, before);

    // An appended claim is asked exactly once; left unverified, it does not
    // change the promoted state.
    let mut extended = readmitted;
    let review = claim(&extended, 8, EvidenceClass::Review, EvidenceOutcome::Passed);
    extended.evidence.push(review);
    fixture
        .library
        .admit(&fixture.namespace, Some(&policy), extended)
        .map_err(|error| format!("appended claim: {error:?}"))?;
    assert_eq!(fixture.calls.get(), 5);
    let promoted = record(&fixture.library, fixture.promoted)?;
    assert_eq!(promoted.evidence.len(), 3);
    assert_eq!(promoted.verified_evidence().len(), 2);
    assert_eq!(promoted.status, Status::Promoted);
    Ok(())
}

#[test]
fn readmission_never_lowers_a_retained_status() -> Result<(), String> {
    let mut fixture = fixture()?;
    let mut lowered = record(&fixture.library, fixture.tested)?.clone();
    lowered.status = Status::Candidate;
    lowered.refresh_retrieval_signature();
    fixture
        .library
        .admit(&fixture.namespace, None, lowered)
        .map_err(|error| format!("candidate readmission: {error:?}"))?;
    assert_eq!(
        record(&fixture.library, fixture.tested)?.status,
        Status::Tested
    );

    let mut demoted = record(&fixture.library, fixture.promoted)?.clone();
    demoted.status = Status::Tested;
    demoted.promotion = None;
    demoted.refresh_retrieval_signature();
    fixture
        .library
        .admit(&fixture.namespace, Some(&proof_policy()), demoted)
        .map_err(|error| format!("tested readmission: {error:?}"))?;
    let promoted = record(&fixture.library, fixture.promoted)?;
    assert_eq!(promoted.status, Status::Promoted);
    assert_eq!(
        promoted.promotion,
        Some(Promotion {
            policy_revision: 3,
            obligations: vec![7],
        })
    );
    Ok(())
}
