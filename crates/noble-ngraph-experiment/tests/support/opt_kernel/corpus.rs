use noble_ngraph_experiment::opt::{
    Candidate, Refusal, RefusalClass, StepDescriptor, TransitionInput, TransitionOutput,
};

const COMMITTED: &str =
    include_str!("../../../../../proofs/foundation/NobleFoundation/SgdReferenceCorpus.lean");

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
    assert_eq!(
        case.parameters.len(),
        case.gradients.len(),
        "{} must have matching parameter and gradient lanes",
        case.name
    );
    assert!(
        !case.parameters.is_empty(),
        "{} must exercise a lane",
        case.name
    );
    assert!(
        case.parameters.len() <= 64,
        "{} exceeds the envelope bound",
        case.name
    );

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
        (Expected::Accepted(expected), Ok(actual)) => accepted_observation(case, expected, actual),
        (expected @ Expected::Refused { .. }, Err(actual)) => {
            refused_observation(case, expected, actual)
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

fn accepted_observation(
    case: &Case,
    expected: &[u32],
    actual: TransitionOutput,
) -> Result<String, String> {
    if actual.parameters.as_slice() == expected && actual.state.is_empty() {
        Ok(format!(".accepted {}", bits_list(&actual.parameters)))
    } else {
        Err(format!(
            "{} produced {:?} with state {:?}, expected {expected:?} and empty state",
            case.name, actual.parameters, actual.state
        ))
    }
}

fn refused_observation(
    case: &Case,
    expected: &Expected,
    actual: Refusal,
) -> Result<String, String> {
    if let Expected::Refused {
        class,
        path,
        lean_location,
    } = expected
    {
        assert!(
            path.starts_with("$."),
            "{} refusal expectation needs a JSON path",
            case.name
        );
        assert!(
            lean_location.starts_with('.'),
            "{} Lean refusal expectation needs a projection",
            case.name
        );
        if actual.class == *class && actual.location == *path {
            return Ok(format!(".refused .{} {}", class.as_str(), lean_location));
        }
        return Err(format!(
            "{} refused {} at {}, expected {} at {}",
            case.name,
            actual.class.as_str(),
            actual.location,
            class.as_str(),
            path
        ));
    }
    Err(format!(
        "{} refused {actual:?} despite an accepted expectation",
        case.name
    ))
}

fn lean_file() -> String {
    assert!(
        CASES
            .iter()
            .any(|case| matches!(&case.expected, Expected::Accepted(_))),
        "the corpus must retain an accepted transition"
    );
    assert!(
        CASES
            .iter()
            .any(|case| matches!(&case.expected, Expected::Refused { .. })),
        "the corpus must retain a refusal transition"
    );

    let candidate =
        Candidate::parse(super::SGD_ENVELOPE.as_bytes()).expect("the fixed SGD envelope admits");
    let mut output = String::from(
        "import NobleFoundation.SgdReference\n\n/-!\n# Rust-to-Lean `ngraph-opt/1` finite comparison corpus (issue #58)\n\nGenerated by `crates/noble-ngraph-experiment/tests/support/opt_kernel/corpus.rs`; do not edit by hand.\nThe Rust test evaluates the exact candidate artifact on every row and emits each observed result.\nThe Lean kernel decides `SgdReference.checkStep` at those exact input bits. This is a finite\ncomparison, not a proof about the Rust checker, parser, interpreter, compiler or artifact boundary.\n-/\n\nnamespace NobleFoundation.Binary32.Sgd.Corpus\n\nopen NobleFoundation.Binary32.Sgd\n\n",
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
fn sgd_rows_match_the_committed_reference() {
    assert_eq!(lean_file(), COMMITTED);
}

#[test]
#[ignore = "writes the generated Lean comparison corpus after deliberate row or transition changes"]
fn write_sgd_reference() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let path = root.join("../../proofs/foundation/NobleFoundation/SgdReferenceCorpus.lean");
    std::fs::write(path, lean_file()).expect("write generated Lean corpus");
}
