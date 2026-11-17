//! Readmitting a retained record with a lower status keeps the status and
//! promotion the library already holds.

use super::*;

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
