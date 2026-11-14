use alloc::vec::Vec;

use super::{Issue, Machine, Operation, OutputRead, Refusal};

const MAGIC: &[u8; 4] = b"NGT0";
const EVENT_BYTES: usize = 21;
const DIGEST_BYTES: usize = 32;
const HEADER_BYTES: usize = 4
    + 1
    + super::FORMAT_REVISION.len()
    + 1
    + 1
    + super::MACHINE_REVISION.len()
    + 10
    + DIGEST_BYTES
    + DIGEST_BYTES
    + 4
    + 4;
/// Largest legal target schedule artifact.
pub const MAX_ARTIFACT_BYTES: usize =
    HEADER_BYTES + super::MAX_EVENTS * EVENT_BYTES + super::MAX_OUTPUTS * 4 + 64;

/// Encode an untrusted schedule using the fixed descriptor selected by `machine`.
///
/// Structural encoding succeeds independently of semantic legality; use [`super::check`]
/// before execution. This boundary permits independent schedulers to submit candidates.
pub fn encode(
    ids: &dyn crate::identity::IdentityScheme,
    checked: &crate::check::CheckedGraph,
    machine: Machine,
    events: &[Issue],
    outputs: &[OutputRead],
) -> Result<Vec<u8>, Refusal> {
    if events.len() > super::MAX_EVENTS || outputs.len() > super::MAX_OUTPUTS {
        return Err(Refusal::EventBound);
    }
    let descriptor = machine.descriptor();
    let descriptor_bytes = descriptor.bytes();
    let descriptor_digest = hash(
        ids,
        b"noble.ngraph.belt.descriptor.v0",
        &[machine as u8],
        &descriptor_bytes,
    );
    let mut body = Vec::with_capacity(events.len() * EVENT_BYTES + outputs.len() * 4);
    for event in events {
        encode_event(*event, &mut body);
    }
    for output in outputs {
        body.extend_from_slice(&output.node.to_le_bytes());
    }
    let schedule_digest = hash(ids, b"noble.ngraph.belt.schedule.v0", &[], &body);
    let graph_id = checked.id();
    let counts = [
        u32::try_from(events.len())
            .map_err(|_| Refusal::EventBound)?
            .to_le_bytes(),
        u32::try_from(outputs.len())
            .map_err(|_| Refusal::EventBound)?
            .to_le_bytes(),
    ];
    let binding = binding_digest(
        ids,
        machine,
        descriptor_digest,
        graph_id,
        counts,
        schedule_digest,
    );
    let total = HEADER_BYTES
        .checked_add(body.len())
        .and_then(|size| size.checked_add(2 * DIGEST_BYTES))
        .filter(|size| *size <= MAX_ARTIFACT_BYTES)
        .ok_or(Refusal::ArtifactBound)?;
    let mut bytes = Vec::with_capacity(total);
    bytes.extend_from_slice(MAGIC);
    put_revision(&mut bytes, super::FORMAT_REVISION)?;
    bytes.push(machine as u8);
    put_revision(&mut bytes, super::MACHINE_REVISION)?;
    bytes.extend_from_slice(&descriptor_bytes);
    bytes.extend_from_slice(&descriptor_digest);
    bytes.extend_from_slice(&graph_id);
    bytes.extend_from_slice(&counts[0]);
    bytes.extend_from_slice(&counts[1]);
    bytes.extend_from_slice(&body);
    bytes.extend_from_slice(&schedule_digest);
    bytes.extend_from_slice(&binding);
    Ok(bytes)
}

/// Decode and authenticate bounded framing and the exact fixed descriptor.
pub fn decode(
    ids: &dyn crate::identity::IdentityScheme,
    bytes: &[u8],
) -> Result<super::DecodedSchedule, Refusal> {
    if bytes.len() > MAX_ARTIFACT_BYTES || bytes.len() < HEADER_BYTES + 2 * DIGEST_BYTES {
        return Err(Refusal::ArtifactBound);
    }
    let mut cursor = 0usize;
    if take(bytes, &mut cursor, MAGIC.len())? != MAGIC {
        return Err(Refusal::Malformed);
    }
    read_revision(bytes, &mut cursor, super::FORMAT_REVISION)?;
    let machine =
        Machine::from_id(take(bytes, &mut cursor, 1)?[0]).ok_or(Refusal::UnknownMachine)?;
    read_revision(bytes, &mut cursor, super::MACHINE_REVISION)?;
    let descriptor_bytes: [u8; 10] = take(bytes, &mut cursor, 10)?
        .try_into()
        .map_err(|_| Refusal::Malformed)?;
    if descriptor_bytes != machine.descriptor().bytes() {
        return Err(Refusal::DescriptorMismatch);
    }
    let descriptor_digest = digest(bytes, &mut cursor)?;
    let expected_descriptor = hash(
        ids,
        b"noble.ngraph.belt.descriptor.v0",
        &[machine as u8],
        &descriptor_bytes,
    );
    if descriptor_digest != expected_descriptor {
        return Err(Refusal::DigestMismatch);
    }
    let graph_id = digest(bytes, &mut cursor)?;
    let event_count = read_u32(bytes, &mut cursor)? as usize;
    let output_count = read_u32(bytes, &mut cursor)? as usize;
    if event_count > super::MAX_EVENTS || output_count > super::MAX_OUTPUTS {
        return Err(Refusal::EventBound);
    }
    let body_start = cursor;
    let event_bytes = event_count
        .checked_mul(EVENT_BYTES)
        .ok_or(Refusal::ArtifactBound)?;
    let output_bytes = output_count.checked_mul(4).ok_or(Refusal::ArtifactBound)?;
    let body_len = event_bytes
        .checked_add(output_bytes)
        .ok_or(Refusal::ArtifactBound)?;
    let body = take(bytes, &mut cursor, body_len)?;
    let body_end = cursor;
    let schedule_digest = digest(bytes, &mut cursor)?;
    let artifact_binding = digest(bytes, &mut cursor)?;
    if cursor != bytes.len() {
        return Err(Refusal::Malformed);
    }
    if schedule_digest != hash(ids, b"noble.ngraph.belt.schedule.v0", &[], body) {
        return Err(Refusal::DigestMismatch);
    }
    let counts = [
        u32::try_from(event_count)
            .map_err(|_| Refusal::EventBound)?
            .to_le_bytes(),
        u32::try_from(output_count)
            .map_err(|_| Refusal::EventBound)?
            .to_le_bytes(),
    ];
    if artifact_binding
        != binding_digest(
            ids,
            machine,
            descriptor_digest,
            graph_id,
            counts,
            schedule_digest,
        )
    {
        return Err(Refusal::DigestMismatch);
    }
    if body_start + body_len != body_end {
        return Err(Refusal::Malformed);
    }
    let mut events = Vec::with_capacity(event_count);
    let mut body_cursor = 0usize;
    for _ in 0..event_count {
        let raw = take(body, &mut body_cursor, EVENT_BYTES)?;
        events.push(decode_event(raw)?);
    }
    let mut outputs = Vec::with_capacity(output_count);
    for _ in 0..output_count {
        let raw: [u8; 4] = take(body, &mut body_cursor, 4)?
            .try_into()
            .map_err(|_| Refusal::Malformed)?;
        outputs.push(OutputRead {
            node: u32::from_le_bytes(raw),
        });
    }
    if body_cursor != body.len() {
        return Err(Refusal::Malformed);
    }
    Ok(super::DecodedSchedule {
        machine,
        descriptor_digest,
        graph_id,
        events,
        outputs,
        schedule_digest,
        artifact_binding,
    })
}

fn encode_event(event: Issue, bytes: &mut Vec<u8>) {
    bytes.extend_from_slice(&event.cycle.to_le_bytes());
    bytes.push(event.lane);
    bytes.push(event.unit);
    let (tag, node, a, b, payload) = match event.operation {
        Operation::Input { node, slot } => {
            let mut payload = [0u8; 8];
            payload[..4].copy_from_slice(&slot.to_le_bytes());
            (0, node, 0, 0, payload)
        }
        Operation::Constant { node, bits } => (1, node, 0, 0, bits.to_le_bytes()),
        Operation::Add {
            node,
            left_age,
            right_age,
        } => (2, node, left_age, right_age, [0; 8]),
        Operation::Sub {
            node,
            left_age,
            right_age,
        } => (3, node, left_age, right_age, [0; 8]),
        Operation::Multiply {
            node,
            left_age,
            right_age,
        } => (4, node, left_age, right_age, [0; 8]),
        Operation::Equal {
            node,
            left_age,
            right_age,
        } => (5, node, left_age, right_age, [0; 8]),
        Operation::Save { node, age, slot } => (6, node, age, slot, [0; 8]),
        Operation::Load { node, slot } => (7, node, slot, 0, [0; 8]),
    };
    bytes.push(tag);
    bytes.extend_from_slice(&node.to_le_bytes());
    bytes.push(a);
    bytes.push(b);
    bytes.extend_from_slice(&payload);
}

fn decode_event(raw: &[u8]) -> Result<Issue, Refusal> {
    let cycle = u32::from_le_bytes(raw[0..4].try_into().map_err(|_| Refusal::Malformed)?);
    let lane = raw[4];
    let unit = raw[5];
    let tag = raw[6];
    let node = u32::from_le_bytes(raw[7..11].try_into().map_err(|_| Refusal::Malformed)?);
    let a = raw[11];
    let b = raw[12];
    let payload = &raw[13..21];
    let zero = || payload.iter().all(|value| *value == 0);
    let operation = match tag {
        0 if a == 0 && b == 0 && payload[4..].iter().all(|value| *value == 0) => {
            let slot = u32::from_le_bytes(payload[..4].try_into().map_err(|_| Refusal::Malformed)?);
            Operation::Input { node, slot }
        }
        1 if a == 0 && b == 0 => Operation::Constant {
            node,
            bits: u64::from_le_bytes(payload.try_into().map_err(|_| Refusal::Malformed)?),
        },
        2 if payload.iter().all(|value| *value == 0) => Operation::Add {
            node,
            left_age: a,
            right_age: b,
        },
        3 if payload.iter().all(|value| *value == 0) => Operation::Sub {
            node,
            left_age: a,
            right_age: b,
        },
        4 if payload.iter().all(|value| *value == 0) => Operation::Multiply {
            node,
            left_age: a,
            right_age: b,
        },
        5 if payload.iter().all(|value| *value == 0) => Operation::Equal {
            node,
            left_age: a,
            right_age: b,
        },
        6 if payload.iter().all(|value| *value == 0) => Operation::Save {
            node,
            age: a,
            slot: b,
        },
        7 if b == 0 && zero() => Operation::Load { node, slot: a },
        _ => return Err(Refusal::Malformed),
    };
    Ok(Issue {
        cycle,
        lane,
        unit,
        operation,
    })
}

fn put_revision(bytes: &mut Vec<u8>, revision: &str) -> Result<(), Refusal> {
    bytes.push(u8::try_from(revision.len()).map_err(|_| Refusal::Malformed)?);
    bytes.extend_from_slice(revision.as_bytes());
    Ok(())
}

fn read_revision(bytes: &[u8], cursor: &mut usize, revision: &str) -> Result<(), Refusal> {
    let length = usize::from(take(bytes, cursor, 1)?[0]);
    if take(bytes, cursor, length)? != revision.as_bytes() {
        return Err(Refusal::Malformed);
    }
    Ok(())
}

fn digest(bytes: &[u8], cursor: &mut usize) -> Result<crate::ir::Digest, Refusal> {
    take(bytes, cursor, DIGEST_BYTES)?
        .try_into()
        .map_err(|_| Refusal::Malformed)
}

fn read_u32(bytes: &[u8], cursor: &mut usize) -> Result<u32, Refusal> {
    Ok(u32::from_le_bytes(
        take(bytes, cursor, 4)?
            .try_into()
            .map_err(|_| Refusal::Malformed)?,
    ))
}

fn take<'a>(bytes: &'a [u8], cursor: &mut usize, length: usize) -> Result<&'a [u8], Refusal> {
    let end = cursor.checked_add(length).ok_or(Refusal::Malformed)?;
    let result = bytes.get(*cursor..end).ok_or(Refusal::Malformed)?;
    *cursor = end;
    Ok(result)
}

fn hash(
    ids: &dyn crate::identity::IdentityScheme,
    domain: &[u8],
    prefix: &[u8],
    data: &[u8],
) -> crate::ir::Digest {
    let mut hasher = ids.start();
    hasher.update(&(domain.len() as u32).to_le_bytes());
    hasher.update(domain);
    hasher.update(&(prefix.len() as u32).to_le_bytes());
    hasher.update(prefix);
    hasher.update(data);
    hasher.finish()
}

fn binding_digest(
    ids: &dyn crate::identity::IdentityScheme,
    machine: Machine,
    descriptor_digest: crate::ir::Digest,
    graph_id: crate::ir::Digest,
    counts: [[u8; 4]; 2],
    schedule_digest: crate::ir::Digest,
) -> crate::ir::Digest {
    let mut fields = Vec::with_capacity(1 + 2 * DIGEST_BYTES + 8);
    fields.push(machine as u8);
    fields.extend_from_slice(&descriptor_digest);
    fields.extend_from_slice(&graph_id);
    fields.extend_from_slice(&counts[0]);
    fields.extend_from_slice(&counts[1]);
    fields.extend_from_slice(&schedule_digest);
    hash(
        ids,
        b"noble.ngraph.belt.binding.v0",
        super::FORMAT_REVISION.as_bytes(),
        &fields,
    )
}
