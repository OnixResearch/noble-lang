use super::*;

#[test]
fn refuses_reordered_edges_at_the_edge_field() -> Result<(), String> {
    let namespace = Session::without_test_hosts();
    let mut library = ProcedureLibrary::new(super::revision(7));
    let first =
        super::literal_record(1, Producer::Chunk).map_err(|error| format!("first: {error:?}"))?;
    let second =
        super::literal_record(2, Producer::Chunk).map_err(|error| format!("second: {error:?}"))?;
    let mut parent_chunk = super::envelope(
        &[],
        &[Ty::I64],
        b"1 2 + ",
        vec![
            Node::Invoke {
                dependency: 0,
                instantiation: 0,
            },
            Node::Invoke {
                dependency: 1,
                instantiation: 0,
            },
            Node::Operation {
                op: Op::Add,
                instantiation: 0,
            },
        ],
        vec![0, 1, 2],
    );
    parent_chunk.dependencies = vec![super::definition(&first), super::definition(&second)];
    parent_chunk.chunk_id = chunks::chunk_identity_of(&parent_chunk);
    let parent = ProcedureRecord::candidate_with_dependencies(
        parent_chunk,
        super::provenance(Producer::Composition),
        vec![
            super::library_dependency(&first),
            super::library_dependency(&second),
        ],
    )
    .map_err(|error| format!("parent: {error:?}"))?;
    library
        .admit_batch(&namespace, None, vec![first, second, parent])
        .map_err(|error| format!("batch: {error:?}"))?;
    let before = library
        .identity_manifest()
        .map_err(|error| format!("manifest: {error:?}"))?;
    let encoded = library
        .encode()
        .map_err(|error| format!("encode: {error:?}"))?;
    let offsets = super::record_offsets(&encoded).map_err(|error| format!("parse: {error}"))?;
    let (_, start_bytes, end_bytes) = offsets
        .iter()
        .map(|offsets| offsets.dependencies)
        .find(|(count_offset_bytes, _, end_bytes)| {
            let Some(count_end_bytes) = count_offset_bytes.checked_add(super::U32_FIELD_BYTES)
            else {
                return false;
            };
            encoded
                .get(*count_offset_bytes..count_end_bytes)
                .and_then(|bytes| <[u8; 4]>::try_from(bytes).ok())
                .map(u32::from_be_bytes)
                == Some(2)
                && *end_bytes > 0
        })
        .ok_or("two ordered dependency edges not found")?;
    let mut reordered = encoded.clone();
    let second_start_bytes = start_bytes
        .checked_add(super::DEPENDENCY_EDGE_BYTES)
        .ok_or("dependency offset overflow")?;
    let mut first_edge = [0u8; super::DEPENDENCY_EDGE_BYTES];
    first_edge.copy_from_slice(
        reordered
            .get(start_bytes..second_start_bytes)
            .ok_or("first dependency edge truncated")?,
    );
    reordered.copy_within(second_start_bytes..end_bytes, start_bytes);
    let second_edge_start_bytes = end_bytes
        .checked_sub(super::DEPENDENCY_EDGE_BYTES)
        .ok_or("dependency edge end precedes its start")?;
    reordered
        .get_mut(second_edge_start_bytes..end_bytes)
        .ok_or("second dependency edge truncated")?
        .copy_from_slice(&first_edge);
    let refusal = ProcedureLibrary::load(&reordered, &namespace)
        .expect_err("reordered edge list must refuse");
    assert_eq!(refusal.kind, RefusalKind::Invalid);
    assert_eq!(refusal.location.field, "library_dependencies");
    assert_eq!(
        library
            .identity_manifest()
            .map_err(|error| format!("original manifest: {error:?}"))?,
        before
    );
    Ok(())
}
