use alloc::vec;
use alloc::vec::Vec;

use super::types::{Instruction, Lowered, Refusal};
use super::{codec, support};

/// Lower a checked total-I64 graph to the bounded canonical belt program.
///
/// The output is untrusted until [`super::check`] accepts its bytes. A refusal
/// returns no artifact, even when the internal lowerer already emitted a prefix.
pub fn lower(
    ids: &dyn crate::identity::IdentityScheme,
    checked: &crate::check::CheckedGraph,
    capacity: u8,
    save_slots: u8,
) -> Result<Lowered, Refusal> {
    support::admit_graph(checked)?;
    validate_limits(capacity, save_slots)?;
    let mut lowerer = Lowerer::new(checked, capacity, save_slots)?;
    lowerer.definitions()?;
    lowerer.outputs()?;
    let instruction_count = lowerer.instruction_count;
    let bytes = codec::encode(
        ids,
        checked,
        capacity,
        save_slots,
        &lowerer.instruction_bytes,
    )?;
    Ok(Lowered {
        bytes,
        instruction_count,
        preservation_saves: lowerer.saves,
        preservation_loads: lowerer.loads,
    })
}

fn validate_limits(capacity: u8, save_slots: u8) -> Result<(), Refusal> {
    if !(super::MIN_CAPACITY..=super::MAX_CAPACITY).contains(&capacity) {
        return Err(Refusal::Capacity);
    }
    if save_slots > super::MAX_SAVE_SLOTS {
        return Err(Refusal::SaveSlots);
    }
    Ok(())
}

struct Lowerer<'a> {
    checked: &'a crate::check::CheckedGraph,
    capacity: usize,
    belt: Vec<crate::ir::NodeId>,
    slots: Vec<Option<crate::ir::NodeId>>,
    remaining: Vec<u32>,
    instruction_bytes: Vec<u8>,
    instruction_count: usize,
    saves: usize,
    loads: usize,
}

impl<'a> Lowerer<'a> {
    fn new(
        checked: &'a crate::check::CheckedGraph,
        capacity: u8,
        save_slots: u8,
    ) -> Result<Self, Refusal> {
        Ok(Self {
            checked,
            capacity: usize::from(capacity),
            belt: Vec::with_capacity(usize::from(capacity)),
            slots: vec![None; usize::from(save_slots)],
            remaining: support::remaining_uses(checked)?,
            instruction_bytes: Vec::new(),
            instruction_count: 0,
            saves: 0,
            loads: 0,
        })
    }

    fn definitions(&mut self) -> Result<(), Refusal> {
        let node_count = self.checked.graph().nodes.len();
        for index in 0..node_count {
            let id = crate::ir::NodeId(u32::try_from(index).map_err(|_| Refusal::GraphBound)?);
            let definition = match &self.checked.graph().nodes[index].op {
                crate::ir::Op::Input(slot) => Definition::Input(*slot),
                crate::ir::Op::Const(crate::ir::Literal::I64(value)) => {
                    Definition::Constant(value.cast_unsigned())
                }
                crate::ir::Op::I64Add(left, right) => Definition::Operation(
                    *left,
                    *right,
                    Operation::Add,
                ),
                crate::ir::Op::I64Sub(left, right) => Definition::Operation(
                    *left,
                    *right,
                    Operation::Sub,
                ),
                crate::ir::Op::I64Mul(left, right) => Definition::Operation(
                    *left,
                    *right,
                    Operation::Mul,
                ),
                crate::ir::Op::I64Eq(left, right) => Definition::Operation(
                    *left,
                    *right,
                    Operation::Eq,
                ),
                _ => return Err(Refusal::UnsupportedGraph),
            };
            match definition {
                Definition::Input(slot) => self.define(id, Instruction::Input(slot))?,
                Definition::Constant(bits) => {
                    self.define(id, Instruction::Constant(bits))?;
                }
                Definition::Operation(left, right, operation) => {
                    self.operation(id, left, right, operation)?;
                }
            }
        }
        Ok(())
    }

    fn define(
        &mut self,
        id: crate::ir::NodeId,
        instruction: Instruction,
    ) -> Result<(), Refusal> {
        self.ensure_room()?;
        self.emit(instruction)?;
        self.insert(id);
        Ok(())
    }

    fn operation(
        &mut self,
        id: crate::ir::NodeId,
        left: crate::ir::NodeId,
        right: crate::ir::NodeId,
        operation: Operation,
    ) -> Result<(), Refusal> {
        self.ensure_available(left)?;
        self.ensure_available(right)?;
        if !self.belt.contains(&left) {
            self.ensure_available(left)?;
        }
        if !self.belt.contains(&right) {
            self.ensure_available(right)?;
        }
        let left_age = self.age(left)?;
        let right_age = self.age(right)?;
        self.consume(left)?;
        self.consume(right)?;
        self.ensure_room()?;
        self.emit(operation.instruction(left_age, right_age))?;
        self.insert(id);
        Ok(())
    }

    fn outputs(&mut self) -> Result<(), Refusal> {
        let output_count = self.checked.graph().outputs.len();
        for index in 0..output_count {
            let output = self.checked.graph().outputs[index];
            self.ensure_available(output)?;
            let age = self.age(output)?;
            self.emit(Instruction::Output(age))?;
            self.consume(output)?;
        }
        Ok(())
    }

    fn ensure_available(&mut self, node: crate::ir::NodeId) -> Result<(), Refusal> {
        if self.belt.contains(&node) {
            return Ok(());
        }
        let slot = self
            .slots
            .iter()
            .position(|value| *value == Some(node))
            .ok_or(Refusal::LiveValueLost)?;
        let slot = u8::try_from(slot).map_err(|_| Refusal::SlotBound)?;
        self.ensure_room()?;
        self.emit(Instruction::Load(slot))?;
        self.loads += 1;
        self.insert(node);
        Ok(())
    }

    fn ensure_room(&mut self) -> Result<(), Refusal> {
        if self.belt.len() < self.capacity {
            return Ok(());
        }
        let age = self.capacity - 1;
        let evicted = *self.belt.get(age).ok_or(Refusal::StaleAge)?;
        if self.remaining[evicted.index()] == 0 || self.slots.contains(&Some(evicted)) {
            return Ok(());
        }
        let slot = self.reclaimable_slot().ok_or(Refusal::LiveValueLost)?;
        let age = u8::try_from(age).map_err(|_| Refusal::Capacity)?;
        let slot_byte = u8::try_from(slot).map_err(|_| Refusal::SlotBound)?;
        self.emit(Instruction::Save(age, slot_byte))?;
        self.slots[slot] = Some(evicted);
        self.saves += 1;
        Ok(())
    }

    fn reclaimable_slot(&self) -> Option<usize> {
        self.slots.iter().enumerate().find_map(|(slot, value)| {
            let Some(old) = value else {
                return Some(slot);
            };
            if self.remaining[old.index()] == 0
                || self.belt.contains(old)
                || self.slots.iter().enumerate().any(|(other, entry)| {
                    other != slot && entry == &Some(*old)
                })
            {
                Some(slot)
            } else {
                None
            }
        })
    }

    fn age(&self, node: crate::ir::NodeId) -> Result<u8, Refusal> {
        let age = self
            .belt
            .iter()
            .position(|value| *value == node)
            .ok_or(Refusal::LiveValueLost)?;
        u8::try_from(age).map_err(|_| Refusal::StaleAge)
    }

    fn consume(&mut self, node: crate::ir::NodeId) -> Result<(), Refusal> {
        support::consume(&mut self.remaining, node)
    }

    fn insert(&mut self, node: crate::ir::NodeId) {
        if self.belt.len() == self.capacity {
            self.belt.pop();
        }
        self.belt.insert(0, node);
    }

    fn emit(&mut self, instruction: Instruction) -> Result<(), Refusal> {
        self.instruction_count = self
            .instruction_count
            .checked_add(1)
            .ok_or(Refusal::InstructionBound)?;
        if self.instruction_count > super::MAX_INSTRUCTIONS {
            return Err(Refusal::InstructionBound);
        }
        codec::push_instruction(&mut self.instruction_bytes, instruction);
        Ok(())
    }
}

#[derive(Clone, Copy)]
enum Definition {
    Input(u32),
    Constant(u64),
    Operation(crate::ir::NodeId, crate::ir::NodeId, Operation),
}

#[derive(Clone, Copy)]
enum Operation {
    Add,
    Sub,
    Mul,
    Eq,
}

impl Operation {
    fn instruction(self, left: u8, right: u8) -> Instruction {
        match self {
            Self::Add => Instruction::Add(left, right),
            Self::Sub => Instruction::Sub(left, right),
            Self::Mul => Instruction::Mul(left, right),
            Self::Eq => Instruction::Eq(left, right),
        }
    }
}
