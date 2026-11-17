use super::super::{Cursor, LIBRARY_HEADER_BYTES};
use super::{
    change_predecessor, literal_record, rebind_head, revision, wire_digest, ProcedureId,
    ProcedureLibrary, Producer, RefusalKind, Session,
};

#[derive(Clone, Copy, Debug)]
struct GenerationOffsets {
    parent_offset_bytes: Option<usize>,
    revision_offset_bytes: usize,
    closure: (usize, usize),
    records: (usize, usize),
    record_count: usize,
}

fn generation_offsets(bytes: &[u8]) -> Result<Vec<GenerationOffsets>, String> {
    let mut cursor = Cursor {
        bytes,
        byte_offset_bytes: 0,
    };
    cursor.skip(LIBRARY_HEADER_BYTES)?;
    let generation_count = cursor.count()?;
    let mut output = Vec::with_capacity(generation_count);
    for _ in 0..generation_count {
        cursor.skip(4)?;
        let parent_offset_bytes = if cursor.byte()? != 0 {
            let offset_bytes = cursor.byte_offset_bytes;
            cursor.skip(4)?;
            Some(offset_bytes)
        } else {
            None
        };
        if cursor.byte()? != 0 {
            cursor.skip(4)?;
        }
        cursor.skip(33)?;
        let revision_offset_bytes = cursor.byte_offset_bytes;
        cursor.skip(32)?;
        let closure_count = cursor.count()?;
        let closure_start_bytes = cursor.byte_offset_bytes;
        cursor.skip(
            closure_count
                .checked_mul(64)
                .ok_or("closure test offset overflow")?,
        )?;
        let closure_end_bytes = cursor.byte_offset_bytes;
        let record_count = cursor.count()?;
        let records_start_bytes = cursor.byte_offset_bytes;
        for _ in 0..record_count {
            cursor.record()?;
        }
        output.push(GenerationOffsets {
            parent_offset_bytes,
            revision_offset_bytes,
            closure: (closure_start_bytes, closure_end_bytes),
            records: (records_start_bytes, cursor.byte_offset_bytes),
            record_count,
        });
    }
    assert_eq!(cursor.byte_offset_bytes, bytes.len());
    Ok(output)
}

fn append_pair(
    namespace: &Session,
    library: &mut ProcedureLibrary,
    values: [i64; 2],
) -> Result<[ProcedureId; 2], String> {
    let first = literal_record(values[0], Producer::Chunk)
        .map_err(|error| format!("first record: {error:?}"))?;
    let second = literal_record(values[1], Producer::Chunk)
        .map_err(|error| format!("second record: {error:?}"))?;
    let identities = library
        .admit_batch(namespace, None, vec![first, second.clone()])
        .map_err(|error| format!("independent batch: {error:?}"))?;
    assert_eq!(identities.len(), 2);
    let successor = library
        .replace(namespace, None, identities[0], second)
        .map_err(|error| format!("independent successor: {error:?}"))?;
    assert_eq!(successor, identities[1]);
    Ok([identities[0], successor])
}

fn assert_round_trip(bytes: &[u8], namespace: &Session) -> Result<(), String> {
    let loaded = ProcedureLibrary::load(bytes, namespace)
        .map_err(|error| format!("positive reload: {error:?}"))?;
    assert_eq!(
        loaded.encode().map_err(|error| format!("{error:?}"))?,
        bytes
    );
    Ok(())
}

#[test]
fn refuses_rollback_parent_with_a_different_retained_closure() -> Result<(), String> {
    let namespace = Session::without_test_hosts();
    let mut library = ProcedureLibrary::new(revision(7));
    let [_, second] = append_pair(&namespace, &mut library, [1, 2])?;
    library
        .replace(
            &namespace,
            None,
            second,
            literal_record(3, Producer::Chunk).map_err(|error| format!("{error:?}"))?,
        )
        .map_err(|error| format!("linear replacement: {error:?}"))?;
    library
        .rollback(&namespace, 2, &[])
        .map_err(|error| format!("rollback: {error:?}"))?;
    assert_eq!(library.current_generation(), 4);
    let generations = library.generation_records();
    assert_ne!(generations[1].closure, generations[2].closure);
    assert_eq!(generations[4].closure, generations[2].closure);
    let encoded = library.encode().map_err(|error| format!("{error:?}"))?;
    assert_round_trip(&encoded, &namespace)?;
    let offsets = generation_offsets(&encoded)?;
    let parent_offset_bytes = offsets
        .last()
        .and_then(|head| head.parent_offset_bytes)
        .ok_or("rollback parent missing")?;
    let parent_end_offset_bytes = parent_offset_bytes
        .checked_add(4)
        .ok_or("parent test offset overflow")?;
    let mut malformed = encoded.clone();
    malformed
        .get_mut(parent_offset_bytes..parent_end_offset_bytes)
        .ok_or("rollback parent range missing")?
        .copy_from_slice(&1u32.to_be_bytes());
    let refusal = ProcedureLibrary::load(&malformed, &namespace)
        .expect_err("rollback must reproduce the closure of its named parent");
    assert_eq!(refusal.kind, RefusalKind::Invalid);
    assert_eq!(refusal.location.field, "generation.closure");
    assert_eq!(
        library.encode().map_err(|error| format!("{error:?}"))?,
        encoded
    );
    Ok(())
}

#[test]
fn refuses_rebound_swapped_existing_predecessors() -> Result<(), String> {
    let namespace = Session::without_test_hosts();
    let mut library = ProcedureLibrary::new(revision(7));
    let [first, second] = append_pair(&namespace, &mut library, [1, 2])?;
    let [other_first, other_second] = append_pair(&namespace, &mut library, [3, 4])?;
    assert_eq!(library.current_generation(), 4);
    let encoded = library.encode().map_err(|error| format!("{error:?}"))?;
    let mut unchanged = encoded.clone();
    rebind_head(&mut unchanged, &library)?;
    assert_eq!(unchanged, encoded);
    assert_round_trip(&encoded, &namespace)?;
    let mut malformed = change_predecessor(encoded.clone(), second, Some(other_first))?;
    malformed = change_predecessor(malformed, other_second, Some(first))?;
    rebind_head(&mut malformed, &library)?;
    assert_ne!(malformed, encoded);
    let refusal = ProcedureLibrary::load(&malformed, &namespace)
        .expect_err("valid-shaped chains cannot rewrite retained predecessors");
    assert_eq!(refusal.kind, RefusalKind::Invalid);
    assert_eq!(refusal.location.field, "supersedes");
    assert_eq!(
        library.encode().map_err(|error| format!("{error:?}"))?,
        encoded
    );
    Ok(())
}

fn duplicate_independent_head(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let offsets = generation_offsets(bytes)?;
    let head = offsets.last().ok_or("head generation missing")?;
    assert_eq!(head.record_count, 1);
    assert_eq!(head.records.1, bytes.len());
    let closure = bytes
        .get(head.closure.0..head.closure.1)
        .ok_or("head closure range missing")?;
    assert_eq!(closure.len(), 64);
    let records = bytes
        .get(head.records.0..head.records.1)
        .ok_or("head record range missing")?;
    let mut repeated_closure = closure.to_vec();
    repeated_closure.extend_from_slice(closure);
    let mut malformed = bytes
        .get(..head.revision_offset_bytes)
        .ok_or("head revision range missing")?
        .to_vec();
    malformed.extend_from_slice(&wire_digest(
        0x4e_50_52_4f_43_00_00_03,
        &repeated_closure,
    ));
    malformed.extend_from_slice(&2u32.to_be_bytes());
    malformed.extend_from_slice(&repeated_closure);
    malformed.extend_from_slice(&2u32.to_be_bytes());
    malformed.extend_from_slice(records);
    malformed.extend_from_slice(records);
    Ok(malformed)
}

#[test]
fn refuses_rebound_duplicate_independent_identities() -> Result<(), String> {
    let namespace = Session::without_test_hosts();
    let mut library = ProcedureLibrary::new(revision(7));
    let record = literal_record(1, Producer::Chunk).map_err(|error| format!("{error:?}"))?;
    let identity = library
        .admit(&namespace, None, record.clone())
        .map_err(|error| format!("first admission: {error:?}"))?;
    let encoded = library.encode().map_err(|error| format!("{error:?}"))?;
    assert_eq!(
        library
            .admit(&namespace, None, record)
            .map_err(|error| format!("unchanged duplicate: {error:?}"))?,
        identity
    );
    assert_eq!(library.current_generation(), 1);
    assert_eq!(library.records().len(), 1);
    assert_eq!(
        library.encode().map_err(|error| format!("{error:?}"))?,
        encoded
    );
    assert_round_trip(&encoded, &namespace)?;
    let malformed = duplicate_independent_head(&encoded)?;
    let refusal = ProcedureLibrary::load(&malformed, &namespace)
        .expect_err("a canonical snapshot cannot repeat an independent identity");
    assert_eq!(refusal.kind, RefusalKind::Invalid);
    assert_eq!(refusal.location.field, "procedure_id");
    assert_eq!(
        library.encode().map_err(|error| format!("{error:?}"))?,
        encoded
    );
    Ok(())
}
