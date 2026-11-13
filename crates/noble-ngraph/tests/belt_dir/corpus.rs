#![allow(
    tigerstyle::assertion_density,
    tigerstyle::missing_const_fn,
    tigerstyle::no_panic,
    tigerstyle::no_unwrap,
    tigerstyle::non_trait_imports,
    tigerstyle::unbounded_collection_growth,
    reason = "Owner: noble-maintainers; this exporter materializes a small fixed checked artifact as Lean syntax and the freshness assertion is the contract."
)]
//! Export the real lowered capacity-2 program and exact input/output vectors as Lean.
//! Regenerate with `cargo test -p noble-ngraph --test schedule_correspondence --
//! --ignored --nocapture belt_corpus::write_the_lean_file`, then run the foundation lane.

use noble_ngraph::belt;
use noble_ngraph::check::CheckedGraph;
use noble_ngraph::ir::{Literal, Op};
use noble_ngraph::sem::Value;
use std::fmt::Write as _;

const CAPACITY: u8 = 2;
const SAVE_SLOTS: u8 = 8;
fn committed_path() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../proofs/foundation/NobleFoundation/BeltCorpus.lean")
}

fn u32_at(bytes: &[u8], cursor: &mut usize) -> u32 {
    let end = *cursor + 4;
    let raw: [u8; 4] = bytes[*cursor..end]
        .try_into()
        .expect("a wire u32 has four bytes");
    *cursor = end;
    u32::from_le_bytes(raw)
}

fn u64_at(bytes: &[u8], cursor: &mut usize) -> u64 {
    let end = *cursor + 8;
    let raw: [u8; 8] = bytes[*cursor..end]
        .try_into()
        .expect("a wire u64 has eight bytes");
    *cursor = end;
    u64::from_le_bytes(raw)
}

fn instruction_terms(bytes: &[u8]) -> Vec<String> {
    let mut cursor = 0usize;
    let mut terms = Vec::new();
    while cursor < bytes.len() {
        let opcode = bytes[cursor];
        cursor += 1;
        terms.push(match opcode {
            0 => format!(".input {}", u32_at(bytes, &mut cursor)),
            1 => format!(".constant (BitVec.ofNat 64 {})", u64_at(bytes, &mut cursor)),
            2 => {
                let (left, right) = (bytes[cursor], bytes[cursor + 1]);
                cursor += 2;
                format!(".add {left} {right}")
            }
            3 => {
                let (left, right) = (bytes[cursor], bytes[cursor + 1]);
                cursor += 2;
                format!(".sub {left} {right}")
            }
            4 => {
                let (left, right) = (bytes[cursor], bytes[cursor + 1]);
                cursor += 2;
                format!(".mul {left} {right}")
            }
            5 => {
                let (left, right) = (bytes[cursor], bytes[cursor + 1]);
                cursor += 2;
                format!(".eq {left} {right}")
            }
            6 => {
                let (age, slot) = (bytes[cursor], bytes[cursor + 1]);
                cursor += 2;
                format!(".save {age} {slot}")
            }
            7 => {
                let slot = bytes[cursor];
                cursor += 1;
                format!(".load {slot}")
            }
            8 => {
                let age = bytes[cursor];
                cursor += 1;
                format!(".output {age}")
            }
            _ => panic!("the checked wire stream has a known opcode"),
        });
    }
    terms
}

fn node_term(graph: &CheckedGraph, index: usize) -> String {
    match &graph.graph().nodes[index].op {
        Op::Input(slot) => format!(".input {slot}"),
        Op::Const(Literal::I64(value)) => {
            format!(".constant (BitVec.ofNat 64 {})", value.cast_unsigned())
        }
        Op::I64Add(left, right) => format!(".add {} {}", left.index(), right.index()),
        Op::I64Sub(left, right) => format!(".sub {} {}", left.index(), right.index()),
        Op::I64Mul(left, right) => format!(".mul {} {}", left.index(), right.index()),
        Op::I64Eq(left, right) => format!(".eq {} {}", left.index(), right.index()),
        _ => panic!("the corpus fixture stays inside the total I64/Bool fragment"),
    }
}

fn lean_i64(value: i64) -> String {
    format!("BitVec.ofInt 64 ({value})")
}

fn lean_value(value: Value) -> String {
    match value {
        Value::I64(value) => format!("some (Val.i64 ({}))", lean_i64(value)),
        Value::Bool(value) => format!("some (Val.boolean {value})"),
        Value::F32(_) => panic!("the corpus contains no F32 values"),
    }
}

fn render() -> String {
    let checked = super::belt_fixtures::checked(super::belt_fixtures::graph());
    let lowered = belt::lower(&super::common::IDS, &checked, CAPACITY, SAVE_SLOTS)
        .expect("the fixed graph lowers");
    let accepted = belt::check(&super::common::IDS, &checked, &lowered.bytes)
        .expect("the lowerer's bytes pass the independent checker");
    let instructions = instruction_terms(
        belt::decode(&super::common::IDS, &lowered.bytes)
            .expect("fresh artifact decodes")
            .instruction_bytes(),
    );
    let mut text = String::from(
        "-- GENERATED from the checked Rust belt lowerer; do not hand-edit.\n\
         -- Model proof and Rust artifact correspondence are separate evidence.\n\
         import NobleFoundation.Belt\n\n\
         namespace NobleFoundation.BeltCorpus\n\n\
         open NobleFoundation.PackedSchedule\n\
         open NobleFoundation.Belt\n\n",
    );
    let graph = checked.graph();
    let first_output = graph.outputs.first().map_or(0, |id| id.index());
    writeln!(
        text,
        "def graph : Graph := {{\n  arity := {},\n  output := {first_output},\n  node := fun index =>",
        graph.nodes.len()
    )
    .expect("writing to a string succeeds");
    for index in 0..graph.nodes.len() {
        let prefix = if index == 0 { "    if" } else { "    else if" };
        writeln!(text, "{prefix} index = {index} then {}", node_term(&checked, index))
            .expect("writing to a string succeeds");
    }
    text.push_str("    else .input 0\n}\n\n");
    writeln!(text, "def outputs : List Nat := {:?}", graph.outputs.iter().map(|id| id.index()).collect::<Vec<_>>())
        .expect("writing to a string succeeds");
    writeln!(text, "def capacity : Nat := {}", accepted.capacity())
        .expect("writing to a string succeeds");
    writeln!(text, "def saveSlots : Nat := {}", accepted.save_slots())
        .expect("writing to a string succeeds");
    text.push_str("def program : List NobleFoundation.Belt.Instr := [\n");
    for instruction in instructions {
        writeln!(text, "  {instruction},").expect("writing to a string succeeds");
    }
    text.push_str("]\n\n");
    writeln!(text, "example : validate graph outputs capacity saveSlots program = true := by decide\n")
        .expect("writing to a string succeeds");

    for (index, (inputs, expected)) in super::belt_fixtures::vectors().into_iter().enumerate() {
        writeln!(text, "def inputs{index} (slot : Nat) : BitVec 64 := match slot with")
            .expect("writing to a string succeeds");
        for (slot, value) in inputs.iter().enumerate() {
            writeln!(text, "  | {slot} => {}", lean_i64(match value {
                Value::I64(value) => *value,
                _ => panic!("the inputs are I64"),
            }))
            .expect("writing to a string succeeds");
        }
        text.push_str("  | _ => 0\n");
        let expected_terms = expected.iter().copied().map(lean_value).collect::<Vec<_>>();
        writeln!(
            text,
            "example : runOutputs graph inputs{index} outputs capacity saveSlots program =\n  some [{}] := by\n    rw [validate_sound graph inputs{index} outputs capacity saveSlots program (by decide)]\n    decide\n",
            expected_terms.join(", ")
        )
        .expect("writing to a string succeeds");
    }
    text.push_str("end NobleFoundation.BeltCorpus\n");
    text
}

#[test]
fn committed_lean_corpus_matches_real_lowered_programs() {
    let committed = std::fs::read_to_string(committed_path())
        .expect("the generated foundation corpus exists");
    assert_eq!(render(), committed);
}

#[test]
#[ignore = "regenerate the source-bound Lean corpus after an intentional lowerer change"]
fn write_the_lean_file() {
    std::fs::write(committed_path(), render()).expect("the generated corpus file is writable");
}
