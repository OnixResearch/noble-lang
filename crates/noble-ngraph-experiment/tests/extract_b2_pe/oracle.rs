//! Node BigInt differential for original, residual, and re-admitted graphs.

use super::*;

struct Comparison<'a> {
    lowered: &'a Lowered,
    specialized: &'a pe::Specialized,
    original_inputs: &'a [Vec<Value>],
    dynamic_inputs: &'a [Vec<Value>],
    dynamic_vectors: &'a [Vec<i64>],
    subject: &'a str,
}

fn reference_outputs(oracle: &Oracle, run: &Comparison<'_>) -> Vec<Vec<String>> {
    let original = oracle
        .evaluate(run.lowered.extraction.checked.graph(), run.original_inputs)
        .unwrap_or_else(|error| panic!("{} original BigInt: {error}", run.subject));
    let residual = oracle
        .evaluate(run.specialized.residual.graph(), run.dynamic_inputs)
        .unwrap_or_else(|error| panic!("{} residual BigInt: {error}", run.subject));
    let readmitted = oracle
        .evaluate(run.specialized.readmitted.graph(), run.dynamic_inputs)
        .unwrap_or_else(|error| panic!("{} readmitted BigInt: {error}", run.subject));
    assert_eq!(
        original, residual,
        "{}: original vs residual BigInt",
        run.subject
    );
    assert_eq!(
        original, readmitted,
        "{}: original vs readmitted BigInt",
        run.subject
    );
    original
}

fn assert_evaluators_match(run: &Comparison<'_>, expected: &[Vec<String>]) {
    for ((dynamic, original_input), expected) in run
        .dynamic_vectors
        .iter()
        .zip(run.original_inputs)
        .zip(expected)
    {
        let dynamic_values: Vec<Value> = dynamic.iter().copied().map(Value::I64).collect();
        let (original_l3, _) = pe::observe_l3(&run.lowered.extraction.checked, original_input)
            .unwrap_or_else(|error| panic!("{} original L3: {error}", run.subject));
        let (original_l4, _) = pe::observe_l4(&run.lowered.compiled, original_input)
            .unwrap_or_else(|error| panic!("{} original L4: {error}", run.subject));
        let (residual_l3, _) = run
            .specialized
            .l3(&dynamic_values)
            .unwrap_or_else(|error| panic!("{} residual L3: {error}", run.subject));
        let (residual_l4, _) = run
            .specialized
            .l4(&dynamic_values)
            .unwrap_or_else(|error| panic!("{} residual L4: {error}", run.subject));
        let (readmitted_l3, _) = run
            .specialized
            .readmitted_l3(&dynamic_values)
            .unwrap_or_else(|error| panic!("{} readmitted L3: {error}", run.subject));
        let (readmitted_l4, _) = run
            .specialized
            .readmitted_l4(&dynamic_values)
            .unwrap_or_else(|error| panic!("{} readmitted L4: {error}", run.subject));
        for (leg, observed) in [
            ("original L3", cells(&original_l3)),
            ("original L4", cells(&original_l4)),
            ("residual L3", cells(&residual_l3)),
            ("residual L4", cells(&residual_l4)),
            ("readmitted L3", cells(&readmitted_l3)),
            ("readmitted L4", cells(&readmitted_l4)),
        ] {
            assert_eq!(&observed, expected, "{} {leg} on {dynamic:?}", run.subject);
        }
    }
}

fn bigint_differential(
    oracle: &Oracle,
    lowered: &Lowered,
    static_inputs: &[Option<i64>],
    dynamic_vectors: &[Vec<i64>],
    subject: &str,
) -> bool {
    let specialized = match lowered.specialized(static_inputs) {
        Ok(specialized) => specialized,
        Err(error)
            if error.contains("E-SUBJECT at Graph:")
                && error.contains("frontend node limit exceeded") =>
        {
            return false;
        }
        Err(error) => panic!("{subject} {static_inputs:?}: {error}"),
    };
    let original_inputs: Vec<Vec<Value>> = dynamic_vectors
        .iter()
        .map(|row| {
            specialized
                .merge(row)
                .unwrap_or_else(|| panic!("{subject}: merge refused {row:?}"))
        })
        .collect();
    let dynamic_inputs: Vec<Vec<Value>> = dynamic_vectors
        .iter()
        .map(|row| row.iter().copied().map(Value::I64).collect())
        .collect();
    let run = Comparison {
        lowered,
        specialized: &specialized,
        original_inputs: &original_inputs,
        dynamic_inputs: &dynamic_inputs,
        dynamic_vectors,
        subject,
    };
    let expected = reference_outputs(oracle, &run);
    assert_evaluators_match(&run, &expected);
    true
}

fn fixture_bigint(oracle: &Oracle, root: &str, inputs: usize, maps: Vec<Vec<Option<i64>>>) {
    let lowered = Lowered::new(&[], root, inputs).expect("fixture extracts");
    for map in maps {
        let dynamic = map.iter().filter(|entry| entry.is_none()).count();
        let vectors = vectors(VectorSpec {
            seed: fixture_seed(inputs),
            dynamic_inputs: dynamic,
            count: 1000,
        });
        assert!(
            bigint_differential(oracle, &lowered, &map, &vectors, root),
            "fixture residual source fits the ordinary frontend"
        );
    }
}

fn random_bigint(oracle: &Oracle) {
    let seed = 0x1404_32a0;
    let mut rng = SplitMix64::new(seed);
    let mut compared = 0usize;
    let mut source_budget_refusals = 0usize;
    for i in 0..16 {
        let program = random::program(seed, i);
        let lowered = Lowered::new(&program.definitions, &program.root, program.inputs)
            .unwrap_or_else(|error| panic!("program {i} {program:?}: {error}"));
        let identity = vec![None; program.inputs];
        let mut interleaved = vec![None; program.inputs];
        for leaf in (0..program.inputs).step_by(2) {
            interleaved[leaf] = Some(random::value(&mut rng));
        }
        for map in [identity, interleaved] {
            let dynamic = map.iter().filter(|entry| entry.is_none()).count();
            let vectors = vectors(VectorSpec {
                seed: seed ^ i,
                dynamic_inputs: dynamic,
                count: 1000,
            });
            if bigint_differential(
                oracle,
                &lowered,
                &map,
                &vectors,
                &format!("program {i} {:?}", program.root),
            ) {
                compared += 1;
            } else {
                source_budget_refusals += 1;
            }
        }
    }
    assert_eq!(
        compared
            .checked_add(source_budget_refusals)
            .expect("program count fits usize"),
        32,
        "sixteen programs, two static maps each"
    );
    assert!(
        compared >= 16,
        "at least half the seeded maps re-admit; compared={compared}, refusals={source_budget_refusals}"
    );
}

#[test]
#[ignore = "needs NOBLE_NODE (pinned Node) for the L2 BigInt oracle"]
fn node_bigint_agrees_with_specialized_evaluators() {
    let node = std::env::var("NOBLE_NODE").expect("NOBLE_NODE");
    let oracle = Oracle {
        node: format!("{node}/bin/node").into(),
    };
    for (root, inputs, maps) in [
        (
            "dup * swap 3 * +",
            2,
            vec![
                vec![None, None],
                vec![Some(5), None],
                vec![None, Some(7)],
                vec![Some(6), Some(7)],
            ],
        ),
        (
            "-",
            2,
            vec![vec![None, None], vec![Some(10), None], vec![None, Some(-4)]],
        ),
        ("9223372036854775807 1 +", 0, vec![vec![]]),
        ("2 + 3 +", 1, vec![vec![None], vec![Some(4)]]),
        ("3 =", 1, vec![vec![None], vec![Some(3)], vec![Some(4)]]),
        ("dup dup * *", 1, vec![vec![Some(2)], vec![None]]),
    ] {
        fixture_bigint(&oracle, root, inputs, maps);
    }
    random_bigint(&oracle);
}
