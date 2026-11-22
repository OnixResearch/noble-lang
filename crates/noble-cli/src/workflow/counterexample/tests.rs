fn prepare(source: &str) -> noble_contracts::Prepared {
    let limits = noble_contracts::Limits {
        bytes: 65_536,
        nodes: 16_384,
        depth: 64,
        work: 2_000_000,
    };
    match noble_contracts::prepare(source.as_bytes(), limits) {
        Ok(prepared) => prepared,
        Err(error) => panic!("fixture must prepare: {}", error.message),
    }
}

const MONOTONIC: &str = "(contract 1 invalid-unconditional-monotonicity
  (input (x I64))
  (output (y I64))
  (program [ 1 + ])
  (requires true)
  (ensures (lt (in x) (out y))))";

#[test]
fn boundary_finds_the_wrapping_maximum_for_the_monotonic_fixture() {
    let prepared = prepare(MONOTONIC);
    let plan = match super::plan(&prepared) {
        Ok(plan) => plan,
        Err(failure) => panic!("plan: {}", failure.message),
    };
    let found = super::boundary_candidates(&plan, &prepared);
    let pairs: std::vec::Vec<(i64, i64)> = found
        .iter()
        .map(|candidate| (candidate.input, candidate.output))
        .collect();
    assert_eq!(pairs, std::vec![(i64::MAX, i64::MIN)]);
    assert!(
        found[0].has_wrapped,
        "the maximum input wraps to the minimum"
    );
    let source = super::observation_source(&plan, &found[0]);
    assert_eq!(source, "9223372036854775807 1 +\n");
    let module = super::refutation_module(&plan, &prepared, &found[0]);
    assert!(module.contains("(input := BitVec.ofInt 64 (9223372036854775807))"));
    assert!(module.contains("(output := BitVec.ofInt 64 (-9223372036854775808))"));
    assert!(module.contains("(Run.cons Step.lit (Run.cons Step.add Run.nil))"));
}

#[test]
fn a_true_postcondition_yields_no_candidate() {
    let prepared = prepare(
        "(contract 1 increments
  (input (x I64))
  (output (y I64))
  (program [ 1 + ])
  (requires true)
  (ensures (eq (out y) (add (in x) 1))))",
    );
    let plan = match super::plan(&prepared) {
        Ok(plan) => plan,
        Err(failure) => panic!("plan: {}", failure.message),
    };
    assert!(super::boundary_candidates(&plan, &prepared).is_empty());
}

#[test]
fn the_precondition_filters_candidates() {
    let prepared = prepare(
        "(contract 1 guarded-monotonicity
  (input (x I64))
  (output (y I64))
  (program [ 1 + ])
  (requires (lt (in x) 9223372036854775807))
  (ensures (lt (in x) (out y))))",
    );
    let plan = match super::plan(&prepared) {
        Ok(plan) => plan,
        Err(failure) => panic!("plan: {}", failure.message),
    };
    assert!(super::boundary_candidates(&plan, &prepared).is_empty());
}

#[test]
fn stack_shuffles_follow_the_mc1_order() {
    let prepared = prepare(
        "(contract 1 subtract-from-ten
  (input (x I64))
  (output (y I64))
  (program [ 10 swap - ])
  (requires true)
  (ensures (lt (out y) 10)))",
    );
    let plan = match super::plan(&prepared) {
        Ok(plan) => plan,
        Err(failure) => panic!("plan: {}", failure.message),
    };
    assert_eq!(super::execute(&plan, 3), Some((7, false)));
    assert_eq!(super::execute(&plan, i64::MIN), Some((i64::MIN + 10, true)));
    let found = super::boundary_candidates(&plan, &prepared);
    let pairs: std::vec::Vec<(i64, i64)> = found
        .iter()
        .map(|candidate| (candidate.input, candidate.output))
        .collect();
    // `10 - x` stays below ten except at 0 and -1; at i64::MIN it wraps to
    // i64::MIN + 10, which is below ten again.
    assert_eq!(pairs, std::vec![(0, 10), (-1, 11)]);
}

#[test]
fn two_inputs_are_outside_the_profile() {
    let prepared = prepare(
        "(contract 1 adds
  (input (a I64) (b I64))
  (output (y I64))
  (program [ + ])
  (requires true)
  (ensures true))",
    );
    match super::plan(&prepared) {
        Ok(_) => panic!("two inputs must be refused"),
        Err(failure) => assert_eq!(failure.code, "counterexample-outside-profile"),
    }
}

#[test]
fn a_supplied_candidate_keeps_its_semantic_output() {
    let prepared = prepare(MONOTONIC);
    let plan = match super::plan(&prepared) {
        Ok(plan) => plan,
        Err(failure) => panic!("plan: {}", failure.message),
    };
    let candidate = match super::supplied_candidate(&plan, 5) {
        Some(candidate) => candidate,
        None => panic!("the program returns one I64"),
    };
    assert_eq!(
        (candidate.input, candidate.output, candidate.has_wrapped),
        (5, 6, false)
    );
}
