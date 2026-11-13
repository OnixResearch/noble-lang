use noble_ngraph::check::CheckedGraph;
use noble_ngraph::compiled::Compiled;
use noble_ngraph::eval::{self, Control, Obs};
use noble_ngraph::ir::NodeId;
use noble_ngraph::layout::PackedLayout;
use noble_ngraph::schedule::I64Instruction;
use noble_ngraph::sem::Value;

use super::emit::{hex, instruction_term, value_term, Execution, Mutation};

/// Convert a bounded schedule index to the host slice index.
pub(crate) fn at(index: u32) -> usize {
    usize::try_from(index).expect("a u32 index fits usize")
}

/// The exporter's claim: nodes in packed order, by layer and then canonical index.
pub(crate) fn packed_order(checked: &CheckedGraph) -> Vec<u32> {
    let count = u32::try_from(checked.graph().nodes.len()).expect("node count fits");
    let mut order: Vec<u32> = (0..count).collect();
    order.sort_by_key(|&index| (checked.layer(NodeId(index)), index));
    order
}

/// Every graph node grouped by the checker's assigned layer, preserving canonical order.
pub(crate) fn packed_layers(checked: &CheckedGraph) -> Vec<Vec<u32>> {
    let count = u32::try_from(checked.graph().nodes.len()).expect("node count fits");
    (0..checked.layer_count())
        .map(|layer| {
            (0..count)
                .filter(|&index| checked.layer(NodeId(index)) == layer)
                .collect()
        })
        .collect()
}

pub(crate) fn layer_guard_order(checked: &CheckedGraph, layers: &[Vec<u32>]) -> Vec<u32> {
    layers
        .iter()
        .flatten()
        .filter(|index| checked.graph().nodes[at(**index)].op.is_guard())
        .copied()
        .collect()
}

/// One preload (slot or constant table entry) or one stream instruction per node, in packed order.
pub(crate) fn exported_stream(
    checked: &CheckedGraph,
    layout: &PackedLayout,
    stream: &[I64Instruction],
) -> Vec<String> {
    let mut computed = stream.iter();
    let mut terms = Vec::with_capacity(checked.graph().nodes.len());
    for index in packed_order(checked) {
        let register = layout.reg[at(index)];
        let slot = layout.slot_regs.iter().find(|(reg, _)| *reg == register);
        let constant = layout.consts.iter().find(|(reg, _)| *reg == register);
        terms.push(match (slot, constant) {
            (Some((_, slot)), None) => format!(".setInput {register} {slot}"),
            (None, Some((_, bits))) => format!(".setConst {register} {}", hex(*bits)),
            (None, None) => {
                instruction_term(computed.next().expect("one instruction per computed node"))
            }
            (Some(_), Some(_)) => panic!("register {register} is in both preload tables"),
        });
    }
    assert!(
        computed.next().is_none(),
        "no instruction is left without a node"
    );
    terms
}

const RUNS: [[i64; 2]; 5] = [
    [3, 8],
    [0, 0],
    [-1, 1],
    [i64::MAX, i64::MIN + 4],
    [i64::MIN, -1],
];

/// The real packed machine's outputs at boundary inputs, checked against the reference interpreter.
pub(crate) fn executions(checked: &CheckedGraph) -> Vec<Execution> {
    let compiled = Compiled::new(&super::common::IDS, checked);
    RUNS.iter()
        .map(|run| {
            let inputs: Vec<Value> = run.iter().map(|&value| Value::I64(value)).collect();
            let packed = compiled
                .evaluate(&inputs, None, &mut Control::default())
                .obs;
            let reference = eval::reference(checked, &inputs, None, &mut Control::default()).obs;
            assert_eq!(
                packed, reference,
                "packed execution agrees with the reference interpreter"
            );
            let Obs::Normal(values) = packed else {
                panic!("a normal observation")
            };
            (run.to_vec(), values.iter().map(value_term).collect())
        })
        .collect()
}

/// Mutated copies of a packed artifact, each with one ordinary stream obligation changed.
pub(crate) fn base_mutations(
    checked: &CheckedGraph,
    layout: &PackedLayout,
    stream: &[I64Instruction],
    instrs: &[String],
    out_regs: &[u32],
) -> Vec<Mutation> {
    let restream = |changed: Vec<I64Instruction>| exported_stream(checked, layout, &changed);
    let mut refused = Vec::new();
    if let Some(index) = stream
        .iter()
        .position(|instruction| matches!(instruction, I64Instruction::Add(..)))
    {
        let mut changed = stream.to_vec();
        if let I64Instruction::Add(destination, left, right) = changed[index] {
            changed[index] = I64Instruction::Sub(destination, left, right);
        }
        refused.push((
            "wrong_operation",
            "the first addition emitted as a subtraction",
            restream(changed),
            out_regs.to_vec(),
        ));
    }
    if let Some(index) = stream.iter().position(
        |instruction| matches!(instruction, I64Instruction::Sub(_, left, right) if left != right),
    ) {
        let mut changed = stream.to_vec();
        if let I64Instruction::Sub(destination, left, right) = changed[index] {
            changed[index] = I64Instruction::Sub(destination, right, left);
        }
        refused.push((
            "swapped_operands",
            "the first subtraction's operands swapped",
            restream(changed),
            out_regs.to_vec(),
        ));
    }
    if let Some(&(start, _)) = layout.layers.iter().find(|(start, end)| end - start >= 2) {
        let mut changed = stream.to_vec();
        changed.swap(at(start), at(start) + 1);
        refused.push((
            "reordered_within_layer",
            "two instructions of one layer swapped",
            restream(changed),
            out_regs.to_vec(),
        ));
    }
    let mut dropped = instrs.to_vec();
    dropped.pop();
    refused.push((
        "dropped_instruction",
        "the last instruction dropped",
        dropped,
        out_regs.to_vec(),
    ));
    if out_regs.len() >= 2 && out_regs[0] != out_regs[1] {
        let mut swapped = out_regs.to_vec();
        swapped.swap(0, 1);
        refused.push((
            "swapped_outputs",
            "the first two output registers swapped",
            instrs.to_vec(),
            swapped,
        ));
    }
    refused
}
