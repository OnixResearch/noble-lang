use alloc::vec;
use alloc::vec::Vec;

use super::super::types::{ArtifactView, Instruction, Refusal};

pub(super) fn validate_stream(
    checked: &crate::check::CheckedGraph,
    artifact: &ArtifactView<'_>,
) -> Result<(), Refusal> {
    let mut machine = Symbolic::new(checked, artifact)?;
    let mut cursor = 0usize;
    while cursor < artifact.instruction_bytes.len() {
        let instruction = super::super::codec::next_instruction(
            artifact.instruction_bytes,
            &mut cursor,
        )?;
        machine.step(instruction)?;
    }
    machine.finish()
}

struct Symbolic<'a> {
    checked: &'a crate::check::CheckedGraph,
    capacity: usize,
    belt: Vec<crate::ir::NodeId>,
    slots: Vec<Option<crate::ir::NodeId>>,
    remaining: Vec<u32>,
    next_node: usize,
    next_output: usize,
    output_phase: bool,
}

impl<'a> Symbolic<'a> {
    fn new(
        checked: &'a crate::check::CheckedGraph,
        artifact: &ArtifactView<'_>,
    ) -> Result<Self, Refusal> {
        Ok(Self {
            checked,
            capacity: usize::from(artifact.capacity),
            belt: Vec::with_capacity(usize::from(artifact.capacity)),
            slots: vec![None; usize::from(artifact.save_slots)],
            remaining: super::super::support::remaining_uses(checked)?,
            next_node: 0,
            next_output: 0,
            output_phase: false,
        })
    }

    fn step(&mut self, instruction: Instruction) -> Result<(), Refusal> {
        if self.complete() {
            return Err(Refusal::OutputMismatch);
        }
        match instruction {
            Instruction::Save(age, slot) => self.save(age, slot),
            Instruction::Load(slot) => self.load(slot),
            Instruction::Output(age) => self.output(age),
            other => self.define(other),
        }
    }

    fn define(&mut self, instruction: Instruction) -> Result<(), Refusal> {
        if self.output_phase {
            return Err(Refusal::BodyMismatch);
        }
        let node = self
            .checked
            .graph()
            .nodes
            .get(self.next_node)
            .ok_or(Refusal::IncompleteProgram)?;
        let node_id = crate::ir::NodeId(
            u32::try_from(self.next_node).map_err(|_| Refusal::GraphBound)?,
        );
        let operation = match (&node.op, instruction) {
            (crate::ir::Op::Input(expected), Instruction::Input(actual))
                if *expected == actual => None,
            (crate::ir::Op::Const(crate::ir::Literal::I64(expected)), Instruction::Constant(actual))
                if expected.cast_unsigned() == actual => None,
            (crate::ir::Op::I64Add(left, right), Instruction::Add(a, b)) => {
                Some((*left, *right, a, b))
            }
            (crate::ir::Op::I64Sub(left, right), Instruction::Sub(a, b)) => {
                Some((*left, *right, a, b))
            }
            (crate::ir::Op::I64Mul(left, right), Instruction::Mul(a, b)) => {
                Some((*left, *right, a, b))
            }
            (crate::ir::Op::I64Eq(left, right), Instruction::Eq(a, b)) => {
                Some((*left, *right, a, b))
            }
            _ => return Err(Refusal::BodyMismatch),
        };
        if let Some((left, right, left_age, right_age)) = operation {
            if self.at(left_age)? != left || self.at(right_age)? != right {
                return Err(Refusal::OperandMismatch);
            }
            super::super::support::consume(&mut self.remaining, left)?;
            super::super::support::consume(&mut self.remaining, right)?;
        }
        self.insert(node_id)?;
        self.next_node += 1;
        Ok(())
    }

    fn save(&mut self, age: u8, slot: u8) -> Result<(), Refusal> {
        if self.output_phase && self.next_output >= self.checked.graph().outputs.len() {
            return Err(Refusal::OutputMismatch);
        }
        let source = self.at(age)?;
        let slot_index = usize::from(slot);
        let old = *self.slots.get(slot_index).ok_or(Refusal::SlotBound)?;
        if let Some(old_node) = old {
            let duplicated = self.belt.contains(&old_node)
                || self
                    .slots
                    .iter()
                    .enumerate()
                    .any(|(other, value)| other != slot_index && *value == Some(old_node));
            if self.remaining[old_node.index()] != 0 && !duplicated {
                return Err(Refusal::LiveSlotOverwrite);
            }
        }
        self.slots[slot_index] = Some(source);
        Ok(())
    }

    fn load(&mut self, slot: u8) -> Result<(), Refusal> {
        if self.output_phase && self.next_output >= self.checked.graph().outputs.len() {
            return Err(Refusal::OutputMismatch);
        }
        let node = self
            .slots
            .get(usize::from(slot))
            .copied()
            .ok_or(Refusal::SlotBound)?
            .ok_or(Refusal::UninitializedSlot)?;
        if self.remaining[node.index()] == 0 {
            return Err(Refusal::LiveValueLost);
        }
        self.insert(node)
    }

    fn output(&mut self, age: u8) -> Result<(), Refusal> {
        if self.next_node != self.checked.graph().nodes.len() {
            return Err(Refusal::OutputMismatch);
        }
        self.output_phase = true;
        let expected = *self
            .checked
            .graph()
            .outputs
            .get(self.next_output)
            .ok_or(Refusal::OutputMismatch)?;
        if self.at(age)? != expected {
            return Err(Refusal::OutputMismatch);
        }
        super::super::support::consume(&mut self.remaining, expected)?;
        self.next_output += 1;
        Ok(())
    }

    fn insert(&mut self, node: crate::ir::NodeId) -> Result<(), Refusal> {
        if self.belt.len() == self.capacity {
            let evicted = *self.belt.last().ok_or(Refusal::StaleAge)?;
            if self.remaining[evicted.index()] != 0 && !self.slots.contains(&Some(evicted)) {
                return Err(Refusal::LiveValueLost);
            }
            self.belt.pop();
        }
        self.belt.insert(0, node);
        Ok(())
    }

    fn at(&self, age: u8) -> Result<crate::ir::NodeId, Refusal> {
        self.belt
            .get(usize::from(age))
            .copied()
            .ok_or(Refusal::StaleAge)
    }

    fn complete(&self) -> bool {
        self.next_node == self.checked.graph().nodes.len()
            && self.next_output == self.checked.graph().outputs.len()
    }

    fn finish(&self) -> Result<(), Refusal> {
        if self.next_node != self.checked.graph().nodes.len() {
            return Err(Refusal::IncompleteProgram);
        }
        if self.next_output != self.checked.graph().outputs.len() {
            return Err(Refusal::OutputMismatch);
        }
        Ok(())
    }
}
