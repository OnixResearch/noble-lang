use super::*;

pub(super) fn hex_bits(bits: i64) -> String {
    format!("0x{:016X}", bits.cast_unsigned())
}

/// The text one operand spells, as the validator reads it.
pub(super) fn operand_text(operand: Operand) -> String {
    match operand {
        Operand::Parameter(slot) => format!("%a{slot}"),
        Operand::Constant(bits) => format!("{bits}"),
        Operand::Value(node) => format!("%v{node}"),
    }
}

/// A graph node as the Lean model's `Node`, or a stop: the exporter refuses anything outside the
/// profile rather than exporting a guess.
pub(super) fn lean_node(op: &Op) -> String {
    match op {
        Op::Input(slot) => format!(".input {slot}"),
        Op::Const(Literal::I64(bits)) => format!(".constant {}", hex_bits(*bits)),
        Op::I64Add(left, right) => format!(".add {} {}", left.0, right.0),
        Op::I64Sub(left, right) => format!(".sub {} {}", left.0, right.0),
        Op::I64Mul(left, right) => format!(".mul {} {}", left.0, right.0),
        other => panic!("outside the exported Lean slice: {other:?}"),
    }
}

pub(super) fn lean_operand(operand: Operand) -> String {
    match operand {
        Operand::Parameter(slot) => format!(".param {slot}"),
        Operand::Constant(bits) => format!(".constant {}", hex_bits(bits)),
        Operand::Value(node) => format!(".value {node}"),
    }
}

pub(super) fn lean_instruction(instruction: &Instruction) -> String {
    format!(
        "{{ dst := {}, opcode := .{}, left := {}, right := {} }}",
        instruction.node,
        instruction.opcode.name(),
        lean_operand(instruction.operands[0]),
        lean_operand(instruction.operands[1])
    )
}

/// A multi-line Lean list, one element per line.
pub(super) fn term_list(items: &[String]) -> String {
    format!("\n  [{}]", items.join(",\n   "))
}

/// The Lean identifier a fixture name maps to.
pub(super) fn lean_name(name: &str) -> String {
    name.replace('-', "_")
}

pub(super) fn reference_value(checked: &CheckedGraph, inputs: [i64; 2]) -> i64 {
    let mut control = eval::Control {
        fuel: None,
        cancel: None,
        scratch: None,
        forward_scratch: None,
        evaluated_nodes: None,
    };
    let report = eval::reference(
        checked,
        &[Value::I64(inputs[0]), Value::I64(inputs[1])],
        None,
        &mut control,
    );
    match report.obs {
        Obs::Normal(values) => match values.as_slice() {
            [Value::I64(bits)] => *bits,
            other => panic!("one I64 output, observed {other:?}"),
        },
        other => panic!("the reference evaluates the fixture: {other:?}"),
    }
}

/// The value the parsed artifact produces at an input pair, checked against the graph reference.
pub(super) fn executed(held: &Artifact, inputs: [i64; 2]) -> i64 {
    let observed = llvm::execute(&held.module, inputs).expect("the artifact produces a value");
    assert_eq!(
        observed,
        reference_value(&held.checked, inputs),
        "the artifact and the graph reference agree at {inputs:?}"
    );
    observed
}
