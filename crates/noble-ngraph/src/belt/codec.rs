use alloc::vec::Vec;

mod hashing;
mod instructions;

use hashing::{artifact_binding, digest};
use instructions::{count_instructions, MAX_BYTES};
use super::types::{ArtifactView, Refusal};

pub(crate) use instructions::{next_instruction, push_instruction};

const MAGIC: &[u8; 4] = b"NGB0";
const FIXED_HEADER: usize = 4 + 1 + super::FORMAT_REVISION.len()
    + 1
    + super::LOWERER_REVISION.len()
    + 1
    + super::CHECKER_REVISION.len()
    + 32
    + 1
    + 1
    + 4
    + 4
    + 32
    + 32;
/// Largest permitted encoded belt artifact, including its fixed header.
pub const MAX_ARTIFACT_BYTES: usize = FIXED_HEADER + super::MAX_INSTRUCTIONS * MAX_BYTES;

/// Bind an instruction body to its checked graph, revisions and machine bounds.
///
/// The body is structurally decoded here but not semantically accepted; use
/// [`super::check`] for the independent graph correspondence check.
pub fn encode(
    ids: &dyn crate::identity::IdentityScheme,
    checked: &crate::check::CheckedGraph,
    capacity: u8,
    save_slots: u8,
    instruction_bytes: &[u8],
) -> Result<Vec<u8>, Refusal> {
    super::support::admit_graph(checked)?;
    validate_bounds(capacity, save_slots, instruction_bytes.len())?;
    let instruction_count = count_instructions(instruction_bytes)?;
    if instruction_count > super::MAX_INSTRUCTIONS {
        return Err(Refusal::InstructionBound);
    }
    let instruction_digest = digest(ids, "noble.ngraph.belt.instructions.v0", instruction_bytes)?;
    let graph_id = checked.id();
    let binding = artifact_binding(
        ids,
        graph_id,
        capacity,
        save_slots,
        instruction_count,
        instruction_bytes.len(),
        instruction_digest,
    )?;
    let total = FIXED_HEADER
        .checked_add(instruction_bytes.len())
        .filter(|length| *length <= MAX_ARTIFACT_BYTES)
        .ok_or(Refusal::ArtifactBound)?;
    let mut bytes = Vec::with_capacity(total);
    write_header(
        &mut bytes,
        graph_id,
        capacity,
        save_slots,
        instruction_count,
        instruction_bytes.len(),
        instruction_digest,
        binding,
    )?;
    bytes.extend_from_slice(instruction_bytes);
    Ok(bytes)
}

/// Decode revisions, bounds and both content digests without accepting graph semantics.
pub fn decode<'a>(
    ids: &dyn crate::identity::IdentityScheme,
    bytes: &'a [u8],
) -> Result<ArtifactView<'a>, Refusal> {
    if bytes.len() > MAX_ARTIFACT_BYTES {
        return Err(Refusal::ArtifactBound);
    }
    let mut cursor = 0usize;
    if take(bytes, &mut cursor, MAGIC.len())? != MAGIC {
        return Err(Refusal::Revision);
    }
    read_revision(bytes, &mut cursor, super::FORMAT_REVISION)?;
    read_revision(bytes, &mut cursor, super::LOWERER_REVISION)?;
    read_revision(bytes, &mut cursor, super::CHECKER_REVISION)?;
    let graph_id = digest_field(bytes, &mut cursor)?;
    let capacity = take(bytes, &mut cursor, 1)?[0];
    let save_slots = take(bytes, &mut cursor, 1)?[0];
    let instruction_count =
        usize::try_from(read_u32(bytes, &mut cursor)?).map_err(|_| Refusal::InstructionBound)?;
    let instruction_len =
        usize::try_from(read_u32(bytes, &mut cursor)?).map_err(|_| Refusal::ArtifactBound)?;
    let declared_digest = digest_field(bytes, &mut cursor)?;
    let declared_binding = digest_field(bytes, &mut cursor)?;
    validate_bounds(capacity, save_slots, instruction_len)?;
    if instruction_count > super::MAX_INSTRUCTIONS {
        return Err(Refusal::InstructionBound);
    }
    if cursor != FIXED_HEADER {
        return Err(Refusal::Malformed);
    }
    let instruction_bytes = take(bytes, &mut cursor, instruction_len)?;
    if cursor != bytes.len() {
        return Err(Refusal::Malformed);
    }
    let observed_digest = digest(ids, "noble.ngraph.belt.instructions.v0", instruction_bytes)?;
    if observed_digest != declared_digest {
        return Err(Refusal::InstructionDigest);
    }
    let observed_count = count_instructions(instruction_bytes)?;
    if observed_count != instruction_count {
        return Err(Refusal::Malformed);
    }
    let observed_binding = artifact_binding(
        ids,
        graph_id,
        capacity,
        save_slots,
        instruction_count,
        instruction_len,
        declared_digest,
    )?;
    if observed_binding != declared_binding {
        return Err(Refusal::ArtifactBinding);
    }
    Ok(ArtifactView {
        bytes,
        graph_id,
        capacity,
        save_slots,
        instruction_bytes,
        instruction_digest: observed_digest,
        artifact_binding: observed_binding,
        instruction_count,
    })
}

fn validate_bounds(capacity: u8, save_slots: u8, instruction_len: usize) -> Result<(), Refusal> {
    if !(super::MIN_CAPACITY..=super::MAX_CAPACITY).contains(&capacity) {
        return Err(Refusal::Capacity);
    }
    if save_slots > super::MAX_SAVE_SLOTS {
        return Err(Refusal::SaveSlots);
    }
    let max_len = super::MAX_INSTRUCTIONS
        .checked_mul(MAX_BYTES)
        .ok_or(Refusal::ArtifactBound)?;
    if instruction_len > max_len {
        return Err(Refusal::ArtifactBound);
    }
    Ok(())
}

fn write_header(
    out: &mut Vec<u8>,
    graph_id: crate::ir::Digest,
    capacity: u8,
    save_slots: u8,
    instruction_count: usize,
    instruction_len: usize,
    instruction_digest: crate::ir::Digest,
    binding: crate::ir::Digest,
) -> Result<(), Refusal> {
    out.extend_from_slice(MAGIC);
    write_revision(out, super::FORMAT_REVISION)?;
    write_revision(out, super::LOWERER_REVISION)?;
    write_revision(out, super::CHECKER_REVISION)?;
    out.extend_from_slice(&graph_id);
    out.extend_from_slice(&[capacity, save_slots]);
    out.extend_from_slice(
        &u32::try_from(instruction_count)
            .map_err(|_| Refusal::InstructionBound)?
            .to_le_bytes(),
    );
    out.extend_from_slice(
        &u32::try_from(instruction_len)
            .map_err(|_| Refusal::ArtifactBound)?
            .to_le_bytes(),
    );
    out.extend_from_slice(&instruction_digest);
    out.extend_from_slice(&binding);
    Ok(())
}

fn write_revision(out: &mut Vec<u8>, revision: &str) -> Result<(), Refusal> {
    out.push(u8::try_from(revision.len()).map_err(|_| Refusal::Malformed)?);
    out.extend_from_slice(revision.as_bytes());
    Ok(())
}

fn read_revision(bytes: &[u8], cursor: &mut usize, expected: &str) -> Result<(), Refusal> {
    let len = usize::from(take(bytes, cursor, 1)?[0]);
    if take(bytes, cursor, len)? != expected.as_bytes() {
        return Err(Refusal::Revision);
    }
    Ok(())
}

fn digest_field(bytes: &[u8], cursor: &mut usize) -> Result<crate::ir::Digest, Refusal> {
    take(bytes, cursor, 32)?
        .try_into()
        .map_err(|_| Refusal::Malformed)
}

fn read_u32(bytes: &[u8], cursor: &mut usize) -> Result<u32, Refusal> {
    let raw: [u8; 4] = take(bytes, cursor, 4)?
        .try_into()
        .map_err(|_| Refusal::Malformed)?;
    Ok(u32::from_le_bytes(raw))
}

fn take<'a>(bytes: &'a [u8], cursor: &mut usize, len: usize) -> Result<&'a [u8], Refusal> {
    let end = cursor.checked_add(len).ok_or(Refusal::Malformed)?;
    let field = bytes.get(*cursor..end).ok_or(Refusal::Malformed)?;
    *cursor = end;
    Ok(field)
}

