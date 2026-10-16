//! Black-box tests of the `noble ngraph` CLI (`Numeric-Graph-v1`).
//!
//! Every expectation is derived from `.cairn/changes/numeric-graph-v1`:
//! design D4–D11, the NGRAPH-CLI-01, NGRAPH-FORMAT-01 and NGRAPH-SNAP-02
//! deltas, and the draft cases NGRAPH-CASE-03..05, -07, -08 and -10..13. Only
//! stable contract fields are asserted: `detail` is informative (D8), and
//! identities that the design does not record are checked for their format
//! and for equality across commands.

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::Value;

type TestResult = Result<(), Box<dyn std::error::Error>>;

// ---------------------------------------------------------------------------
// Graph files (D4 `ngraph-ir/0`). Every fixture is written in the printed
// record order: schema, subject, shapes, slots, nodes, facts, outputs.
// ---------------------------------------------------------------------------

// Authored check fixture (`authored-name` sugar, D4): 2 `f32` inputs, no
// params, 4 live nodes, 3 layers (sources at 0; `%2` at 1; `%3` at 2) and
// 3 fact records: two input-validated ranges and a `finite` fact the checker
// derives from them (`[-1,1] × [-1,1]`).
const CHECK_GRAPH: &str = "\
ngraph-ir/0 ngraph-f32/0
subject authored-name cli-check-demo
slot input f32
slot input f32
%0 = input 0
%1 = input 1
%2 = f32.mul %0 %1
%3 = f32.add %2 %0
fact %0 range 0xbf800000 0x3f800000 input-validated
fact %1 range 0xbf800000 0x3f800000 input-validated
fact %2 finite derived interval
out %3
";

// `Subject::authored("cli-check-demo")` in canonical hex: BLAKE3 of
// `noble.ngraph.subject.v0` ‖ `authored` ‖ u64le(len) ‖ name
// (`Subject::derive`; D3 keeps every identity byte-identical, and the same
// derivation reproduces the recorded F1 identity below).
const CHECK_SUBJECT: &str = "2b5c1bc0789c09d102fcc38185cf360bd64ab7d3fed77de821f4f89efb773b0a";

// Body shared by the subject-identity variants (one `f32` input, `relu`).
const SUGAR_BODY: &str = "\
slot input f32
%0 = input 0
%1 = f32.relu %0
out %1
";

// Token of the `authored-name` sugar variant of `SUGAR_BODY`.
const SUGAR_NAME: &str = "cli-sugar-demo";

// `Subject::authored("cli-sugar-demo")` in canonical hex.
const SUGAR_SUBJECT: &str = "a70a0ec7390d9a72c9376d49aaf980d33a49f88cecbd9b9facb0e11108c7af4f";

// F1's subject, `Subject::authored("issue14/F1 2-3-1 ReGLU")`, in canonical
// hex. The name contains spaces, so only the canonical form can spell it.
const F1_SUBJECT: &str = "816941950e745b2a722e1fb6ca1b2471559d9993966e89d75ac7f7914d31effe";

// NGRAPH-CASE-03: the recorded `GraphLogicalId` of F1.
const F1_GRAPH: &str = "6065e93ff4262398603bd7c25392b6461335dbc24867024f25910cee0758d661";

// F1 (the Phase B fixture) in the canonical printed form of D4: a 2→3→1
// ReGLU network. Inputs are slots 0 and 1; the 22 weights are `param` slots
// 2..=23 (per hidden unit: left w0, w1, bias, gate w0, w1, bias; then the
// output weights v0..v2 and bias). Each pre-activation is
// `rn(rn(bias + rn(w0·x0)) + rn(w1·x1))`; 57 nodes and 9 layers in all.
fn f1_text() -> String {
    fn push(nodes: &mut Vec<String>, op: String) -> usize {
        nodes.push(op);
        nodes.len() - 1
    }
    fn affine(nodes: &mut Vec<String>, w0: usize, w1: usize, bias: usize) -> usize {
        let p0 = push(nodes, format!("f32.mul %{w0} %0"));
        let s0 = push(nodes, format!("f32.add %{bias} %{p0}"));
        let p1 = push(nodes, format!("f32.mul %{w1} %1"));
        push(nodes, format!("f32.add %{s0} %{p1}"))
    }
    let mut nodes = vec![String::from("input 0"), String::from("input 1")];
    nodes.extend((2..24).map(|slot| format!("param {slot}")));
    let mut hidden = Vec::new();
    for unit in 0..3 {
        let q = 2 + 6 * unit;
        let left = affine(&mut nodes, q, q + 1, q + 2);
        let gate = affine(&mut nodes, q + 3, q + 4, q + 5);
        hidden.push(push(&mut nodes, format!("f32.reglu %{left} %{gate}")));
    }
    let mut acc = 23;
    for (j, h) in hidden.into_iter().enumerate() {
        let t = push(&mut nodes, format!("f32.mul %{} %{h}", 20 + j));
        acc = push(&mut nodes, format!("f32.add %{acc} %{t}"));
    }
    let mut lines = vec![
        String::from("ngraph-ir/0 ngraph-f32/0"),
        format!("subject authored {F1_SUBJECT} closure -"),
    ];
    lines.extend(std::iter::repeat_n(String::from("slot input f32"), 2));
    lines.extend(std::iter::repeat_n(String::from("slot param f32"), 22));
    lines.extend(
        nodes
            .iter()
            .enumerate()
            .map(|(i, op)| format!("%{i} = {op}")),
    );
    lines.push(format!("out %{acc}"));
    lines.join("\n") + "\n"
}

// NGRAPH-CASE-07 evaluation probes: three `f32` inputs and six outputs,
// `mul`, `rn(mul + x2)` (the FMA probe), the stored-order `affine` fold
// `rn(rn(rn(-0 + x0) + x1) + x2)` (the reassociation probe), `reglu`, `relu`
// and a lone-term `affine(-0; 1·x2)`.
const PROBE_GRAPH: &str = "\
ngraph-ir/0 ngraph-f32/0
subject authored-name cli-eval-probes
slot input f32
slot input f32
slot input f32
%0 = input 0
%1 = input 1
%2 = input 2
%3 = f32.mul %0 %1
%4 = f32.add %3 %2
%5 = f32.affine 0x80000000 0x3f800000 %0 0x3f800000 %1 0x3f800000 %2
%6 = f32.reglu %0 %1
%7 = f32.relu %0
%8 = f32.affine 0x80000000 0x3f800000 %2
out %3
out %4
out %5
out %6
out %7
out %8
";

// Vectors for `PROBE_GRAPH`; `0x` bit tokens and exact decimals are mixed on
// purpose (D5), including `-0` and a NaN with a payload.
const PROBE_VECTORS: &str = "\
noble-ngraph-vectors/0
types f32 f32 f32
v 0x4b800000 0x3f800000 0x3f800000
v 0x3f800800 0x3f800800 0xbf801000
v 0x80000000 0x00000000 -0
v 0xbf800000 0 0x00000000
v 0x7f800000 0x00000000 0x00000000
v 0x7f000000 2 0x80000000
v 0x00800000 0.5 0x80000000
v 1.5 -0.5 0.25
v 0x7fc00001 1 1
";

// Expected outputs of `PROBE_VECTORS` (out %3 %4 %5 %6 %7 %8), by exact
// binary32 arithmetic with roundTiesToEven, no FMA and no flush to zero.
const PROBE_EXPECTED: [[&str; 6]; 9] = [
    // (2^24, 1, 1). mul 2^24·1 = 2^24 (0x4b800000). add: 2^24 + 1 is a tie
    // between 2^24 (even significand) and 2^24 + 2, so 2^24. affine folds in
    // stored order: -0 + 2^24 = 2^24, then each +1 ties back to 2^24
    // (reassociating (1 + 1) + 2^24 would give 0x4b800001). reglu 2^24·relu(1)
    // = 2^24, relu(2^24) = 2^24, lone term -0 + 1·1 = 1 (0x3f800000).
    [
        "f32:0x4b800000",
        "f32:0x4b800000",
        "f32:0x4b800000",
        "f32:0x4b800000",
        "f32:0x4b800000",
        "f32:0x3f800000",
    ],
    // a = 1 + 2^-12 (0x3f800800), c = -(1 + 2^-11) (0xbf801000).
    // mul: a·a = 1 + 2^-11 + 2^-24 is a tie at half an ulp (2^-24); the even
    // neighbour is 1 + 2^-11 (0x3f801000). add: (1 + 2^-11) + c = +0 exactly
    // (an FMA would give 2^-24 = 0x33800000). affine: -0 + a = a, a + a =
    // 2 + 2^-11 (exact), + c = 1 (0x3f800000). reglu a·relu(a) = a·a
    // (0x3f801000), relu(a) = a, lone term -0 + 1·c = c.
    [
        "f32:0x3f801000",
        "f32:0x00000000",
        "f32:0x3f800000",
        "f32:0x3f801000",
        "f32:0x3f800800",
        "f32:0xbf801000",
    ],
    // (-0, +0, -0). mul -0·+0 = -0. add -0 + -0 = -0. affine: -0 + -0 = -0,
    // -0 + +0 = +0, +0 + -0 = +0. reglu -0·relu(+0) = -0·+0 = -0.
    // relu(-0) = +0. lone term: -0 + 1·-0 = -0 (0x80000000).
    [
        "f32:0x80000000",
        "f32:0x80000000",
        "f32:0x00000000",
        "f32:0x80000000",
        "f32:0x00000000",
        "f32:0x80000000",
    ],
    // (-1, +0, +0). mul -1·+0 = -0. add -0 + +0 = +0. affine -0 + -1 = -1,
    // then + 0 twice = -1 (0xbf800000). reglu(-1, 0) = -1·relu(+0) = -0.
    // relu(-1) = +0. lone term -0 + 1·+0 = +0.
    [
        "f32:0x80000000",
        "f32:0x00000000",
        "f32:0xbf800000",
        "f32:0x80000000",
        "f32:0x00000000",
        "f32:0x00000000",
    ],
    // (+inf, +0, +0). mul inf·+0 = NaN. add NaN + 0 = NaN. affine -0 + inf
    // = inf, + 0 twice = inf (0x7f800000). reglu(inf, 0) = inf·+0 = NaN.
    // relu(inf) = inf. lone term +0. Every NaN renders as the one class.
    [
        "f32:nan",
        "f32:nan",
        "f32:0x7f800000",
        "f32:nan",
        "f32:0x7f800000",
        "f32:0x00000000",
    ],
    // (2^127, 2, -0). mul 2^127·2 = 2^128 overflows to +inf (0x7f800000).
    // add inf + -0 = inf. affine -0 + 2^127 = 2^127, + 2 is below half an
    // ulp (2^103) so 2^127, + -0 = 2^127 (0x7f000000). reglu 2^127·relu(2)
    // = +inf. relu(2^127) = 2^127. lone term -0 + 1·-0 = -0.
    [
        "f32:0x7f800000",
        "f32:0x7f800000",
        "f32:0x7f000000",
        "f32:0x7f800000",
        "f32:0x7f000000",
        "f32:0x80000000",
    ],
    // (2^-126, 0.5, -0). mul 2^-126·0.5 = 2^-127, the exact subnormal
    // 0x00400000 (no flush to zero). add 2^-127 + -0 = 2^-127. affine -0 +
    // 2^-126 = 2^-126, + 0.5 rounds to 0.5 (0x3f000000), + -0 = 0.5. reglu
    // 2^-126·relu(0.5) = 2^-127. relu(2^-126) = 2^-126 (0x00800000).
    // lone term -0.
    [
        "f32:0x00400000",
        "f32:0x00400000",
        "f32:0x3f000000",
        "f32:0x00400000",
        "f32:0x00800000",
        "f32:0x80000000",
    ],
    // (1.5, -0.5, 0.25) as exact decimals. mul -0.75 (0xbf400000). add
    // -0.75 + 0.25 = -0.5 (0xbf000000). affine -0 + 1.5 = 1.5, - 0.5 = 1,
    // + 0.25 = 1.25 (0x3fa00000). reglu 1.5·relu(-0.5) = 1.5·+0 = +0.
    // relu(1.5) = 1.5 (0x3fc00000). lone term 0.25 (0x3e800000).
    [
        "f32:0xbf400000",
        "f32:0xbf000000",
        "f32:0x3fa00000",
        "f32:0x00000000",
        "f32:0x3fc00000",
        "f32:0x3e800000",
    ],
    // (NaN with payload 0x7fc00001, 1, 1): every result that reads x0 is a
    // NaN and renders as `f32:nan`; the lone term is -0 + 1·1 = 1.
    [
        "f32:nan",
        "f32:nan",
        "f32:nan",
        "f32:nan",
        "f32:nan",
        "f32:0x3f800000",
    ],
];

// NGRAPH-CASE-08 outcome fixture: `guard %0 bit` (a runtime guard) and an
// input-validated range `[0, 1]` on `%1`; output `rn(x0 + x1)`.
const GUARD_GRAPH: &str = "\
ngraph-ir/0 ngraph-f32/0
subject authored-name cli-eval-guard
slot input f32
slot input f32
%0 = input 0
%1 = input 1
%2 = guard %0 bit
%3 = f32.add %0 %1
fact %1 range 0x00000000 0x3f800000 input-validated
out %3
";

// Guard rows. Normal: x0 = 1 is a bit, x1 = 0.5 is in range; 1 + 0.5 = 1.5.
const GUARD_NORMAL: &str = "v 1 0.5";
// The normal row's output.
const GUARD_NORMAL_OUT: &str = "f32:0x3fc00000";
// Second normal row: 0 + 0.25 = 0.25.
const GUARD_NORMAL_2: &str = "v 0 0.25";
// Its output.
const GUARD_NORMAL_2_OUT: &str = "f32:0x3e800000";
// Trap: 0.5 held in the guarded bit (x1 = 0.5 passes its input fact).
const GUARD_TRAP: &str = "v 0.5 0.5";
// Reject: x1 = 2 violates the input-validated range before evaluation.
const GUARD_REJECT: &str = "v 1 2";

// ---------------------------------------------------------------------------
// Training fixtures (D10 loss admission, D6 snapshots).
// ---------------------------------------------------------------------------

// Tiny trainable model `y = rn(rn(w·x) + b)` with `w`, `b` in `param` slots
// 1 and 2 and an input-validated range `x ∈ [-1, 1]`: 5 live nodes, 3 layers.
const MODEL: &str = "\
ngraph-ir/0 ngraph-f32/0
subject authored-name cli-train-model
slot input f32
slot param f32
slot param f32
%0 = input 0
%1 = param 1
%2 = param 2
%3 = f32.mul %1 %0
%4 = f32.add %3 %2
fact %0 range 0xbf800000 0x3f800000 input-validated
out %4
";

// `Subject::authored("cli-train-model")` in canonical hex.
const MODEL_SUBJECT: &str = "bfda41523102888f163c6a34fe8bf061ac391d1f5a14e5a5c9cf716ac0d403df";

// The fact record of `MODEL` (removed to make the changed-facts variant).
const MODEL_FACT: &str = "fact %0 range 0xbf800000 0x3f800000 input-validated\n";

// MSE point loss `(y, t) → rn(d·d)`, `d = rn(y − t)`: no params, inputs =
// the model output then one target, exactly one `f32` output (D10).
const LOSS: &str = "\
ngraph-ir/0 ngraph-f32/0
subject authored-name cli-train-mse
slot input f32
slot input f32
%0 = input 0
%1 = input 1
%2 = f32.sub %0 %1
%3 = f32.mul %2 %2
out %3
";

// Loss with one input too few: model output only, no target slot.
const LOSS_SHORT: &str = "\
ngraph-ir/0 ngraph-f32/0
subject authored-name cli-train-loss-short
slot input f32
%0 = input 0
%1 = f32.mul %0 %0
out %1
";

// Loss with two outputs (D10 requires exactly one `f32` output).
const LOSS_TWO_OUTPUTS: &str = "\
ngraph-ir/0 ngraph-f32/0
subject authored-name cli-train-loss-two
slot input f32
slot input f32
%0 = input 0
%1 = input 1
%2 = f32.sub %0 %1
%3 = f32.mul %2 %2
out %3
out %2
";

// Loss with a `param` slot (D10 forbids params in the loss graph).
const LOSS_WITH_PARAM: &str = "\
ngraph-ir/0 ngraph-f32/0
subject authored-name cli-train-loss-param
slot input f32
slot input f32
slot param f32
%0 = input 0
%1 = input 1
%2 = param 2
%3 = f32.sub %0 %1
%4 = f32.mul %3 %2
out %4
";

// Training data for `MODEL` and `LOSS`: targets on `y = 2x + 1`.
const DATA: &str = "\
noble-ngraph-data/0
types f32 ; f32
point 0 ; 1
point 0.5 ; 2
point -0.5 ; 0
point 1 ; 3
";

// `DATA` with one target changed (a different training-data digest).
const DATA_CHANGED: &str = "\
noble-ngraph-data/0
types f32 ; f32
point 0 ; 1
point 0.5 ; 2
point -0.5 ; 0
point 1 ; 3.5
";

// A point whose input violates `MODEL`'s input-validated range (D10).
const DATA_OUT_OF_RANGE: &str = "\
noble-ngraph-data/0
types f32 ; f32
point 0.5 ; 2
point 0x40000000 ; 5
";

// A point with an infinite target: the loss and gradient are not finite.
const DATA_INFINITE_TARGET: &str = "\
noble-ngraph-data/0
types f32 ; f32
point 0.5 ; 0x7f800000
";

// A param reaches the loss through `step` (`%4`). The guard `%3` gives the
// step operand its checked `[0, 1]` range. With every data input `x = 0`,
// `w·x` is `±0`, so the guard holds for any `w` and the gradient is `0`.
const STEP_MODEL: &str = "\
ngraph-ir/0 ngraph-f32/0
subject authored-name cli-train-step
slot input f32
slot param f32
%0 = input 0
%1 = param 1
%2 = f32.mul %1 %0
%3 = guard %2 range 0x00000000 0x3f800000
%4 = f32.step %2
%5 = f32.add %4 %0
out %5
";

// Data for `STEP_MODEL`: every input is 0.
const STEP_DATA: &str = "\
noble-ngraph-data/0
types f32 ; f32
point 0 ; 1
point 0 ; 0.5
";

// A NaN input makes `w·x` NaN, so the guard fails under any params.
const STEP_DATA_NAN: &str = "\
noble-ngraph-data/0
types f32 ; f32
point 0x7fc00000 ; 1
";

// An `ngraph-i64/0` model (no params exist in that sub-profile).
const I64_MODEL: &str = "\
ngraph-ir/0 ngraph-i64/0
subject authored-name cli-train-i64
slot input i64
%0 = input 0
%1 = const i64 3
%2 = i64.mul %0 %1
out %2
";

// Data for `I64_MODEL` with `LOSS`'s single target.
const I64_DATA: &str = "\
noble-ngraph-data/0
types i64 ; f32
point 2 ; 6
";

// ---------------------------------------------------------------------------
// Core extraction sources (NGRAPH-CASE-05; `noble session` line framing).
// ---------------------------------------------------------------------------

// I1 (`extract_b1.rs`): `y·y + 3x − 7` on `(x, y)`; (5, 4) → 24.
const I1: &str = "dup * swap 3 * + 7 -\n";
// I2: one shared capture of `2`, composed twice; 40 → 44.
const I2: &str = "2 quote [ + ] compose dup compose run\n";
// I3: a definition then the root; two calls of one identity; (3, 4) → 25.
const I3: &str = "def sq [ dup * ]\nsq swap sq +\n";
// I4: the transition `(s, i, k) → (s·k + i, s·k + i)`.
const I4: &str = "swap [ * ] dip + dup\n";
// B3 `test.emit` control with one `i64` input: `( I64 -- I64 ) ! test.emit`.
const EFFECT_SOURCE: &str = "\"hi\" test.emit 1 +\n";
// B3 `if` control with one `i64` input.
const CONTROL_SOURCE: &str = "true [ 1 + ] [ 2 + ] if\n";
// B3 program-output control with one `i64` input: `( I64 -- Program )`.
const PROGRAM_OUTPUT_SOURCE: &str = "drop [ 1 + ]\n";
// A Core frontend refusal: an unbound word.
const UNBOUND_SOURCE: &str = "1 nosuchword +\n";

// ---------------------------------------------------------------------------
// Training contract and snapshot constants (D6, D7).
// ---------------------------------------------------------------------------

const POLICY: &str = "ngraph-deriv-real/0";
const OPTIMIZER: &str = "adam/0";
// The B5 seed 0x14140005, given in decimal; D6 records it as 16 hex digits.
const SEED: &str = "336855045";
const SEED_RECORD: &str = "0x0000000014140005";
// `adam/0` defaults 0.02, 0.9, 0.999 and 1e-8 as binary64 bits (D7).
const ADAM_LR: &str = "0x3f947ae147ae147b";
const ADAM_BETA1: &str = "0x3feccccccccccccd";
const ADAM_BETA2: &str = "0x3feff7ced916872b";
const ADAM_EPS: &str = "0x3e45798ee2308c3a";
const TOOLS: &str = "noble-ngraph/0; ngraph-ir/0; ngraph-sem/0; ngraph-facts/0; ngraph-layered/0";

// D6 record keys in their printed order.
const SNAPSHOT_KEYS: [&str; 19] = [
    "snapshot",
    "graph",
    "layout",
    "param",
    "optimizer-state",
    "parent",
    "subject",
    "profile",
    "facts",
    "loss",
    "data",
    "policy",
    "opt-in",
    "seed",
    "prng",
    "optimizer",
    "steps",
    "tools",
    "provenance",
];

// ---------------------------------------------------------------------------
// Report contract (D7, D8).
// ---------------------------------------------------------------------------

const REPORT_SCHEMA: &str = "noble-ngraph-report/v1";

const STAGES: [&str; 10] = [
    "request",
    "input",
    "admission",
    "extraction",
    "check",
    "binding",
    "evaluation",
    "training",
    "replay",
    "output",
];

const TRAP_CODES: [&str; 3] = ["G-FACT-FALSE", "E-NONFINITE-UPDATE", "C-CANCELLED"];

const REJECT_CODES: [&str; 17] = [
    "E-GRAPH-MALFORMED",
    "E-FORMAT",
    "E-LITERAL-INEXACT",
    "E-LITERAL-NONFINITE",
    "E-TYPE-MIX",
    "E-INTERFACE",
    "E-SUBJECT",
    "E-CLOSURE",
    "E-PROFILE",
    "E-FACT-UNCHECKED",
    "E-FACT-INPUT",
    "E-PARAM-UNDECLARED",
    "E-SNAPSHOT-STALE",
    "E-SNAPSHOT-LAYOUT",
    "E-SNAPSHOT-ID",
    "E-SNAPSHOT-INTEGRITY",
    "E-SNAPSHOT-REPLAY",
];

const UNSUPPORTED_CODES: [&str; 12] = [
    "E-EFFECT",
    "E-RESOURCE",
    "E-DYNAMIC-RUN",
    "E-LIVE-SLOT",
    "E-RECURSION",
    "E-CONTROL",
    "E-AGGREGATE",
    "E-REFLECT",
    "E-PROGRAM-OUTPUT",
    "E-DERIV-BOOLEAN",
    "E-DERIV-INDEX",
    "E-DERIV-I64",
];

// `E-BUDGET-<kind>` for every D9 kind, plus the runtime `FUEL`.
const BUDGET_CODES: [&str; 17] = [
    "E-BUDGET-WORK",
    "E-BUDGET-EXPANSION",
    "E-BUDGET-INLINE-DEPTH",
    "E-BUDGET-LIVE-NODES",
    "E-BUDGET-LAYERS",
    "E-BUDGET-INPUTS",
    "E-BUDGET-OUTPUTS",
    "E-BUDGET-AFFINE-TERMS",
    "E-BUDGET-TABLE-LENGTH",
    "E-BUDGET-TABLE-ENTRIES",
    "E-BUDGET-FACTS",
    "E-BUDGET-GRAPH-BYTES",
    "E-BUDGET-SNAPSHOT-BYTES",
    "E-BUDGET-FORWARD-SCRATCH",
    "E-BUDGET-BACKWARD-SCRATCH",
    "E-BUDGET-STEPS",
    "E-BUDGET-FUEL",
];

// D7 `check` fields; the `extract` report repeats them.
const CHECK_FIELDS: [&str; 11] = [
    "graph",
    "profile",
    "subject",
    "closure",
    "inputs",
    "params",
    "outputs",
    "live_nodes",
    "layers",
    "facts",
    "param_layout",
];

// D7 extraction counters of the `extract` report.
const EXTRACTION_FIELDS: [&str; 4] = [
    "work_units",
    "expanded_nodes",
    "pruned_nodes",
    "inline_depth",
];

// D8: the outcome class of a stable code.
fn outcome_class(code: &str) -> &'static str {
    if TRAP_CODES.contains(&code) {
        "trap"
    } else if REJECT_CODES.contains(&code) {
        "reject"
    } else if UNSUPPORTED_CODES.contains(&code) {
        "unsupported"
    } else if BUDGET_CODES.contains(&code) {
        "exhausted"
    } else {
        panic!("{code} is not a D8 code")
    }
}

// D8: the process exit of an outcome class.
fn exit_of(outcome: &str) -> i32 {
    match outcome {
        "normal" => 0,
        "trap" => 1,
        "reject" => 2,
        "unsupported" => 4,
        "exhausted" => 5,
        other => panic!("{other:?} is not a D8 outcome"),
    }
}

fn is_lower_hex(byte: u8) -> bool {
    byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)
}

fn is_hex64(text: &str) -> bool {
    text.len() == 64 && text.bytes().all(is_lower_hex)
}

fn hex64(value: &Value) -> bool {
    value.as_str().is_some_and(is_hex64)
}

// D7 value rendering: `f32:0x%08x` for non-NaN encodings, `f32:nan` for
// every NaN, `i64:<decimal>`, `bool:true|false`.
fn is_rendered_value(text: &str) -> bool {
    if let Some(bits) = text.strip_prefix("f32:0x") {
        bits.len() == 8
            && bits.bytes().all(is_lower_hex)
            && u32::from_str_radix(bits, 16).is_ok_and(|bits| !f32::from_bits(bits).is_nan())
    } else if let Some(decimal) = text.strip_prefix("i64:") {
        decimal
            .parse::<i64>()
            .is_ok_and(|value| value.to_string() == decimal)
    } else {
        matches!(text, "f32:nan" | "bool:true" | "bool:false")
    }
}

fn render_f32(value: f32) -> String {
    if value.is_nan() {
        String::from("f32:nan")
    } else {
        format!("f32:0x{:08x}", value.to_bits())
    }
}

// The number of fact records in a `facts` field (a count or a list).
fn fact_count(facts: &Value) -> Option<u64> {
    facts.as_u64().or_else(|| {
        facts
            .as_array()
            .and_then(|records| u64::try_from(records.len()).ok())
    })
}

// ---------------------------------------------------------------------------
// Process harness.
// ---------------------------------------------------------------------------

static NEXT_SCRATCH: AtomicU64 = AtomicU64::new(0);

// A per-test directory under `std::env::temp_dir()`, removed on drop.
struct Scratch {
    root: PathBuf,
}

impl Scratch {
    fn new(test: &str) -> std::io::Result<Self> {
        let serial = NEXT_SCRATCH.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "noble-ngraph-{}-{test}-{serial}",
            std::process::id()
        ));
        if root.exists() {
            std::fs::remove_dir_all(&root)?;
        }
        std::fs::create_dir_all(&root)?;
        Ok(Self { root })
    }

    fn path(&self, name: &str) -> PathBuf {
        self.root.join(name)
    }

    fn write(&self, name: &str, contents: &str) -> std::io::Result<PathBuf> {
        let path = self.path(name);
        std::fs::write(&path, contents)?;
        Ok(path)
    }

    // Runs one command. D11: no report line prints a file path.
    fn run(&self, command: &Ngraph) -> std::io::Result<Run> {
        let run = command.run()?;
        let root = self.root.to_string_lossy();
        assert!(
            !String::from_utf8_lossy(&run.stdout).contains(root.as_ref()),
            "D11: reports print no file path\n{}",
            run.describe()
        );
        Ok(run)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

// One finished `noble` process.
struct Run {
    exit: Option<i32>,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

impl Run {
    fn describe(&self) -> String {
        format!(
            "exit {:?}\nstdout:\n{}\nstderr:\n{}",
            self.exit,
            String::from_utf8_lossy(&self.stdout),
            String::from_utf8_lossy(&self.stderr)
        )
    }

    fn reports(&self) -> Result<Vec<Value>, Box<dyn std::error::Error>> {
        let mut reports = Vec::new();
        for line in std::str::from_utf8(&self.stdout)?.lines() {
            reports.push(serde_json::from_str(line)?);
        }
        Ok(reports)
    }
}

// `noble ngraph …` with its arguments.
#[derive(Clone)]
struct Ngraph {
    args: Vec<OsString>,
}

impl Ngraph {
    fn new(subcommand: &str) -> Self {
        Self {
            args: vec![OsString::from("ngraph"), OsString::from(subcommand)],
        }
    }

    fn bare() -> Self {
        Self {
            args: vec![OsString::from("ngraph")],
        }
    }

    fn arg<A: AsRef<OsStr> + ?Sized>(mut self, arg: &A) -> Self {
        self.args.push(arg.as_ref().to_owned());
        self
    }

    fn run(&self) -> std::io::Result<Run> {
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_noble"))
            .args(&self.args)
            .output()?;
        Ok(Run {
            exit: output.status.code(),
            stdout: output.stdout,
            stderr: output.stderr,
        })
    }
}

fn with_flags(mut command: Ngraph, flags: &[(&str, OsString)]) -> Ngraph {
    for (flag, value) in flags {
        command = command.arg(*flag).arg(value);
    }
    command
}

// ---------------------------------------------------------------------------
// Report assertions.
// ---------------------------------------------------------------------------

// D8: the fields of every report line and their consistency.
fn check_envelope(line: &Value, command: &str) {
    for key in ["schema", "command", "stage", "outcome", "code", "site"] {
        assert!(
            line.get(key).is_some(),
            "D8: every line has `{key}`: {line}"
        );
    }
    assert_eq!(line["schema"], REPORT_SCHEMA, "{line}");
    assert_eq!(line["command"], command, "{line}");
    let stage = line["stage"].as_str().unwrap_or_default();
    assert!(STAGES.contains(&stage), "D8 stage: {line}");
    let outcome = line["outcome"].as_str().unwrap_or_default();
    assert!(
        matches!(
            outcome,
            "normal" | "trap" | "reject" | "unsupported" | "exhausted"
        ),
        "D8 outcome: {line}"
    );
    assert!(line["site"].is_null() || line["site"].is_string(), "{line}");
    if let Some(detail) = line.get("detail") {
        assert!(detail.is_string(), "{line}");
    }
    if let Some(code) = line["code"].as_str() {
        assert_eq!(outcome, outcome_class(code), "D8 class of {code}: {line}");
        // Runtime fuel exhaustion is an observation, not a diagnosed refusal,
        // so its site may be null.
        assert!(
            line["site"].is_string() || code == "E-BUDGET-FUEL",
            "NGRAPH-CLI-01: a refusal carries a stable site: {line}"
        );
    } else {
        assert!(line["code"].is_null(), "{line}");
        assert!(
            matches!(outcome, "normal" | "reject"),
            "only normal lines and host refusals have no code: {line}"
        );
    }
}

// The single report line of `check`, `extract`, `train` or `verify`; the
// process exit follows its outcome.
fn single_report(run: &Run, command: &str) -> Result<Value, Box<dyn std::error::Error>> {
    let mut reports = run.reports()?;
    assert_eq!(reports.len(), 1, "one report line\n{}", run.describe());
    let line = reports.remove(0);
    check_envelope(&line, command);
    let outcome = line["outcome"].as_str().unwrap_or_default();
    assert_eq!(
        run.exit,
        Some(exit_of(outcome)),
        "D8 exit\n{}",
        run.describe()
    );
    Ok(line)
}

fn expect_normal(run: &Run, command: &str) -> Result<Value, Box<dyn std::error::Error>> {
    let line = single_report(run, command)?;
    assert_eq!(line["outcome"], "normal", "{}", run.describe());
    Ok(line)
}

fn expect_code(run: &Run, command: &str, code: &str) -> Result<Value, Box<dyn std::error::Error>> {
    let line = single_report(run, command)?;
    assert_eq!(line["code"], code, "{}", run.describe());
    Ok(line)
}

// D8 host condition (I/O failure, input bound, existing output): `reject`,
// `code: null`, exit 2.
fn expect_host_refusal(run: &Run, command: &str) -> Result<Value, Box<dyn std::error::Error>> {
    let line = single_report(run, command)?;
    assert_eq!(line["outcome"], "reject", "{}", run.describe());
    assert!(line["code"].is_null(), "{}", run.describe());
    Ok(line)
}

// D7/D8 usage error: usage on stderr, no report line, exit 2.
fn expect_usage(run: &Run) {
    assert_eq!(run.exit, Some(2), "{}", run.describe());
    assert!(
        run.stdout.is_empty(),
        "a usage error prints no report\n{}",
        run.describe()
    );
    assert!(
        String::from_utf8_lossy(&run.stderr).contains("ngraph"),
        "usage on stderr\n{}",
        run.describe()
    );
}

// D7 `eval` output: the header line first, then one line per vector. The
// process exit is that of the first non-normal vector in file order (Q7).
fn eval_lines(run: &Run) -> Result<(Value, Vec<Value>), Box<dyn std::error::Error>> {
    let mut reports = run.reports()?;
    assert!(!reports.is_empty(), "a header line\n{}", run.describe());
    let header = reports.remove(0);
    check_envelope(&header, "eval");
    assert!(
        header["vector"].is_null(),
        "the header comes first: {header}"
    );
    assert_eq!(header["outcome"], "normal", "{header}");
    assert!(hex64(&header["graph"]), "{header}");
    assert!(header.get("snapshot").is_some(), "{header}");
    for (index, line) in reports.iter().enumerate() {
        check_envelope(line, "eval");
        assert!(!line["vector"].is_null(), "a vector line: {line}");
        assert!(
            reports[..index]
                .iter()
                .all(|earlier| earlier["vector"] != line["vector"]),
            "each vector line names its own vector: {line}"
        );
        if line["outcome"] == "normal" {
            assert!(
                line["outputs"].as_array().is_some_and(|outputs| outputs
                    .iter()
                    .all(|output| output.as_str().is_some_and(is_rendered_value))),
                "D7 outputs: {line}"
            );
        } else {
            assert!(line["outputs"].is_null(), "no outputs: {line}");
        }
    }
    let first = reports
        .iter()
        .map(|line| exit_of(line["outcome"].as_str().unwrap_or_default()))
        .find(|exit| *exit != 0)
        .unwrap_or(0);
    assert_eq!(run.exit, Some(first), "D8 eval exit\n{}", run.describe());
    Ok((header, reports))
}

// An `eval` refused before any vector is evaluated: no vector line, and the
// refusal is the last line and sets the exit. A graph-level refusal (checker
// or snapshot) is the only line (D8); a refusal of the vector file may follow
// a header line.
fn eval_refusal(
    run: &Run,
    code: &str,
    graph_level: bool,
) -> Result<Value, Box<dyn std::error::Error>> {
    let mut reports = run.reports()?;
    let most = if graph_level { 1 } else { 2 };
    assert!(
        !reports.is_empty() && reports.len() <= most,
        "refused before evaluation\n{}",
        run.describe()
    );
    for line in &reports {
        check_envelope(line, "eval");
        assert!(line["vector"].is_null(), "no vector line: {line}");
    }
    let refusal = reports.pop().ok_or("no refusal line")?;
    if let Some(header) = reports.first() {
        assert_eq!(header["outcome"], "normal", "{header}");
    }
    assert_eq!(refusal["code"], code, "{}", run.describe());
    let outcome = refusal["outcome"].as_str().unwrap_or_default();
    assert_eq!(run.exit, Some(exit_of(outcome)), "{}", run.describe());
    Ok(refusal)
}

// Known D7 `check` values of a graph.
struct Expected<'a> {
    profile: &'a str,
    inputs: &'a [&'a str],
    params: u64,
    outputs: &'a [&'a str],
    live_nodes: u64,
    layers: u64,
    facts: u64,
}

fn check_fields(line: &Value, want: &Expected<'_>) {
    for key in CHECK_FIELDS {
        assert!(line.get(key).is_some(), "D7 `{key}`: {line}");
    }
    assert!(hex64(&line["graph"]), "{line}");
    assert!(hex64(&line["param_layout"]), "{line}");
    assert_eq!(line["profile"], want.profile, "{line}");
    assert_eq!(line["inputs"], Value::from(want.inputs.to_vec()), "{line}");
    assert_eq!(line["params"], want.params, "{line}");
    assert_eq!(
        line["outputs"],
        Value::from(want.outputs.to_vec()),
        "{line}"
    );
    assert_eq!(line["live_nodes"], want.live_nodes, "{line}");
    assert_eq!(line["layers"], want.layers, "{line}");
    assert_eq!(fact_count(&line["facts"]), Some(want.facts), "{line}");
}

fn vector_file(types: &str, rows: &[&str]) -> String {
    let mut text = format!("noble-ngraph-vectors/0\ntypes {types}\n");
    for row in rows {
        text.push_str(row);
        text.push('\n');
    }
    text
}

// D6: a training call that stops (or is refused) leaves neither `<out>` nor
// `<out>.partial`.
fn partial(out: &Path) -> PathBuf {
    let mut name = out.as_os_str().to_owned();
    name.push(".partial");
    PathBuf::from(name)
}

fn assert_no_publication(out: &Path) {
    assert!(!out.exists(), "nothing published at {}", out.display());
    assert!(
        !partial(out).exists(),
        "no partial file next to {}",
        out.display()
    );
}

fn assert_published(out: &Path) {
    assert!(out.is_file(), "published {}", out.display());
    assert!(
        !partial(out).exists(),
        "the partial file of {} was removed",
        out.display()
    );
}

// ---------------------------------------------------------------------------
// Snapshot files (D6).
// ---------------------------------------------------------------------------

// The records of a snapshot file after its schema line, as `(key, rest)`.
struct Snapshot {
    records: Vec<(String, String)>,
}

impl Snapshot {
    fn read(path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let text = std::fs::read_to_string(path)?;
        let mut lines = text
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.starts_with('#'));
        assert_eq!(
            lines.next(),
            Some("noble-ngraph-snapshot/0"),
            "D6 schema line first:\n{text}"
        );
        let records: Vec<(String, String)> = lines
            .map(|line| {
                let (key, rest) = line.split_once(' ').unwrap_or((line, ""));
                (key.to_owned(), rest.trim().to_owned())
            })
            .collect();
        let mut previous = 0;
        for (key, _) in &records {
            let position = SNAPSHOT_KEYS
                .iter()
                .position(|known| known == key)
                .ok_or_else(|| format!("unknown snapshot record `{key}`:\n{text}"))?;
            assert!(position >= previous, "D6 record order:\n{text}");
            previous = position;
        }
        assert_eq!(
            records.last().map(|(key, _)| key.as_str()),
            Some("provenance"),
            "the provenance digest is the last record:\n{text}"
        );
        Ok(Self { records })
    }

    fn all(&self, key: &str) -> Vec<&str> {
        self.records
            .iter()
            .filter(|(known, _)| known == key)
            .map(|(_, rest)| rest.as_str())
            .collect()
    }

    fn one(&self, key: &str) -> &str {
        let found = self.all(key);
        assert_eq!(found.len(), 1, "exactly one `{key}` record");
        found[0]
    }

    // Param values in slot order.
    fn params(&self) -> Result<Vec<f32>, Box<dyn std::error::Error>> {
        let mut values = Vec::new();
        for record in self.all("param") {
            let (_, bits) = record.split_once(' ').ok_or("param record")?;
            let bits = bits.strip_prefix("0x").ok_or("param bits")?;
            values.push(f32::from_bits(u32::from_str_radix(bits, 16)?));
        }
        Ok(values)
    }
}

// The model, loss and data files of the training tests and their `check`
// reports.
struct TrainFiles {
    model: PathBuf,
    loss: PathBuf,
    data: PathBuf,
    model_report: Value,
    loss_report: Value,
}

impl TrainFiles {
    fn new(scratch: &Scratch) -> Result<Self, Box<dyn std::error::Error>> {
        let model = scratch.write("model.ngraph", MODEL)?;
        let loss = scratch.write("loss.ngraph", LOSS)?;
        let data = scratch.write("data.points", DATA)?;
        let model_report =
            expect_normal(&scratch.run(&Ngraph::new("check").arg(&model))?, "check")?;
        check_fields(
            &model_report,
            &Expected {
                profile: "ngraph-f32/0",
                inputs: &["f32"],
                params: 2,
                outputs: &["f32"],
                live_nodes: 5,
                layers: 3,
                facts: 1,
            },
        );
        let loss_report = expect_normal(&scratch.run(&Ngraph::new("check").arg(&loss))?, "check")?;
        Ok(Self {
            model,
            loss,
            data,
            model_report,
            loss_report,
        })
    }

    fn train(&self, steps: u64, out: &Path) -> Ngraph {
        train_command(&self.model, &self.loss, &self.data, steps, out)
    }

    fn replay(&self, snapshot: &Path) -> Ngraph {
        Ngraph::new("verify")
            .arg(&self.model)
            .arg(snapshot)
            .arg("--replay")
            .arg("--loss")
            .arg(&self.loss)
            .arg("--data")
            .arg(&self.data)
    }
}

// The required D7 `train` flags, in order.
fn train_flags(loss: &Path, data: &Path, steps: u64, out: &Path) -> Vec<(&'static str, OsString)> {
    vec![
        ("--loss", OsString::from(loss)),
        ("--data", OsString::from(data)),
        ("--policy", OsString::from(POLICY)),
        ("--optimizer", OsString::from(OPTIMIZER)),
        ("--seed", OsString::from(SEED)),
        ("--steps", OsString::from(steps.to_string())),
        ("--out", OsString::from(out)),
    ]
}

fn train_command(model: &Path, loss: &Path, data: &Path, steps: u64, out: &Path) -> Ngraph {
    with_flags(
        Ngraph::new("train").arg(model),
        &train_flags(loss, data, steps, out),
    )
}

// D7 `train` report fields.
fn check_train_report(line: &Value, model: &Value, steps: u64) {
    for key in [
        "snapshot",
        "graph",
        "parent",
        "steps",
        "loss_initial",
        "loss_final",
    ] {
        assert!(line.get(key).is_some(), "D7 `{key}`: {line}");
    }
    assert!(hex64(&line["snapshot"]), "{line}");
    assert!(hex64(&line["parent"]), "{line}");
    assert_eq!(line["graph"], model["graph"], "{line}");
    assert_eq!(line["steps"], steps, "{line}");
    assert!(!line["loss_initial"].is_null(), "{line}");
    assert!(!line["loss_final"].is_null(), "{line}");
}

// D7 `verify` report fields.
fn check_verify_report(line: &Value, snapshot: &Value, graph: &Value, replayed: bool) {
    assert_eq!(line["snapshot"], *snapshot, "{line}");
    assert_eq!(line["graph"], *graph, "{line}");
    assert_eq!(line["bound"], Value::Bool(true), "{line}");
    assert_eq!(line["replayed"], Value::Bool(replayed), "{line}");
}

// Every D6 record of a `MODEL` snapshot trained from `DATA` with `LOSS`.
fn check_snapshot_records(
    snapshot: &Snapshot,
    report: &Value,
    files: &TrainFiles,
    steps: u64,
    lr: &str,
) {
    assert_eq!(snapshot.one("snapshot"), report["snapshot"]);
    assert_eq!(snapshot.one("graph"), files.model_report["graph"]);
    assert_eq!(snapshot.one("layout"), files.model_report["param_layout"]);
    let params = snapshot.all("param");
    assert_eq!(params.len(), 2, "one record per param slot");
    for (record, slot) in params.iter().zip(["1", "2"]) {
        let (recorded, bits) = record.split_once(' ').unwrap_or_default();
        assert_eq!(recorded, slot, "param records in slot order");
        assert!(
            bits.strip_prefix("0x")
                .is_some_and(|hex| hex.len() == 8 && hex.bytes().all(is_lower_hex)),
            "exact binary32 bits: {record}"
        );
    }
    assert!(is_hex64(snapshot.one("optimizer-state")));
    assert_eq!(snapshot.one("parent"), report["parent"]);
    assert_eq!(
        snapshot.one("subject"),
        format!("authored {MODEL_SUBJECT} closure -")
    );
    assert_eq!(snapshot.one("profile"), "ngraph-f32/0 ngraph-sem/0");
    assert!(is_hex64(snapshot.one("facts")));
    assert_eq!(snapshot.one("loss"), files.loss_report["graph"]);
    assert!(is_hex64(snapshot.one("data")));
    assert_eq!(snapshot.one("policy"), POLICY);
    assert!(snapshot.all("opt-in").is_empty());
    assert_eq!(snapshot.one("seed"), SEED_RECORD);
    assert_eq!(snapshot.one("prng"), "splitmix64/0");
    assert_eq!(
        snapshot.one("optimizer"),
        format!("adam/0 lr {lr} beta1 {ADAM_BETA1} beta2 {ADAM_BETA2} eps {ADAM_EPS}")
    );
    assert_eq!(snapshot.one("steps"), steps.to_string());
    assert_eq!(snapshot.one("tools"), TOOLS);
    assert!(is_hex64(snapshot.one("provenance")));
}

// Flips the lowest bit of the first `param` record's value.
fn flip_first_param(text: &str) -> Result<String, Box<dyn std::error::Error>> {
    let mut lines: Vec<String> = text.lines().map(str::to_owned).collect();
    let index = lines
        .iter()
        .position(|line| line.starts_with("param "))
        .ok_or("no param record")?;
    let (slot, bits) = lines[index]["param ".len()..]
        .split_once(' ')
        .ok_or("malformed param record")?;
    let value = u32::from_str_radix(bits.strip_prefix("0x").ok_or("param bits")?, 16)?;
    let tampered = format!("param {slot} 0x{:08x}", value ^ 1);
    lines[index] = tampered;
    Ok(lines.join("\n") + "\n")
}

// ---------------------------------------------------------------------------
// check
// ---------------------------------------------------------------------------

#[test]
fn check_reports_every_d7_field() -> TestResult {
    // NGRAPH-CASE-03: `check` admits authored graph files and reports every D7
    // field; F1 in the canonical printed form has its recorded identity.
    let scratch = Scratch::new("check-fields")?;
    let graph = scratch.write("demo.ngraph", CHECK_GRAPH)?;
    let command = Ngraph::new("check").arg(&graph);
    let run = scratch.run(&command)?;
    assert_eq!(
        scratch.run(&command)?.stdout,
        run.stdout,
        "D11: repeated runs print identical bytes"
    );
    let line = expect_normal(&run, "check")?;
    check_fields(
        &line,
        &Expected {
            profile: "ngraph-f32/0",
            inputs: &["f32", "f32"],
            params: 0,
            outputs: &["f32"],
            live_nodes: 4,
            layers: 3,
            facts: 3,
        },
    );
    assert!(
        line["subject"].to_string().contains(CHECK_SUBJECT),
        "`authored-name` is `Subject::authored(token)`: {line}"
    );

    let f1 = scratch.write("f1.ngraph", &f1_text())?;
    let command = Ngraph::new("check").arg(&f1);
    let run = scratch.run(&command)?;
    assert_eq!(scratch.run(&command)?.stdout, run.stdout, "D11");
    let line = expect_normal(&run, "check")?;
    assert_eq!(line["graph"], F1_GRAPH, "the recorded F1 GraphLogicalId");
    check_fields(
        &line,
        &Expected {
            profile: "ngraph-f32/0",
            inputs: &["f32", "f32"],
            params: 22,
            outputs: &["f32"],
            live_nodes: 57,
            layers: 9,
            facts: 0,
        },
    );
    assert!(line["subject"].to_string().contains(F1_SUBJECT), "{line}");
    Ok(())
}

#[test]
fn check_identity_follows_the_d4_records() -> TestResult {
    // NGRAPH-CASE-03 (D4): `graph` is recomputed from the identity-bearing
    // records (subject, shapes); comments, spacing and fact records carry no
    // identity, and the `authored-name` sugar equals its canonical hex form.
    let scratch = Scratch::new("check-identity")?;
    let check = |name: &str, text: &str| -> Result<Value, Box<dyn std::error::Error>> {
        let path = scratch.write(name, text)?;
        expect_normal(&scratch.run(&Ngraph::new("check").arg(&path))?, "check")
    };
    let sugar = check(
        "sugar.ngraph",
        &format!("ngraph-ir/0 ngraph-f32/0\nsubject authored-name {SUGAR_NAME}\n{SUGAR_BODY}"),
    )?;
    let canonical = check(
        "canonical.ngraph",
        &format!(
            "ngraph-ir/0 ngraph-f32/0\nsubject authored {SUGAR_SUBJECT} closure -\n{SUGAR_BODY}"
        ),
    )?;
    assert_eq!(
        canonical["graph"], sugar["graph"],
        "sugar and canonical form"
    );
    for line in [&sugar, &canonical] {
        assert!(
            line["subject"].to_string().contains(SUGAR_SUBJECT),
            "{line}"
        );
    }
    let renamed = check(
        "renamed.ngraph",
        &format!("ngraph-ir/0 ngraph-f32/0\nsubject authored-name {SUGAR_NAME}-2\n{SUGAR_BODY}"),
    )?;
    assert_ne!(
        renamed["graph"], sugar["graph"],
        "the subject is identity-bearing"
    );
    let spaced = check(
        "spaced.ngraph",
        &format!(
            "# a comment before the schema line\n\n\
             ngraph-ir/0   ngraph-f32/0   # comments and runs of spaces carry no meaning\n\
             subject  authored-name  {SUGAR_NAME}\n\
             slot   input   f32\n\
             %0  =  input  0\n\
             \n\
             %1 = f32.relu    %0   # relu\n\
             out    %1\n"
        ),
    )?;
    assert_eq!(spaced["graph"], sugar["graph"], "comments and spacing");
    let with_fact = check(
        "fact.ngraph",
        &format!(
            "ngraph-ir/0 ngraph-f32/0\nsubject authored-name {SUGAR_NAME}\n{}",
            SUGAR_BODY.replace("out %1", "fact %0 finite input-validated\nout %1")
        ),
    )?;
    assert_eq!(
        with_fact["graph"], sugar["graph"],
        "facts are not identity-bearing"
    );
    assert_eq!(fact_count(&with_fact["facts"]), Some(1));
    assert_eq!(fact_count(&sugar["facts"]), Some(0));
    let shaped = check(
        "shaped.ngraph",
        &format!(
            "ngraph-ir/0 ngraph-f32/0\nsubject authored-name {SUGAR_NAME}\nshape input s\nshape output s\n{SUGAR_BODY}"
        ),
    )?;
    assert_ne!(
        shaped["graph"], sugar["graph"],
        "Some(shapes) differs from None"
    );
    Ok(())
}

#[test]
fn check_refusal_classes() -> TestResult {
    // NGRAPH-CASE-04: malformed, inexact, unchecked-fact and profile variants
    // are refused with their D8 code and exit 2; a missing file and a file
    // over the 64 MiB bound are host refusals (`code: null`, exit 2).
    let scratch = Scratch::new("check-refusals")?;
    let fact = "fact %2 finite derived interval";
    let variants: [(&str, String, &str); 12] = [
        (
            "missing schema line",
            CHECK_GRAPH.replacen("ngraph-ir/0 ngraph-f32/0\n", "", 1),
            "E-GRAPH-MALFORMED",
        ),
        (
            "unknown op",
            CHECK_GRAPH.replace("f32.add", "f32.div"),
            "E-GRAPH-MALFORMED",
        ),
        (
            "missing subject line",
            CHECK_GRAPH.replace("subject authored-name cli-check-demo\n", ""),
            "E-GRAPH-MALFORMED",
        ),
        (
            "duplicate subject line",
            CHECK_GRAPH.replace(
                "subject authored-name cli-check-demo\n",
                "subject authored-name cli-check-demo\nsubject authored-name cli-check-demo\n",
            ),
            "E-GRAPH-MALFORMED",
        ),
        (
            "malformed subject identity",
            CHECK_GRAPH.replace(
                "subject authored-name cli-check-demo",
                "subject authored 3be1 closure -",
            ),
            "E-GRAPH-MALFORMED",
        ),
        (
            "odd-length evidence hex",
            CHECK_GRAPH.replace(fact, "fact %2 finite evidence abc"),
            "E-GRAPH-MALFORMED",
        ),
        (
            "non-hex evidence",
            CHECK_GRAPH.replace(fact, "fact %2 finite evidence zz"),
            "E-GRAPH-MALFORMED",
        ),
        (
            "derived without its rule token",
            CHECK_GRAPH.replace(fact, "fact %2 finite derived"),
            "E-GRAPH-MALFORMED",
        ),
        (
            "evidence provenance",
            CHECK_GRAPH.replace(fact, "fact %2 finite evidence abcd"),
            "E-FACT-UNCHECKED",
        ),
        (
            "caller-asserted range fact",
            CHECK_GRAPH.replace(fact, "fact %2 range 0xbf800000 0x3f800000 asserted"),
            "E-FACT-UNCHECKED",
        ),
        (
            "const f32 0.1",
            CHECK_GRAPH
                .replace(
                    "%3 = f32.add %2 %0",
                    "%3 = const f32 0.1\n%4 = f32.add %2 %3",
                )
                .replace("out %3", "out %4"),
            "E-LITERAL-INEXACT",
        ),
        (
            "unknown sub-profile",
            CHECK_GRAPH.replace("ngraph-f32/0", "ngraph-f64/0"),
            "E-PROFILE",
        ),
    ];
    for (index, (name, text, code)) in variants.into_iter().enumerate() {
        let path = scratch.write(&format!("variant-{index}.ngraph"), &text)?;
        let line = expect_code(
            &scratch.run(&Ngraph::new("check").arg(&path))?,
            "check",
            code,
        )?;
        assert_eq!(line["outcome"], "reject", "{name}");
    }

    let missing = scratch.path("missing.ngraph");
    expect_host_refusal(&scratch.run(&Ngraph::new("check").arg(&missing))?, "check")?;
    let oversized = scratch.write("oversized.ngraph", CHECK_GRAPH)?;
    std::fs::OpenOptions::new()
        .write(true)
        .open(&oversized)?
        .set_len(64 * 1024 * 1024 + 1)?;
    expect_host_refusal(
        &scratch.run(&Ngraph::new("check").arg(&oversized))?,
        "check",
    )?;
    Ok(())
}

#[test]
fn check_budgets_and_usage() -> TestResult {
    // NGRAPH-CASE-04: a lowered budget the graph exceeds is exhausted (exit 5),
    // a raised one is E-PROFILE (exit 2), and unknown, duplicated, malformed,
    // missing or inapplicable flags are usage errors (exit 2).
    let scratch = Scratch::new("check-budgets")?;
    let graph = scratch.write("demo.ngraph", CHECK_GRAPH)?;
    let f1 = scratch.write("f1.ngraph", &f1_text())?;
    let check = |path: &Path, flags: &[&str]| -> std::io::Result<Run> {
        let mut command = Ngraph::new("check").arg(path);
        for flag in flags {
            command = command.arg(*flag);
        }
        scratch.run(&command)
    };
    expect_code(
        &check(&f1, &["--budget-live-nodes", "4"])?,
        "check",
        "E-BUDGET-LIVE-NODES",
    )?;
    for (flag, limit, code) in [
        ("--budget-live-nodes", "3", "E-BUDGET-LIVE-NODES"),
        ("--budget-layers", "2", "E-BUDGET-LAYERS"),
        ("--budget-facts", "2", "E-BUDGET-FACTS"),
    ] {
        let line = expect_code(&check(&graph, &[flag, limit])?, "check", code)?;
        assert_eq!(line["outcome"], "exhausted", "{flag}");
    }
    let at_limits = [
        "--budget-live-nodes",
        "4",
        "--budget-layers",
        "3",
        "--budget-facts",
        "3",
    ];
    expect_normal(&check(&graph, &at_limits)?, "check")?;
    expect_normal(&check(&graph, &["--budget-live-nodes", "262144"])?, "check")?;
    for raised in ["262145", "18446744073709551615"] {
        expect_code(
            &check(&graph, &["--budget-live-nodes", raised])?,
            "check",
            "E-PROFILE",
        )?;
    }
    let usage: [&[&str]; 9] = [
        &["--bogus"],
        &["--budget-live-nodes", "8", "--budget-live-nodes", "8"],
        &["--budget-live-nodes", "-1"],
        &["--budget-live-nodes", "4x"],
        &["--budget-live-nodes", "18446744073709551616"],
        &["--budget-live-nodes"],
        &["--budget-steps", "1"],
        &["--fuel", "1"],
        &["--reference"],
    ];
    for flags in usage {
        expect_usage(&check(&graph, flags)?);
    }
    expect_usage(&scratch.run(&Ngraph::new("check"))?);
    expect_usage(&scratch.run(&Ngraph::bare())?);
    expect_usage(&scratch.run(&Ngraph::new("frobnicate").arg(&graph))?);
    Ok(())
}

// ---------------------------------------------------------------------------
// eval
// ---------------------------------------------------------------------------

#[test]
fn eval_probes_are_bitwise_in_both_evaluators() -> TestResult {
    // NGRAPH-CASE-07: exact binary32 results (ties to even, signed zeros,
    // overflow, subnormals, one NaN class), the header line first, and
    // byte-identical stdout from the default and `--reference` evaluators.
    let scratch = Scratch::new("eval-probes")?;
    let graph = scratch.write("probes.ngraph", PROBE_GRAPH)?;
    let vectors = scratch.write("probes.vectors", PROBE_VECTORS)?;
    let checked = expect_normal(&scratch.run(&Ngraph::new("check").arg(&graph))?, "check")?;
    let default = Ngraph::new("eval").arg(&graph).arg(&vectors);
    let run = scratch.run(&default)?;
    let (header, lines) = eval_lines(&run)?;
    assert_eq!(
        header["graph"], checked["graph"],
        "eval and check name one graph"
    );
    assert!(header["snapshot"].is_null(), "{header}");
    assert_eq!(lines.len(), PROBE_EXPECTED.len());
    for (line, expected) in lines.iter().zip(PROBE_EXPECTED) {
        assert_eq!(line["outcome"], "normal", "{line}");
        assert_eq!(line["outputs"], Value::from(expected.to_vec()), "{line}");
    }
    assert_eq!(run.exit, Some(0));
    let reference = scratch.run(&default.clone().arg("--reference"))?;
    assert_eq!(reference.stdout, run.stdout, "D11: both evaluators");
    assert_eq!(reference.exit, run.exit);
    assert_eq!(
        scratch.run(&default)?.stdout,
        run.stdout,
        "D11: repeated runs"
    );
    let fueled = scratch.run(&default.clone().arg("--fuel").arg("1000000"))?;
    assert_eq!(fueled.stdout, run.stdout, "ample fuel changes nothing");
    Ok(())
}

#[test]
fn eval_outcome_classes_and_first_failure_exit() -> TestResult {
    // NGRAPH-CASE-08: a guard failure is a `trap` line (G-FACT-FALSE), an input
    // fact violation a `reject` line (E-FACT-INPUT); the exit is the first
    // non-normal vector's; both evaluators print the same bytes; `--fuel 1`
    // is exhausted (E-BUDGET-FUEL, exit 5).
    let scratch = Scratch::new("eval-outcomes")?;
    let graph = scratch.write("guard.ngraph", GUARD_GRAPH)?;
    let files: [(&str, [&str; 3], [&str; 3], i32); 3] = [
        (
            "trap-first",
            [GUARD_NORMAL, GUARD_TRAP, GUARD_REJECT],
            ["normal", "trap", "reject"],
            1,
        ),
        (
            "reject-first",
            [GUARD_NORMAL, GUARD_REJECT, GUARD_TRAP],
            ["normal", "reject", "trap"],
            2,
        ),
        (
            "all-normal",
            [GUARD_NORMAL, GUARD_NORMAL_2, GUARD_NORMAL],
            ["normal", "normal", "normal"],
            0,
        ),
    ];
    for (name, rows, outcomes, exit) in files {
        let vectors = scratch.write(&format!("{name}.vectors"), &vector_file("f32 f32", &rows))?;
        let default = Ngraph::new("eval").arg(&graph).arg(&vectors);
        let run = scratch.run(&default)?;
        let (_, lines) = eval_lines(&run)?;
        let observed: Vec<&str> = lines
            .iter()
            .map(|line| line["outcome"].as_str().unwrap_or_default())
            .collect();
        assert_eq!(observed, outcomes, "{name}");
        assert_eq!(run.exit, Some(exit), "{name}: the first non-normal vector");
        for (row, line) in rows.iter().zip(&lines) {
            match *row {
                GUARD_NORMAL => assert_eq!(line["outputs"], Value::from(vec![GUARD_NORMAL_OUT])),
                GUARD_NORMAL_2 => {
                    assert_eq!(line["outputs"], Value::from(vec![GUARD_NORMAL_2_OUT]));
                }
                GUARD_TRAP => assert_eq!(line["code"], "G-FACT-FALSE", "{line}"),
                _ => assert_eq!(line["code"], "E-FACT-INPUT", "{line}"),
            }
        }
        let reference = scratch.run(&default.clone().arg("--reference"))?;
        assert_eq!(reference.stdout, run.stdout, "D11: {name}");
        assert_eq!(reference.exit, run.exit, "{name}");
    }

    let one = scratch.write("one.vectors", &vector_file("f32 f32", &[GUARD_NORMAL]))?;
    for evaluator in [None, Some("--reference")] {
        let mut command = Ngraph::new("eval")
            .arg(&graph)
            .arg(&one)
            .arg("--fuel")
            .arg("1");
        if let Some(flag) = evaluator {
            command = command.arg(flag);
        }
        let run = scratch.run(&command)?;
        let (_, lines) = eval_lines(&run)?;
        assert_eq!(lines.len(), 1, "{}", run.describe());
        assert_eq!(lines[0]["outcome"], "exhausted", "{}", run.describe());
        assert_eq!(lines[0]["code"], "E-BUDGET-FUEL", "{}", run.describe());
        assert_eq!(run.exit, Some(5));
    }
    Ok(())
}

#[test]
fn eval_refuses_inputs_before_evaluation() -> TestResult {
    // NGRAPH-CASE-08: a `types` mismatch (E-INTERFACE), a bad schema, row or
    // count (E-FORMAT) and an inexact decimal (E-LITERAL-INEXACT) print no
    // vector line and exit 2; so do graph-level refusals: params without
    // `--snapshot` (E-SNAPSHOT-LAYOUT) and a malformed graph.
    let scratch = Scratch::new("eval-refusals")?;
    let graph = scratch.write("probes.ngraph", PROBE_GRAPH)?;
    let good = "v 1 2 0.5";
    let files: [(String, &str); 7] = [
        (vector_file("i64 f32 f32", &[good]), "E-INTERFACE"),
        (vector_file("f32 f32", &["v 1 2"]), "E-INTERFACE"),
        (
            format!("noble-ngraph-vectors/1\ntypes f32 f32 f32\n{good}\n"),
            "E-FORMAT",
        ),
        (format!("types f32 f32 f32\n{good}\n"), "E-FORMAT"),
        (vector_file("f32 f32 f32", &[good, "v 1 2"]), "E-FORMAT"),
        (vector_file("f32 f32 f32", &[good, "v 1 2 3 4"]), "E-FORMAT"),
        (
            vector_file("f32 f32 f32", &[good, "v 0.1 1 1"]),
            "E-LITERAL-INEXACT",
        ),
    ];
    for (index, (text, code)) in files.into_iter().enumerate() {
        let vectors = scratch.write(&format!("refused-{index}.vectors"), &text)?;
        let run = scratch.run(&Ngraph::new("eval").arg(&graph).arg(&vectors))?;
        let refusal = eval_refusal(&run, code, false)?;
        assert_eq!(refusal["outcome"], "reject", "{refusal}");
    }

    let model = scratch.write("model.ngraph", MODEL)?;
    let model_vectors = scratch.write("model.vectors", &vector_file("f32", &["v 0.5"]))?;
    eval_refusal(
        &scratch.run(&Ngraph::new("eval").arg(&model).arg(&model_vectors))?,
        "E-SNAPSHOT-LAYOUT",
        true,
    )?;
    let malformed = scratch.write(
        "malformed.ngraph",
        &PROBE_GRAPH.replace("f32.relu", "f32.tanh"),
    )?;
    let probes = scratch.write("probes.vectors", PROBE_VECTORS)?;
    eval_refusal(
        &scratch.run(&Ngraph::new("eval").arg(&malformed).arg(&probes))?,
        "E-GRAPH-MALFORMED",
        true,
    )?;
    expect_usage(&scratch.run(&Ngraph::new("eval").arg(&graph))?);
    expect_usage(&scratch.run(&Ngraph::new("eval").arg(&graph).arg(&probes).arg("--bogus"))?);
    Ok(())
}

// ---------------------------------------------------------------------------
// extract
// ---------------------------------------------------------------------------

fn extract_command(source: &Path, inputs: &str, out: &Path) -> Ngraph {
    Ngraph::new("extract")
        .arg(source)
        .arg("--inputs")
        .arg(inputs)
        .arg("--out")
        .arg(out)
}

// One extraction source with its interface and Core-equal vectors.
struct ExtractCase<'a> {
    name: &'a str,
    source: &'a str,
    inputs: &'a str,
    outputs: &'a [&'a str],
    vectors: &'a [(&'a str, &'a [&'a str])],
}

#[test]
fn extract_i1_to_i4_round_trip_through_check_and_eval() -> TestResult {
    // NGRAPH-CASE-05 (with NGRAPH-CASE-03 for I1's check report and
    // NGRAPH-CASE-07 for its vector): I1–I4 extract to new graph files whose
    // `check` report repeats the extract report's check fields and whose
    // evaluation gives the Core results (K-NUM-01 wrapping included).
    let scratch = Scratch::new("extract-sources")?;
    let cases = [
        ExtractCase {
            name: "i1",
            source: I1,
            inputs: "i64,i64",
            outputs: &["i64"],
            // 4·4 + 3·5 − 7 = 24; 0 + 3·MIN − 7 wraps to MAX − 6.
            vectors: &[
                ("v 5 4", &["i64:24"]),
                ("v -9223372036854775808 0", &["i64:9223372036854775801"]),
            ],
        },
        ExtractCase {
            name: "i2",
            source: I2,
            inputs: "i64",
            outputs: &["i64"],
            vectors: &[("v 40", &["i64:44"])],
        },
        ExtractCase {
            name: "i3",
            source: I3,
            inputs: "i64,i64",
            outputs: &["i64"],
            vectors: &[("v 3 4", &["i64:25"])],
        },
        ExtractCase {
            name: "i4",
            source: I4,
            inputs: "i64,i64,i64",
            outputs: &["i64", "i64"],
            // 2·5 + 3 = 13; MAX·2 + 1 wraps to −1.
            vectors: &[
                ("v 2 3 5", &["i64:13", "i64:13"]),
                ("v 9223372036854775807 1 2", &["i64:-1", "i64:-1"]),
            ],
        },
    ];
    let mut reports = Vec::new();
    for case in &cases {
        let source = scratch.write(&format!("{}.noble", case.name), case.source)?;
        let out = scratch.path(&format!("{}.ngraph", case.name));
        let run = scratch.run(&extract_command(&source, case.inputs, &out))?;
        let line = expect_normal(&run, "extract")?;
        assert_published(&out);
        let types: Vec<&str> = case.inputs.split(',').collect();
        assert_eq!(line["profile"], "ngraph-i64/0", "{line}");
        assert_eq!(line["inputs"], Value::from(types.clone()), "{line}");
        assert_eq!(
            line["outputs"],
            Value::from(case.outputs.to_vec()),
            "{line}"
        );
        assert_eq!(line["params"], 0, "{line}");
        for key in EXTRACTION_FIELDS {
            assert!(line[key].is_u64(), "D7 `{key}`: {line}");
        }
        assert!(
            line["expanded_nodes"].as_u64() >= line["live_nodes"].as_u64(),
            "expansion is counted before pruning: {line}"
        );

        let checked = expect_normal(&scratch.run(&Ngraph::new("check").arg(&out))?, "check")?;
        for key in CHECK_FIELDS {
            assert_eq!(checked[key], line[key], "`{key}` of {}", case.name);
        }
        let rows: Vec<&str> = case.vectors.iter().map(|(row, _)| *row).collect();
        let vectors = scratch.write(
            &format!("{}.vectors", case.name),
            &vector_file(&types.join(" "), &rows),
        )?;
        let (header, lines) =
            eval_lines(&scratch.run(&Ngraph::new("eval").arg(&out).arg(&vectors))?)?;
        assert_eq!(header["graph"], line["graph"]);
        assert_eq!(lines.len(), case.vectors.len());
        for (vector, (_, expected)) in lines.iter().zip(case.vectors) {
            assert_eq!(
                vector["outputs"],
                Value::from(expected.to_vec()),
                "{vector}"
            );
        }
        reports.push((line, run.stdout));
    }
    let (i1, i3) = (&reports[0].0, &reports[2].0);
    assert_eq!(i1["live_nodes"], 8, "{i1}");
    assert_eq!(i1["layers"], 4, "{i1}");
    assert_eq!(i1["pruned_nodes"], 0, "{i1}");
    assert_eq!(i1["inline_depth"], 0, "{i1}");
    assert_eq!(i3["inline_depth"], 1, "one named call: {i3}");
    assert_ne!(i3["closure"], i1["closure"], "I3's closure names `sq`");

    // D11: extracting the same source again prints the same report (no path
    // is printed) and writes the same bytes; `--profile ngraph-i64/0` is the
    // default made explicit.
    let source = scratch.path("i1.noble");
    let again = scratch.path("i1-again.ngraph");
    let repeated = scratch.run(&extract_command(&source, "i64,i64", &again))?;
    assert_eq!(
        repeated.stdout, reports[0].1,
        "D11: identical extract reports"
    );
    assert_eq!(
        std::fs::read(&again)?,
        std::fs::read(scratch.path("i1.ngraph"))?,
        "D11: identical graph files"
    );
    let explicit = scratch.run(
        &extract_command(&source, "i64,i64", &scratch.path("i1-explicit.ngraph"))
            .arg("--profile")
            .arg("ngraph-i64/0"),
    )?;
    assert_eq!(explicit.stdout, reports[0].1, "the only accepted profile");

    // `--framed`: session framing (a decimal byte count line, then the bytes).
    let framed_text: String = ["def sq [ dup * ]", "sq swap sq +"]
        .iter()
        .map(|submission| format!("{}\n{submission}", submission.len()))
        .collect::<Vec<_>>()
        .concat();
    let framed = scratch.write("i3-framed.noble", &framed_text)?;
    let framed_out = scratch.path("i3-framed.ngraph");
    let line = expect_normal(
        &scratch.run(&extract_command(&framed, "i64,i64", &framed_out).arg("--framed"))?,
        "extract",
    )?;
    let i3_vectors = scratch.path("i3.vectors");
    let (header, lines) =
        eval_lines(&scratch.run(&Ngraph::new("eval").arg(&framed_out).arg(&i3_vectors))?)?;
    assert_eq!(header["graph"], line["graph"]);
    assert_eq!(lines[0]["outputs"], Value::from(vec!["i64:25"]));

    // D5: an `i64` token out of range is E-FORMAT.
    let out_of_range = scratch.write(
        "i1-range.vectors",
        &vector_file("i64 i64", &["v 9223372036854775808 0"]),
    )?;
    eval_refusal(
        &scratch.run(
            &Ngraph::new("eval")
                .arg(&scratch.path("i1.ngraph"))
                .arg(&out_of_range),
        )?,
        "E-FORMAT",
        false,
    )?;
    Ok(())
}

#[test]
fn extract_refusals_publish_nothing() -> TestResult {
    // NGRAPH-CASE-05: effect, control, Program-output, frontend, profile and
    // budget refusals report their code and exit and write no file; flag
    // errors are usage errors.
    let scratch = Scratch::new("extract-refusals")?;
    let cases: [(&str, &str, &str, &[&str], &str); 8] = [
        ("effect", EFFECT_SOURCE, "i64", &[], "E-EFFECT"),
        ("control", CONTROL_SOURCE, "i64", &[], "E-CONTROL"),
        (
            "program-output",
            PROGRAM_OUTPUT_SOURCE,
            "i64",
            &[],
            "E-PROGRAM-OUTPUT",
        ),
        ("frontend", UNBOUND_SOURCE, "i64", &[], "E-SUBJECT"),
        (
            "f32-profile",
            I1,
            "i64,i64",
            &["--profile", "ngraph-f32/0"],
            "E-PROFILE",
        ),
        (
            "unknown-profile",
            I1,
            "i64,i64",
            &["--profile", "ngraph-f64/0"],
            "E-PROFILE",
        ),
        (
            "raised-budget",
            I1,
            "i64,i64",
            &["--budget-work", "4194305"],
            "E-PROFILE",
        ),
        (
            "expansion",
            I3,
            "i64,i64",
            &["--budget-expansion", "2"],
            "E-BUDGET-EXPANSION",
        ),
    ];
    for (name, text, inputs, flags, code) in cases {
        let source = scratch.write(&format!("{name}.noble"), text)?;
        let out = scratch.path(&format!("{name}.ngraph"));
        let mut command = extract_command(&source, inputs, &out);
        for flag in flags {
            command = command.arg(*flag);
        }
        expect_code(&scratch.run(&command)?, "extract", code)?;
        assert_no_publication(&out);
    }

    let source = scratch.write("i1.noble", I1)?;
    let out = scratch.path("usage.ngraph");
    for command in [
        Ngraph::new("extract").arg(&source).arg("--out").arg(&out),
        Ngraph::new("extract")
            .arg(&source)
            .arg("--inputs")
            .arg("i64,i64"),
        Ngraph::new("extract")
            .arg("--inputs")
            .arg("i64,i64")
            .arg("--out")
            .arg(&out),
        extract_command(&source, "i64,i64", &out).arg("--bogus"),
        extract_command(&source, "i64,i64", &out)
            .arg("--inputs")
            .arg("i64,i64"),
        extract_command(&source, "i64,i64", &out)
            .arg("--fuel")
            .arg("1"),
        extract_command(&source, "i64,i64", &out)
            .arg("--budget-steps")
            .arg("1"),
    ] {
        expect_usage(&scratch.run(&command)?);
        assert_no_publication(&out);
    }
    Ok(())
}

#[test]
fn extract_publication_never_replaces_existing_paths() -> TestResult {
    // NGRAPH-CASE-05 (D6 publication): an existing `--out` or `--out.partial`
    // is a host refusal (exit 2) that leaves both byte-unchanged, and a
    // missing source file is a host refusal.
    let scratch = Scratch::new("extract-existing")?;
    let source = scratch.write("i1.noble", I1)?;
    let existing = scratch.write("existing.ngraph", "existing bytes\n")?;
    expect_host_refusal(
        &scratch.run(&extract_command(&source, "i64,i64", &existing))?,
        "extract",
    )?;
    assert_eq!(std::fs::read(&existing)?, b"existing bytes\n");
    assert!(!partial(&existing).exists());

    let out = scratch.path("fresh.ngraph");
    let stale = partial(&out);
    std::fs::write(&stale, "partial bytes\n")?;
    expect_host_refusal(
        &scratch.run(&extract_command(&source, "i64,i64", &out))?,
        "extract",
    )?;
    assert!(!out.exists());
    assert_eq!(std::fs::read(&stale)?, b"partial bytes\n");

    let missing = scratch.path("missing.noble");
    let other = scratch.path("other.ngraph");
    expect_host_refusal(
        &scratch.run(&extract_command(&missing, "i64,i64", &other))?,
        "extract",
    )?;
    assert_no_publication(&other);
    Ok(())
}

// ---------------------------------------------------------------------------
// train, verify
// ---------------------------------------------------------------------------

#[test]
fn train_publishes_immutable_snapshots() -> TestResult {
    // NGRAPH-CASE-10: `--steps 0` publishes S0 and `--steps N` a new snapshot
    // with every D6 record; identical runs write identical bytes; an existing
    // `--out` or `.partial` is refused with every file unchanged; a parent is
    // only read, and evaluating it is unchanged after training from it.
    let scratch = Scratch::new("train-publish")?;
    let files = TrainFiles::new(&scratch)?;

    let s0 = scratch.path("s0.snapshot");
    let r0 = expect_normal(&scratch.run(&files.train(0, &s0))?, "train")?;
    check_train_report(&r0, &files.model_report, 0);
    assert_eq!(r0["loss_initial"], r0["loss_final"], "no step taken");
    assert_published(&s0);
    let snapshot0 = Snapshot::read(&s0)?;
    check_snapshot_records(&snapshot0, &r0, &files, 0, ADAM_LR);

    let sn = scratch.path("s25.snapshot");
    let run_n = scratch.run(&files.train(25, &sn))?;
    let rn = expect_normal(&run_n, "train")?;
    check_train_report(&rn, &files.model_report, 25);
    assert_eq!(rn["parent"], r0["snapshot"], "seed training starts from S0");
    assert_eq!(rn["loss_initial"], r0["loss_final"], "the loss at S0");
    assert_ne!(
        rn["snapshot"], r0["snapshot"],
        "training changed the params"
    );
    assert_published(&sn);
    let snapshot_n = Snapshot::read(&sn)?;
    check_snapshot_records(&snapshot_n, &rn, &files, 25, ADAM_LR);

    let again = scratch.path("s25-again.snapshot");
    let run_again = scratch.run(&files.train(25, &again))?;
    assert_eq!(
        run_again.stdout, run_n.stdout,
        "D11: identical train reports"
    );
    assert_eq!(
        std::fs::read(&again)?,
        std::fs::read(&sn)?,
        "D11: identical snapshot files"
    );

    // Evaluation with SN uses exactly its param bytes: y = rn(rn(w·x) + b).
    let vectors = scratch.write("model.vectors", &vector_file("f32", &["v 0.5", "v -1"]))?;
    let eval = Ngraph::new("eval")
        .arg(&files.model)
        .arg(&vectors)
        .arg("--snapshot")
        .arg(&sn);
    let eval_run = scratch.run(&eval)?;
    let (header, lines) = eval_lines(&eval_run)?;
    assert_eq!(header["graph"], files.model_report["graph"]);
    assert_eq!(header["snapshot"], rn["snapshot"]);
    let params = snapshot_n.params()?;
    assert_eq!(params.len(), 2);
    for (line, x) in lines.iter().zip([0.5_f32, -1.0]) {
        let product = params[0] * x;
        let y = product + params[1];
        assert_eq!(line["outputs"], Value::from(vec![render_f32(y)]), "{line}");
    }
    assert_eq!(
        scratch.run(&eval.clone().arg("--reference"))?.stdout,
        eval_run.stdout
    );

    // Existing paths are refused; nothing is truncated, replaced or modified.
    let s0_bytes = std::fs::read(&s0)?;
    let sn_bytes = std::fs::read(&sn)?;
    let from_sn = |out: &Path| files.train(5, out).arg("--from").arg(&sn);
    expect_host_refusal(&scratch.run(&from_sn(&s0))?, "train")?;
    assert_eq!(
        std::fs::read(&s0)?,
        s0_bytes,
        "the existing --out is unchanged"
    );
    assert_eq!(std::fs::read(&sn)?, sn_bytes, "the parent is unchanged");
    assert!(!partial(&s0).exists());
    let child = scratch.path("child.snapshot");
    let stale = partial(&child);
    std::fs::write(&stale, "an unrelated partial file\n")?;
    expect_host_refusal(&scratch.run(&from_sn(&child))?, "train")?;
    assert!(!child.exists());
    assert_eq!(std::fs::read(&stale)?, b"an unrelated partial file\n");
    std::fs::remove_file(&stale)?;

    let rc = expect_normal(&scratch.run(&from_sn(&child))?, "train")?;
    check_train_report(&rc, &files.model_report, 5);
    assert_eq!(rc["parent"], rn["snapshot"], "--from records its parent");
    assert_published(&child);
    let snapshot_c = Snapshot::read(&child)?;
    assert_eq!(snapshot_c.one("parent"), rn["snapshot"]);
    assert_eq!(snapshot_c.one("steps"), "5");
    assert_eq!(std::fs::read(&sn)?, sn_bytes, "the parent is only read");
    assert_eq!(
        scratch.run(&eval)?.stdout,
        eval_run.stdout,
        "SN evaluates exactly as before"
    );

    // `--adam-lr` takes binary64 bits or a decimal rounded to nearest even;
    // the exact bits are recorded. ParamSnapshotId binds the graph, slots and
    // bytes only, so S0 keeps its id while its provenance changes.
    let decimal = scratch.path("lr-decimal.snapshot");
    let bits = scratch.path("lr-bits.snapshot");
    let decimal_run = scratch.run(&files.train(0, &decimal).arg("--adam-lr").arg("0.5"))?;
    let bits_run = scratch.run(
        &files
            .train(0, &bits)
            .arg("--adam-lr")
            .arg("0x3fe0000000000000"),
    )?;
    let rd = expect_normal(&decimal_run, "train")?;
    expect_normal(&bits_run, "train")?;
    assert_eq!(decimal_run.stdout, bits_run.stdout);
    assert_eq!(std::fs::read(&decimal)?, std::fs::read(&bits)?);
    let snapshot_d = Snapshot::read(&decimal)?;
    check_snapshot_records(&snapshot_d, &rd, &files, 0, "0x3fe0000000000000");
    assert_eq!(rd["snapshot"], r0["snapshot"]);
    assert_ne!(snapshot_d.one("provenance"), snapshot0.one("provenance"));
    let tenth = scratch.path("lr-tenth.snapshot");
    let rt = expect_normal(
        &scratch.run(&files.train(0, &tenth).arg("--adam-lr").arg("0.1"))?,
        "train",
    )?;
    check_snapshot_records(
        &Snapshot::read(&tenth)?,
        &rt,
        &files,
        0,
        "0x3fb999999999999a",
    );
    assert_eq!(
        std::fs::read_to_string(&files.model)?,
        MODEL,
        "the graph file is only read"
    );
    Ok(())
}

#[test]
fn train_contract_refusals_publish_nothing() -> TestResult {
    // NGRAPH-CASE-11: missing contract flags are usage errors; loss and data
    // interface, budget, input-fact, non-finite, guard, derivative-policy,
    // sub-profile and foreign-parent refusals report their D8 code and exit;
    // none creates `--out` or `--out.partial`; an opt-in admits the boolean
    // path and is recorded in the snapshot.
    let scratch = Scratch::new("train-refusals")?;
    let files = TrainFiles::new(&scratch)?;
    let out = scratch.path("refused.snapshot");

    for missing in [
        "--loss",
        "--data",
        "--policy",
        "--optimizer",
        "--seed",
        "--steps",
        "--out",
    ] {
        let flags: Vec<(&str, OsString)> = train_flags(&files.loss, &files.data, 1, &out)
            .into_iter()
            .filter(|(flag, _)| *flag != missing)
            .collect();
        expect_usage(&scratch.run(&with_flags(Ngraph::new("train").arg(&files.model), &flags))?);
        assert_no_publication(&out);
    }
    for (flag, malformed) in [("--seed", "not-a-seed"), ("--steps", "-1")] {
        let mut flags = train_flags(&files.loss, &files.data, 1, &out);
        for (name, value) in &mut flags {
            if *name == flag {
                *value = OsString::from(malformed);
            }
        }
        expect_usage(&scratch.run(&with_flags(Ngraph::new("train").arg(&files.model), &flags))?);
        assert_no_publication(&out);
    }
    let usage: [&[&str]; 4] = [
        &["--bogus"],
        &["--seed", SEED],
        &["--fuel", "1"],
        &["--reference"],
    ];
    for extra in usage {
        let mut command = files.train(1, &out);
        for flag in extra {
            command = command.arg(*flag);
        }
        expect_usage(&scratch.run(&command)?);
        assert_no_publication(&out);
    }

    let refused = |command: Ngraph, code: &str| -> TestResult {
        expect_code(&scratch.run(&command)?, "train", code)?;
        assert_no_publication(&out);
        Ok(())
    };
    let model = files.model.as_path();
    let (loss, data) = (files.loss.as_path(), files.data.as_path());
    for (name, text) in [
        ("loss-short.ngraph", LOSS_SHORT),
        ("loss-two.ngraph", LOSS_TWO_OUTPUTS),
        ("loss-param.ngraph", LOSS_WITH_PARAM),
    ] {
        let bad_loss = scratch.write(name, text)?;
        refused(
            train_command(model, &bad_loss, data, 3, &out),
            "E-INTERFACE",
        )?;
    }
    let wide = scratch.write(
        "wide.points",
        "noble-ngraph-data/0\ntypes f32 f32 ; f32\npoint 0 0 ; 1\n",
    )?;
    refused(train_command(model, loss, &wide, 3, &out), "E-INTERFACE")?;
    let schema = scratch.write(
        "schema.points",
        &DATA.replace("noble-ngraph-data/0", "noble-ngraph-data/1"),
    )?;
    refused(train_command(model, loss, &schema, 3, &out), "E-FORMAT")?;
    let no_split = scratch.write(
        "no-split.points",
        "noble-ngraph-data/0\ntypes f32\npoint 0 ; 1\n",
    )?;
    refused(train_command(model, loss, &no_split, 3, &out), "E-FORMAT")?;
    refused(
        train_command(model, loss, data, 1_000_001, &out),
        "E-BUDGET-STEPS",
    )?;
    refused(
        train_command(model, loss, data, 5, &out)
            .arg("--budget-steps")
            .arg("3"),
        "E-BUDGET-STEPS",
    )?;
    refused(
        train_command(model, loss, data, 5, &out)
            .arg("--budget-steps")
            .arg("1000001"),
        "E-PROFILE",
    )?;
    let out_of_range = scratch.write("out-of-range.points", DATA_OUT_OF_RANGE)?;
    refused(
        train_command(model, loss, &out_of_range, 3, &out),
        "E-FACT-INPUT",
    )?;
    let infinite = scratch.write("infinite.points", DATA_INFINITE_TARGET)?;
    refused(
        train_command(model, loss, &infinite, 1, &out),
        "E-NONFINITE-UPDATE",
    )?;

    let step = scratch.write("step.ngraph", STEP_MODEL)?;
    let step_data = scratch.write("step.points", STEP_DATA)?;
    refused(
        train_command(&step, loss, &step_data, 2, &out),
        "E-DERIV-BOOLEAN",
    )?;
    let nan = scratch.write("nan.points", STEP_DATA_NAN)?;
    refused(
        train_command(&step, loss, &nan, 2, &out)
            .arg("--opt-in")
            .arg("%4:boolean=straight-through"),
        "G-FACT-FALSE",
    )?;
    let i64_model = scratch.write("i64.ngraph", I64_MODEL)?;
    let i64_data = scratch.write("i64.points", I64_DATA)?;
    refused(
        train_command(&i64_model, loss, &i64_data, 2, &out),
        "E-DERIV-I64",
    )?;

    let other = scratch.write(
        "other.ngraph",
        &MODEL.replace("cli-train-model\n", "cli-train-model-other\n"),
    )?;
    let other_s0 = scratch.path("other-s0.snapshot");
    expect_normal(
        &scratch.run(&train_command(&other, loss, data, 0, &other_s0))?,
        "train",
    )?;
    refused(
        train_command(model, loss, data, 3, &out)
            .arg("--from")
            .arg(&other_s0),
        "E-SNAPSHOT-STALE",
    )?;

    let opted = scratch.path("opt-in.snapshot");
    let line = expect_normal(
        &scratch.run(
            &train_command(&step, loss, &step_data, 2, &opted)
                .arg("--opt-in")
                .arg("%4:boolean=straight-through"),
        )?,
        "train",
    )?;
    assert_eq!(line["steps"], 2);
    assert_published(&opted);
    assert_eq!(
        Snapshot::read(&opted)?.all("opt-in"),
        ["%4 boolean straight-through"],
        "D6 records the opt-in"
    );
    Ok(())
}

#[test]
fn snapshot_integrity_and_binding_refusals() -> TestResult {
    // NGRAPH-CASE-12: a tampered param or provenance field (E-SNAPSHOT-INTEGRITY),
    // an unknown schema (E-FORMAT), another graph or changed fact records
    // (E-SNAPSHOT-STALE) and a different pinned id (E-SNAPSHOT-ID) are refused
    // by `eval` and `verify` with one line and exit 2, before evaluation.
    let scratch = Scratch::new("snapshot-binding")?;
    let files = TrainFiles::new(&scratch)?;
    let s0 = scratch.path("s0.snapshot");
    let r0 = expect_normal(&scratch.run(&files.train(0, &s0))?, "train")?;
    let sn = scratch.path("s3.snapshot");
    let rn = expect_normal(&scratch.run(&files.train(3, &sn))?, "train")?;
    let text = std::fs::read_to_string(&sn)?;
    let vectors = scratch.write("model.vectors", &vector_file("f32", &["v 0.5"]))?;

    let other = scratch.write(
        "other.ngraph",
        &MODEL.replace("cli-train-model\n", "cli-train-model-other\n"),
    )?;
    let other_report = expect_normal(&scratch.run(&Ngraph::new("check").arg(&other))?, "check")?;
    assert_ne!(other_report["graph"], files.model_report["graph"]);
    assert_eq!(
        other_report["param_layout"],
        files.model_report["param_layout"]
    );
    let no_fact = scratch.write("no-fact.ngraph", &MODEL.replace(MODEL_FACT, ""))?;
    let no_fact_report =
        expect_normal(&scratch.run(&Ngraph::new("check").arg(&no_fact))?, "check")?;
    assert_eq!(
        no_fact_report["graph"], files.model_report["graph"],
        "fact records are not identity-bearing"
    );

    let tampered_param = scratch.write("tampered-param.snapshot", &flip_first_param(&text)?)?;
    let seed_changed = text.replace(&format!("seed {SEED_RECORD}"), "seed 0x0000000014140006");
    assert_ne!(seed_changed, text);
    let tampered_seed = scratch.write("tampered-seed.snapshot", &seed_changed)?;
    let schema = scratch.write(
        "schema.snapshot",
        &text.replacen("noble-ngraph-snapshot/0", "noble-ngraph-snapshot/1", 1),
    )?;
    let s0_id = r0["snapshot"].as_str().ok_or("S0 id")?;
    let cases: [(&Path, &Path, Option<&str>, &str); 6] = [
        (&files.model, &tampered_param, None, "E-SNAPSHOT-INTEGRITY"),
        (&files.model, &tampered_seed, None, "E-SNAPSHOT-INTEGRITY"),
        (&files.model, &schema, None, "E-FORMAT"),
        (&other, &sn, None, "E-SNAPSHOT-STALE"),
        (&no_fact, &sn, None, "E-SNAPSHOT-STALE"),
        (&files.model, &sn, Some(s0_id), "E-SNAPSHOT-ID"),
    ];
    for (graph, snapshot, pinned, code) in cases {
        let mut eval = Ngraph::new("eval")
            .arg(graph)
            .arg(&vectors)
            .arg("--snapshot")
            .arg(snapshot);
        let mut verify = Ngraph::new("verify").arg(graph).arg(snapshot);
        if let Some(id) = pinned {
            eval = eval.arg("--snapshot-id").arg(id);
            verify = verify.arg("--snapshot-id").arg(id);
        }
        let refusal = eval_refusal(&scratch.run(&eval)?, code, true)?;
        assert_eq!(refusal["outcome"], "reject", "{refusal}");
        expect_code(&scratch.run(&verify)?, "verify", code)?;
    }

    let sn_id = rn["snapshot"].as_str().ok_or("SN id")?;
    let pinned_eval = Ngraph::new("eval")
        .arg(&files.model)
        .arg(&vectors)
        .arg("--snapshot")
        .arg(&sn)
        .arg("--snapshot-id")
        .arg(sn_id);
    let (header, lines) = eval_lines(&scratch.run(&pinned_eval)?)?;
    assert_eq!(header["snapshot"], rn["snapshot"]);
    assert_eq!(lines[0]["outcome"], "normal");
    let line = expect_normal(
        &scratch.run(
            &Ngraph::new("verify")
                .arg(&files.model)
                .arg(&sn)
                .arg("--snapshot-id")
                .arg(sn_id),
        )?,
        "verify",
    )?;
    check_verify_report(&line, &rn["snapshot"], &files.model_report["graph"], false);
    assert_eq!(
        std::fs::read_to_string(&sn)?,
        text,
        "loading never modifies a snapshot"
    );
    Ok(())
}

#[test]
fn verify_binds_and_replays_provenance() -> TestResult {
    // NGRAPH-CASE-13: `verify` binds (`bound: true`); `--replay` reproduces the
    // seed-trained, the parent-trained and the opt-in snapshots
    // (`replayed: true`); changed data, another loss graph or a wrong or
    // missing parent is E-SNAPSHOT-STALE.
    let scratch = Scratch::new("verify-replay")?;
    let files = TrainFiles::new(&scratch)?;
    let graph = &files.model_report["graph"];
    let s0 = scratch.path("s0.snapshot");
    let r0 = expect_normal(&scratch.run(&files.train(0, &s0))?, "train")?;
    let sn = scratch.path("s25.snapshot");
    let rn = expect_normal(&scratch.run(&files.train(25, &sn))?, "train")?;
    let child = scratch.path("child.snapshot");
    let rc = expect_normal(
        &scratch.run(&files.train(5, &child).arg("--from").arg(&sn))?,
        "train",
    )?;

    for (snapshot, report) in [(&s0, &r0), (&sn, &rn), (&child, &rc)] {
        let command = Ngraph::new("verify").arg(&files.model).arg(snapshot);
        let run = scratch.run(&command)?;
        assert_eq!(scratch.run(&command)?.stdout, run.stdout, "D11");
        let line = expect_normal(&run, "verify")?;
        check_verify_report(&line, &report["snapshot"], graph, false);
    }
    for (snapshot, report) in [(&s0, &r0), (&sn, &rn)] {
        let command = files.replay(snapshot);
        let run = scratch.run(&command)?;
        assert_eq!(scratch.run(&command)?.stdout, run.stdout, "D11");
        let line = expect_normal(&run, "verify")?;
        check_verify_report(&line, &report["snapshot"], graph, true);
    }
    let line = expect_normal(
        &scratch.run(&files.replay(&child).arg("--from").arg(&sn))?,
        "verify",
    )?;
    check_verify_report(&line, &rc["snapshot"], graph, true);
    expect_code(
        &scratch.run(&files.replay(&child))?,
        "verify",
        "E-SNAPSHOT-STALE",
    )?;
    expect_code(
        &scratch.run(&files.replay(&child).arg("--from").arg(&s0))?,
        "verify",
        "E-SNAPSHOT-STALE",
    )?;

    let changed = scratch.write("changed.points", DATA_CHANGED)?;
    let other_loss = scratch.write(
        "other-loss.ngraph",
        &LOSS.replace("cli-train-mse\n", "cli-train-mse-other\n"),
    )?;
    for (loss, data) in [(&files.loss, &changed), (&other_loss, &files.data)] {
        let command = Ngraph::new("verify")
            .arg(&files.model)
            .arg(&sn)
            .arg("--replay")
            .arg("--loss")
            .arg(loss)
            .arg("--data")
            .arg(data);
        expect_code(&scratch.run(&command)?, "verify", "E-SNAPSHOT-STALE")?;
    }

    let step = scratch.write("step.ngraph", STEP_MODEL)?;
    let step_data = scratch.write("step.points", STEP_DATA)?;
    let opted = scratch.path("opt-in.snapshot");
    let ro = expect_normal(
        &scratch.run(
            &train_command(&step, &files.loss, &step_data, 2, &opted)
                .arg("--opt-in")
                .arg("%4:boolean=straight-through"),
        )?,
        "train",
    )?;
    let line = expect_normal(
        &scratch.run(
            &Ngraph::new("verify")
                .arg(&step)
                .arg(&opted)
                .arg("--replay")
                .arg("--loss")
                .arg(&files.loss)
                .arg("--data")
                .arg(&step_data),
        )?,
        "verify",
    )?;
    check_verify_report(&line, &ro["snapshot"], &ro["graph"], true);

    for command in [
        Ngraph::new("verify")
            .arg(&files.model)
            .arg(&sn)
            .arg("--replay")
            .arg("--data")
            .arg(&files.data),
        Ngraph::new("verify")
            .arg(&files.model)
            .arg(&sn)
            .arg("--replay")
            .arg("--loss")
            .arg(&files.loss),
        Ngraph::new("verify").arg(&files.model),
        Ngraph::new("verify")
            .arg(&files.model)
            .arg(&sn)
            .arg("--bogus"),
    ] {
        expect_usage(&scratch.run(&command)?);
    }
    Ok(())
}
