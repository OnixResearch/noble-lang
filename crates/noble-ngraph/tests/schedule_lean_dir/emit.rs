#![allow(
    tigerstyle::assertion_density,
    tigerstyle::missing_const_fn,
    tigerstyle::no_panic,
    tigerstyle::no_unwrap,
    tigerstyle::non_trait_imports,
    tigerstyle::unbounded_collection_growth,
    reason = "Owner: noble-maintainers; the Lean text of small fixed fixtures is built by string concatenation and must stop at anything outside the exported slice, so expect/panic on unsupported input and owner-path imports are the readable form."
)]
//! Lean text for the real packed artifacts `corpus.rs` exports (issue #43).

use noble_ngraph::ir::{Graph, Literal, Op, ScalarType, SlotKind};
use noble_ngraph::schedule::I64Instruction;
use noble_ngraph::sem::Value;

/// A refused copy: theorem suffix, what was changed, and the changed instructions and output
/// registers.
pub(crate) type Mutation = (&'static str, &'static str, Vec<String>, Vec<u32>);

/// One execution of the real packed machine: its I64 inputs, by slot, and its outputs as Lean
/// `Option Val` terms.
pub(crate) type Execution = (Vec<i64>, Vec<String>);

/// One real artifact in Lean terms.
pub(crate) struct Artifact {
    pub(crate) name: &'static str,
    pub(crate) doc: &'static str,
    pub(crate) nodes: Vec<String>,
    pub(crate) regs: Vec<u32>,
    pub(crate) order: Vec<u32>,
    pub(crate) instrs: Vec<String>,
    pub(crate) outputs: Vec<u32>,
    pub(crate) out_regs: Vec<u32>,
    pub(crate) mutations: Vec<Mutation>,
    pub(crate) executions: Vec<Execution>,
}

pub(crate) fn hex(bits: u64) -> String {
    format!("0x{bits:016X}")
}

/// A packed-machine output as a Lean `Option Val`.
pub(crate) fn value_term(value: &Value) -> String {
    match *value {
        Value::I64(bits) => format!("some (.i64 {})", hex(bits.cast_unsigned())),
        Value::Bool(bit) => format!("some (.boolean {bit})"),
        Value::F32(_) => panic!("F32 is outside the exported Lean slice"),
    }
}

/// A graph node as a Lean `Node`; anything outside the model's I64/Bool slice stops the export.
pub(crate) fn node_term(graph: &Graph, op: &Op) -> String {
    match op {
        Op::Input(slot) => {
            let declared = &graph.slots[usize::try_from(*slot).expect("a u32 slot fits usize")];
            assert!(
                matches!(declared.kind, SlotKind::Input),
                "only input slots are in the Lean slice"
            );
            assert!(
                declared.ty == ScalarType::I64,
                "only I64 slots are in the Lean slice"
            );
            format!(".input {slot}")
        }
        Op::Const(Literal::I64(value)) => format!(".constant {}", hex(value.cast_unsigned())),
        Op::I64Add(a, b) => format!(".add {} {}", a.0, b.0),
        Op::I64Sub(a, b) => format!(".sub {} {}", a.0, b.0),
        Op::I64Mul(a, b) => format!(".mul {} {}", a.0, b.0),
        Op::I64Eq(a, b) => format!(".eq {} {}", a.0, b.0),
        other => panic!("outside the exported Lean slice: {other:?}"),
    }
}

/// A stream instruction as a Lean `Instr`, destination first, as both sides order it.
pub(crate) fn instruction_term(instruction: &I64Instruction) -> String {
    match *instruction {
        I64Instruction::Add(d, a, b) => format!(".add {d} {a} {b}"),
        I64Instruction::Sub(d, a, b) => format!(".sub {d} {a} {b}"),
        I64Instruction::Mul(d, a, b) => format!(".mul {d} {a} {b}"),
        I64Instruction::Eq(d, a, b) => format!(".eq {d} {a} {b}"),
        I64Instruction::Guard { .. } => panic!("guards are outside the exported Lean slice"),
    }
}

pub(crate) fn nat_list(items: &[u32]) -> String {
    let parts: Vec<String> = items.iter().map(u32::to_string).collect();
    format!("[{}]", parts.join(", "))
}

/// A multi-line Lean list, one element per line.
fn term_list(items: &[String]) -> String {
    format!("\n  [{}]", items.join(",\n   "))
}

/// A `validate` call on the artifact's graph, register map and order, with the given instructions
/// (including their leading separator) and output registers.
fn validate_call(name: &str, instrs: &str, out_regs: &str) -> String {
    format!("validate {name}Graph (regOfList {name}Regs) {name}Order{instrs}\n      {name}Outputs {out_regs}")
}

pub(crate) fn artifact_text(artifact: &Artifact) -> String {
    let name = artifact.name;
    let output = artifact.outputs[0];
    let mut text = format!(
        "/-! ## `{name}`: {doc} -/\n\n\
         def {name}Nodes : List Node :={nodes}\n\n\
         def {name}Graph : Graph := FirstProjection.ofList {name}Nodes {output}\n\n\
         def {name}Regs : List Nat := {regs}\n\n\
         def {name}Order : List Nat := {order}\n\n\
         def {name}Instrs : List Instr :={instrs}\n\n\
         def {name}Outputs : List Nat := {outputs}\n\n\
         def {name}OutRegs : List Nat := {out_regs}\n\n\
         theorem {name}_validated :\n    {call} = true := by\n  decide\n\n\
         theorem {name}_wellFormed : WellFormed {name}Graph :=\n  \
         FirstProjection.ofList_wellFormed {name}Nodes {output} (by decide) (by decide)\n\n\
         /-- The exported stream returns the graph reference at every output, for every input. -/\n\
         theorem {name}_sound (inputs : Nat → BitVec 64) :\n    \
         runOutputs inputs {name}Instrs {name}OutRegs =\n      \
         referenceOutputs {name}Graph inputs {name}Outputs :=\n  \
         validate_sound {name}Graph {name}_wellFormed (regOfList {name}Regs) {name}Order\n    \
         {name}Instrs {name}Outputs {name}OutRegs {name}_validated inputs\n",
        doc = artifact.doc,
        nodes = term_list(&artifact.nodes),
        regs = nat_list(&artifact.regs),
        order = nat_list(&artifact.order),
        instrs = term_list(&artifact.instrs),
        outputs = nat_list(&artifact.outputs),
        out_regs = nat_list(&artifact.out_regs),
        call = validate_call(name, &format!(" {name}Instrs"), &format!("{name}OutRegs")),
    );
    for (suffix, change, instrs, out_regs) in &artifact.mutations {
        text.push_str(&format!(
            "\n/-- Refused: {change}. -/\n\
             theorem {name}_{suffix}_refused :\n    {call} = false := by\n  decide\n",
            call = validate_call(
                name,
                &term_list(instrs).replace('\n', "\n    "),
                &nat_list(out_regs)
            ),
        ));
    }
    for (index, (inputs, outputs)) in artifact.executions.iter().enumerate() {
        let values: Vec<String> = inputs
            .iter()
            .map(|value| hex(value.cast_unsigned()))
            .collect();
        text.push_str(&format!(
            "\n/-- The Rust packed machine, agreeing with its reference interpreter here, computed \
             these outputs, and the kernel evaluates `run` to the same. -/\n\
             theorem {name}_execution_{index}_agrees :\n    \
             runOutputs (inputsOf [{values}]) {name}Instrs {name}OutRegs =\n      \
             [{outputs}] := by\n  decide\n",
            values = values.join(", "),
            outputs = outputs.join(", "),
        ));
    }
    text
}

pub(crate) const HEADER: &str = "import NobleFoundation.FirstProjection

/-!
# Real packed I64/Bool artifacts, validated by the Lean kernel (issue #43)

Generated by `crates/noble-ngraph/tests/schedule_lean_dir/corpus.rs` from the declarations and
streams `Compiled::new` emits; do not edit by hand. `the_lean_file_is_the_real_emission` fails
when the emission changes, until this file is regenerated and the foundation lane rerun.

For each artifact the kernel decides `validate` (`*_validated`), and `validate_sound` gives the
semantic theorem `*_sound`. Every `*_refused` theorem is a copy of the same artifact with one
obligation broken, and the kernel decides that `validate` refuses it. Every `*_agrees` theorem
states the outputs the Rust packed machine computed at some inputs, after the generator checked
them against the Rust reference interpreter, and the kernel evaluates `run` to the same outputs;
by `*_sound` they are also the graph reference's.

The exporter is untrusted: it claims which node each instruction computes, and a wrong claim is
refused. The one trusted translation is the preload: the packed machine loads the slot and constant
tables into their registers before the first layer, written here as `setInput`/`setConst` at their
packed positions. This is per-artifact validation of these artifacts, not a proof about the
scheduler, the Rust executor, the host or the engine.
-/

namespace NobleFoundation.PackedScheduleCorpus

open NobleFoundation.PackedSchedule

/-- The register map a finite list denotes. -/
def regOfList (regs : List Nat) (index : Nat) : Nat :=
  regs.getD index 0

/-- The inputs a finite list of slot values denotes. -/
def inputsOf (values : List (BitVec 64)) (slot : Nat) : BitVec 64 :=
  values.getD slot 0

";
