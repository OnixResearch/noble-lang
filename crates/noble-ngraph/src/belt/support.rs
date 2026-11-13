use alloc::vec;
use alloc::vec::Vec;

use super::types::Refusal;

pub(crate) fn admit_graph(checked: &crate::check::CheckedGraph) -> Result<(), Refusal> {
    let graph = checked.graph();
    if graph.profile != crate::ir::SubProfile::I64
        || !graph.facts.is_empty()
        || graph.nodes.len() > super::MAX_NODES
        || graph.outputs.len() > super::MAX_OUTPUTS
        || graph.slots.len() > super::MAX_NODES
    {
        return Err(if graph.nodes.len() > super::MAX_NODES
            || graph.outputs.len() > super::MAX_OUTPUTS
            || graph.slots.len() > super::MAX_NODES
        {
            Refusal::GraphBound
        } else {
            Refusal::UnsupportedGraph
        });
    }
    if graph
        .slots
        .iter()
        .any(|slot| slot.kind != crate::ir::SlotKind::Input || slot.ty != crate::ir::ScalarType::I64)
    {
        return Err(Refusal::UnsupportedGraph);
    }
    for (index, node) in graph.nodes.iter().enumerate() {
        let id = crate::ir::NodeId(u32::try_from(index).map_err(|_| Refusal::GraphBound)?);
        let valid = match &node.op {
            crate::ir::Op::Input(slot) => {
                usize::try_from(*slot)
                    .ok()
                    .and_then(|slot_index| graph.slots.get(slot_index))
                    .is_some_and(|declared| declared.kind == crate::ir::SlotKind::Input)
                    && checked.ty(id) == Some(crate::ir::ScalarType::I64)
            }
            crate::ir::Op::Const(crate::ir::Literal::I64(_)) => {
                checked.ty(id) == Some(crate::ir::ScalarType::I64)
            }
            crate::ir::Op::I64Add(a, b)
            | crate::ir::Op::I64Sub(a, b)
            | crate::ir::Op::I64Mul(a, b) => {
                checked.ty(id) == Some(crate::ir::ScalarType::I64)
                    && checked.ty(*a) == Some(crate::ir::ScalarType::I64)
                    && checked.ty(*b) == Some(crate::ir::ScalarType::I64)
            }
            crate::ir::Op::I64Eq(a, b) => {
                checked.ty(id) == Some(crate::ir::ScalarType::Bool)
                    && checked.ty(*a) == Some(crate::ir::ScalarType::I64)
                    && checked.ty(*b) == Some(crate::ir::ScalarType::I64)
            }
            _ => false,
        };
        if !valid {
            return Err(Refusal::UnsupportedGraph);
        }
    }
    Ok(())
}

pub(crate) fn remaining_uses(checked: &crate::check::CheckedGraph) -> Result<Vec<u32>, Refusal> {
    let graph = checked.graph();
    let mut remaining = vec![0u32; graph.nodes.len()];
    for node in &graph.nodes {
        match &node.op {
            crate::ir::Op::I64Add(a, b)
            | crate::ir::Op::I64Sub(a, b)
            | crate::ir::Op::I64Mul(a, b)
            | crate::ir::Op::I64Eq(a, b) => {
                increment(&mut remaining, *a)?;
                increment(&mut remaining, *b)?;
            }
            crate::ir::Op::Input(_) | crate::ir::Op::Const(_) => {}
            _ => return Err(Refusal::UnsupportedGraph),
        }
    }
    for &output in &graph.outputs {
        increment(&mut remaining, output)?;
    }
    Ok(remaining)
}

fn increment(remaining: &mut [u32], node: crate::ir::NodeId) -> Result<(), Refusal> {
    let count = remaining
        .get_mut(node.index())
        .ok_or(Refusal::UnsupportedGraph)?;
    *count = count.checked_add(1).ok_or(Refusal::GraphBound)?;
    Ok(())
}

pub(crate) fn consume(remaining: &mut [u32], node: crate::ir::NodeId) -> Result<(), Refusal> {
    let count = remaining
        .get_mut(node.index())
        .ok_or(Refusal::OperandMismatch)?;
    *count = count.checked_sub(1).ok_or(Refusal::OperandMismatch)?;
    Ok(())
}
