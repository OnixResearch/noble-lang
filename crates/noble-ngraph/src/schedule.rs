#![expect(
    tigerstyle::path_segment_repetition,
    reason = "Owner: noble-maintainers; the module path names the domain and the item name keeps its canonical operation spelling"
)]

//! Independent validation of the **actual packed instruction stream** against its checked graph
//! (issue #43, the graph-to-packed-schedule edge).
//!
//! [`crate::layout`] validates a *declaration*: register map, constant and slot tables, register
//! count, layer ranges and failure-suffix minima. Instruction bodies stay the scheduler's own
//! business there, which leaves the real gap this module closes: a stream may name the right
//! registers and still compute the wrong operation, the wrong operand order or the wrong
//! destination.
//!
//! The relation checked here is the Rust counterpart of the Lean model's `bodiesMatch` and
//! `operandsDefined` over the total I64/Bool fragment
//! (`proofs/foundation/NobleFoundation/PackedSchedule.lean`):
//!
//! - **Declarations first.** `layout::check` runs here, so a caller cannot validate bodies against
//!   a layout that was never admitted, and its refusal keeps its own class.
//! - **Ordered bodies.** The stream is in layer order and each layer's instruction-emitting nodes
//!   are in canonical order, so position `i` must be *exactly* the body the graph node at position
//!   `i` supplies under the declared register map: operation identity, ordered source registers and
//!   destination register. Operand order is part of the body, so a swapped-operand stream is
//!   refused instead of accepted because both registers happen to be reachable.
//! - **Shape and layer coverage.** The stream length is exactly the number of
//!   instruction-emitting nodes, and each declared layer range holds exactly that layer's count.
//!
//! Definition-before-use, single-writer registers and ordered output retention are *implied* by
//! those obligations for this slice rather than checked a second time: the graph is acyclic, the
//! declared register map is injective over the scheduled nodes, and a matching body names its own
//! node's register as destination. A slice with reordering freedom inside a layer, or with guards
//! that write, would have to check them explicitly; this is stated so a future slice cannot
//! inherit the implication by accident.
//!
//! Not covered: F32 (it needs #38's expectations and its own semantics), guard ordering beyond the
//! bodies, any byte-level decoding (there is no decoder here), and anything about the Rust
//! compiler, the host or the engine.

/// One instruction of the packed I64/Bool stream. The instruction carries no node identity of its
/// own: the stream is in layer order and each layer's instruction-emitting nodes are in canonical
/// order, so the graph supplies the identity the validator compares against.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum I64Instruction {
    /// Wrapping `a + b` into `d`.
    Add(u32, u32, u32),
    /// Wrapping `a - b` into `d`.
    Sub(u32, u32, u32),
    /// Wrapping `a * b` into `d`.
    Mul(u32, u32, u32),
    /// `a = b` into `d`.
    Eq(u32, u32, u32),
    /// A guard; it writes no register.
    Guard {
        /// Canonical index of the guarded node.
        node: u32,
    },
}

/// A refused packed stream.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StreamRefusal {
    /// The graph is not the total I64/Bool fragment this validator covers.
    UnsupportedProfile,
    /// The declared layout was refused; the class is the layout's own.
    Declarations(crate::layout::Refusal),
    /// The stream length is not the number of instruction-emitting nodes.
    StreamShape,
    /// A declared layer range holds a different number of instructions than that layer has nodes.
    LayerCoverage,
    /// An instruction body is not the body its node supplies, or its destination is not the node's
    /// declared register.
    BodyMismatch,
}

/// The nodes that emit an instruction, in layer order then canonical order. Computed from the
/// graph's layer assignment, never from the stream.
fn instruction_nodes(
    checked: &crate::check::CheckedGraph,
    layers: &[(u32, u32)],
) -> alloc::vec::Vec<crate::ir::NodeId> {
    let graph = checked.graph();
    let mut nodes = alloc::vec::Vec::with_capacity(graph.nodes.len());
    for layer in 0..layers.len() {
        for index in 0..graph.nodes.len() {
            let id = crate::ir::NodeId(index as u32);
            if checked.layer(id) as usize != layer {
                continue;
            }
            if matches!(
                graph.nodes[index].op,
                crate::ir::Op::Input(_) | crate::ir::Op::Param(_) | crate::ir::Op::Const(_)
            ) {
                continue;
            }
            nodes.push(id);
        }
    }
    nodes
}

fn register(layout: &crate::layout::PackedLayout, id: crate::ir::NodeId) -> Option<u32> {
    layout
        .reg
        .get(id.0 as usize)
        .copied()
        .filter(|slot| *slot != u32::MAX)
}

/// The body the graph supplies for one instruction-emitting node under the declared register map.
/// `None` means an operand or the destination has no declared register, which is already a
/// declaration refusal; the caller reports `BodyMismatch` rather than inventing a register.
fn expected(
    checked: &crate::check::CheckedGraph,
    layout: &crate::layout::PackedLayout,
    id: crate::ir::NodeId,
) -> Option<I64Instruction> {
    let destination = register(layout, id)?;
    Some(match checked.graph().nodes[id.0 as usize].op {
        crate::ir::Op::I64Add(a, b) => {
            I64Instruction::Add(destination, register(layout, a)?, register(layout, b)?)
        }
        crate::ir::Op::I64Sub(a, b) => {
            I64Instruction::Sub(destination, register(layout, a)?, register(layout, b)?)
        }
        crate::ir::Op::I64Mul(a, b) => {
            I64Instruction::Mul(destination, register(layout, a)?, register(layout, b)?)
        }
        crate::ir::Op::I64Eq(a, b) => {
            I64Instruction::Eq(destination, register(layout, a)?, register(layout, b)?)
        }
        crate::ir::Op::Guard { .. } => I64Instruction::Guard { node: id.0 },
        _ => return None,
    })
}

/// Validate a packed I64/Bool stream against the graph it claims to implement.
pub fn check(
    checked: &crate::check::CheckedGraph,
    layout: &crate::layout::PackedLayout,
    ins: &[I64Instruction],
) -> Result<(), StreamRefusal> {
    if checked.graph().profile != crate::ir::SubProfile::I64 {
        return Err(StreamRefusal::UnsupportedProfile);
    }
    crate::layout::check(checked, layout).map_err(StreamRefusal::Declarations)?;
    let nodes = instruction_nodes(checked, &layout.layers);
    if nodes.len() != ins.len() {
        return Err(StreamRefusal::StreamShape);
    }
    let mut at = 0usize;
    for (layer, (start, end)) in layout.layers.iter().enumerate() {
        let count = nodes
            .iter()
            .filter(|id| checked.layer(**id) as usize == layer)
            .count();
        if (*end as usize).saturating_sub(*start as usize) != count || at != *start as usize {
            return Err(StreamRefusal::LayerCoverage);
        }
        at = *end as usize;
    }
    for (index, id) in nodes.iter().enumerate() {
        let want = expected(checked, layout, *id).ok_or(StreamRefusal::BodyMismatch)?;
        if ins[index] != want {
            return Err(StreamRefusal::BodyMismatch);
        }
    }
    Ok(())
}
