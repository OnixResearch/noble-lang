//! Publication copy budgets for transitive imported proof dependencies.

use noble_contracts::source::{ModuleSession, Stage};

const LIMITS: noble_contracts::Limits = noble_contracts::Limits {
    bytes: 65_536,
    nodes: 16_384,
    depth: 64,
    work: 2_000_000,
};
const BASE: &[u8] = b"module a@1 [ proof 1 base : [ (Pi (x I64) (Eq I64 x x)) ] [ (intro (x I64) (refl x)) ] export base ]";
const MIDDLE: &[u8] = b"module b@1 [ proof 1 mid : [ (Pi (x I64) (Eq I64 x x)) ] [ (intro (x I64) (apply (use a.base) x)) ] proof 1 other : [ (Pi (y I64) (Eq I64 y y)) ] [ (intro (y I64) (apply (use a.base) y)) ] export mid export other ]";

fn commit(session: ModuleSession, source: &[u8]) -> Result<ModuleSession, String> {
    let prepared = session.prepare(source, &[], LIMITS).map_err(|error| format!("{error:?}"))?;
    let (session, result) = session.commit(prepared);
    result.map_err(|error| format!("{error:?}"))?;
    Ok(session)
}

fn publish(
    session: ModuleSession,
    source: &[u8],
    limits: noble_contracts::Limits,
) -> Result<(ModuleSession, Result<(), noble_contracts::source::Error>), String> {
    let prepared = session.prepare(source, &[], limits).map_err(|error| format!("{error:?}"))?;
    Ok(session.commit_verified(prepared, |batch| {
        let mut checked = noble_contracts::intrinsic::check_batch(batch, limits)
            .map_err(|error| format!("{error:?}"))?;
        for proof in &mut checked {
            // Syntactically valid host revisions stand in for the external checker.
            proof.model_revision = "a".repeat(64);
            proof.checker_revision = "b".repeat(64);
        }
        Ok(checked)
    }))
}

fn imported_chain() -> Result<ModuleSession, String> {
    let (session, result) = publish(ModuleSession::new(&[]).map_err(|error| format!("{error:?}"))?, BASE, LIMITS)?;
    result.map_err(|error| format!("{error:?}"))?;
    let session = commit(session, b"import a@1 as a")?;
    let (session, result) = publish(session, MIDDLE, LIMITS)?;
    result.map_err(|error| format!("{error:?}"))?;
    commit(session, b"import b@1 as b")
}

#[test]
fn nested_imported_dependencies_are_charged_for_every_published_use() -> Result<(), String> {
    // A long original source keeps the per-copy publication budget, not the
    // staged snapshot share or retained-source limit, as the first refusal.
    let padding = "# Padding retained by every published copy of this module.\n".repeat(8);
    let source = format!(
        "module c@1 [\n{padding} proof 1 top : [ (Pi (x I64) (Eq I64 x x)) ] [ (intro (x I64) (apply (use b.mid) x)) ] proof 1 side : [ (Pi (z I64) (Eq I64 z z)) ] [ (intro (z I64) (apply (use b.other) z)) ] ]"
    );
    // Each published proof retains its own module source and a complete copy of
    // each used dependency with its transitive closure; the shared `base` is
    // therefore copied once under `mid` and once under `other`.
    let per_proof = source
        .len()
        .checked_add(MIDDLE.len())
        .and_then(|total| total.checked_add(BASE.len()))
        .ok_or("fixture byte count overflow")?;
    let exact = u32::try_from(per_proof.checked_mul(2).ok_or("fixture byte count overflow")?)
        .map_err(|_| "fixture exceeds u32 byte limit")?;
    let below = exact.checked_sub(1).ok_or("empty fixture budget")?;
    let session = imported_chain()?;
    let prior = session.generation();
    let (session, refused) = publish(session, source.as_bytes(), noble_contracts::Limits { bytes: below, ..LIMITS })?;
    let error = refused
        .err()
        .ok_or("one byte below the nested copy budget must refuse publication")?;
    assert_eq!(error.stage(), Stage::Acceptance);
    assert_eq!(error.diagnostic().message, "proof publication dependency budget exhausted");
    assert_eq!(session.generation(), prior);
    let (session, published) = publish(session, source.as_bytes(), noble_contracts::Limits { bytes: exact, ..LIMITS })?;
    published.map_err(|error| format!("{error:?}"))?;
    assert_eq!(Some(session.generation()), prior.checked_add(1));
    Ok(())
}
