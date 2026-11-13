#![allow(
    tigerstyle::assertion_density,
    tigerstyle::missing_const_fn,
    tigerstyle::no_panic,
    tigerstyle::no_unwrap,
    tigerstyle::non_trait_imports,
    tigerstyle::unbounded_collection_growth,
    reason = "Owner: noble-maintainers; the Lean text of small fixed fixtures is built by string concatenation and must stop at anything outside the exported slice, so expect/panic on unsupported input and owner-path imports are the readable form."
)]
//! Lean text for the real packed artifacts exported by the adjacent corpus modules (issue #43).

use noble_ngraph::ir::{Fact, Graph, Literal, Op, ScalarType, SlotKind};
use noble_ngraph::schedule::I64Instruction;
use noble_ngraph::sem::Value;

/// A refused copy: theorem suffix, what was changed, and the changed instructions and output
/// registers.
pub(crate) type Mutation = (&'static str, &'static str, Vec<String>, Vec<u32>);

/// One execution of the real packed machine: its I64 inputs, by slot, and its outputs as Lean terms.
pub(crate) type Execution = (Vec<i64>, Vec<String>);

/// A guard-fixture execution: its inputs and the observed Lean model constructor.
pub(crate) type GuardExecution = (Vec<i64>, String);

/// One real artifact in Lean terms.
pub(crate) struct Artifact {
    pub(crate) name: &'static str,
    pub(crate) doc: &'static str,
    pub(crate) nodes: Vec<String>,
    pub(crate) regs: Vec<u32>,
    pub(crate) order: Vec<u32>,
    pub(crate) layers: Vec<Vec<u32>>,
    pub(crate) suffix_min: Vec<Option<u32>>,
    pub(crate) layer_guard_order: Vec<u32>,
    pub(crate) layer_order_witness: Option<Vec<i64>>,
    pub(crate) instrs: Vec<String>,
    pub(crate) outputs: Vec<u32>,
    pub(crate) out_regs: Vec<u32>,
    pub(crate) mutations: Vec<Mutation>,
    pub(crate) executions: Vec<Execution>,
    pub(crate) is_guarded: bool,
    pub(crate) guarded_executions: Vec<GuardExecution>,
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
        Op::Guard {
            fact: Fact::Bit,
            value,
        } => format!(".guard {}", value.0),
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
        I64Instruction::Guard { node } => format!(".guard {node}"),
    }
}

pub(crate) fn nat_list(items: &[u32]) -> String {
    let parts: Vec<String> = items.iter().map(u32::to_string).collect();
    format!("[{}]", parts.join(", "))
}

pub(crate) fn layer_list(layers: &[Vec<u32>]) -> String {
    let parts: Vec<String> = layers.iter().map(|layer| nat_list(layer)).collect();
    format!("[{}]", parts.join(", "))
}

pub(crate) fn suffix_list(items: &[Option<u32>]) -> String {
    let parts: Vec<String> = items
        .iter()
        .map(|value| match value {
            Some(index) => format!("some {index}"),
            None => String::from("none"),
        })
        .collect();
    format!("[{}]", parts.join(", "))
}

/// A multi-line Lean list, one element per line.
pub(crate) fn term_list(items: &[String]) -> String {
    format!("\n  [{}]", items.join(",\n   "))
}

fn validate_call(name: &str, instrs: &str, out_regs: &str) -> String {
    format!("validate {name}Graph (regOfList {name}Regs) {name}Order{instrs}\n      {name}Outputs {out_regs}")
}

pub(crate) fn artifact_text(artifact: &Artifact) -> String {
    if artifact.is_guarded {
        return super::emit_guard::artifact_text(artifact);
    }
    let name = artifact.name;
    let output = artifact.outputs[0];
    let mut text = format!(
        r##"/-! ## `{name}`: {doc} -/

def {name}Nodes : List Node :={nodes}

def {name}Graph : Graph := FirstProjection.ofList {name}Nodes {output}

def {name}Regs : List Nat := {regs}

def {name}Order : List Nat := {order}

def {name}Layers : List (List Nat) := {layers}

def {name}FailSuffixMin : List (Option Nat) := {suffix_min}

def {name}LayerGuardOrder : List Nat := {layer_guard_order}

def {name}Instrs : List Instr :={instrs}

def {name}Outputs : List Nat := {outputs}

def {name}OutRegs : List Nat := {out_regs}

theorem {name}_validated :
    {call} = true := by
  decide

theorem {name}_wellFormed : WellFormed {name}Graph :=
  FirstProjection.ofList_wellFormed {name}Nodes {output} (by decide) (by decide)

/-- The exported stream returns the graph reference at every output, for every input. -/
theorem {name}_sound (inputs : Nat → BitVec 64) :
    runOutputs inputs {name}Instrs {name}OutRegs =
      referenceOutputs {name}Graph inputs {name}Outputs :=
  validate_sound {name}Graph {name}_wellFormed (regOfList {name}Regs) {name}Order
    {name}Instrs {name}Outputs {name}OutRegs {name}_validated inputs
"##,
        doc = artifact.doc,
        nodes = term_list(&artifact.nodes),
        regs = nat_list(&artifact.regs),
        order = nat_list(&artifact.order),
        layers = layer_list(&artifact.layers),
        suffix_min = suffix_list(&artifact.suffix_min),
        layer_guard_order = nat_list(&artifact.layer_guard_order),
        instrs = term_list(&artifact.instrs),
        outputs = nat_list(&artifact.outputs),
        out_regs = nat_list(&artifact.out_regs),
        call = validate_call(name, &format!(" {name}Instrs"), &format!("{name}OutRegs")),
    );
    text.push_str(&mutations_text(artifact));
    text.push_str(&executions_text(artifact));
    text
}

fn mutations_text(artifact: &Artifact) -> String {
    let name = artifact.name;
    artifact
        .mutations
        .iter()
        .map(|(suffix, change, instrs, out_regs)| {
            format!(
            "\n/-- Refused: {change}. -/\ntheorem {name}_{suffix}_refused :\n    {call} = false := by\n  decide\n",
            call = validate_call(
                name,
                &term_list(instrs).replace('\n', "\n    "),
                &nat_list(out_regs)
            ),
            )
        })
        .collect()
}

fn executions_text(artifact: &Artifact) -> String {
    let name = artifact.name;
    let mut text = String::new();
    for (index, (inputs, outputs)) in artifact.executions.iter().enumerate() {
        let values: Vec<String> = inputs
            .iter()
            .map(|value| hex(value.cast_unsigned()))
            .collect();
        text.push_str(&format!(
            r##"
/-- The Rust packed machine, agreeing with its reference interpreter here, computed these
outputs, and the kernel evaluates `run` to the same. -/
theorem {name}_execution_{index}_agrees :
    runOutputs (inputsOf [{values}]) {name}Instrs {name}OutRegs =
      [{outputs}] := by
  decide
"##,
            values = values.join(", "),
            outputs = outputs.join(", "),
        ));
    }
    text
}

pub(crate) const HEADER: &str = r##"import NobleFoundation.FirstProjection

/-!
# Real packed I64/Bool artifacts, validated by the Lean kernel (issue #43)

Generated by `corpus.rs` from the declarations and streams `Compiled::new` emits; do not edit by
hand. The guarded artifact is generated in `PackedScheduleGuardCorpus.lean`.

For each artifact the kernel decides `validate` (`*_validated`), and `validate_sound` gives the
semantic theorem `*_sound`. Every `*_refused` theorem is a copy of the same artifact with one
obligation broken, and the kernel decides that `validate` refuses it. Every `*_agrees` theorem
states outputs the Rust packed machine computed at an input that the generator checked against the
Rust reference interpreter; the kernel evaluates `run` to the same outputs.

The exporter is untrusted: it claims which node each instruction computes, and a wrong claim is
refused. The packed machine preloads slot and constant tables into registers before the first layer;
those entries are exported as `setInput`/`setConst` at their packed positions. This validates these
artifacts, not the scheduler, Rust executor, host or engine.
-/

namespace NobleFoundation.PackedScheduleCorpus

open NobleFoundation.PackedSchedule

/-- Version of the generated packed-artifact Lean export vocabulary. -/
def packedExportTag : String := "ngraph-packed-export/0"

/-- The register map a finite list denotes. -/
def regOfList (regs : List Nat) (index : Nat) : Nat :=
  regs.getD index 0

/-- The inputs a finite list of slot values denotes. -/
def inputsOf (values : List (BitVec 64)) (slot : Nat) : BitVec 64 :=
  values.getD slot 0

"##;
