use super::*;

#[test]
fn refuses_changed_record_encoder_drift_and_evidence_deletion() -> Result<(), String> {
    let namespace = Session::without_test_hosts();
    let mut library = ProcedureLibrary::new(super::revision(7));
    let record =
        super::literal_record(1, Producer::Chunk).map_err(|error| format!("record: {error:?}"))?;
    library
        .admit(&namespace, None, record.clone())
        .map_err(|error| format!("admit: {error:?}"))?;
    let expected_manifest = library
        .identity_manifest()
        .map_err(|error| format!("manifest: {error:?}"))?;
    let encoded = library
        .encode()
        .map_err(|error| format!("encode: {error:?}"))?;

    let offsets = super::record_offsets(&encoded).map_err(|error| format!("parse: {error}"))?;
    let body_offset_bytes = offsets
        .first()
        .and_then(|offsets| offsets.body_i64_byte_offset)
        .ok_or("missing encoded literal")?;
    let mut changed_body = encoded.clone();
    let body_value_offset_bytes = body_offset_bytes
        .checked_add(super::I64_LAST_BYTE_OFFSET)
        .ok_or("literal byte offset overflow")?;
    let body_byte = changed_body
        .get_mut(body_value_offset_bytes)
        .ok_or("missing literal byte")?;
    *body_byte ^= 1;
    let refusal = ProcedureLibrary::load(&changed_body, &namespace)
        .expect_err("changed body must not reload");
    assert_eq!(refusal.kind, RefusalKind::Invalid);
    assert_eq!(refusal.location.field, "procedure_id");
    assert_eq!(
        library
            .identity_manifest()
            .map_err(|error| format!("original manifest: {error:?}"))?,
        expected_manifest
    );

    let mut changed_encoder = encoded.clone();
    let encoder = noble_contracts::source::procedures::ENCODER_ID.as_bytes();
    let encoder_offset_bytes = changed_encoder
        .windows(encoder.len())
        .position(|window| window == encoder)
        .ok_or("encoder id not serialized")?;
    let revision_offset_bytes = encoder_offset_bytes
        .checked_add(encoder.len())
        .and_then(|offset_bytes| offset_bytes.checked_add(super::ENCODER_REVISION_OFFSET_BYTES))
        .ok_or("encoder revision offset overflow")?;
    *changed_encoder
        .get_mut(revision_offset_bytes)
        .ok_or("encoder revision byte missing")? = 2;
    let refusal = ProcedureLibrary::load(&changed_encoder, &namespace)
        .expect_err("encoder drift must not reload");
    assert_eq!(refusal.kind, RefusalKind::Invalid);
    assert_eq!(refusal.location.field, "procedure_id");

    let mut with_evidence = super::literal_record(2, Producer::Chunk)
        .map_err(|error| format!("evidence record: {error:?}"))?;
    with_evidence.evidence.push(super::evidence(&with_evidence));
    library
        .admit(&namespace, None, with_evidence)
        .map_err(|error| format!("evidence admit: {error:?}"))?;
    let encoded_with_evidence = library
        .encode()
        .map_err(|error| format!("evidence encode: {error:?}"))?;
    let offsets = super::record_offsets(&encoded_with_evidence)
        .map_err(|error| format!("evidence parse: {error}"))?;
    let evidence_offsets = offsets
        .iter()
        .find(|offsets| offsets.evidence.2 > offsets.evidence.1)
        .ok_or("evidence bytes not found")?
        .evidence;
    let mut deleted_evidence = encoded_with_evidence.clone();
    let (count_offset_bytes, evidence_start_bytes, evidence_end_bytes) = evidence_offsets;
    deleted_evidence.splice(
        evidence_start_bytes..evidence_end_bytes,
        core::iter::empty(),
    );
    let count_end_bytes = count_offset_bytes
        .checked_add(super::U32_FIELD_BYTES)
        .ok_or("evidence count range overflow")?;
    deleted_evidence
        .get_mut(count_offset_bytes..count_end_bytes)
        .ok_or("evidence count field missing")?
        .copy_from_slice(&0u32.to_be_bytes());

    let mut without_evidence = ProcedureLibrary::new(super::revision(7));
    without_evidence
        .admit(&namespace, None, record)
        .map_err(|error| format!("baseline record: {error:?}"))?;
    without_evidence
        .admit(
            &namespace,
            None,
            super::literal_record(2, Producer::Chunk)
                .map_err(|error| format!("baseline evidence record: {error:?}"))?,
        )
        .map_err(|error| format!("baseline evidence record admit: {error:?}"))?;
    let old_revision = *library.procedure_library_revision();
    let new_revision = *without_evidence.procedure_library_revision();
    let revision_offset_bytes = deleted_evidence
        .windows(old_revision.len())
        .rposition(|window| window == old_revision)
        .ok_or("current procedure-library revision was not serialized")?;
    let revision_end_bytes = revision_offset_bytes
        .checked_add(new_revision.len())
        .ok_or("procedure-library revision range overflow")?;
    deleted_evidence
        .get_mut(revision_offset_bytes..revision_end_bytes)
        .ok_or("procedure-library revision bytes are missing")?
        .copy_from_slice(&new_revision);
    let refusal = ProcedureLibrary::load(&deleted_evidence, &namespace)
        .expect_err("deleted evidence must fail the generation closure");
    assert_eq!(refusal.kind, RefusalKind::Invalid);
    assert_eq!(refusal.location.field, "closure.state_digest");
    assert_eq!(library.records().len(), 2);
    assert_eq!(library.current_generation(), 2);
    Ok(())
}
