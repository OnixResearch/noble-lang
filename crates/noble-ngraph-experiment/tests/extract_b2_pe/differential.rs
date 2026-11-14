//! Finite differential and refusal controls for checked-graph specialization.

use super::*;

#[test]
fn model_fixtures_agree_across_residual_and_readmitted_graph() {
    type Map = Vec<Option<i64>>;
    let cases: Vec<(&str, usize, Vec<Map>)> = vec![
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
    ];
    for (root, inputs, maps) in cases {
        let lowered = Lowered::new(&[], root, inputs).expect("extracts");
        for map in &maps {
            let dynamic_inputs = map.iter().filter(|entry| entry.is_none()).count();
            let rows = vectors(VectorSpec {
                seed: fixture_seed(inputs),
                dynamic_inputs,
                count: 1000,
            });
            let counts = pe::differential(&lowered, map, &rows)
                .unwrap_or_else(|error| panic!("{root} {map:?}: {error}"));
            assert!(
                counts.agree,
                "{root} {map:?}: {}",
                counts.first_mismatch.unwrap_or_default()
            );
            assert_eq!(counts.vectors, 1000, "{root} {map:?}");
            assert!(counts.l3_original >= counts.l3_residual, "{root} {map:?}");
            if root == "dup * swap 3 * +" && map.iter().all(Option::is_some) {
                assert!(counts.l3_residual < counts.l3_original, "{root} {map:?}");
                assert!(counts.l4_residual < counts.l4_original, "{root} {map:?}");
            }
        }
    }
}

#[test]
fn seeded_core_programs_agree_across_residual_and_readmitted_graph() {
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
            let dynamic_inputs = map.iter().filter(|entry| entry.is_none()).count();
            let rows = vectors(VectorSpec {
                seed: seed ^ i,
                dynamic_inputs,
                count: 200,
            });
            let counts = match pe::differential(&lowered, &map, &rows) {
                Ok(counts) => counts,
                // The ordinary frontend can refuse emitted source that exceeds
                // its expansion bound even when the graph itself fits.
                Err(error)
                    if error.contains("E-SUBJECT at Graph:")
                        && error.contains("frontend node limit exceeded") =>
                {
                    source_budget_refusals += 1;
                    continue;
                }
                Err(error) => panic!("program {i} {program:?} {map:?}: {error}"),
            };
            assert!(
                counts.agree,
                "program {i} {program:?} {map:?}: {}",
                counts.first_mismatch.unwrap_or_default()
            );
            compared += 1;
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
        "at least half of the seeded maps re-admit; compared={compared}, source-budget refusals={source_budget_refusals}"
    );
}

#[test]
fn extraction_refusals_precede_graph_construction() {
    let cases = noble_ngraph_experiment::controls::run();
    for (name, code) in [
        ("test.emit", "E-EFFECT"),
        ("resource-input", "E-RESOURCE"),
        ("self-recursion", "E-RECURSION"),
    ] {
        let case = cases
            .iter()
            .find(|case| case.name == name)
            .unwrap_or_else(|| panic!("missing control {name}"));
        assert_eq!(case.observed(), code, "{name}: {case:?}");
        assert!(case.result.is_err(), "{name} produced a graph");
        if name == "resource-input" {
            assert!(matches!(
                &case.ordinary,
                noble_ngraph_experiment::controls::Ordinary::NotExpressible(_)
            ));
        }
        if name == "self-recursion" {
            assert!(matches!(
                &case.ordinary,
                noble_ngraph_experiment::controls::Ordinary::Session(lines)
                    if lines.len() == 2
                        && lines[0] == "def f [ f ]"
                        && lines[1] == "5 f"
            ));
            let ordinary = noble_contracts::source::Session::new().prepare(
                b"def f [ f ]",
                &[],
                noble_contracts::Limits {
                    bytes: 65_536,
                    nodes: 16_384,
                    depth: 64,
                    work: 2_000_000,
                },
            );
            assert!(
                ordinary.is_err(),
                "ordinary source path refuses unsigned recursion"
            );
        }
    }
    assert!(Lowered::new(&[], "1 + \"hi\" test.emit", 1).is_err());
    assert!(Lowered::new(&[], "drop true [ 1 ] [ 2 ] if", 1).is_err());
    assert!(Lowered::new(&[], "drop [ 1 + ]", 1).is_err());
}
