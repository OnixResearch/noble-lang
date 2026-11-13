//! Rust candidate observations exported to the Lean `checkStep` corpus.

use noble_ngraph_experiment::opt::{
    classify_bits, evaluate_frozen, Candidate, EvaluationRow, FrozenCandidate, FrozenEvaluation,
    NonFinite, RefusalClass, StepDescriptor, TransitionInput,
};

const SGD_ENVELOPE: &str = r#"{
  "format":"ngraph-opt/1","n":2,
  "inputs":["p","g","eta","step","state"],"state":"State0","effects":[],
  "budgets":{"nodes":16,"state_bytes":0,"work":128,"output_lanes":64},
  "nodes":[
    {"id":0,"op":"broadcast","operands":["$eta"],"type":"F32[n]"},
    {"id":1,"op":"Binary32.mul","operands":[0,"$g"],"type":"F32[n]"},
    {"id":2,"op":"Binary32.sub","operands":["$p",1],"type":"F32[n]"}
  ],"outputs":{"parameters":2,"state":"State0"}
}"#;

const COMMITTED: &str =
    include_str!("../../../proofs/foundation/NobleFoundation/SgdReferenceCorpus.lean");

struct Case {
    name: &'static str,
    eta: u32,
    parameters: &'static [u32],
    gradients: &'static [u32],
    expected: Expected,
}

enum Expected {
    Accepted(&'static [u32]),
    Refused {
        class: RefusalClass,
        path: &'static str,
        lean_location: &'static str,
    },
}

const CASES: [Case; 8] = [
    Case {
        name: "normal",
        eta: 0x3f00_0000,
        parameters: &[0x3f80_0000, 0x4000_0000],
        gradients: &[0x3f80_0000, 0xc000_0000],
        expected: Expected::Accepted(&[0x3f00_0000, 0x4040_0000]),
    },
    Case {
        name: "signed_zero",
        eta: 0x3f00_0000,
        parameters: &[0x8000_0000, 0x0000_0000],
        gradients: &[0x0000_0000, 0x0000_0000],
        expected: Expected::Accepted(&[0x8000_0000, 0x0000_0000]),
    },
    Case {
        name: "subnormal",
        eta: 0x3f00_0000,
        parameters: &[0x0080_0000, 0x0080_0000],
        gradients: &[0x0080_0000, 0x0000_0000],
        expected: Expected::Accepted(&[0x0040_0000, 0x0080_0000]),
    },
    Case {
        name: "boundary_tie",
        eta: 0x3f80_0000,
        parameters: &[0x3f80_0000, 0x3f80_0000],
        gradients: &[0xb380_0000, 0x0000_0000],
        expected: Expected::Accepted(&[0x3f80_0000, 0x3f80_0000]),
    },
    Case {
        name: "overflow_output",
        eta: 0x3f80_0000,
        parameters: &[0x3f80_0000, 0x7f7f_ffff],
        gradients: &[0x3f80_0000, 0xff7f_ffff],
        expected: Expected::Refused {
            class: RefusalClass::Unsupported,
            path: "$.outputs.parameters[1]",
            lean_location: ".outputParam 1",
        },
    },
    Case {
        name: "nan_parameter",
        eta: 0x3f00_0000,
        parameters: &[0x3f80_0000, 0x7fc0_0000],
        gradients: &[0x3f80_0000, 0x3f80_0000],
        expected: Expected::Refused {
            class: RefusalClass::Unsupported,
            path: "$.inputs.p[1]",
            lean_location: ".param 1",
        },
    },
    Case {
        name: "infinite_gradient",
        eta: 0x3f00_0000,
        parameters: &[0x3f80_0000, 0x4000_0000],
        gradients: &[0x7f80_0000, 0x3f80_0000],
        expected: Expected::Refused {
            class: RefusalClass::Unsupported,
            path: "$.inputs.g[0]",
            lean_location: ".grad 0",
        },
    },
    Case {
        name: "infinite_eta",
        eta: 0x7f80_0000,
        parameters: &[0x3f80_0000, 0x4000_0000],
        gradients: &[0x3f80_0000, 0x3f80_0000],
        expected: Expected::Refused {
            class: RefusalClass::Unsupported,
            path: "$.inputs.eta",
            lean_location: ".eta",
        },
    },
];

fn bits_list(values: &[u32]) -> String {
    let values = values
        .iter()
        .map(|bits| format!("0x{bits:08X}"))
        .collect::<Vec<_>>()
        .join(", ");
    format!("[{values}]")
}

fn expected_lean(case: &Case, candidate: &Candidate) -> Result<String, String> {
    let input = TransitionInput {
        parameters: case.parameters,
        gradients: case.gradients,
        eta: case.eta,
        step: StepDescriptor {
            version: 1,
            step_index: 0,
            batch_index: 0,
        },
        state: &[],
    };
    let observed = candidate.evaluate(&input);
    match (&case.expected, observed) {
        (Expected::Accepted(expected), Ok(actual)) => {
            if actual.parameters.as_slice() == *expected && actual.state.is_empty() {
                Ok(format!(".accepted {}", bits_list(&actual.parameters)))
            } else {
                Err(format!(
                    "{} produced {:?} with state {:?}, expected {expected:?} and empty state",
                    case.name, actual.parameters, actual.state
                ))
            }
        }
        (
            Expected::Refused {
                class,
                path,
                lean_location,
            },
            Err(actual),
        ) => {
            if actual.class == *class && actual.location == *path {
                Ok(format!(".refused .{} {}", class.as_str(), lean_location))
            } else {
                Err(format!(
                    "{} refused {} at {}, expected {} at {}",
                    case.name,
                    actual.class.as_str(),
                    actual.location,
                    class.as_str(),
                    path
                ))
            }
        }
        (Expected::Accepted(expected), Err(actual)) => Err(format!(
            "{} refused {actual:?}; expected {expected:?}",
            case.name
        )),
        (Expected::Refused { .. }, Ok(actual)) => Err(format!(
            "{} accepted {:?}; expected refusal",
            case.name, actual.parameters
        )),
    }
}

fn lean_file() -> String {
    let candidate =
        Candidate::parse(SGD_ENVELOPE.as_bytes()).expect("the fixed SGD envelope admits");
    let mut output = String::from(
        "import NobleFoundation.SgdReference\n\n/-!\n# Rust-to-Lean `ngraph-opt/1` finite comparison corpus (issue #58)\n\nGenerated by `crates/noble-ngraph-experiment/tests/opt_kernel.rs`; do not edit by hand.\nThe Rust test evaluates the exact candidate artifact on every row and emits each observed result.\nThe Lean kernel decides `SgdReference.checkStep` at those exact input bits. This is a finite\ncomparison, not a proof about the Rust checker, parser, interpreter, compiler or artifact boundary.\n-/\n\nnamespace NobleFoundation.Binary32.Sgd.Corpus\n\nopen NobleFoundation.Binary32.Sgd\n\n",
    );
    for case in &CASES {
        let result = expected_lean(case, &candidate).expect("candidate observation is expected");
        output.push_str(&format!(
            "theorem rust_candidate_{} :\n  checkStep 0x{:08X} {} {} = {} := by\n  decide +kernel\n\n",
            case.name,
            case.eta,
            bits_list(case.parameters),
            bits_list(case.gradients),
            result,
        ));
    }
    output.push_str("end NobleFoundation.Binary32.Sgd.Corpus\n");
    output
}

#[test]
fn rust_candidate_rows_are_kernel_decided_and_the_corpus_is_fresh() {
    assert_eq!(lean_file(), COMMITTED);
}

#[test]
#[ignore = "writes the generated Lean comparison corpus after deliberate row or transition changes"]
fn write_the_lean_corpus() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let path = root.join("../../proofs/foundation/NobleFoundation/SgdReferenceCorpus.lean");
    std::fs::write(path, lean_file()).expect("write generated Lean corpus");
}

#[test]
fn parser_controls_refuse_at_the_first_offending_field() {
    let effectful = SGD_ENVELOPE.replace("\"effects\":[]", "\"effects\":[\"read\"]");
    assert_refusal(&effectful, RefusalClass::Unsupported, "$.effects[0]");

    let opcode = SGD_ENVELOPE.replace("Binary32.mul", "Binary32.add");
    assert_refusal(&opcode, RefusalClass::Unsupported, "$.nodes[1].op");

    let duplicate = SGD_ENVELOPE.replace(
        "\"id\":1,\"op\":\"Binary32.mul\"",
        "\"id\":0,\"op\":\"Binary32.mul\"",
    );
    assert_refusal(&duplicate, RefusalClass::Malformed, "$.nodes[1].id");

    let forward = SGD_ENVELOPE.replace("[\"$p\",1]", "[\"$p\",9]");
    assert_refusal(&forward, RefusalClass::Malformed, "$.nodes[2].operands[1]");

    let widened_budget = SGD_ENVELOPE.replace("\"work\":128", "\"work\":129");
    assert_refusal(&widened_budget, RefusalClass::Malformed, "$.budgets.work");
    let duplicate_field = SGD_ENVELOPE.replace(
        "\"format\":\"ngraph-opt/1\"",
        "\"format\":\"ngraph-opt/1\",\"format\":\"ngraph-opt/1\"",
    );
    assert_refusal(&duplicate_field, RefusalClass::Malformed, "$.format");
    let foreign_capability = SGD_ENVELOPE.replace(
        "\"outputs\":{",
        "\"capability\":\"filesystem\",\"outputs\":{",
    );
    assert_refusal(
        &foreign_capability,
        RefusalClass::Unsupported,
        "$.capability",
    );
    assert_refusal("{", RefusalClass::Malformed, "$.envelope");
}

#[test]
fn parser_enforces_the_fixed_node_work_and_output_budgets() {
    let too_many_nodes = (0..17)
        .map(|id| format!("{{\"id\":{id},\"op\":\"constant\",\"operands\":[],\"type\":\"F32\",\"bits\":\"0x3F800000\"}}"))
        .collect::<Vec<_>>()
        .join(",");
    let over_nodes = envelope(2, &too_many_nodes, 0);
    assert_refusal(&over_nodes, RefusalClass::Exhausted, "$.nodes");

    let work_nodes = r#"{"id":0,"op":"Binary32.mul","operands":["$p","$g"],"type":"F32[n]"},{"id":1,"op":"Binary32.sub","operands":["$p",0],"type":"F32[n]"},{"id":2,"op":"Binary32.mul","operands":["$p","$g"],"type":"F32[n]"}"#;
    let over_work = envelope(64, work_nodes, 2);
    assert_refusal(&over_work, RefusalClass::Exhausted, "$.work");

    let over_lanes = SGD_ENVELOPE.replace("\"n\":2", "\"n\":65");
    assert_refusal(&over_lanes, RefusalClass::Exhausted, "$.outputs");
}

#[test]
fn nonfinite_classification_preserves_finite_bit_patterns() {
    assert_eq!(classify_bits(0x7f80_0000), Some(NonFinite::Infinity));
    assert_eq!(classify_bits(0xff80_0000), Some(NonFinite::Infinity));
    assert_eq!(classify_bits(0x7fc0_0000), Some(NonFinite::NaN));
    assert_eq!(classify_bits(0x8000_0000), None);
    assert_eq!(classify_bits(0x0000_0001), None);
}

#[test]
fn runtime_shape_and_state_controls_fail_closed() {
    let candidate = Candidate::parse(SGD_ENVELOPE.as_bytes()).expect("fixed candidate admits");

    let short = TransitionInput {
        parameters: &[0x3f80_0000],
        gradients: &[0x3f80_0000, 0x3f80_0000],
        eta: 0x3f00_0000,
        step: StepDescriptor::new(1, 0, 0).expect("descriptor"),
        state: &[],
    };
    let refusal = candidate
        .evaluate(&short)
        .expect_err("shape mismatch refuses");
    assert_eq!(refusal.class, RefusalClass::Unsupported);
    assert_eq!(refusal.location, "$.inputs.p");

    let state = TransitionInput {
        parameters: &[0x3f80_0000, 0x4000_0000],
        gradients: &[0x3f80_0000, 0x3f80_0000],
        eta: 0x3f00_0000,
        step: StepDescriptor::new(1, 0, 0).expect("descriptor"),
        state: &[1],
    };
    let refusal = candidate
        .evaluate(&state)
        .expect_err("nonempty state refuses");
    assert_eq!(refusal.class, RefusalClass::Exhausted);
    assert_eq!(refusal.location, "$.state");
}

#[test]
fn step_descriptor_version_is_fixed() {
    let refusal = StepDescriptor::new(2, 0, 0).expect_err("descriptor version refuses");
    assert_eq!(refusal.class, RefusalClass::Malformed);
    assert_eq!(refusal.location, "$.inputs.step.version");
}

fn evaluation_rows() -> Vec<EvaluationRow> {
    vec![
        EvaluationRow {
            parameters: vec![0x3f80_0000, 0x4000_0000],
            gradients: vec![0x3f80_0000, 0xc000_0000],
            eta: 0x3f00_0000,
            step: StepDescriptor::new(1, 0, 0).expect("descriptor"),
        },
        EvaluationRow {
            parameters: vec![0x4040_0000, 0x3f80_0000],
            gradients: vec![0x4000_0000, 0x3f80_0000],
            eta: 0x3e80_0000,
            step: StepDescriptor::new(1, 1, 1).expect("descriptor"),
        },
    ]
}

fn transition_input(row: &EvaluationRow) -> TransitionInput<'_> {
    TransitionInput {
        parameters: &row.parameters,
        gradients: &row.gradients,
        eta: row.eta,
        step: row.step,
        state: &[],
    }
}

fn assert_restarted(rows: &[noble_ngraph_experiment::opt::EvaluationRowResult], state: &[u8]) {
    let output = std::process::Command::new(std::env::current_exe().expect("test executable"))
        .args([
            "opt_kernel::subprocess_reload_probe",
            "--exact",
            "--nocapture",
        ])
        .env("NGRAPH_OPT_RESTART_PROBE", "1")
        .output()
        .expect("spawn clean test process");
    assert!(
        output.status.success(),
        "restart probe failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout)
            .contains(&format!("restart-observation=({rows:?}, {state:?})")),
        "restarted evaluation differs: {}",
        String::from_utf8_lossy(&output.stdout),
    );
}

#[test]
fn frozen_candidate_results_ignore_unrelated_candidates_order_and_process_restart() {
    let candidate = Candidate::parse(SGD_ENVELOPE.as_bytes()).expect("fixed candidate admits");
    let frozen = FrozenCandidate::freeze(&candidate);
    let evaluation = FrozenEvaluation::freeze(evaluation_rows());
    let first = evaluate_frozen(&frozen, &evaluation).expect("first frozen evaluation");
    let repeated_input = evaluation_rows().remove(0);
    let before_unrelated = candidate
        .evaluate(&transition_input(&repeated_input))
        .expect("explicit transition");
    assert!(before_unrelated.state.is_empty());

    let unrelated = SGD_ENVELOPE.replace(
        "\"parameters\":2,\"state\":\"State0\"",
        "\"parameters\":0,\"state\":\"State0\"",
    );
    let unrelated = Candidate::parse(unrelated.as_bytes()).expect("unrelated candidate admits");
    let _ = evaluate_frozen(&FrozenCandidate::freeze(&unrelated), &evaluation)
        .expect("unrelated candidate evaluation");
    let repeated = evaluate_frozen(&frozen, &evaluation).expect("repeated frozen evaluation");
    assert_eq!(first, repeated);
    assert_ne!(first.rows[0].candidate, first.rows[1].candidate);
    let after_unrelated = candidate
        .evaluate(&transition_input(&repeated_input))
        .expect("explicit transition after unrelated candidate");
    assert_eq!(before_unrelated.parameters, after_unrelated.parameters);
    assert_eq!(before_unrelated.state, after_unrelated.state);

    let mut reversed_rows = evaluation_rows();
    reversed_rows.reverse();
    let reversed = evaluate_frozen(&frozen, &FrozenEvaluation::freeze(reversed_rows))
        .expect("reordered frozen evaluation");
    assert_eq!(
        reversed.rows,
        first.rows.iter().rev().cloned().collect::<Vec<_>>()
    );

    assert_restarted(&first.rows, &before_unrelated.state);
}

#[test]
fn subprocess_reload_probe() {
    if std::env::var_os("NGRAPH_OPT_RESTART_PROBE").is_none() {
        return;
    }
    let candidate = Candidate::parse(SGD_ENVELOPE.as_bytes()).expect("candidate reloaded");
    let frozen = FrozenCandidate::from_parts(
        std::sync::Arc::from(SGD_ENVELOPE.as_bytes()),
        candidate.id(),
    )
    .expect("exact candidate bytes verify after process start");
    let result = evaluate_frozen(&frozen, &FrozenEvaluation::freeze(evaluation_rows()))
        .expect("restarted frozen evaluation");
    let input = evaluation_rows().remove(0);
    let transition = candidate
        .evaluate(&transition_input(&input))
        .expect("restarted transition");
    println!(
        "restart-observation=({:?}, {:?})",
        result.rows, transition.state
    );
}

fn envelope(lanes: usize, nodes: &str, output: u16) -> String {
    format!(
        r#"{{"format":"ngraph-opt/1","n":{lanes},"inputs":["p","g","eta","step","state"],"state":"State0","effects":[],"budgets":{{"nodes":16,"state_bytes":0,"work":128,"output_lanes":64}},"nodes":[{nodes}],"outputs":{{"parameters":{output},"state":"State0"}}}}"#
    )
}

fn assert_refusal(source: &str, class: RefusalClass, location: &str) {
    let refusal = Candidate::parse(source.as_bytes()).expect_err("candidate refuses");
    assert_eq!(refusal.class, class);
    assert_eq!(refusal.location, location);
}
