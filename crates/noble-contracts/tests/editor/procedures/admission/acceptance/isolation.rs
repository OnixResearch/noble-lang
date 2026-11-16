//! Receipts do not travel with a record, and inspection never consults the
//! owner's verifier.

use super::*;

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
