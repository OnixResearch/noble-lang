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

use crate::ir::{Digest, Literal, Op};

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
    /// Declared instruction range `(start, end)` of each layer.
    pub layers: alloc::vec::Vec<(u32, u32)>,
    /// Declared failure-suffix minimum of each layer, plus one trailing entry.
    pub fail_suffix_min: alloc::vec::Vec<u32>,
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
    /// The declared layer table does not have one range per assigned layer.
    LayerShape,
    /// A layer range is not ordered inside itself or against its predecessor.
    LayerOrder,
    /// The declared failure-suffix minima are not the graph's own prefix minima.
    FailSuffixMismatch,
    /// The declared layer ranges do not cover exactly the graph's instruction nodes.
    LayerCoverage,
}

fn guard(op: &Op) -> bool {
    matches!(op, Op::Guard { .. })
}

/// The exact bits a constant node occupies in its register, for every literal
/// kind the compiler packs: `I64` as wrapping unsigned bits, `Bool` as 0 or 1 and
/// `F32` as its binary32 bits.
fn constant_bits(lit: Literal) -> u64 {
    match lit {
        Literal::I64(value) => value.cast_unsigned(),
        Literal::Bool(value) => u64::from(value),
        Literal::F32(bits) => u64::from(bits),
    }
}

fn fails_suffix(node: &crate::ir::Node) -> bool {
    match &node.op {
        Op::Guard { .. } => true,
        Op::F32Index { below, above, .. } => {
            *below == crate::ir::OutOfRange::Refuse || *above == crate::ir::OutOfRange::Refuse
        }
        _ => false,
    }
}

/// Validates a declared layout against the graph it claims to pack. The layer
/// assignment comes from the checker, not from the scheduler: the declared layer
/// ranges and failure-suffix minima are compared against it, never regenerated.
pub fn check(checked: &crate::check::CheckedGraph, layout: &PackedLayout) -> Result<(), Refusal> {
    let graph = checked.graph();
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
                matches!(node.op, Op::Const(lit) if constant_bits(lit) == bits)
                    && layout.reg.get(*index).copied() == Some(register)
            })
            .count();
        if matching != 1 {
            return Err(Refusal::ConstantTable);
        }
    }
    // Both slot kinds share one table in the compiler, so an entry may describe an input node or a
    // parameter node. Each declared slot is still covered exactly once: a second entry for the same
    // slot, a swapped kind, or a register that no node of that slot carries is refused.
    let mut covered = alloc::vec![false; graph.slots.len()];
    for &(register, slot) in &layout.slot_regs {
        let matching = graph
            .nodes
            .iter()
            .enumerate()
            .filter(|(index, node)| {
                matches!(node.op, Op::Input(k) | Op::Param(k) if k == slot)
                    && layout.reg.get(*index).copied() == Some(register)
            })
            .count();
        if matching != 1 {
            return Err(Refusal::SlotTable);
        }
        let Some(seen) = covered.get_mut(slot as usize) else {
            return Err(Refusal::SlotTable);
        };
        if *seen {
            return Err(Refusal::SlotTable);
        }
        *seen = true;
    }
    let declared_constants = layout.consts.len();
    let constant_nodes = graph
        .nodes
        .iter()
        .filter(|node| matches!(node.op, Op::Const(_)))
        .count();
    if declared_constants != constant_nodes {
        return Err(Refusal::ConstantTable);
    }
    let layer_count = usize::try_from(checked.layer_count()).unwrap_or(usize::MAX);
    if layout.layers.len() != layer_count || layout.fail_suffix_min.len() != layer_count + 1 {
        return Err(Refusal::LayerShape);
    }
    let mut previous_end = 0u32;
    for &(start, end) in &layout.layers {
        if start > end || start < previous_end {
            return Err(Refusal::LayerOrder);
        }
        previous_end = end;
    }
    // The graph's own prefix minima: the least canonical index of a node that
    // needs a failure suffix, over every layer at or after each layer.
    let mut minima = alloc::vec![u32::MAX; layer_count + 1];
    let mut at_layer = alloc::vec![u32::MAX; layer_count.max(1)];
    for (index, node) in graph.nodes.iter().enumerate() {
        if !fails_suffix(node) {
            continue;
        }
        let Ok(index) = u32::try_from(index) else {
            return Err(Refusal::FailSuffixMismatch);
        };
        let layer = usize::try_from(checked.layer(crate::ir::NodeId(index)))
            .map_err(|_| Refusal::FailSuffixMismatch)?;
        let slot = at_layer.get_mut(layer).ok_or(Refusal::LayerShape)?;
        *slot = (*slot).min(index);
    }
    for layer in (0..layer_count).rev() {
        minima[layer] = minima[layer + 1].min(at_layer[layer]);
    }
    if minima != layout.fail_suffix_min {
        return Err(Refusal::FailSuffixMismatch);
    }
    // Every node that produces an instruction is packed exactly once: slots and
    // constants are table entries, not instructions.
    let instructions = graph
        .nodes
        .iter()
        .filter(|node| !matches!(node.op, Op::Input(_) | Op::Param(_) | Op::Const(_)))
        .count();
    let Ok(instructions) = u32::try_from(instructions) else {
        return Err(Refusal::LayerCoverage);
    };
    if layout.layers.last().map(|&(_, end)| end) != Some(instructions) {
        return Err(Refusal::LayerCoverage);
    }
    let declared_slots = layout.slot_regs.len();
    let slot_nodes = graph
        .nodes
        .iter()
        .filter(|node| matches!(node.op, Op::Input(_) | Op::Param(_)))
        .count();
    if declared_slots != slot_nodes {
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
    use crate::ir::Fact;
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

    pub(super) fn sample() -> (crate::ir::Graph, crate::check::CheckedGraph, PackedLayout) {
        let scheme = TestIds;
        let mut builder =
            GraphBuilder::new(SubProfile::I64, Subject::authored(&scheme, "layout-sample"));
        let input = builder.input(crate::ir::ScalarType::I64);
        let three = builder.i64_const(3);
        let sum = builder.op(Op::I64Add(input, three));
        builder.guard(Fact::Bit, sum);
        builder.output(sum);
        let graph = builder.finish();
        let checked = check::check(&scheme, graph.clone()).expect("the sample checks");
        let layout = Compiled::new(&scheme, &checked).packed_layout();
        (graph, checked, layout)
    }

    #[test]
    fn a_real_compiled_layout_is_accepted() {
        let (_graph, checked, layout) = sample();
        assert_eq!(layout.revision, LAYOUT_REVISION);
        assert!(super::check(&checked, &layout).is_ok());
    }

    #[test]
    fn mutated_layouts_are_refused_for_their_own_premise() {
        let (graph, checked, layout) = sample();
        let check = |layout: &PackedLayout| super::check(&checked, layout);
        let mut aliased = layout.clone();
        aliased.reg = alloc::vec![0; layout.reg.len()];
        assert_eq!(check(&aliased), Err(Refusal::AliasedRegisters));

        let mut out_of_range = layout.clone();
        out_of_range.regs = 0;
        assert_eq!(check(&out_of_range), Err(Refusal::RegisterOutOfRange));

        let mut wrong_bits = layout.clone();
        if let Some(entry) = wrong_bits.consts.first_mut() {
            entry.1 = entry.1.wrapping_add(1);
        }
        assert_eq!(check(&wrong_bits), Err(Refusal::ConstantTable));

        let mut wrong_slot = layout.clone();
        if let Some(entry) = wrong_slot.slot_regs.first_mut() {
            entry.1 = entry.1.saturating_add(9);
        }
        assert_eq!(check(&wrong_slot), Err(Refusal::SlotTable));

        let mut dropped = layout.clone();
        dropped.consts.clear();
        assert_eq!(check(&dropped), Err(Refusal::ConstantTable));

        let mut stale = layout.clone();
        stale.revision = alloc::string::String::from("ngraph-layout/1");
        assert_eq!(check(&stale), Err(Refusal::StaleRevision));

        let mut wrong_count = layout.clone();
        wrong_count.regs = layout.regs.saturating_add(2);
        assert_eq!(check(&wrong_count), Err(Refusal::RegisterCount));

        // Declaring the sample's real guard a register is refused by the guard rule.
        let mut guard_register = layout.clone();
        let guard_index = graph
            .nodes
            .iter()
            .position(|node| matches!(node.op, Op::Guard { .. }))
            .expect("the sample declares one guard");
        guard_register.reg[guard_index] = 0;
        assert_eq!(check(&guard_register), Err(Refusal::GuardRegister));

        let mut dropped_layer = layout.clone();
        dropped_layer.layers.pop();
        assert_eq!(check(&dropped_layer), Err(Refusal::LayerShape));

        let mut unordered_layer = layout.clone();
        unordered_layer.layers = alloc::vec![(5, 2); layout.layers.len()];
        assert_eq!(check(&unordered_layer), Err(Refusal::LayerOrder));

        let mut smoothed_suffix = layout.clone();
        smoothed_suffix.fail_suffix_min = alloc::vec![u32::MAX; layout.fail_suffix_min.len()];
        assert_eq!(check(&smoothed_suffix), Err(Refusal::FailSuffixMismatch));

        let mut shifted_suffix = layout.clone();
        if let Some(entry) = shifted_suffix.fail_suffix_min.last_mut() {
            *entry = 0;
        }
        assert_eq!(check(&shifted_suffix), Err(Refusal::FailSuffixMismatch));
    }
}
