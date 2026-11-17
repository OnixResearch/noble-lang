use super::{
    literal_record, record_offsets, revision, ProcedureId, ProcedureLibrary, Producer, RefusalKind,
    Session,
};

// Test-only wire producer: rebind the version-1 integrity indexes after a
// hostile state edit, so reload must check lineage rather than stale hashes.
fn wire_digest(domain: u64, bytes: &[u8]) -> ProcedureId {
    let mut output = [0; 32];
    for (lane, mask) in output.chunks_exact_mut(8).zip([
        0,
        0x9e37_79b9_7f4a_7c15,
        0x3c6e_f372_fe94_f82a,
        0xdaa6_6d2c_7ddf_743f,
    ]) {
        let prefix = (domain ^ mask)
            .to_le_bytes()
            .into_iter()
            .chain(1u64.to_le_bytes())
            .chain((bytes.len() as u64).to_le_bytes());
        let mut hash = prefix
            .chain(bytes.iter().copied())
            .fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
                (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3)
            });
        hash = (hash ^ (hash >> 33)).wrapping_mul(0xff51_afd7_ed55_8ccd);
        hash = (hash ^ (hash >> 33)).wrapping_mul(0xc4ce_b9fe_1a85_ec53);
        lane.copy_from_slice(&(hash ^ (hash >> 33)).to_be_bytes());
    }
    output
}

fn change_predecessor(
    mut bytes: Vec<u8>,
    id: ProcedureId,
    predecessor: Option<ProcedureId>,
) -> Result<Vec<u8>, String> {
    let offsets = record_offsets(&bytes)?
        .into_iter()
        .rev()
        .find(|offsets| {
            offsets
                .procedure_id_offset_bytes
                .checked_add(32)
                .and_then(|end_byte_offset| {
                    bytes.get(offsets.procedure_id_offset_bytes..end_byte_offset)
                })
                == Some(id.as_slice())
        })
        .ok_or("lineage record missing from wire")?;
    let edge =
        core::iter::once(u8::from(predecessor.is_some())).chain(predecessor.into_iter().flatten());
    bytes.splice(offsets.supersedes.0..offsets.supersedes.1, edge);
    Ok(bytes)
}

fn rebind_head(bytes: &mut [u8], library: &ProcedureLibrary) -> Result<(), String> {
    let offsets = record_offsets(bytes)?;
    let records = library.records();
    let generations = library.generation_records();
    let head = generations.last().ok_or("head generation missing")?;
    let mut closure = Vec::with_capacity(records.len().saturating_mul(64));
    for ((record, offsets), entry) in records
        .iter()
        .zip(offsets.iter().rev().take(records.len()).rev())
        .zip(&head.closure)
    {
        let identity_end_byte_offset = offsets
            .procedure_id_offset_bytes
            .checked_add(32)
            .ok_or("identity offset overflow")?;
        assert_eq!(
            bytes.get(offsets.procedure_id_offset_bytes..identity_end_byte_offset),
            Some(record.procedure_id.as_slice())
        );
        let state = bytes
            .get(offsets.state.0..offsets.state.1)
            .ok_or("state range missing")?;
        let digest = wire_digest(0x4e_50_52_4f_43_00_00_02, state);
        let mut old_binding = [0u8; 64];
        let (identity, state_digest) = old_binding.split_at_mut(32);
        identity.copy_from_slice(&entry.procedure_id);
        state_digest.copy_from_slice(&entry.state_digest);
        let binding_offset_bytes = bytes
            .windows(old_binding.len())
            .rposition(|window| window == old_binding)
            .ok_or("closure entry missing")?;
        let digest_start_byte_offset = binding_offset_bytes
            .checked_add(32)
            .ok_or("closure offset overflow")?;
        let binding_end_byte_offset = digest_start_byte_offset
            .checked_add(32)
            .ok_or("closure digest offset overflow")?;
        bytes
            .get_mut(digest_start_byte_offset..binding_end_byte_offset)
            .ok_or("closure digest missing")?
            .copy_from_slice(&digest);
        closure.extend_from_slice(&record.procedure_id);
        closure.extend_from_slice(&digest);
    }
    let revision_offset_bytes = bytes
        .windows(32)
        .rposition(|window| window == library.procedure_library_revision())
        .ok_or("head revision missing")?;
    let revision_end_byte_offset = revision_offset_bytes
        .checked_add(32)
        .ok_or("revision offset overflow")?;
    bytes
        .get_mut(revision_offset_bytes..revision_end_byte_offset)
        .ok_or("revision range missing")?
        .copy_from_slice(&wire_digest(0x4e_50_52_4f_43_00_00_03, &closure));
    Ok(())
}

#[test]
fn refuses_rebound_forks_cycles_and_dangling_replacement_links() -> Result<(), String> {
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
    let third = library
        .replace(
            &namespace,
            None,
            second,
            literal_record(3, Producer::Chunk).map_err(|error| format!("{error:?}"))?,
        )
        .map_err(|error| format!("{error:?}"))?;
    let independent = library
        .admit(
            &namespace,
            None,
            literal_record(4, Producer::Chunk).map_err(|error| format!("{error:?}"))?,
        )
        .map_err(|error| format!("{error:?}"))?;
    let encoded = library.encode().map_err(|error| format!("{error:?}"))?;
    let mut unchanged = encoded.clone();
    rebind_head(&mut unchanged, &library)?;
    assert_eq!(
        unchanged, encoded,
        "wire producer must reproduce all bindings"
    );
    ProcedureLibrary::load(&unchanged, &namespace)
        .map_err(|error| format!("positive reload: {error:?}"))?;
    for (case, changes) in [
        ("fork", vec![(independent, Some(first))]),
        ("cycle", vec![(first, Some(second)), (third, None)]),
        ("dangling", vec![(third, Some([0xff; 32]))]),
        ("live target", vec![(third, Some(independent))]),
        ("orphan", vec![(third, None)]),
        ("self cycle", vec![(first, Some(first)), (second, None)]),
    ] {
        let mut malformed = encoded.clone();
        for (id, predecessor) in changes {
            malformed = change_predecessor(malformed, id, predecessor)?;
        }
        rebind_head(&mut malformed, &library)?;
        let refusal = ProcedureLibrary::load(&malformed, &namespace)
            .expect_err("rebound invalid lineage must refuse");
        assert_eq!(refusal.kind, RefusalKind::Invalid, "{case}: {refusal:?}");
        assert_eq!(refusal.location.field, "supersedes", "{case}: {refusal:?}");
        assert_eq!(library.current_generation(), 4);
        assert_eq!(
            library.encode().map_err(|error| format!("{error:?}"))?,
            encoded
        );
    }
    Ok(())
}
