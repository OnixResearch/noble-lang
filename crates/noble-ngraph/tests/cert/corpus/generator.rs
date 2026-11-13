use super::{
    fixture, input_terms, node_list, observed_outputs, outputs, step_list, value_terms,
    wire_chunks, HEADER,
};
use noble_ngraph::sem::Value;

struct FixtureText {
    zero_inputs: String,
    boundary_inputs: String,
    slots: String,
    chunks: String,
    zero_values: String,
    boundary_values: String,
}

fn fixture_text() -> FixtureText {
    let (before, derivation, after) = fixture();
    let (chunks, _) = wire_chunks(&derivation);
    let zero = [Value::I64(0), Value::I64(0)];
    let boundary = [Value::I64(-3), Value::I64(1)];
    let expected = |first: i64, second: i64| {
        let shifted = first.wrapping_mul(8).wrapping_add(i64::MIN);
        let difference = shifted.wrapping_sub(second);
        vec![3, 5, i64::MAX, shifted, difference, difference, second]
    };
    let zero_before = observed_outputs(&before, &zero, &expected(0, 0));
    let boundary_before = observed_outputs(&before, &boundary, &expected(-3, 1));
    assert_eq!(
        zero_before,
        observed_outputs(&after, &zero, &expected(0, 0))
    );
    assert_eq!(
        boundary_before,
        observed_outputs(&after, &boundary, &expected(-3, 1))
    );
    FixtureText {
        zero_inputs: input_terms(&zero),
        boundary_inputs: input_terms(&boundary),
        slots: outputs(&before)
            .iter()
            .map(u32::to_string)
            .collect::<Vec<String>>()
            .join(", "),
        chunks,
        zero_values: value_terms(&zero_before),
        boundary_values: value_terms(&boundary_before),
    }
}

fn render_theorems(values: &FixtureText, count: usize) -> String {
    std::format!(
        concat!(
            "theorem wire_envelope_decodes :\n    ",
            "decode \"{}\" \"{}\" {} certChunks = some certDerivation := by\n  decide\n\n",
            "theorem stale_model_token_refused :\n    ",
            "decode \"{}\" \"ngraph-sem/1\" {} certChunks = none := by\n  decide\n\n",
            "theorem stale_revision_token_refused :\n    ",
            "decode \"ngraph-cert/1\" \"{}\" {} certChunks = none := by\n  decide\n\n",
            "theorem derivation_is_accepted : accepted certGraph certDerivation = true := by\n  decide\n\n",
            "/-- The model's reference values at the fixture's declared output slots, repeated slot\n",
            "included, at the all-zero input. -/\n",
            "theorem reference_outputs_at_zero :\n    ",
            "referenceOutputs certGraph (inputsOf {}) [{}] = {} := by\n  decide\n\n",
            "/-- The same at a boundary input pair. -/\n",
            "theorem reference_outputs_at_boundary :\n    ",
            "referenceOutputs certGraph (inputsOf {}) [{}] = {} := by\n  decide\n\n",
            "/-- The model's replay theorem at this fixture: every node's denotation is preserved. -/\n",
            "theorem accepted_endpoint_agreement :\n    ",
            "∀ index, value certGraph (inputsOf {}) index =\n      ",
            "value (applyDerivation certGraph certDerivation) (inputsOf {}) index :=\n  ",
            "accepted_endpoint_equivalence certGraph certGraph_wellFormed certDerivation\n    ",
            "derivation_is_accepted (inputsOf {})\n\n",
            "end NobleFoundation.Certificate.Corpus\n"
        ),
        noble_ngraph::cert::CERT_REVISION,
        noble_ngraph::cert::MODEL_REVISION,
        count,
        noble_ngraph::cert::CERT_REVISION,
        count,
        noble_ngraph::cert::MODEL_REVISION,
        count,
        values.zero_inputs,
        values.slots,
        values.zero_values,
        values.boundary_inputs,
        values.slots,
        values.boundary_values,
        values.zero_inputs,
        values.zero_inputs,
        values.zero_inputs,
    )
}

fn render_module(values: &FixtureText) -> String {
    let (before, derivation, after) = fixture();
    let mut lean = String::from(HEADER);
    lean.push_str(&std::format!(
        concat!(
            "/-- The original endpoint's nodes, in stored order. -/\n",
            "def certNodes : List Node :=\n{}\n\n",
            "/-- The original endpoint; the single `output` field names the subtraction, and the\n",
            "full ordered output list is passed to `referenceOutputs` below. -/\n",
            "def certGraph : Graph := NobleFoundation.FirstProjection.ofList certNodes 11\n\n",
            "/-- The emitted derivation, decoded from the chunks below. -/\n",
            "def certDerivation : Derivation where\n  revision := \"{}\"\n  model := \"{}\"\n  steps :=\n{}\n\n",
            "/-- The exact seven-token chunks `cert::encode` wrote, in wire order. -/\n",
            "def certChunks : List (List String) :=\n{}\n\n",
            "theorem certGraph_wellFormed : WellFormed certGraph :=\n  ",
            "NobleFoundation.FirstProjection.ofList_wellFormed certNodes 11 (by decide) (by decide)\n\n",
            "/-- The replay's node shapes are the Rust result graph's, node by node. -/\n",
            "theorem replay_matches_the_rust_result :\n    ",
            "[{}] =\n      {} := by\n  decide\n\n"
        ),
        node_list(&before),
        noble_ngraph::cert::CERT_REVISION,
        noble_ngraph::cert::MODEL_REVISION,
        step_list(&derivation),
        values.chunks,
        (0..before.nodes.len())
            .map(|index| std::format!("(applyDerivation certGraph certDerivation).node {index}"))
            .collect::<Vec<String>>()
            .join(",\n     "),
        node_list(&after),
    ));
    lean.push_str(&render_theorems(values, derivation.steps.len()));
    lean
}

/// The generated Lean source for the fixture and its kernel-decided expectations.
pub(super) fn lean_file() -> String {
    render_module(&fixture_text())
}
