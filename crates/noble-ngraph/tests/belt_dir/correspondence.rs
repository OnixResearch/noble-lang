// justification: fixed differential cases compare every output to independent evaluators and exact values.
#![allow(
    clippy::unwrap_used,
    tigerstyle::assertion_density,
    tigerstyle::no_panic,
    tigerstyle::no_unwrap,
    reason = "Owner: noble-maintainers; these fixed differential cases assert every observable output against independent evaluators and exact I64 expectations."
)]

use noble_ngraph::belt;
use noble_ngraph::compiled::Compiled;
use noble_ngraph::eval::{self, Control, Obs};
use noble_ngraph::sem::Value;

#[test]
fn lowered_programs_match_reference_compiled_and_exact_vectors() {
    let checked = super::belt_fixtures::checked(super::belt_fixtures::graph());
    let compiled = Compiled::new(&super::common::IDS, &checked);
    for capacity in 2..=4 {
        let lowered = belt::lower(&super::common::IDS, &checked, capacity, 8)
            .expect("the supported graph lowers within explicit bounds");
        assert!(lowered.preservation_saves > 0, "capacity {capacity}");
        assert!(lowered.preservation_loads > 0, "capacity {capacity}");
        assert_eq!(
            belt::decode(&super::common::IDS, &lowered.bytes)
                .expect("fresh artifacts decode")
                .instruction_count(),
            lowered.instruction_count
        );

        let accepted = belt::check(&super::common::IDS, &checked, &lowered.bytes)
            .expect("the independent checker accepts the lowered stream");
        assert_eq!(accepted.capacity_values(), capacity);
        assert_eq!(accepted.save_slots(), 8);
        for (inputs, exact) in super::belt_fixtures::vectors() {
            let inputs = inputs.to_vec();
            let actual = belt::execute_checked(&super::common::IDS, &checked, accepted, &inputs)
                .expect("checker-issued belt artifact executes");
            assert_eq!(actual, exact.to_vec());

            let belt_report = belt::execute(&super::common::IDS, &checked, &lowered.bytes, &inputs)
                .expect("the check-and-execute path succeeds");
            assert_eq!(belt_report, exact.to_vec());

            let reference = eval::reference(&checked, &inputs, None, &mut Control::default()).obs;
            let packed = compiled
                .evaluate(&inputs, None, &mut Control::default())
                .obs;
            assert_eq!(reference, Obs::Normal(exact.to_vec()));
            assert_eq!(packed, reference);
        }
    }
}

#[test]
fn malformed_runtime_inputs_are_refused_before_publishing_results() {
    let checked = super::belt_fixtures::checked(super::belt_fixtures::graph());
    let lowered =
        belt::lower(&super::common::IDS, &checked, 2, 8).expect("the supported graph lowers");
    assert_eq!(
        belt::execute(
            &super::common::IDS,
            &checked,
            &lowered.bytes,
            &[Value::I64(1)],
        ),
        Err(belt::Refusal::Inputs)
    );
}
