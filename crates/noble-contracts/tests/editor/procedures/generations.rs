#![allow(
    tigerstyle::function_length,
    reason = "Owner: noble-contracts; replacement and rollback integration tests check complete append-only histories and exact restored closures in deterministic transition traces"
)]

use super::super::procedures::support::{literal_record, revision};
use noble_contracts::source::procedures::{
    EvidenceClass, EvidenceOutcome, EvidenceRecord, GenerationKind, ProcedureId, ProcedureLibrary,
    Producer, Query, QueryBudget, RefusalKind, Status,
};
use noble_contracts::source::Session;
use std::string::String;

fn identities(library: &ProcedureLibrary) -> Vec<[u8; 32]> {
    library
        .records()
        .iter()
        .map(|record| record.procedure_id)
        .collect()
}

#[test]
fn replacements_and_rollback_append_exact_history() -> Result<(), String> {
    let namespace = Session::without_test_hosts();
    let mut library = ProcedureLibrary::new(revision(7));
    let first = literal_record(1, Producer::Chunk).map_err(|error| format!("first: {error:?}"))?;
    let first_id = library
        .admit(&namespace, None, first)
        .map_err(|error| format!("first admit: {error:?}"))?;
    let second =
        literal_record(2, Producer::Chunk).map_err(|error| format!("second: {error:?}"))?;
    let second_id = library
        .admit(&namespace, None, second)
        .map_err(|error| format!("second admit: {error:?}"))?;
    let generation_two = library
        .generation_records()
        .into_iter()
        .find(|record| record.generation == 2)
        .ok_or("generation two missing")?;
    let generation_two_closure = generation_two.closure.clone();
    let generation_two_procedure_revision = generation_two.procedure_library_revision;
    let generation_two_chunk_revision = generation_two.chunk_library_revision;
    let generation_two_order = library
        .canonical_chunk_order()
        .map_err(|error| format!("chunk order: {error:?}"))?;

    let replacement =
        literal_record(3, Producer::Chunk).map_err(|error| format!("replacement: {error:?}"))?;
    let replacement_id = library
        .replace(&namespace, None, second_id, replacement)
        .map_err(|error| format!("replace: {error:?}"))?;
    assert_ne!(replacement_id, second_id);
    assert!(library
        .records()
        .iter()
        .any(|record| record.procedure_id == second_id
            && record.status == noble_contracts::source::procedures::Status::Superseded));
    assert!(library
        .records()
        .iter()
        .any(|record| record.procedure_id == replacement_id));
    assert_eq!(library.generation_records().len(), 4);

    let mut evidence_record = literal_record(3, Producer::Chunk)
        .map_err(|error| format!("evidence record: {error:?}"))?;
    evidence_record.evidence.push(EvidenceRecord {
        obligation: 3,
        class: EvidenceClass::Test,
        outcome: EvidenceOutcome::Passed,
        toolchain: vec![String::from("toolchain-present-only")],
        claim: String::from("replacement test evidence"),
        subject_binding: replacement_id,
        policy_revision: 1,
        capability: None,
    });
    library
        .admit(&namespace, None, evidence_record)
        .map_err(|error| format!("append evidence: {error:?}"))?;
    assert_eq!(library.current_generation(), 4);
    let generation_four = library
        .generation_records()
        .into_iter()
        .find(|record| record.generation == 4)
        .ok_or("generation four missing")?;
    let generation_four_closure = generation_four.closure.clone();
    let before_failed_rollback = library
        .identity_manifest()
        .map_err(|error| format!("manifest before refused rollback: {error:?}"))?;
    let refusal = library
        .rollback(&namespace, 4, &[])
        .expect_err("rollback must refuse unavailable evidence toolchain");
    assert_eq!(refusal.kind, RefusalKind::Invalid);
    assert_eq!(refusal.location.field, "evidence.toolchain");
    assert_eq!(
        library
            .identity_manifest()
            .map_err(|error| format!("manifest after refused rollback: {error:?}"))?,
        before_failed_rollback
    );
    assert_eq!(
        library
            .generation_records()
            .into_iter()
            .find(|record| record.generation == 4)
            .ok_or("generation four lost")?
            .closure,
        generation_four_closure
    );

    library
        .rollback(&namespace, 2, &[])
        .map_err(|error| format!("rollback retained generation: {error:?}"))?;
    assert_eq!(library.current_generation(), 5);
    assert_eq!(
        library.procedure_library_revision(),
        &generation_two_procedure_revision
    );
    assert_eq!(
        library.chunk_library_revision(),
        &generation_two_chunk_revision
    );
    assert_eq!(
        library
            .canonical_chunk_order()
            .map_err(|error| format!("restored chunk order: {error:?}"))?,
        generation_two_order
    );
    let restored = library
        .generation_records()
        .into_iter()
        .find(|record| record.generation == 5)
        .ok_or("rollback generation missing")?;
    assert_eq!(restored.kind, GenerationKind::Rollback);
    assert_eq!(restored.parent_generation, Some(2));
    assert_eq!(restored.rollback_of, Some(4));
    assert_eq!(restored.closure, generation_two_closure);
    let restored_ids = identities(&library);
    assert_eq!(restored_ids.len(), 2);
    assert!(restored_ids.contains(&first_id));
    assert!(restored_ids.contains(&second_id));
    assert!(!restored_ids.contains(&replacement_id));
    assert_eq!(
        library.generation_records().len(),
        6,
        "all generations remain retained"
    );

    let before_unknown = library
        .identity_manifest()
        .map_err(|error| format!("manifest: {error:?}"))?;
    let unknown = library
        .rollback(&namespace, 99, &[])
        .expect_err("unavailable generation must refuse");
    assert_eq!(unknown.kind, RefusalKind::UnknownGeneration);
    assert_eq!(
        library
            .identity_manifest()
            .map_err(|error| format!("manifest after unknown rollback: {error:?}"))?,
        before_unknown
    );
    Ok(())
}

#[path = "generations/lineage.rs"]
mod lineage;
