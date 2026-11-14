use alloc::vec;
use alloc::vec::Vec;

use crate::ir::{Literal, NodeId, Op};

use super::{DecodedSchedule, Issue, Operation, Refusal, UnitClass};

#[derive(Clone, Copy)]
enum Completion {
    Node(NodeId),
    Save { slot: usize, node: NodeId },
}

#[derive(Clone, Copy)]
struct Pending {
    completion_cycle: u32,
    issue_cycle: u32,
    unit: u8,
    completion: Completion,
}

pub(super) fn check_stream(
    checked: &crate::check::CheckedGraph,
    schedule: &mut DecodedSchedule,
) -> Result<(), Refusal> {
    let graph = checked.graph();
    let descriptor = schedule.machine.descriptor();
    if schedule.events.len() > super::MAX_EVENTS {
        return Err(Refusal::EventBound);
    }
    if schedule.outputs.len() != graph.outputs.len()
        || schedule
            .outputs
            .iter()
            .zip(&graph.outputs)
            .any(|(actual, expected)| actual.node != expected.0)
    {
        return Err(Refusal::OutputMismatch);
    }
    if schedule.outputs.len() > usize::from(descriptor.belt_capacity) {
        return Err(Refusal::OutputMismatch);
    }
    schedule
        .events
        .sort_unstable_by_key(|event| (event.cycle, event.lane, event.unit));
    let ordered = &schedule.events;
    if ordered
        .iter()
        .any(|event| event.cycle >= super::MAX_HORIZON)
    {
        return Err(Refusal::HorizonBound);
    }
    let units =
        usize::from(descriptor.alu_units + descriptor.multiply_units + descriptor.move_units);
    let mut lane_used = vec![false; usize::from(descriptor.issue_width)];
    let mut unit_used = vec![false; units];
    let mut last_issue = vec![None::<u32>; units];
    let mut issued = vec![false; graph.nodes.len()];
    let mut completed = vec![false; graph.nodes.len()];
    let mut remaining = use_counts(checked);
    let output_live = output_counts(checked);
    let mut belt: Vec<NodeId> = Vec::with_capacity(usize::from(descriptor.belt_capacity));
    let mut saves = vec![None::<NodeId>; usize::from(descriptor.save_slots)];
    let mut reserved_slots = vec![false; usize::from(descriptor.save_slots)];
    let mut pending = Vec::with_capacity(ordered.len());
    let mut cursor = 0usize;
    let mut consumed_this_cycle = Vec::with_capacity(usize::from(descriptor.issue_width) * 2);
    let mut max_cycle = ordered.last().map_or(0, |event| event.cycle);
    for event in ordered {
        let class = operation_class(event.operation);
        let latency = u32::from(descriptor.latency(class));
        max_cycle = max_cycle.max(
            event
                .cycle
                .checked_add(latency - 1)
                .ok_or(Refusal::CompletionHorizon)?,
        );
    }
    if max_cycle >= super::MAX_HORIZON {
        return Err(Refusal::CompletionHorizon);
    }
    let work_bound = ordered
        .len()
        .checked_add(max_cycle as usize + 1)
        .ok_or(Refusal::CheckingExhausted)?;
    if work_bound > super::MAX_CHECK_WORK {
        return Err(Refusal::CheckingExhausted);
    }
    for cycle in 0..=max_cycle {
        for used in &mut lane_used {
            *used = false;
        }
        for used in &mut unit_used {
            *used = false;
        }
        let start = cursor;
        while cursor < ordered.len() && ordered[cursor].cycle == cycle {
            cursor += 1;
        }
        if cursor.saturating_sub(start) > usize::from(descriptor.issue_width) {
            return Err(Refusal::IssueConflict);
        }
        consumed_this_cycle.clear();
        for event in &ordered[start..cursor] {
            check_issue_resources(
                *event,
                descriptor,
                &mut lane_used,
                &mut unit_used,
                &mut last_issue,
            )?;
            let node = event_node(event.operation);
            let completion = match event.operation {
                Operation::Input { slot, .. } => {
                    expect_input(graph, node, slot)?;
                    issue_graph_node(&mut issued, node)?;
                    Completion::Node(node)
                }
                Operation::Constant { bits, .. } => {
                    expect_constant(graph, node, bits)?;
                    issue_graph_node(&mut issued, node)?;
                    Completion::Node(node)
                }
                Operation::Add {
                    left_age,
                    right_age,
                    ..
                }
                | Operation::Sub {
                    left_age,
                    right_age,
                    ..
                }
                | Operation::Multiply {
                    left_age,
                    right_age,
                    ..
                }
                | Operation::Equal {
                    left_age,
                    right_age,
                    ..
                } => {
                    let (left, right) = graph_operands(graph, node, event.operation)?;
                    require_age(&belt, left_age, left)?;
                    require_age(&belt, right_age, right)?;
                    if !completed.get(left.0 as usize).copied().unwrap_or(false)
                        || !completed.get(right.0 as usize).copied().unwrap_or(false)
                    {
                        return Err(Refusal::DependencyNotReady);
                    }
                    issue_graph_node(&mut issued, node)?;
                    consumed_this_cycle.push(left);
                    consumed_this_cycle.push(right);
                    Completion::Node(node)
                }
                Operation::Save { age, slot, node } => {
                    let slot = usize::from(slot);
                    if slot >= saves.len() {
                        return Err(Refusal::SaveSlotBound);
                    }
                    require_age(&belt, age, NodeId(node))?;
                    if let Some(previous) = saves[slot] {
                        if is_live(previous, &remaining, &output_live)
                            && !belt.contains(&previous)
                            && !saves
                                .iter()
                                .enumerate()
                                .any(|(other, saved)| other != slot && *saved == Some(previous))
                        {
                            return Err(Refusal::LiveSaveOverwrite);
                        }
                        if is_live(previous, &remaining, &output_live) {
                            return Err(Refusal::LiveSaveOverwrite);
                        }
                    }
                    if reserved_slots[slot] {
                        return Err(Refusal::LiveSaveOverwrite);
                    }
                    reserved_slots[slot] = true;
                    Completion::Save {
                        slot,
                        node: NodeId(node),
                    }
                }
                Operation::Load { node, slot } => {
                    let slot = usize::from(slot);
                    let saved = saves
                        .get(slot)
                        .copied()
                        .flatten()
                        .ok_or(Refusal::UninitializedSave)?;
                    if saved.0 != node {
                        return Err(Refusal::UninitializedSave);
                    }
                    if !completed.get(node as usize).copied().unwrap_or(false) {
                        return Err(Refusal::DependencyNotReady);
                    }
                    Completion::Node(saved)
                }
            };
            let class = operation_class(event.operation);
            let completion_cycle = event
                .cycle
                .checked_add(u32::from(descriptor.latency(class)) - 1)
                .ok_or(Refusal::CompletionHorizon)?;
            pending.push(Pending {
                completion_cycle,
                issue_cycle: event.cycle,
                unit: event.unit,
                completion,
            });
        }
        for node in consumed_this_cycle.iter().copied() {
            if let Some(count) = remaining.get_mut(node.0 as usize) {
                *count = count.saturating_sub(1);
            }
        }
        pending.sort_unstable_by_key(|item| (item.completion_cycle, item.issue_cycle, item.unit));
        let mut completed_at_cycle = 0usize;
        while completed_at_cycle < pending.len()
            && pending[completed_at_cycle].completion_cycle == cycle
        {
            match pending[completed_at_cycle].completion {
                Completion::Node(node) => {
                    if !completed[node.0 as usize] {
                        completed[node.0 as usize] = true;
                    }
                    insert_value(
                        node,
                        descriptor.belt_capacity,
                        &mut belt,
                        &saves,
                        &remaining,
                        &output_live,
                    )?;
                }
                Completion::Save { slot, node } => {
                    saves[slot] = Some(node);
                    reserved_slots[slot] = false;
                }
            }
            completed_at_cycle += 1;
        }
        pending.drain(..completed_at_cycle);
    }
    if cursor != ordered.len() {
        return Err(Refusal::CheckingExhausted);
    }
    if !pending.is_empty() {
        return Err(Refusal::CompletionHorizon);
    }
    if issued.iter().any(|value| !value) || completed.iter().any(|value| !value) {
        return Err(Refusal::MissingNode);
    }
    for output in &schedule.outputs {
        if !belt.iter().any(|node| node.0 == output.node) {
            return Err(Refusal::OutputMismatch);
        }
    }
    Ok(())
}

fn check_issue_resources(
    event: Issue,
    descriptor: super::MachineDescriptor,
    lane_used: &mut [bool],
    unit_used: &mut [bool],
    last_issue: &mut [Option<u32>],
) -> Result<(), Refusal> {
    let lane = usize::from(event.lane);
    if lane >= lane_used.len() {
        return Err(Refusal::IssueLane);
    }
    let unit = usize::from(event.unit);
    let class = descriptor
        .unit_class(event.unit)
        .ok_or(Refusal::UnitBound)?;
    if class != operation_class(event.operation) {
        return Err(Refusal::UnitClass);
    }
    if lane_used[lane] {
        return Err(Refusal::IssueConflict);
    }
    if unit_used[unit] {
        return Err(Refusal::UnitConflict);
    }
    if last_issue[unit].is_some_and(|last| {
        event.cycle.saturating_sub(last) < u32::from(descriptor.initiation_interval)
    }) {
        return Err(Refusal::InitiationInterval);
    }
    lane_used[lane] = true;
    unit_used[unit] = true;
    last_issue[unit] = Some(event.cycle);
    Ok(())
}

fn issue_graph_node(issued: &mut [bool], node: NodeId) -> Result<(), Refusal> {
    let present = issued
        .get_mut(node.0 as usize)
        .ok_or(Refusal::NodeMismatch)?;
    if *present {
        return Err(Refusal::DuplicateNode);
    }
    *present = true;
    Ok(())
}

fn expect_input(graph: &crate::ir::Graph, node: NodeId, slot: u32) -> Result<(), Refusal> {
    if graph
        .nodes
        .get(node.0 as usize)
        .is_some_and(|entry| entry.op == Op::Input(slot))
    {
        Ok(())
    } else {
        Err(Refusal::NodeMismatch)
    }
}

fn expect_constant(graph: &crate::ir::Graph, node: NodeId, bits: u64) -> Result<(), Refusal> {
    let expected = graph
        .nodes
        .get(node.0 as usize)
        .ok_or(Refusal::NodeMismatch)?;
    match expected.op {
        Op::Const(Literal::I64(value)) if value as u64 == bits => Ok(()),
        _ => Err(Refusal::NodeMismatch),
    }
}

fn graph_operands(
    graph: &crate::ir::Graph,
    node: NodeId,
    operation: Operation,
) -> Result<(NodeId, NodeId), Refusal> {
    let graph_node = graph
        .nodes
        .get(node.0 as usize)
        .ok_or(Refusal::NodeMismatch)?;
    let (left, right) = match (&graph_node.op, operation) {
        (Op::I64Add(left, right), Operation::Add { .. })
        | (Op::I64Sub(left, right), Operation::Sub { .. })
        | (Op::I64Mul(left, right), Operation::Multiply { .. })
        | (Op::I64Eq(left, right), Operation::Equal { .. }) => (*left, *right),
        _ => return Err(Refusal::NodeMismatch),
    };
    Ok((left, right))
}

fn require_age(belt: &[NodeId], age: u8, expected: NodeId) -> Result<(), Refusal> {
    if belt.get(usize::from(age)) == Some(&expected) {
        Ok(())
    } else {
        Err(Refusal::OperandMismatch)
    }
}

fn insert_value(
    node: NodeId,
    capacity: u8,
    belt: &mut Vec<NodeId>,
    saves: &[Option<NodeId>],
    remaining: &[u32],
    outputs: &[u32],
) -> Result<(), Refusal> {
    if belt.len() == usize::from(capacity) {
        let evicted = belt.pop().ok_or(Refusal::RuntimeFault)?;
        // Liveness is checked after all issues in this cycle; remaining uses encode every
        // not-yet-issued graph dependency and every declared output remains live to final read.
        if is_live(evicted, remaining, outputs) && !saves.contains(&Some(evicted)) {
            return Err(Refusal::LiveValueLost);
        }
    }
    belt.insert(0, node);
    Ok(())
}

fn use_counts(checked: &crate::check::CheckedGraph) -> Vec<u32> {
    let graph = checked.graph();
    let mut counts = vec![0u32; graph.nodes.len()];
    for node in &graph.nodes {
        node.op.visit_operands(|operand| {
            if let Some(count) = counts.get_mut(operand.0 as usize) {
                *count = count.saturating_add(1);
            }
        });
    }
    counts
}

fn output_counts(checked: &crate::check::CheckedGraph) -> Vec<u32> {
    let graph = checked.graph();
    let mut counts = vec![0u32; graph.nodes.len()];
    for output in &graph.outputs {
        if let Some(count) = counts.get_mut(output.0 as usize) {
            *count = count.saturating_add(1);
        }
    }
    counts
}

fn is_live(node: NodeId, remaining: &[u32], outputs: &[u32]) -> bool {
    remaining.get(node.0 as usize).copied().unwrap_or(0) != 0
        || outputs.get(node.0 as usize).copied().unwrap_or(0) != 0
}

fn operation_class(operation: Operation) -> UnitClass {
    match operation {
        Operation::Input { .. }
        | Operation::Constant { .. }
        | Operation::Save { .. }
        | Operation::Load { .. } => UnitClass::Move,
        Operation::Multiply { .. } => UnitClass::Multiply,
        Operation::Add { .. } | Operation::Sub { .. } | Operation::Equal { .. } => UnitClass::Alu,
    }
}

fn event_node(operation: Operation) -> NodeId {
    NodeId(match operation {
        Operation::Input { node, .. }
        | Operation::Constant { node, .. }
        | Operation::Add { node, .. }
        | Operation::Sub { node, .. }
        | Operation::Multiply { node, .. }
        | Operation::Equal { node, .. }
        | Operation::Save { node, .. }
        | Operation::Load { node, .. } => node,
    })
}
