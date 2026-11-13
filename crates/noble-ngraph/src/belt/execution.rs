use alloc::vec::Vec;

use super::types::{CheckedArtifact, Instruction, Refusal};

/// Check and execute an artifact, publishing values only after the entire run succeeds.
pub fn execute(
    ids: &dyn crate::identity::IdentityScheme,
    checked: &crate::check::CheckedGraph,
    bytes: &[u8],
    inputs: &[crate::sem::Value],
) -> Result<Vec<crate::sem::Value>, Refusal> {
    let accepted = super::validate::check(ids, checked, bytes)?;
    execute_checked(ids, checked, accepted, inputs)
}

/// Execute a checker-issued token after rechecking its artifact digest and binding.
pub fn execute_checked(
    ids: &dyn crate::identity::IdentityScheme,
    checked: &crate::check::CheckedGraph,
    accepted: CheckedArtifact<'_>,
    inputs: &[crate::sem::Value],
) -> Result<Vec<crate::sem::Value>, Refusal> {
    super::support::admit_graph(checked)?;
    let view = super::codec::decode(ids, accepted.bytes)?;
    if view.graph_id != accepted.graph_id || view.graph_id != checked.id() {
        return Err(Refusal::GraphIdentity);
    }
    if view.capacity != accepted.capacity || view.save_slots != accepted.save_slots {
        return Err(Refusal::ArtifactBinding);
    }
    if view.instruction_count != accepted.instruction_count
        || view.instruction_digest != accepted.instruction_digest
    {
        return Err(Refusal::InstructionDigest);
    }
    if view.artifact_binding != accepted.artifact_binding {
        return Err(Refusal::ArtifactBinding);
    }
    let slot_values = crate::eval::preflight(checked, inputs, None).map_err(|_| Refusal::Inputs)?;
    interpret(checked, &view, &slot_values)
}

fn interpret(
    checked: &crate::check::CheckedGraph,
    artifact: &super::types::ArtifactView<'_>,
    inputs: &[crate::sem::Value],
) -> Result<Vec<crate::sem::Value>, Refusal> {
    let mut belt = Vec::with_capacity(usize::from(artifact.capacity));
    let mut slots = alloc::vec![None; usize::from(artifact.save_slots)];
    let mut outputs = Vec::with_capacity(checked.graph().outputs.len());
    let mut cursor = 0usize;
    while cursor < artifact.instruction_bytes.len() {
        let instruction = super::codec::next_instruction(
            artifact.instruction_bytes,
            &mut cursor,
        )?;
        match instruction {
            Instruction::Input(slot) => {
                let index = usize::try_from(slot).map_err(|_| Refusal::RuntimeFault)?;
                let value = *inputs.get(index).ok_or(Refusal::RuntimeFault)?;
                insert(&mut belt, artifact.capacity, value);
            }
            Instruction::Constant(bits) => {
                let value = i64::from_le_bytes(bits.to_le_bytes());
                insert(&mut belt, artifact.capacity, crate::sem::Value::I64(value));
            }
            Instruction::Add(left, right) => {
                let (a, b) = operands(&belt, left, right)?;
                insert(
                    &mut belt,
                    artifact.capacity,
                    crate::sem::Value::I64(a.wrapping_add(b)),
                );
            }
            Instruction::Sub(left, right) => {
                let (a, b) = operands(&belt, left, right)?;
                insert(
                    &mut belt,
                    artifact.capacity,
                    crate::sem::Value::I64(a.wrapping_sub(b)),
                );
            }
            Instruction::Mul(left, right) => {
                let (a, b) = operands(&belt, left, right)?;
                insert(
                    &mut belt,
                    artifact.capacity,
                    crate::sem::Value::I64(a.wrapping_mul(b)),
                );
            }
            Instruction::Eq(left, right) => {
                let (a, b) = operands(&belt, left, right)?;
                insert(&mut belt, artifact.capacity, crate::sem::Value::Bool(a == b));
            }
            Instruction::Save(age, slot) => {
                let value = *belt
                    .get(usize::from(age))
                    .ok_or(Refusal::RuntimeFault)?;
                let target = slots
                    .get_mut(usize::from(slot))
                    .ok_or(Refusal::RuntimeFault)?;
                *target = Some(value);
            }
            Instruction::Load(slot) => {
                let value = slots
                    .get(usize::from(slot))
                    .copied()
                    .ok_or(Refusal::RuntimeFault)?
                    .ok_or(Refusal::RuntimeFault)?;
                insert(&mut belt, artifact.capacity, value);
            }
            Instruction::Output(age) => outputs.push(
                *belt
                    .get(usize::from(age))
                    .ok_or(Refusal::RuntimeFault)?,
            ),
        }
    }
    Ok(outputs)
}

fn operands(
    belt: &[crate::sem::Value],
    left: u8,
    right: u8,
) -> Result<(i64, i64), Refusal> {
    let left = belt
        .get(usize::from(left))
        .copied()
        .ok_or(Refusal::RuntimeFault)?;
    let right = belt
        .get(usize::from(right))
        .copied()
        .ok_or(Refusal::RuntimeFault)?;
    match (left, right) {
        (crate::sem::Value::I64(a), crate::sem::Value::I64(b)) => Ok((a, b)),
        _ => Err(Refusal::RuntimeFault),
    }
}

fn insert(
    belt: &mut Vec<crate::sem::Value>,
    capacity: u8,
    value: crate::sem::Value,
) {
    if belt.len() == usize::from(capacity) {
        belt.pop();
    }
    belt.insert(0, value);
}
