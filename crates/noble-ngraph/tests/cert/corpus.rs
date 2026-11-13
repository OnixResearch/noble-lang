#![allow(
    tigerstyle::assertion_density,
    tigerstyle::missing_const_fn,
    tigerstyle::no_panic,
    tigerstyle::no_unwrap,
    tigerstyle::non_trait_imports,
    tigerstyle::unbounded_collection_growth,
    reason = "Owner: noble-maintainers; this exporter writes one fixed fixture as Lean text and must stop at anything outside the exported slice, so expect/panic on unsupported input, owner-path imports and string building are the readable form."
)]
//! The certificate fixture exported into the vocabularies of `NobleFoundation.Certificate` and
//! `NobleFoundation.CertificateCodec` (issue #42). `CertificateCorpus.lean` is generated from the
//! fixture, its emitted derivation and the exact tokens `cert::encode` writes; its header states
//! what the kernel decides and which translation is trusted. Regenerate after a deliberate fixture
//! or format change with `cargo test -p noble-ngraph --test cert_differential -- --ignored
//! write_the_lean_file`, then rerun the foundation lane.

use noble_ngraph::cert::{self, Derivation};
use noble_ngraph::ir::{Graph, Literal, Op};
use noble_ngraph::sem::Value;

use super::differential::{self, TestIds};

#[path = "corpus/generator.rs"]
mod generator;

use generator::lean_file;

/// The generated file the foundation lane kernel-checks.
const COMMITTED: &str =
    include_str!("../../../../proofs/foundation/NobleFoundation/CertificateCorpus.lean");

/// The fixture's original graph, its emitted derivation and the emitted endpoint.
pub(super) fn fixture() -> (Graph, Derivation, Graph) {
    let before = differential::independent_folds();
    let (derivation, after) = differential::fold(&before);
    (before, derivation, after)
}

/// One 64-bit value as a hex `BitVec 64` literal.
fn hex(bits: u64) -> String {
    std::format!("0x{bits:016X}")
}

/// One Rust node operation as the modelled `Node` term it denotes.
fn node_term(op: &Op) -> String {
    match op {
        Op::Input(slot) => std::format!(".input {slot}"),
        Op::Const(Literal::I64(value)) => std::format!(".constant {}", hex(value.cast_unsigned())),
        Op::I64Add(first, second) => std::format!(".add {} {}", first.0, second.0),
        Op::I64Sub(first, second) => std::format!(".sub {} {}", first.0, second.0),
        Op::I64Mul(first, second) => std::format!(".mul {} {}", first.0, second.0),
        Op::I64Eq(first, second) => std::format!(".eq {} {}", first.0, second.0),
        other => panic!(
            "the certificate fixture is I64-only; found {}",
            other.name()
        ),
    }
}

/// A whole graph's nodes as a Lean list literal.
pub(super) fn node_list(graph: &Graph) -> String {
    let terms: Vec<String> = graph.nodes.iter().map(|node| node_term(&node.op)).collect();
    std::format!("  [{}]", terms.join(",\n   "))
}

/// The output node indices the Rust graph declares.
pub(super) fn outputs(graph: &Graph) -> Vec<u32> {
    graph.outputs.iter().map(|output| output.0).collect()
}

/// The derivation's steps as modelled `Step` literals, in stored order.
pub(super) fn step_list(derivation: &Derivation) -> String {
    let terms: Vec<String> = derivation
        .steps
        .iter()
        .map(|step| {
            std::format!(
                "⟨{}, {}, {}, {}, {}, {}, {}⟩",
                match step.rule {
                    cert::Rule::FoldAddConstants => 0,
                    cert::Rule::NeutralAddLeft => 1,
                    cert::Rule::NeutralMulRight => 2,
                },
                step.node.0,
                step.operands[0].0,
                step.operands[1].0,
                hex(step.first_bits),
                hex(step.second_bits),
                hex(step.claimed)
            )
        })
        .collect();
    std::format!("  [{}]", terms.join(",\n   "))
}

/// The wire envelope's seven-token chunks, and the tokens themselves.
pub(super) fn wire_chunks(derivation: &Derivation) -> (String, Vec<String>) {
    let wire = cert::encode(derivation);
    let decoded = cert::decode(&wire).expect("the emitted envelope decodes");
    assert_eq!(decoded.revision, derivation.revision);
    assert_eq!(decoded.model, derivation.model);
    assert_eq!(
        decoded.steps, derivation.steps,
        "the transported steps are the emitted ones"
    );
    let tokens: Vec<String> = wire.split(' ').map(String::from).collect();
    assert_eq!(tokens[0], cert::CERT_REVISION);
    assert_eq!(tokens[1], cert::MODEL_REVISION);
    assert_eq!(tokens[2], std::format!("{}", derivation.steps.len()));
    let step_token_count = cert::STEP_TOKENS
        .checked_mul(derivation.steps.len())
        .expect("the emitted derivation has at most MAX_STEPS");
    let envelope_token_count = cert::ENVELOPE_TOKENS
        .checked_add(step_token_count)
        .expect("the bounded certificate token count fits usize");
    assert_eq!(tokens.len(), envelope_token_count);
    let chunks: Vec<String> = tokens[cert::ENVELOPE_TOKENS..]
        .chunks(cert::STEP_TOKENS)
        .map(|chunk| {
            let quoted: Vec<String> = chunk
                .iter()
                .map(|token| std::format!("\"{token}\""))
                .collect();
            std::format!("[{}]", quoted.join(", "))
        })
        .collect();
    (std::format!("  [{}]", chunks.join(",\n   ")), tokens)
}

/// One observed output list as Lean `some (.i64 ...)` terms.
pub(super) fn value_terms(observed: &[Value]) -> String {
    let terms: Vec<String> = observed
        .iter()
        .map(|value| match value {
            Value::I64(bits) => std::format!("some (.i64 {})", hex(bits.cast_unsigned())),
            other => panic!("the certificate fixture produces I64 outputs; found {other:?}"),
        })
        .collect();
    std::format!("[{}]", terms.join(", "))
}

/// One input pair as a Lean `BitVec 64` list.
pub(super) fn input_terms(inputs: &[Value]) -> String {
    let terms: Vec<String> = inputs
        .iter()
        .map(|value| match value {
            Value::I64(bits) => hex(bits.cast_unsigned()),
            other => panic!("the certificate fixture consumes I64 inputs; found {other:?}"),
        })
        .collect();
    std::format!("[{}]", terms.join(", "))
}

/// The observed outputs of `graph` at `inputs`, checked against the independent expectation.
pub(super) fn observed_outputs(graph: &Graph, inputs: &[Value], expected: &[i64]) -> Vec<Value> {
    let (compiled, _, reference, _) =
        differential::observations(graph, inputs).expect("both endpoints are admitted");
    let expected = noble_ngraph::eval::Obs::Normal(
        expected
            .iter()
            .map(|value| Value::I64(*value))
            .collect::<Vec<Value>>(),
    );
    for (observation, endpoint) in [(&compiled, "compiled"), (&reference, "reference")] {
        assert_eq!(
            observation,
            &expected,
            "{endpoint} endpoint at {}",
            input_terms(inputs)
        );
    }
    let Some(outputs) = compiled.outputs() else {
        panic!("expected an admitted observation");
    };
    outputs.to_vec()
}

pub(super) const HEADER: &str = "import NobleFoundation.CertificateCodec
import NobleFoundation.FirstProjection

/-!
# The replayable-certificate fixture, kernel-checked at its wire tokens (issue #42)

Generated by `crates/noble-ngraph/tests/cert/corpus.rs` from the fixture `independent_folds` in
`crates/noble-ngraph/tests/cert/differential.rs`, its `cert::emit` derivation and the exact tokens
`cert::encode` wrote; do not edit by hand. `the_generated_lean_is_fresh` fails when the fixture,
the derivation or the envelope changes, until this file is regenerated and the foundation lane
rerun.

For this one fixture the kernel decides:

* `wire_envelope_decodes`: the exact seven-token chunks the Rust encoder wrote decode to the
  modelled derivation under the literal revision and model tokens below;
* `derivation_is_accepted`: the modelled checker accepts the decoded derivation;
* `replay_matches_the_rust_result`: `applyDerivation` produces, node by node, the shapes the Rust
  result graph has;
* `reference_outputs_at_zero` and `reference_outputs_at_boundary`: the model's
  `referenceOutputs` at the Rust graph's declared output slots equal the values the compiled and
  reference evaluators returned (the generator checked both against an independent expectation);
* `stale_model_token_refused` and `stale_revision_token_refused`: the same chunks under the other
  model or revision token decode to nothing, so a transported certificate cannot silently apply to
  a different format or semantic model.

`accepted_endpoint_agreement` is not decided: it instantiates
`Certificate.accepted_endpoint_equivalence` at this fixture, so the model's replay theorem covers
the accepted derivation, and `certGraph_wellFormed` supplies the graph premise through
`FirstProjection.ofList_wellFormed`.

The one trusted translation is reading each Rust node operation as its `Node` term, the output
list as indices and the input values as `inputsOf`. This is kernel agreement and model coverage at
these exact bytes; it is not a proof about `cert.rs`, its encoder or any other graph.
-/

namespace NobleFoundation.Certificate.Corpus

open NobleFoundation.PackedSchedule
open NobleFoundation.Certificate
open NobleFoundation.CertificateCodec

/-- The inputs a finite list of slot values denotes. -/
def inputsOf (values : List (BitVec 64)) (slot : Nat) : BitVec 64 :=
  values.getD slot 0

/-! ## The fixture -/

";

#[test]
fn the_generated_lean_is_fresh() {
    assert!(
        lean_file() == COMMITTED,
        "the Rust fixture, its derivation or its wire envelope changed: regenerate \
         CertificateCorpus.lean with `cargo test -p noble-ngraph --test cert_differential -- \
         --ignored write_the_lean_file` and rerun the foundation lane"
    );
}

#[test]
#[ignore = "writes proofs/foundation/NobleFoundation/CertificateCorpus.lean; run after a deliberate change"]
fn write_the_lean_file() {
    std::fs::write(
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../proofs/foundation/NobleFoundation/CertificateCorpus.lean"
        ),
        lean_file(),
    )
    .expect("generated file written");
}

/// The fixture's identity scheme and graph builder are the differential module's own, so the
/// corpus cannot drift onto a second fixture.
#[test]
fn the_generated_file_is_fixture_bound() {
    let (before, derivation, _) = fixture();
    assert_eq!(
        before.logical_id(&TestIds),
        differential::independent_folds().logical_id(&TestIds)
    );
    assert_eq!(derivation.steps.len(), 3);
}
