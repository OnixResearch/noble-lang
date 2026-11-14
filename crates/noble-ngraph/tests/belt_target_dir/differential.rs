use noble_ngraph::belt;
use noble_ngraph::belt::target::{self, Machine};
use noble_ngraph::compiled::Compiled;
use noble_ngraph::eval::{self, Control, Obs};
use noble_ngraph::sem::Value;

#[test]
fn target_results_match_reference_compiled_and_sequential_belt() {
    let accepted_graph = super::belt_target_tests::checked(super::belt_target_tests::graph());
    let compiled = Compiled::new(&super::common::IDS, &accepted_graph);
    let vectors = [
        [Value::I64(5), Value::I64(3)],
        [Value::I64(0), Value::I64(-1)],
        [Value::I64(i64::MIN), Value::I64(i64::MAX)],
        [Value::I64(i64::MAX), Value::I64(1)],
    ];
    for machine in [Machine::T1, Machine::T2] {
        let descriptor = machine.descriptor();
        let target_schedule = target::lower(&super::common::IDS, &accepted_graph, machine)
            .expect("target profile lowers the fixed graph");
        let belt_schedule = belt::lower(
            &super::common::IDS,
            &accepted_graph,
            descriptor.belt_capacity,
            descriptor.save_slots,
        )
        .expect("the sequential belt baseline lowers at the same capacity");
        for inputs in &vectors {
            let inputs = inputs.as_slice();
            let reference =
                eval::reference(&accepted_graph, inputs, None, &mut Control::default()).obs;
            let packed = compiled.evaluate(inputs, None, &mut Control::default()).obs;
            let target_result = target::execute(
                &super::common::IDS,
                &accepted_graph,
                machine,
                &target_schedule.bytes,
                inputs,
            )
            .expect("the target schedule executes");
            let belt_result = belt::execute(
                &super::common::IDS,
                &accepted_graph,
                &belt_schedule.bytes,
                inputs,
            )
            .expect("the sequential belt schedule executes");
            assert_eq!(reference, Obs::Normal(target_result.clone()));
            assert_eq!(packed, reference);
            assert_eq!(belt_result, target_result);
        }
    }
}
