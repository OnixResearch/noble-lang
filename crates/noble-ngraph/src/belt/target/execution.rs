use alloc::vec;
use alloc::vec::Vec;

use crate::ir::NodeId;
use crate::sem::Value;

use super::{CheckedSchedule, DecodedSchedule, Machine, Operation, Refusal};

#[derive(Clone, Copy)]
struct BeltValue {
    node: NodeId,
    value: Value,
}

#[derive(Clone, Copy)]
enum ResultValue {
    Insert(BeltValue),
    Save { slot: usize, value: BeltValue },
}

#[derive(Clone, Copy)]
struct Pending {
    completion_cycle: u32,
    issue_cycle: u32,
    unit: u8,
    value: ResultValue,
}

/// Check and execute a target schedule, publishing outputs only after successful admission.
pub fn execute(
    ids: &dyn crate::identity::IdentityScheme,
    checked: &crate::check::CheckedGraph,
    machine: Machine,
    bytes: &[u8],
    inputs: &[Value],
) -> Result<Vec<Value>, Refusal> {
    let accepted = super::check(ids, checked, machine, bytes)?;
    execute_checked(ids, checked, accepted, inputs)
}

/// Interpret a checker-issued schedule, rechecking its exact bound bytes first.
pub fn execute_checked(
    ids: &dyn crate::identity::IdentityScheme,
    checked: &crate::check::CheckedGraph,
    accepted: CheckedSchedule<'_>,
    inputs: &[Value],
) -> Result<Vec<Value>, Refusal> {
    let rechecked = super::check(ids, checked, accepted.machine, accepted.bytes)?;
    if rechecked.graph_id != accepted.graph_id
        || rechecked.descriptor_digest != accepted.descriptor_digest
        || rechecked.schedule_digest != accepted.schedule_digest
        || rechecked.artifact_binding != accepted.artifact_binding
        || rechecked.event_count != accepted.event_count
        || rechecked.output_count != accepted.output_count
    {
        return Err(Refusal::PostCheckMutation);
    }
    let mut schedule = super::decode(ids, accepted.bytes)?;
    let slot_values = crate::eval::preflight(checked, inputs, None).map_err(|_| Refusal::Inputs)?;
    interpret(checked, &mut schedule, &slot_values)
}

fn interpret(
    checked: &crate::check::CheckedGraph,
    schedule: &mut DecodedSchedule,
    slot_values: &[Value],
) -> Result<Vec<Value>, Refusal> {
    let graph = checked.graph();
    let descriptor = schedule.machine.descriptor();
    schedule.events.sort_unstable_by_key(|event| (event.cycle, event.lane, event.unit));
    let events = &schedule.events;
    let mut belt = Vec::<BeltValue>::with_capacity(usize::from(descriptor.belt_capacity));
    let mut saves = vec![None::<BeltValue>; usize::from(descriptor.save_slots)];
    let mut pending = Vec::<Pending>::with_capacity(events.len());
    let max_cycle = events.iter().map(|event| {
        event.cycle + u32::from(descriptor.latency(operation_class(event.operation))) - 1
    }).max().unwrap_or(0);
    let mut cursor = 0usize;
    for cycle in 0..=max_cycle {
        let start = cursor;
        while cursor < events.len() && events[cursor].cycle == cycle { cursor += 1; }
        for event in &events[start..cursor] {
            let result = match event.operation {
                Operation::Input { node, slot } => {
                    let value = *slot_values.get(slot as usize).ok_or(Refusal::RuntimeFault)?;
                    ResultValue::Insert(BeltValue { node: NodeId(node), value })
                }
                Operation::Constant { node, bits } => ResultValue::Insert(BeltValue {
                    node: NodeId(node),
                    value: Value::I64(i64::from_le_bytes(bits.to_le_bytes())),
                }),
                Operation::Add { node, left_age, right_age } => {
                    let (left, right) = operands(&belt, left_age, right_age)?;
                    let (Value::I64(left), Value::I64(right)) = (left.value, right.value) else { return Err(Refusal::RuntimeFault); };
                    ResultValue::Insert(BeltValue { node: NodeId(node), value: Value::I64(crate::sem::i64_add(left, right)) })
                }
                Operation::Sub { node, left_age, right_age } => {
                    let (left, right) = operands(&belt, left_age, right_age)?;
                    let (Value::I64(left), Value::I64(right)) = (left.value, right.value) else { return Err(Refusal::RuntimeFault); };
                    ResultValue::Insert(BeltValue { node: NodeId(node), value: Value::I64(crate::sem::i64_sub(left, right)) })
                }
                Operation::Multiply { node, left_age, right_age } => {
                    let (left, right) = operands(&belt, left_age, right_age)?;
                    let (Value::I64(left), Value::I64(right)) = (left.value, right.value) else { return Err(Refusal::RuntimeFault); };
                    ResultValue::Insert(BeltValue { node: NodeId(node), value: Value::I64(crate::sem::i64_mul(left, right)) })
                }
                Operation::Equal { node, left_age, right_age } => {
                    let (left, right) = operands(&belt, left_age, right_age)?;
                    let (Value::I64(left), Value::I64(right)) = (left.value, right.value) else { return Err(Refusal::RuntimeFault); };
                    ResultValue::Insert(BeltValue { node: NodeId(node), value: Value::Bool(left == right) })
                }
                Operation::Save { node, age, slot } => {
                    let value = *belt.get(usize::from(age)).ok_or(Refusal::RuntimeFault)?;
                    if value.node.0 != node { return Err(Refusal::RuntimeFault); }
                    ResultValue::Save { slot: usize::from(slot), value }
                }
                Operation::Load { node, slot } => {
                    let value = saves.get(usize::from(slot)).copied().flatten().ok_or(Refusal::RuntimeFault)?;
                    if value.node.0 != node { return Err(Refusal::RuntimeFault); }
                    ResultValue::Insert(value)
                }
            };
            let due = event.cycle + u32::from(descriptor.latency(operation_class(event.operation))) - 1;
            pending.push(Pending { completion_cycle: due, issue_cycle: event.cycle, unit: event.unit, value: result });
        }
        pending.sort_unstable_by_key(|event| (event.completion_cycle, event.issue_cycle, event.unit));
        let mut done = 0usize;
        while done < pending.len() && pending[done].completion_cycle == cycle {
            match pending[done].value {
                ResultValue::Insert(value) => insert(value, descriptor.belt_capacity, &mut belt)?,
                ResultValue::Save { slot, value } => {
                    let target = saves.get_mut(slot).ok_or(Refusal::RuntimeFault)?;
                    *target = Some(value);
                }
            }
            done += 1;
        }
        pending.drain(..done);
    }
    if cursor != events.len() || !pending.is_empty() { return Err(Refusal::RuntimeFault); }
    let mut outputs = Vec::with_capacity(schedule.outputs.len());
    for output in &schedule.outputs {
        let value = belt.iter().find(|value| value.node.0 == output.node).ok_or(Refusal::OutputMismatch)?;
        outputs.push(value.value);
    }
    if graph.outputs.len() != outputs.len() { return Err(Refusal::OutputMismatch); }
    Ok(outputs)
}

fn operands(belt: &[BeltValue], left_age: u8, right_age: u8) -> Result<(BeltValue, BeltValue), Refusal> {
    let left = *belt.get(usize::from(left_age)).ok_or(Refusal::RuntimeFault)?;
    let right = *belt.get(usize::from(right_age)).ok_or(Refusal::RuntimeFault)?;
    Ok((left, right))
}

fn insert(value: BeltValue, capacity: u8, belt: &mut Vec<BeltValue>) -> Result<(), Refusal> {
    if belt.len() == usize::from(capacity) { belt.pop().ok_or(Refusal::RuntimeFault)?; }
    belt.insert(0, value);
    Ok(())
}

fn operation_class(operation: Operation) -> super::UnitClass {
    match operation {
        Operation::Input { .. } | Operation::Constant { .. } | Operation::Save { .. } | Operation::Load { .. } => super::UnitClass::Move,
        Operation::Multiply { .. } => super::UnitClass::Multiply,
        Operation::Add { .. } | Operation::Sub { .. } | Operation::Equal { .. } => super::UnitClass::Alu,
    }
}
