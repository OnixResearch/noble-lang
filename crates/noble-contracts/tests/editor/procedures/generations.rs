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

fn replacement_pair() -> Result<(Session, ProcedureLibrary, [ProcedureId; 2]), String> {
    let namespace = Session::without_test_hosts();
    let mut library = ProcedureLibrary::new(revision(7));
    let first = library
        .admit(
            &namespace,
            None,
            literal_record(1, Producer::Chunk).map_err(|error| format!("{error:?}"))?,
        )
        .map_err(|error| format!("{error:?}"))?;
    let second = library
        .replace(
            &namespace,
            None,
            first,
            literal_record(2, Producer::Chunk).map_err(|error| format!("{error:?}"))?,
        )
        .map_err(|error| format!("{error:?}"))?;
    Ok((namespace, library, [first, second]))
}

fn assert_lineage(
    library: &ProcedureLibrary,
    expected: &[(ProcedureId, Status, Option<ProcedureId>)],
) {
    assert_eq!(library.records().len(), expected.len());
    for (id, status, predecessor) in expected {
        let record = library
            .records()
            .iter()
            .find(|record| record.procedure_id == *id)
            .expect("lineage record must remain retained");
        assert_eq!(record.status, *status);
        assert_eq!(record.supersedes, *predecessor);
    }
}

#[test]
fn replacement_must_not_fork_live_successors() -> Result<(), String> {
    let (namespace, mut library, [first, second]) = replacement_pair()?;
    let before = library.encode().map_err(|error| format!("{error:?}"))?;
    let records = library.records().to_vec();
    let generation = library.current_generation();
    // Both a new identity and an already-retained successor must refuse.
    for value in [3, 2] {
        let refusal = library
            .replace(
                &namespace,
                None,
                first,
                literal_record(value, Producer::Chunk).map_err(|error| format!("{error:?}"))?,
            )
            .expect_err("replacement must not branch A -> {B, C}");
        assert_eq!(refusal.kind, RefusalKind::StaleContext);
        assert_eq!(refusal.location.field, "supersedes");
        assert_eq!(library.current_generation(), generation);
        assert_eq!(library.records(), records);
        assert_eq!(
            library.encode().map_err(|error| format!("{error:?}"))?,
            before
        );
    }
    assert_lineage(
        &library,
        &[
            (first, Status::Superseded, None),
            (second, Status::Candidate, Some(first)),
        ],
    );
    Ok(())
}

#[test]
fn replacing_current_successor_should_support_a_linear_chain() -> Result<(), String> {
    let (namespace, mut library, [first, second]) = replacement_pair()?;
    let pair_records = library.records().to_vec();
    let third = library
        .replace(
            &namespace,
            None,
            second,
            literal_record(3, Producer::Chunk).map_err(|error| format!("{error:?}"))?,
        )
        .map_err(|error| format!("linear replacement refused: {error:?}"))?;
    let chain = [
        (first, Status::Superseded, None),
        (second, Status::Superseded, Some(first)),
        (third, Status::Candidate, Some(second)),
    ];
    assert_lineage(&library, &chain);
    assert_eq!(library.current_generation(), 3);
    let candidates = library
        .retrieve(
            &Query {
                exact_interface: None,
                composition_context: None,
                dependency_root: None,
                status: Some(Status::Candidate),
            },
            &mut QueryBudget::new(),
        )
        .map_err(|error| format!("retrieve: {error:?}"))?;
    assert_eq!(candidates.candidates, [third]);
    library
        .admit(
            &namespace,
            None,
            literal_record(3, Producer::Chunk).map_err(|error| format!("{error:?}"))?,
        )
        .map_err(|error| format!("duplicate: {error:?}"))?;
    assert_lineage(&library, &chain);
    assert_eq!(library.current_generation(), 3);
    let chain_records = library.records().to_vec();
    let chain_generation = library
        .generation_records()
        .last()
        .copied()
        .cloned()
        .ok_or("chain generation missing")?;
    let encoded = library.encode().map_err(|error| format!("{error:?}"))?;
    let mut loaded = ProcedureLibrary::load(&encoded, &namespace)
        .map_err(|error| format!("chain reload: {error:?}"))?;
    assert_eq!(loaded.records(), chain_records);
    assert_eq!(
        loaded.encode().map_err(|error| format!("{error:?}"))?,
        encoded
    );
    loaded
        .rollback(&namespace, 2, &[])
        .map_err(|error| format!("pair rollback: {error:?}"))?;
    assert_eq!(loaded.current_generation(), 4);
    assert_eq!(loaded.records(), pair_records);
    assert_eq!(
        loaded.generation_records().get(3).copied(),
        Some(&chain_generation)
    );
    let rollback = loaded.generation_records();
    let restored = rollback.last().ok_or("rollback generation missing")?;
    assert_eq!(restored.kind, GenerationKind::Rollback);
    assert_eq!(restored.parent_generation, Some(2));
    assert_eq!(restored.rollback_of, Some(3));
    let rolled_back = loaded.encode().map_err(|error| format!("{error:?}"))?;
    let mut restarted = ProcedureLibrary::load(&rolled_back, &namespace)
        .map_err(|error| format!("rollback reload: {error:?}"))?;
    assert_eq!(restarted.records(), pair_records);
    assert_eq!(
        restarted.encode().map_err(|error| format!("{error:?}"))?,
        rolled_back
    );
    restarted
        .rollback(&namespace, 3, &[])
        .map_err(|error| format!("chain rollback: {error:?}"))?;
    assert_eq!(restarted.current_generation(), 5);
    assert_eq!(restarted.generation_records().len(), 6);
    assert_lineage(&restarted, &chain);
    let final_bytes = restarted.encode().map_err(|error| format!("{error:?}"))?;
    let final_reload = ProcedureLibrary::load(&final_bytes, &namespace)
        .map_err(|error| format!("restored chain reload: {error:?}"))?;
    assert_eq!(final_reload.records(), chain_records);
    assert_eq!(
        final_reload.generation_records(),
        restarted.generation_records()
    );
    Ok(())
}

#[test]
fn admission_cannot_forge_or_rewrite_replacement_predecessors() -> Result<(), String> {
    let (namespace, mut library, [first, second]) = replacement_pair()?;
    let independent = literal_record(3, Producer::Chunk).map_err(|error| format!("{error:?}"))?;
    let independent_id = library
        .admit(&namespace, None, independent)
        .map_err(|error| format!("{error:?}"))?;
    let before = library.encode().map_err(|error| format!("{error:?}"))?;
    for predecessor in [first, second, independent_id, [0xff; 32]] {
        for value in [2, 4] {
            let mut record =
                literal_record(value, Producer::Chunk).map_err(|error| format!("{error:?}"))?;
            record.supersedes = Some(predecessor);
            if value == 2 && predecessor == first {
                library
                    .admit(&namespace, None, record)
                    .map_err(|error| format!("same predecessor: {error:?}"))?;
            } else {
                let refusal = library
                    .admit(&namespace, None, record)
                    .expect_err("admission cannot inject a replacement edge");
                assert_eq!(refusal.kind, RefusalKind::Invalid);
                assert_eq!(refusal.location.field, "supersedes");
            }
            assert_eq!(
                library.encode().map_err(|error| format!("{error:?}"))?,
                before
            );
        }
    }
    let refusal = library
        .replace(
            &namespace,
            None,
            independent_id,
            literal_record(2, Producer::Chunk).map_err(|error| format!("{error:?}"))?,
        )
        .expect_err("replacement cannot rewrite a retained predecessor");
    assert_eq!(refusal.kind, RefusalKind::Invalid);
    assert_eq!(refusal.location.field, "supersedes");
    assert_eq!(
        library.encode().map_err(|error| format!("{error:?}"))?,
        before
    );
    Ok(())
}

#[test]
fn replacing_with_an_existing_independent_record_retires_the_target() -> Result<(), String> {
    let namespace = Session::without_test_hosts();
    let mut library = ProcedureLibrary::new(revision(7));
    let first = literal_record(1, Producer::Chunk).map_err(|error| format!("{error:?}"))?;
    let second = literal_record(2, Producer::Chunk).map_err(|error| format!("{error:?}"))?;
    let ids = library
        .admit_batch(&namespace, None, vec![first, second.clone()])
        .map_err(|error| format!("{error:?}"))?;
    let replacement = library
        .replace(&namespace, None, ids[0], second)
        .map_err(|error| format!("existing replacement: {error:?}"))?;
    assert_eq!(replacement, ids[1]);
    assert_eq!(library.current_generation(), 2);
    assert_lineage(
        &library,
        &[
            (ids[0], Status::Superseded, None),
            (ids[1], Status::Candidate, Some(ids[0])),
        ],
    );
    Ok(())
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
