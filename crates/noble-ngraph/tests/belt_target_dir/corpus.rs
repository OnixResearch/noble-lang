#![allow(
    tigerstyle::assertion_density,
    tigerstyle::missing_const_fn,
    tigerstyle::no_unwrap,
    tigerstyle::non_trait_imports,
    tigerstyle::unbounded_collection_growth,
    reason = "Owner: noble-maintainers; this exporter materializes the actual checked target artifacts as a bounded Lean corpus and the byte-for-byte freshness assertion is the contract."
)]
//! Export schedules produced by both fixed target models and their exact checked output vectors.
//! Regenerate with `cargo test -p noble-ngraph --test schedule_correspondence -- --ignored --nocapture belt_target_corpus::write_the_lean_file`.

use noble_ngraph::belt::target::{self, Issue, Machine, Operation, OutputRead};
use noble_ngraph::check::{self, CheckedGraph};
use noble_ngraph::ir::{Graph, GraphBuilder, Literal, Op, ScalarType, SubProfile, Subject};
use noble_ngraph::sem::Value;
use std::fmt::Write as _;

fn committed_path() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../proofs/foundation/NobleFoundation/BeltTargetCorpus.lean")
}

fn graph() -> Graph {
    let mut builder = GraphBuilder::new(
        SubProfile::I64,
        Subject::authored(&super::common::IDS, "belt-target-114-corpus"),
    );
    let x = builder.input(ScalarType::I64);
    let y = builder.input(ScalarType::I64);
    let scale = builder.i64_const(-3);
    let sum = builder.op(Op::I64Add(x, y));
    let difference = builder.op(Op::I64Sub(y, x));
    let product = builder.op(Op::I64Mul(sum, scale));
    builder.output(sum);
    builder.output(difference);
    builder.output(product);
    builder.finish()
}

fn checked() -> CheckedGraph {
    check::check(&super::common::IDS, graph()).expect("the fixed corpus graph checks")
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
        _ => panic!("the corpus fixture is inside the total I64/Bool fragment"),
    }
}

fn graph_operands(graph: &CheckedGraph, node: u32) -> (usize, usize) {
    match &graph.graph().nodes[node as usize].op {
        Op::I64Add(left, right)
        | Op::I64Sub(left, right)
        | Op::I64Mul(left, right)
        | Op::I64Eq(left, right) => (left.index(), right.index()),
        _ => panic!("only binary events require graph operands"),
    }
}

fn issue_term(graph: &CheckedGraph, event: Issue) -> String {
    let operation = match event.operation {
        Operation::Input { slot, .. } => format!(".input {slot}"),
        Operation::Constant { bits, .. } => format!(".constant (BitVec.ofNat 64 {bits})"),
        Operation::Add {
            left_age,
            right_age,
            node,
        } => {
            let (left, right) = graph_operands(graph, node);
            format!(".add {left} {right} {left_age} {right_age}")
        }
        Operation::Sub {
            left_age,
            right_age,
            node,
        } => {
            let (left, right) = graph_operands(graph, node);
            format!(".sub {left} {right} {left_age} {right_age}")
        }
        Operation::Multiply {
            left_age,
            right_age,
            node,
        } => {
            let (left, right) = graph_operands(graph, node);
            format!(".multiply {left} {right} {left_age} {right_age}")
        }
        Operation::Equal {
            left_age,
            right_age,
            node,
        } => {
            let (left, right) = graph_operands(graph, node);
            format!(".equal {left} {right} {left_age} {right_age}")
        }
        Operation::Save { age, slot, .. } => format!(".save {age} {slot}"),
        Operation::Load { slot, .. } => format!(".load {slot}"),
    };
    format!(
        "{{ cycle := {}, lane := {}, unit := {}, node := {}, operation := {operation} }}",
        event.cycle,
        event.lane,
        event.unit,
        match event.operation {
            Operation::Input { node, .. }
            | Operation::Constant { node, .. }
            | Operation::Add { node, .. }
            | Operation::Sub { node, .. }
            | Operation::Multiply { node, .. }
            | Operation::Equal { node, .. }
            | Operation::Save { node, .. }
            | Operation::Load { node, .. } => node,
        }
    )
}

fn lean_value(value: Value) -> String {
    match value {
        Value::I64(value) => format!("some (Val.i64 (BitVec.ofInt 64 ({value})))"),
        Value::Bool(value) => format!("some (Val.boolean {value})"),
        Value::F32(_) => panic!("the target corpus contains no F32 values"),
    }
}

fn append_schedule(
    text: &mut impl std::fmt::Write,
    checked: &CheckedGraph,
    machine: Machine,
    schedule_name: &str,
    lowered: &target::Lowered,
    expected: &[Value],
) {
    let decoded = target::decode(&super::common::IDS, &lowered.bytes)
        .expect("the lowered target artifact decodes");
    writeln!(
        text,
        "def {schedule_name} : NobleFoundation.BeltTarget.Schedule := {{\n  target := .{},\n  horizon := {},\n  events := [",
        match machine { Machine::T1 => "t1", Machine::T2 => "t2" },
        lowered.modeled_cycles
    )
    .expect("writing to a string succeeds");
    for event in decoded.events() {
        writeln!(text, "    {},", issue_term(checked, *event))
            .expect("writing to a string succeeds");
    }
    text.write_str("  ],\n  outputs := [")
        .expect("writing to a string succeeds");
    for (index, OutputRead { node }) in decoded.outputs().iter().enumerate() {
        if index > 0 {
            text.write_str(", ").expect("writing to a string succeeds");
        }
        write!(text, "{node}").expect("writing to a string succeeds");
    }
    text.write_str("]\n}\n\n")
        .expect("writing to a string succeeds");
    writeln!(
        text,
        "example : validate graph .{} outputs {schedule_name} = true := by decide",
        match machine {
            Machine::T1 => "t1",
            Machine::T2 => "t2",
        }
    )
    .expect("writing to a string succeeds");
    writeln!(
        text,
        "example : runOutputs graph inputs {schedule_name} = referenceOutputs graph inputs outputs :=\n  validate_sound graph inputs outputs .{} {schedule_name} (by decide)",
        match machine { Machine::T1 => "t1", Machine::T2 => "t2" }
    )
    .expect("writing to a string succeeds");
    let result = expected.iter().copied().map(lean_value).collect::<Vec<_>>();
    writeln!(
        text,
        "example : runOutputs graph inputs {schedule_name} = [{}] := by decide\n",
        result.join(", ")
    )
    .expect("writing to a string succeeds");
}

fn render() -> String {
    let checked = checked();
    let inputs = [Value::I64(5), Value::I64(3)];
    let mut text = String::from(
        "import NobleFoundation.BeltTarget\n\nnamespace NobleFoundation.BeltTargetCorpus\n\nopen NobleFoundation.PackedSchedule\nopen NobleFoundation.BeltTarget\n\n",
    );
    let graph = checked.graph();
    let first_output = graph.outputs.first().map_or(0, |node| node.index());
    writeln!(
        text,
        "def graph : Graph := {{\n  arity := {},\n  output := {first_output},\n  node := fun index =>",
        graph.nodes.len()
    )
    .expect("writing to a string succeeds");
    for index in 0..graph.nodes.len() {
        let prefix = if index == 0 { "    if" } else { "    else if" };
        writeln!(
            text,
            "{prefix} index = {index} then {}",
            node_term(&checked, index)
        )
        .expect("writing to a string succeeds");
    }
    text.write_str("    else .input 0\n}\n\n")
        .expect("writing to a string succeeds");
    text.write_str("def inputs (slot : Nat) : BitVec 64 := match slot with\n  | 0 => BitVec.ofInt 64 (5)\n  | 1 => BitVec.ofInt 64 (3)\n  | _ => 0\n\n")
        .expect("writing to a string succeeds");
    writeln!(
        text,
        "def outputs : List Nat := {:?}\n",
        graph
            .outputs
            .iter()
            .map(|node| node.index())
            .collect::<Vec<_>>()
    )
    .expect("writing to a string succeeds");
    for machine in [Machine::T1, Machine::T2] {
        let lowered = target::lower(&super::common::IDS, &checked, machine)
            .expect("fixed target schedule lowers");
        let expected = target::execute(
            &super::common::IDS,
            &checked,
            machine,
            &lowered.bytes,
            &inputs,
        )
        .expect("lowered target schedule executes");
        append_schedule(
            &mut text,
            &checked,
            machine,
            match machine {
                Machine::T1 => "t1Schedule",
                Machine::T2 => "t2Schedule",
            },
            &lowered,
            &expected,
        );
    }
    text.write_str("end NobleFoundation.BeltTargetCorpus\n")
        .expect("writing to a string succeeds");
    text
}

#[test]
fn committed_lean_file_matches_real_lowered_target_schedules() {
    let committed = std::fs::read_to_string(committed_path())
        .expect("the generated target foundation corpus exists");
    assert_eq!(render(), committed);
}

#[test]
#[ignore = "regenerate the source-bound target schedule corpus after an intentional lowerer change"]
fn write_the_lean_file() {
    std::fs::write(committed_path(), render()).expect("the generated target corpus is writable");
}
