//! Narrow checked-graph interchange for the external ONNX exporter.
//! Every unsupported operator refuses rather than silently changing its meaning.

use super::checker::Candidate;
use super::programs::{self, Program};
use noble_ngraph::ir::{Fact, Graph, Literal, Op, ScalarType, SlotKind, SubProfile};

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn node(op: &Op) -> String {
    match op {
        Op::Input(slot) => format!("[\"input\",{slot}]"),
        Op::Param(slot) => format!("[\"param\",{slot}]"),
        Op::Const(Literal::F32(bits)) => format!("[\"const\",{bits}]"),
        Op::F32Select { bit, off, on } => {
            format!("[\"select\",{},{},{}]", bit.0, off.0, on.0)
        }
        Op::Guard {
            fact: Fact::Bit,
            value,
        } => format!("[\"guard_bit\",{}]", value.0),
        _ => panic!(
            "unsupported operator in stock-runtime export: {}",
            op.name()
        ),
    }
}

pub(super) fn document(
    name: &str,
    graph: &Graph,
    weights: &[f32],
    candidate: &Candidate,
    vectors: &str,
) -> String {
    assert_eq!(graph.profile, SubProfile::F32);
    assert!(
        graph.slots.iter().all(|slot| slot.ty == ScalarType::F32),
        "stock-runtime ABI is binary32 only"
    );
    assert_eq!(
        graph
            .slots
            .iter()
            .filter(|slot| slot.kind == SlotKind::Param)
            .count(),
        weights.len()
    );
    let slots = graph
        .slots
        .iter()
        .map(|slot| match slot.kind {
            SlotKind::Input => "0",
            SlotKind::Param => "1",
        })
        .collect::<Vec<_>>()
        .join(",");
    let nodes = graph
        .nodes
        .iter()
        .map(|entry| node(&entry.op))
        .collect::<Vec<_>>()
        .join(",");
    let outputs = graph
        .outputs
        .iter()
        .map(|id| id.0.to_string())
        .collect::<Vec<_>>()
        .join(",");
    let weight_bits = weights
        .iter()
        .map(|value| value.to_bits().to_string())
        .collect::<Vec<_>>()
        .join(",");
    let (graph_id, snapshot_id, snapshot_bytes) = candidate.identities();
    let canonical_graph = candidate.graph_bytes();
    format!(
        "{{\"schema\":\"noble.recurrent-stock-export/v1\",\"program\":\"{name}\",\
         \"graph_id\":\"{}\",\"snapshot_id\":\"{}\",\"snapshot_bytes\":\"{}\",\
         \"canonical_graph\":\"{}\",\"slots\":[{slots}],\"weights\":[{weight_bits}],\
         \"nodes\":[{nodes}],\"outputs\":[{outputs}],\"vectors\":{vectors}}}",
        hex(&graph_id),
        hex(&snapshot_id),
        hex(&snapshot_bytes),
        hex(&canonical_graph),
    )
}

fn row(state: &[u64], input: &[u64], output: &[u64]) -> String {
    let list = |words: &[u64]| {
        words
            .iter()
            // JSON numbers pass through the Node receipt runner, whose Number
            // cannot represent all 64-bit words (including u64::MAX).
            .map(|word| format!("\"{word}\""))
            .collect::<Vec<_>>()
            .join(",")
    };
    format!("[[{}],[{}],[{}]]", list(state), list(input), list(output))
}

fn checked_row(candidate: &Candidate, state: &[u64], input: &[u64]) -> String {
    let output = candidate
        .one_step(state, input)
        .expect("Noble compiled, reference and source agree for stock vector");
    row(state, input, &output)
}

pub(super) fn vectors(program: Program, candidate: &Candidate) -> String {
    let tiny = (0..4)
        .flat_map(|first| {
            (0..4).map(move |second| match program {
                Program::Accumulator => checked_row(candidate, &[first], &[second]),
                Program::StackSwap => checked_row(candidate, &[first, second], &[]),
            })
        })
        .collect::<Vec<_>>()
        .join(",");
    let boundary = match program {
        Program::Accumulator => [
            (0, 0),
            (1, 1),
            (u64::MAX, 1),
            (1u64 << 63, 1u64 << 63),
            (0x5555_5555_5555_5555, 0xaaaa_aaaa_aaaa_aaaa),
            (0x0000_0000_ffff_ffff, 1),
        ]
        .into_iter()
        .map(|(state, input)| checked_row(candidate, &[state], &[input]))
        .collect::<Vec<_>>()
        .join(","),
        Program::StackSwap => String::new(),
    };
    let trajectory = |teacher_forced: bool| {
        let mut state = match program {
            Program::Accumulator => vec![0],
            Program::StackSwap => vec![1, 2],
        };
        let mut rows = Vec::with_capacity(1_000);
        for step in 0..1_000u64 {
            let input = match program {
                Program::Accumulator if teacher_forced => {
                    vec![step.wrapping_mul(0x9e37_79b9_7f4a_7c15)]
                }
                Program::Accumulator => vec![step.wrapping_mul(0xd6e8_feb8_6659_fd93)],
                Program::StackSwap => vec![],
            };
            let output = candidate
                .one_step(&state, &input)
                .expect("Noble stock-vector transition");
            rows.push(row(&state, &input, &output));
            state = if teacher_forced {
                programs::source_transition(program, &state, &input)
                    .expect("declared source transition")
            } else {
                output
            };
        }
        rows.join(",")
    };
    format!(
        "{{\"tiny\":[{tiny}],\"boundary\":[{boundary}],\"teacher_forced\":[{}],\
         \"free_running\":[{}]}}",
        trajectory(true),
        trajectory(false)
    )
}
