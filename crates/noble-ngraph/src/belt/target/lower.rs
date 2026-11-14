use alloc::vec;
use alloc::vec::Vec;

use super::{Issue, Lowered, Machine, Operation, OutputRead, Refusal, UnitClass};
use crate::ir::{Literal, NodeId, Op};

/// Deterministic bounded list scheduler for the selected fixed target model.
///
/// It issues graph nodes in canonical order, waits for each latency, and inserts explicit
/// move-unit save/load operations whenever a live value would otherwise be evicted. It is an
/// untrusted schedule producer; [`super::check`] independently validates the result.
pub fn lower(
    ids: &dyn crate::identity::IdentityScheme,
    checked: &crate::check::CheckedGraph,
    machine: Machine,
) -> Result<Lowered, Refusal> {
    crate::belt::support::admit_graph(checked).map_err(|_| Refusal::UnsupportedGraph)?;
    let descriptor = machine.descriptor();
    if checked.graph().outputs.len() > usize::from(descriptor.belt_capacity) {
        return Err(Refusal::OutputMismatch);
    }
    let mut scheduler = Scheduler::new(checked, machine);
    scheduler.schedule_nodes()?;
    scheduler.prepare_outputs()?;
    let outputs = checked
        .graph()
        .outputs
        .iter()
        .map(|node| OutputRead { node: node.0 })
        .collect::<Vec<_>>();
    let bytes = super::codec::encode(ids, checked, machine, &scheduler.events, &outputs)?;
    let max_completion = scheduler.events.iter().map(|event| {
        let class = descriptor.unit_class(event.unit).unwrap_or(UnitClass::Move);
        event.cycle + u32::from(descriptor.latency(class)) - 1
    }).max().unwrap_or(0);
    Ok(Lowered {
        bytes,
        event_count: scheduler.events.len(),
        modeled_cycles: max_completion.saturating_add(1),
        preservation_saves: scheduler.saves_count,
        preservation_loads: scheduler.loads,
        scheduling_work: scheduler.work,
    })
}

struct Scheduler<'a> {
    checked: &'a crate::check::CheckedGraph,
    machine: Machine,
    cycle: u32,
    belt: Vec<NodeId>,
    saves: Vec<Option<NodeId>>,
    remaining: Vec<u32>,
    events: Vec<Issue>,
    saves_count: usize,
    loads: usize,
    work: usize,
}

impl<'a> Scheduler<'a> {
    fn new(checked: &'a crate::check::CheckedGraph, machine: Machine) -> Self {
        let descriptor = machine.descriptor();
        let graph = checked.graph();
        let mut remaining = vec![0u32; graph.nodes.len()];
        for graph_node in &graph.nodes {
            graph_node.op.visit_operands(|node| {
                if let Some(count) = remaining.get_mut(node.0 as usize) {
                    *count = count.saturating_add(1);
                }
            });
        }
        for node in &graph.outputs {
            if let Some(count) = remaining.get_mut(node.0 as usize) {
                *count = count.saturating_add(1);
            }
        }
        Self {
            checked,
            machine,
            cycle: 0,
            belt: Vec::with_capacity(usize::from(descriptor.belt_capacity)),
            saves: vec![None; usize::from(descriptor.save_slots)],
            remaining,
            events: Vec::new(),
            saves_count: 0,
            loads: 0,
            work: 0,
        }
    }

    fn schedule_nodes(&mut self) -> Result<(), Refusal> {
        for index in 0..self.checked.node_count() {
            self.work = self.work.checked_add(1).ok_or(Refusal::SchedulingExhausted)?;
            if self.work > super::MAX_CHECK_WORK {
                return Err(Refusal::SchedulingExhausted);
            }
            let node_id = NodeId(index as u32);
            let node_operation = node_operation(self.checked, index)?;
            let operation = match node_operation {
                NodeOperation::Input(slot) => Operation::Input { node: node_id.0, slot },
                NodeOperation::Constant(bits) => Operation::Constant { node: node_id.0, bits },
                NodeOperation::Add(left, right) => {
                    self.ensure_operand(left)?;
                    self.ensure_operand(right)?;
                    let left_age = self.age(left)?;
                    let right_age = self.age(right)?;
                    self.make_room()?;
                    Operation::Add { node: node_id.0, left_age, right_age }
                }
                NodeOperation::Subtract(left, right) => {
                    self.ensure_operand(left)?;
                    self.ensure_operand(right)?;
                    let left_age = self.age(left)?;
                    let right_age = self.age(right)?;
                    self.make_room()?;
                    Operation::Sub { node: node_id.0, left_age, right_age }
                }
                NodeOperation::Multiply(left, right) => {
                    self.ensure_operand(left)?;
                    self.ensure_operand(right)?;
                    let left_age = self.age(left)?;
                    let right_age = self.age(right)?;
                    self.make_room()?;
                    Operation::Multiply { node: node_id.0, left_age, right_age }
                }
                NodeOperation::Equal(left, right) => {
                    self.ensure_operand(left)?;
                    self.ensure_operand(right)?;
                    let left_age = self.age(left)?;
                    let right_age = self.age(right)?;
                    self.make_room()?;
                    Operation::Equal { node: node_id.0, left_age, right_age }
                }
            };
            if matches!(operation, Operation::Input { .. } | Operation::Constant { .. }) {
                self.make_room()?;
            }
            self.issue(operation)?;
            match node_operation {
                NodeOperation::Add(left, right)
                | NodeOperation::Subtract(left, right)
                | NodeOperation::Multiply(left, right)
                | NodeOperation::Equal(left, right) => self.consume(left, right),
                NodeOperation::Input(_) | NodeOperation::Constant(_) => {}
            }
            let class = self.class(operation);
            self.cycle = self.cycle
                .checked_add(u32::from(self.machine.descriptor().latency(class)))
                .ok_or(Refusal::HorizonBound)?;
            self.insert(node_id)?;
        }
        Ok(())
    }

    fn prepare_outputs(&mut self) -> Result<(), Refusal> {
        for index in 0..self.checked.graph().outputs.len() {
            let node = self.checked.graph().outputs[index];
            if self.age(node).is_err() {
                self.ensure_operand(node)?;
                if self.age(node).is_err() {
                    return Err(Refusal::OutputMismatch);
                }
            }
        }
        Ok(())
    }

    fn ensure_operand(&mut self, node: NodeId) -> Result<(), Refusal> {
        if self.age(node).is_ok() {
            return Ok(());
        }
        let slot = self.saves.iter().position(|saved| *saved == Some(node))
            .ok_or(Refusal::DependencyNotReady)?;
        self.make_room()?;
        let move_unit = self.unit(UnitClass::Move, 0)?;
        self.push_event(move_unit, Operation::Load { node: node.0, slot: slot as u8 })?;
        self.loads += 1;
        self.cycle = self.cycle.checked_add(u32::from(self.machine.descriptor().move_latency))
            .ok_or(Refusal::HorizonBound)?;
        self.insert(node)
    }

    fn make_room(&mut self) -> Result<(), Refusal> {
        let descriptor = self.machine.descriptor();
        if self.belt.len() < usize::from(descriptor.belt_capacity) {
            return Ok(());
        }
        let victim = *self.belt.last().ok_or(Refusal::RuntimeFault)?;
        if self.live(victim) && !self.saves.contains(&Some(victim)) {
            let slot = self.saves.iter().position(|saved| match saved {
                None => true,
                Some(old) => !self.live(*old),
            }).ok_or(Refusal::SchedulingExhausted)?;
            let age = self.age(victim)?;
            let unit = self.unit(UnitClass::Move, 0)?;
            self.push_event(unit, Operation::Save { node: victim.0, age, slot: slot as u8 })?;
            self.saves[slot] = Some(victim);
            self.saves_count += 1;
            self.cycle = self.cycle.checked_add(u32::from(descriptor.move_latency))
                .ok_or(Refusal::HorizonBound)?;
        }
        Ok(())
    }

    fn issue(&mut self, operation: Operation) -> Result<(), Refusal> {
        let class = self.class(operation);
        let unit = self.unit(class, 0)?;
        self.push_event(unit, operation)
    }

    fn push_event(&mut self, unit: u8, operation: Operation) -> Result<(), Refusal> {
        if self.events.len() >= super::MAX_EVENTS || self.cycle >= super::MAX_HORIZON {
            return Err(Refusal::SchedulingExhausted);
        }
        self.events.push(Issue { cycle: self.cycle, lane: 0, unit, operation });
        Ok(())
    }

    fn insert(&mut self, node: NodeId) -> Result<(), Refusal> {
        if self.belt.len() == usize::from(self.machine.descriptor().belt_capacity) {
            let removed = self.belt.pop().ok_or(Refusal::RuntimeFault)?;
            if self.live(removed) && !self.saves.contains(&Some(removed)) {
                return Err(Refusal::SchedulingExhausted);
            }
        }
        self.belt.insert(0, node);
        Ok(())
    }

    fn age(&self, node: NodeId) -> Result<u8, Refusal> {
        self.belt.iter().position(|value| *value == node)
            .and_then(|age| u8::try_from(age).ok())
            .ok_or(Refusal::DependencyNotReady)
    }

    fn live(&self, node: NodeId) -> bool {
        self.remaining.get(node.0 as usize).is_some_and(|remaining| *remaining != 0)
    }

    fn consume(&mut self, left: NodeId, right: NodeId) {
        for node in [left, right] {
            if let Some(remaining) = self.remaining.get_mut(node.0 as usize) {
                *remaining = remaining.saturating_sub(1);
            }
        }
    }

    fn class(&self, operation: Operation) -> UnitClass {
        match operation {
            Operation::Input { .. } | Operation::Constant { .. } | Operation::Save { .. } | Operation::Load { .. } => UnitClass::Move,
            Operation::Multiply { .. } => UnitClass::Multiply,
            Operation::Add { .. } | Operation::Sub { .. } | Operation::Equal { .. } => UnitClass::Alu,
        }
    }

    fn unit(&self, class: UnitClass, index: u8) -> Result<u8, Refusal> {
        let descriptor = self.machine.descriptor();
        let id = match class {
            UnitClass::Alu if index < descriptor.alu_units => index,
            UnitClass::Multiply if index < descriptor.multiply_units => descriptor.alu_units + index,
            UnitClass::Move if index < descriptor.move_units => descriptor.alu_units + descriptor.multiply_units + index,
            _ => return Err(Refusal::UnitBound),
        };
        Ok(id)
    }
}

#[derive(Clone, Copy)]
enum NodeOperation {
    Input(u32),
    Constant(u64),
    Add(NodeId, NodeId),
    Subtract(NodeId, NodeId),
    Multiply(NodeId, NodeId),
    Equal(NodeId, NodeId),
}

fn node_operation(checked: &crate::check::CheckedGraph, index: usize) -> Result<NodeOperation, Refusal> {
    let graph_node = checked.graph().nodes.get(index).ok_or(Refusal::NodeMismatch)?;
    match &graph_node.op {
        Op::Input(slot) => Ok(NodeOperation::Input(*slot)),
        Op::Const(Literal::I64(value)) => Ok(NodeOperation::Constant(*value as u64)),
        Op::I64Add(left, right) => Ok(NodeOperation::Add(*left, *right)),
        Op::I64Sub(left, right) => Ok(NodeOperation::Subtract(*left, *right)),
        Op::I64Mul(left, right) => Ok(NodeOperation::Multiply(*left, *right)),
        Op::I64Eq(left, right) => Ok(NodeOperation::Equal(*left, *right)),
        Op::Const(_) | Op::Param(_) | Op::F32Add(..) | Op::F32Sub(..) | Op::F32Mul(..)
        | Op::F32Affine { .. } | Op::F32Relu(_) | Op::F32Reglu { .. } | Op::F32Step(_)
        | Op::F32Select { .. } | Op::F32Index { .. } | Op::Guard { .. } => Err(Refusal::UnsupportedGraph),
    }
}
