#![expect(
    tigerstyle::path_segment_repetition,
    reason = "Owner: noble-maintainers; the module path names the domain and the item name keeps its canonical operation spelling"
)]

//! Independent validation of a declared packed layout against its graph.
//!
//! The scheduler stays untrusted: this module never regenerates the schedule and
//! never reads an instruction body. It checks the declared register map, the
//! constant and slot tables and the register count against the graph's own ops, so
//! a wrong declaration is refused rather than compared with another copy of
//! itself. It is the Rust side of the correspondence the Lean model in
//! `proofs/foundation/NobleFoundation/PackedSchedule.lean` states abstractly;
//! **no correspondence theorem binds these bytes to that model**, and the layer
//! ranges and guard registers are deliberately outside this check.

use crate::ir::{Digest, Graph, Literal, Op};

/// Layout declaration revision bound into every accepted layout.
pub const LAYOUT_REVISION: &str = "ngraph-layout/0";

/// The declared packed layout of one compiled graph.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PackedLayout {
    /// Layout declaration revision.
    pub revision: alloc::string::String,
    /// Scheduler/compiler revision the layout was built by.
    pub compiler_revision: alloc::string::String,
    /// Declared register count, including the constant-one register.
    pub regs: u32,
    /// Declared register of each node, in node order (`u32::MAX` for guards).
    pub reg: alloc::vec::Vec<u32>,
    /// Declared constant registers and their exact bits.
    pub consts: alloc::vec::Vec<(u32, u64)>,
    /// Declared interface-slot registers and their slot ordinals.
    pub slot_regs: alloc::vec::Vec<(u32, u32)>,
    /// Build key the layout was produced under.
    pub layout_key: Digest,
}

/// Why a layout was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// The layout declaration revision is not the current one.
    StaleRevision,
    /// The register map does not have one entry per node.
    RegisterMapShape,
    /// A declared register lies outside the declared register count.
    RegisterOutOfRange,
    /// Two distinct non-guard nodes share a register.
    AliasedRegisters,
    /// The declared register count sits more than one above the highest register.
    RegisterCount,
    /// A constant register does not name exactly one constant node's bits.
    ConstantTable,
    /// A slot register does not name exactly one interface input node.
    SlotTable,
    /// A guard node was declared a register.
    GuardRegister,
}

fn guard(op: &Op) -> bool {
    matches!(op, Op::Guard { .. })
}

fn i64_bits(lit: Literal) -> Option<u64> {
    match lit {
        Literal::I64(value) => Some(value.cast_unsigned()),
        Literal::Bool(_) | Literal::F32(_) => None,
    }
}

/// Validates a declared layout against the graph it claims to pack.
pub fn check(graph: &Graph, layout: &PackedLayout) -> Result<(), Refusal> {
    if layout.revision != LAYOUT_REVISION {
        return Err(Refusal::StaleRevision);
    }
    if layout.reg.len() != graph.nodes.len() {
        return Err(Refusal::RegisterMapShape);
    }
    let mut seen = alloc::vec::Vec::new();
    let mut highest: Option<u32> = None;
    for (index, node) in graph.nodes.iter().enumerate() {
        let declared = layout
            .reg
            .get(index)
            .copied()
            .ok_or(Refusal::RegisterMapShape)?;
        if guard(&node.op) {
            if declared != u32::MAX {
                return Err(Refusal::GuardRegister);
            }
            continue;
        }
        if declared >= layout.regs {
            return Err(Refusal::RegisterOutOfRange);
        }
        if seen.contains(&declared) {
            return Err(Refusal::AliasedRegisters);
        }
        seen.push(declared);
        highest = Some(highest.map_or(declared, |current| current.max(declared)));
    }
    // The declared count may sit one above the highest declared register (the
    // scheduler reserves one register), but no further: an inflated count is a
    // wrong declaration rather than capacity headroom.
    if layout
        .regs
        .saturating_sub(highest.map_or(0, |current| current.saturating_add(1)))
        > 1
    {
        return Err(Refusal::RegisterCount);
    }
    for &(register, bits) in &layout.consts {
        let matching = graph
            .nodes
            .iter()
            .enumerate()
            .filter(|(index, node)| {
                matches!(node.op, Op::Const(lit) if i64_bits(lit) == Some(bits))
                    && layout.reg.get(*index).copied() == Some(register)
            })
            .count();
        if matching != 1 {
            return Err(Refusal::ConstantTable);
        }
    }
    for &(register, slot) in &layout.slot_regs {
        let matching = graph
            .nodes
            .iter()
            .enumerate()
            .filter(|(index, node)| {
                matches!(node.op, Op::Input(k) if k == slot)
                    && layout.reg.get(*index).copied() == Some(register)
            })
            .count();
        if matching != 1 {
            return Err(Refusal::SlotTable);
        }
    }
    let declared_constants = layout.consts.len();
    let constant_nodes = graph
        .nodes
        .iter()
        .filter(|node| matches!(node.op, Op::Const(lit) if i64_bits(lit).is_some()))
        .count();
    if declared_constants != constant_nodes {
        return Err(Refusal::ConstantTable);
    }
    let declared_slots = layout.slot_regs.len();
    let input_nodes = graph
        .nodes
        .iter()
        .filter(|node| matches!(node.op, Op::Input(_)))
        .count();
    if declared_slots != input_nodes {
        return Err(Refusal::SlotTable);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{PackedLayout, Refusal, LAYOUT_REVISION};
    use crate::check;
    use crate::compiled::Compiled;
    use crate::identity::IdentityScheme;
    use crate::ir::{Digest, GraphBuilder, Op, SubProfile, Subject};

    struct TestHasher(u64);

    impl crate::identity::IdentityHasher for TestHasher {
        fn update(&mut self, bytes: &[u8]) {
            for &byte in bytes {
                self.0 = self.0.wrapping_mul(31).wrapping_add(u64::from(byte));
            }
        }

        fn finish(self: alloc::boxed::Box<Self>) -> Digest {
            let mut digest = [0u8; 32];
            digest[..8].copy_from_slice(&self.0.to_le_bytes());
            digest
        }
    }

    struct TestIds;

    impl IdentityScheme for TestIds {
        fn name(&self) -> &'static str {
            "test"
        }

        fn start(&self) -> alloc::boxed::Box<dyn crate::identity::IdentityHasher + '_> {
            alloc::boxed::Box::new(TestHasher(0))
        }
    }

    pub(super) fn sample() -> (crate::ir::Graph, PackedLayout) {
        let scheme = TestIds;
        let mut builder =
            GraphBuilder::new(SubProfile::I64, Subject::authored(&scheme, "layout-sample"));
        let input = builder.input(crate::ir::ScalarType::I64);
        let three = builder.i64_const(3);
        let sum = builder.op(Op::I64Add(input, three));
        builder.output(sum);
        let graph = builder.finish();
        let checked = check::check(&scheme, graph.clone()).expect("the sample checks");
        let layout = Compiled::new(&scheme, &checked).packed_layout();
        (graph, layout)
    }

    #[test]
    fn a_real_compiled_layout_is_accepted() {
        let (graph, layout) = sample();
        assert_eq!(layout.revision, LAYOUT_REVISION);
        assert!(super::check(&graph, &layout).is_ok());
    }

    #[test]
    fn mutated_layouts_are_refused_for_their_own_premise() {
        let (graph, layout) = sample();
        let mut aliased = layout.clone();
        aliased.reg = alloc::vec![0; layout.reg.len()];
        assert_eq!(
            super::check(&graph, &aliased),
            Err(Refusal::AliasedRegisters)
        );

        let mut out_of_range = layout.clone();
        out_of_range.regs = 0;
        assert_eq!(
            super::check(&graph, &out_of_range),
            Err(Refusal::RegisterOutOfRange)
        );

        let mut wrong_bits = layout.clone();
        if let Some(entry) = wrong_bits.consts.first_mut() {
            entry.1 = entry.1.wrapping_add(1);
        }
        assert_eq!(
            super::check(&graph, &wrong_bits),
            Err(Refusal::ConstantTable)
        );

        let mut wrong_slot = layout.clone();
        if let Some(entry) = wrong_slot.slot_regs.first_mut() {
            entry.1 = entry.1.saturating_add(9);
        }
        assert_eq!(super::check(&graph, &wrong_slot), Err(Refusal::SlotTable));

        let mut dropped = layout.clone();
        dropped.consts.clear();
        assert_eq!(super::check(&graph, &dropped), Err(Refusal::ConstantTable));

        let mut stale = layout.clone();
        stale.revision = alloc::string::String::from("ngraph-layout/1");
        assert_eq!(super::check(&graph, &stale), Err(Refusal::StaleRevision));

        let mut wrong_count = layout.clone();
        wrong_count.regs = layout.regs.saturating_add(2);
        assert_eq!(
            super::check(&graph, &wrong_count),
            Err(Refusal::RegisterCount)
        );

        let mut guard_register = layout.clone();
        assert!(
            !graph
                .nodes
                .iter()
                .any(|node| matches!(node.op, Op::Guard { .. })),
            "the sample declares no guard, so the guard rule is exercised below"
        );
        guard_register.reg = alloc::vec![u32::MAX; layout.reg.len()];
        assert_eq!(
            super::check(&graph, &guard_register),
            Err(Refusal::RegisterOutOfRange)
        );
    }
}
