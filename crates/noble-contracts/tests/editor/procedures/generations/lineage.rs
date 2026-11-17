use super::{
    literal_record, revision, GenerationKind, ProcedureId, ProcedureLibrary, Producer, Query,
    QueryBudget, RefusalKind, Session, Status, String,
};

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
    assert_ne!(first, second);
    assert_eq!(library.current_generation(), 2);
    Ok((namespace, library, [first, second]))
}

fn assert_lineage(
    library: &ProcedureLibrary,
    expected: &[(ProcedureId, Status, Option<ProcedureId>)],
) -> Result<(), String> {
    assert_eq!(library.records().len(), expected.len());
    for (id, status, predecessor) in expected {
        let record = library
            .records()
            .iter()
            .find(|record| record.procedure_id == *id)
            .ok_or("lineage record must remain retained")?;
        assert_eq!(record.status, *status);
        assert_eq!(record.supersedes, *predecessor);
    }
    Ok(())
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
    )?;
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
    assert_lineage(&library, &chain)?;
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
    assert_lineage(&library, &chain)?;
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
    assert_lineage(&restarted, &chain)?;
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
    )?;
    Ok(())
}
