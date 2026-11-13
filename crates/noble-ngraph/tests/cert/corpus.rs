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

/// The generated file the foundation lane kernel-checks.
const COMMITTED: &str =
    include_str!("../../../../proofs/foundation/NobleFoundation/CertificateCorpus.lean");

/// The fixture's original graph, its emitted derivation and the emitted endpoint.
fn fixture() -> (Graph, Derivation, Graph) {
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
fn node_list(graph: &Graph) -> String {
    let terms: Vec<String> = graph.nodes.iter().map(|node| node_term(&node.op)).collect();
    std::format!("  [{}]", terms.join(",\n   "))
}

/// The output node indices the Rust graph declares.
fn outputs(graph: &Graph) -> Vec<u32> {
    graph.outputs.iter().map(|output| output.0).collect()
}

/// The derivation's steps as modelled `Step` literals, in stored order.
fn step_list(derivation: &Derivation) -> String {
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
fn wire_chunks(derivation: &Derivation) -> (String, Vec<String>) {
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
    assert_eq!(
        tokens.len(),
        cert::ENVELOPE_TOKENS + cert::STEP_TOKENS * derivation.steps.len()
    );
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
fn value_terms(observed: &[Value]) -> String {
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
fn input_terms(inputs: &[Value]) -> String {
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
fn observed_outputs(graph: &Graph, inputs: &[Value], expected: &[i64]) -> Vec<Value> {
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

const HEADER: &str = "import NobleFoundation.CertificateCodec
import NobleFoundation.FirstProjection

/-!
# The replayable-certificate fixture, kernel-checked at its wire tokens (issue #42)

Generated by `crates/noble-ngraph/tests/cert/corpus.rs` from the fixture `independent_folds` in
`crates/noble-ngraph/tests/cert/differential.rs`, its `cert::emit` derivation and the exact tokens
`cert::encode` wrote; do not edit by hand. `the_corpus_is_the_rust_fixture` fails when the fixture,
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

/// The whole generated Lean module.
fn lean_file() -> String {
    let (before, derivation, after) = fixture();
    let output_slots = outputs(&before);
    let (chunks, _) = wire_chunks(&derivation);
    let zero = [Value::I64(0), Value::I64(0)];
    let boundary = [Value::I64(-3), Value::I64(1)];
    let expected = |first: i64, second: i64| {
        let shifted = first.wrapping_mul(8).wrapping_add(i64::MIN);
        let difference = shifted.wrapping_sub(second);
        vec![3, 5, i64::MAX, shifted, difference, difference, second]
    };
    let at_zero = observed_outputs(&before, &zero, &expected(0, 0));
    let at_boundary = observed_outputs(&before, &boundary, &expected(-3, 1));
    let zero_after = observed_outputs(&after, &zero, &expected(0, 0));
    let boundary_after = observed_outputs(&after, &boundary, &expected(-3, 1));
    assert_eq!(at_zero, zero_after, "the folded endpoint agrees at zero");
    assert_eq!(
        at_boundary, boundary_after,
        "the folded endpoint agrees at the boundary input"
    );
    let slots = output_slots
        .iter()
        .map(u32::to_string)
        .collect::<Vec<String>>()
        .join(", ");
    let mut lean = String::from(HEADER);
    lean.push_str(&std::format!(
        "/-- The original endpoint's nodes, in stored order. -/\n\
         def certNodes : List Node :=\n{nodes}\n\n\
         /-- The original endpoint; the single `output` field names the subtraction, and the\n\
         full ordered output list is passed to `referenceOutputs` below. -/\n\
         def certGraph : Graph := NobleFoundation.FirstProjection.ofList certNodes 11\n\n\
         /-- The emitted derivation, decoded from the chunks below. -/\n\
         def certDerivation : Derivation where\n  \
         revision := \"{revision}\"\n  \
         model := \"{model}\"\n  \
         steps :=\n{steps}\n\n\
         /-- The exact seven-token chunks `cert::encode` wrote, in wire order. -/\n\
         def certChunks : List (List String) :=\n{chunks}\n\n\
         theorem certGraph_wellFormed : WellFormed certGraph :=\n  \
         NobleFoundation.FirstProjection.ofList_wellFormed certNodes 11 (by decide) (by decide)\n\n\
         theorem wire_envelope_decodes :\n    \
         decode \"{revision}\" \"{model}\" {count} certChunks = some certDerivation := by\n  \
         decide\n\n\
         theorem stale_model_token_refused :\n    \
         decode \"{revision}\" \"{other_model}\" {count} certChunks = none := by\n  \
         decide\n\n\
         theorem stale_revision_token_refused :\n    \
         decode \"{other_revision}\" \"{model}\" {count} certChunks = none := by\n  \
         decide\n\n\
         theorem derivation_is_accepted : accepted certGraph certDerivation = true := by\n  \
         decide\n\n\
         /-- The replay's node shapes are the Rust result graph's, node by node. -/\n\
         theorem replay_matches_the_rust_result :\n    \
         [{replay}] =\n      {result} := by\n  decide\n\n\
         /-- The model's reference values at the fixture's declared output slots, repeated slot\n\
         included, at the all-zero input. -/\n\
         theorem reference_outputs_at_zero :\n    \
         referenceOutputs certGraph (inputsOf {zero_inputs}) [{slots}] =\n      \
         {zero_values} := by\n  decide\n\n\
         /-- The same at a boundary input pair. -/\n\
         theorem reference_outputs_at_boundary :\n    \
         referenceOutputs certGraph (inputsOf {boundary_inputs}) [{slots}] =\n      \
         {boundary_values} := by\n  decide\n\n\
         /-- The model's replay theorem at this fixture: every node's denotation is preserved. -/\n\
         theorem accepted_endpoint_agreement :\n    \
         ∀ index, value certGraph (inputsOf {zero_inputs}) index =\n      \
         value (applyDerivation certGraph certDerivation) (inputsOf {zero_inputs}) index :=\n  \
         accepted_endpoint_equivalence certGraph certGraph_wellFormed certDerivation\n    \
         derivation_is_accepted (inputsOf {zero_inputs})\n\n\
         end NobleFoundation.Certificate.Corpus\n",
        nodes = node_list(&before),
        steps = step_list(&derivation),
        chunks = chunks,
        revision = cert::CERT_REVISION,
        model = cert::MODEL_REVISION,
        other_model = "ngraph-sem/1",
        other_revision = "ngraph-cert/1",
        count = derivation.steps.len(),
        replay = (0..before.nodes.len())
            .map(|index| std::format!("(applyDerivation certGraph certDerivation).node {index}"))
            .collect::<Vec<String>>()
            .join(",\n     "),
        result = node_list(&after),
        zero_inputs = input_terms(&zero),
        boundary_inputs = input_terms(&boundary),
        slots = slots,
        zero_values = value_terms(&at_zero),
        boundary_values = value_terms(&at_boundary),
    ));
    lean
}

#[test]
fn the_corpus_is_the_rust_fixture() {
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
fn the_corpus_uses_the_differential_fixture() {
    let (before, derivation, _) = fixture();
    assert_eq!(
        before.logical_id(&TestIds),
        differential::independent_folds().logical_id(&TestIds)
    );
    assert_eq!(derivation.steps.len(), 3);
}
