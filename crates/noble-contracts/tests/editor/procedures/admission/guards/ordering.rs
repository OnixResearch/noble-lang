//! Admission examines claims and batch records in order and reports the
//! first one that fails, without looking at the ones after it.

use super::*;

#[test]
fn the_first_failing_claim_and_batch_record_are_reported() -> Result<(), String> {
    let namespace = Session::without_test_hosts();
    let mut library = ProcedureLibrary::new(revision(7));
    let mut claims =
        literal_record(12, Producer::Chunk).map_err(|error| format!("claims: {error:?}"))?;
    let valid = evidence(&claims, EvidenceClass::Test, EvidenceOutcome::Passed);
    let mut stale = evidence(&claims, EvidenceClass::Test, EvidenceOutcome::Passed);
    stale.subject_binding = [0; 32];
    let mut capability = evidence(&claims, EvidenceClass::Test, EvidenceOutcome::Passed);
    capability.capability = Some(CapabilityMarker::LiveCapability);
    claims.evidence = vec![valid, stale, capability];
    let refused = library
        .admit(&namespace, None, claims.clone())
        .expect_err("the stale second claim must refuse before the third is examined");
    assert_eq!(refused.kind, RefusalKind::StaleContext);
    assert_eq!(refused.location.field, "evidence.subject_binding");
    assert_eq!(refused.location.index, Some(1));

    let first = literal_record(13, Producer::Chunk).map_err(|error| format!("first: {error:?}"))?;
    let mut forged =
        literal_record(14, Producer::Chunk).map_err(|error| format!("forged: {error:?}"))?;
    forged.procedure_id = [0; 32];
    let refused = library
        .admit_batch(&namespace, None, vec![first, claims, forged])
        .expect_err("the second batch record must refuse before the third is examined");
    assert_eq!(refused.kind, RefusalKind::StaleContext);
    assert_eq!(refused.location.field, "evidence.subject_binding");
    assert_eq!(refused.location.index, Some(1));
    assert!(library.records().is_empty());
    assert_eq!(library.current_generation(), 0);
    Ok(())
}
