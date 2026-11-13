use super::super::types::Refusal;

pub(super) fn digest(
    ids: &dyn crate::identity::IdentityScheme,
    domain: &str,
    bytes: &[u8],
) -> Result<crate::ir::Digest, Refusal> {
    let mut hasher = ids.start();
    hasher.update(domain.as_bytes());
    hasher.update(&[0]);
    let len = u64::try_from(bytes.len()).map_err(|_| Refusal::ArtifactBound)?;
    hasher.update(&len.to_le_bytes());
    hasher.update(bytes);
    Ok(hasher.finish())
}

pub(super) fn artifact_binding(
    ids: &dyn crate::identity::IdentityScheme,
    graph_id: crate::ir::Digest,
    capacity: u8,
    save_slots: u8,
    instruction_count: usize,
    instruction_len: usize,
    instruction_digest: crate::ir::Digest,
) -> Result<crate::ir::Digest, Refusal> {
    let mut hasher = ids.start();
    hasher.update(b"noble.ngraph.belt.artifact.v0\0");
    for revision in [
        super::super::FORMAT_REVISION,
        super::super::LOWERER_REVISION,
        super::super::CHECKER_REVISION,
    ] {
        let len = u64::try_from(revision.len()).map_err(|_| Refusal::Revision)?;
        hasher.update(&len.to_le_bytes());
        hasher.update(revision.as_bytes());
    }
    hasher.update(&graph_id);
    hasher.update(&[capacity, save_slots]);
    hasher.update(
        &u32::try_from(instruction_count)
            .map_err(|_| Refusal::InstructionBound)?
            .to_le_bytes(),
    );
    hasher.update(
        &u32::try_from(instruction_len)
            .map_err(|_| Refusal::ArtifactBound)?
            .to_le_bytes(),
    );
    hasher.update(&instruction_digest);
    Ok(hasher.finish())
}
