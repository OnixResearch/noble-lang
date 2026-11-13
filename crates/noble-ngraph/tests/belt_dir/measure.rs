#![allow(
    tigerstyle::ambient_clock,
    tigerstyle::assertion_density,
    tigerstyle::function_length,
    tigerstyle::missing_const_fn,
    tigerstyle::no_unwrap,
    tigerstyle::non_trait_imports,
    tigerstyle::numeric_units,
    tigerstyle::unbounded_collection_growth,
    reason = "Owner: noble-maintainers; this ignored reporting lane times one fixed bounded graph and reports measurements without asserting a timing threshold."
)]
//! Cold and warm stage costs for an independently checked capacity-2 belt versus the
//! existing warm `Compiled` and a direct scalar baseline. The output-copy stage is an
//! explicit consumer readback copy; normal belt execution already materializes outputs.
//!
//! Run with `cargo test --release -p noble-ngraph --test schedule_correspondence
//! -- --ignored --nocapture cost_report`.

use std::time::Instant;

use noble_ngraph::belt;
use noble_ngraph::check;
use noble_ngraph::compiled::Compiled;
use noble_ngraph::eval::{Control, Obs};
use noble_ngraph::sem::Value;

const RUNS: usize = 9;
const CAPACITY: u8 = 2;
const SAVE_SLOTS: u8 = 8;
const INPUTS: [Value; 2] = [Value::I64(i64::MAX), Value::I64(1)];

fn median(mut samples: Vec<u128>) -> u128 {
    samples.sort_unstable();
    samples[samples.len() / 2]
}

fn reference_vector() -> Vec<Value> {
    let [Value::I64(x), Value::I64(y)] = INPUTS else {
        unreachable!("the fixed measurement inputs are I64")
    };
    let sum = x.wrapping_add(y);
    let difference = y.wrapping_sub(x);
    let product = difference.wrapping_mul(i64::MIN);
    let fanout = x.wrapping_add(x);
    vec![
        Value::I64(sum),
        Value::I64(difference),
        Value::Bool(sum == product),
        Value::I64(sum),
        Value::I64(product),
        Value::I64(fanout),
        Value::I64(difference),
    ]
}

fn report() {
    let ids = super::common::IDS;
    let graph = super::belt_fixtures::graph();
    let started = Instant::now();
    let checked = check::check(&ids, graph).expect("the fixed graph is admitted");
    let admission_ns = started.elapsed().as_nanos();

    let started = Instant::now();
    let compiled = Compiled::new(&ids, &checked);
    let compiled_cold_ns = started.elapsed().as_nanos();
    let compiled_retained_bytes = compiled.retained_bytes();
    let compiled_workspace_bytes = compiled.scratch_bytes();

    let started = Instant::now();
    let lowered =
        belt::lower(&ids, &checked, CAPACITY, SAVE_SLOTS).expect("the bounded graph lowers");
    let lowering_ns = started.elapsed().as_nanos();
    let started = Instant::now();
    let accepted = belt::check(&ids, &checked, &lowered.bytes)
        .expect("the lowerer's bytes independently check");
    let belt_check_ns = started.elapsed().as_nanos();
    let view = belt::decode(&ids, &lowered.bytes).expect("fresh bytes decode");
    let instruction_bytes = view.instruction_bytes().len();
    let artifact_bytes = lowered.bytes.len();

    let expected = reference_vector();
    let reference = noble_ngraph::eval::reference(&checked, &INPUTS, None, &mut Control::default());
    assert_eq!(reference.obs, Obs::Normal(expected.clone()));
    let belt_cold_started = Instant::now();
    let cold_belt = belt::execute_checked(&ids, &checked, accepted, &INPUTS)
        .expect("the first checked execution succeeds");
    let belt_cold_execution_ns = belt_cold_started.elapsed().as_nanos();
    assert_eq!(cold_belt, expected);
    let compiled_cold_started = Instant::now();
    let cold_compiled = compiled.evaluate(&INPUTS, None, &mut Control::default());
    let compiled_cold_execution_ns = compiled_cold_started.elapsed().as_nanos();
    assert_eq!(cold_compiled.obs, Obs::Normal(expected.clone()));

    let mut belt_times = Vec::with_capacity(RUNS);
    let mut compiled_times = Vec::with_capacity(RUNS);
    let mut scalar_times = Vec::with_capacity(RUNS);
    let mut readback_times = Vec::with_capacity(RUNS);
    for _ in 0..RUNS {
        let started = Instant::now();
        let outputs = belt::execute_checked(&ids, &checked, accepted, &INPUTS)
            .expect("warm belt execution succeeds");
        belt_times.push(started.elapsed().as_nanos());
        assert_eq!(outputs, expected);

        let started = Instant::now();
        let result = compiled.evaluate(&INPUTS, None, &mut Control::default());
        compiled_times.push(started.elapsed().as_nanos());
        assert_eq!(result.obs, Obs::Normal(expected.clone()));

        let started = Instant::now();
        let scalar = reference_vector();
        scalar_times.push(started.elapsed().as_nanos());
        assert_eq!(scalar, expected);

        let started = Instant::now();
        let readback = std::hint::black_box(outputs.to_vec());
        readback_times.push(started.elapsed().as_nanos());
        assert_eq!(readback, expected);
    }

    let value_bytes = core::mem::size_of::<Value>();
    let option_value_bytes = core::mem::size_of::<Option<Value>>();
    let graph_nodes = checked.graph().nodes.len();
    let output_count = checked.graph().outputs.len();
    let input_count = INPUTS.len();
    let belt_peak_payload_bytes = artifact_bytes
        + usize::from(CAPACITY) * value_bytes
        + usize::from(SAVE_SLOTS) * option_value_bytes
        + input_count * value_bytes
        + output_count * value_bytes;
    let checker_peak_aux_bytes = graph_nodes * core::mem::size_of::<u32>()
        + usize::from(CAPACITY) * core::mem::size_of::<noble_ngraph::ir::NodeId>()
        + usize::from(SAVE_SLOTS) * core::mem::size_of::<Option<noble_ngraph::ir::NodeId>>();
    let compiled_peak_payload_bytes = usize::try_from(compiled_retained_bytes)
        .expect("the fixed compiled-code bound fits usize")
        + usize::try_from(compiled_workspace_bytes).expect("the fixed workspace bound fits usize")
        + output_count * value_bytes;

    println!(
        "BELT_MEASURE {{\"nodes\":{graph_nodes},\"outputs\":{output_count},\
\"capacity\":{},\"save_slots\":{},\"admission_ns\":{admission_ns},\
\"compiled_cold_compile_ns\":{compiled_cold_ns},\"belt_cold_lower_ns\":{lowering_ns},\
\"belt_cold_check_ns\":{belt_check_ns},\"belt_first_execution_ns\":{belt_cold_execution_ns},\
\"compiled_first_execution_ns\":{compiled_cold_execution_ns},\
\"belt_warm_execution_median_ns\":{},\"compiled_warm_execution_median_ns\":{},\
\"direct_scalar_median_ns\":{},\"consumer_readback_copy_median_ns\":{},\
\"instruction_count\":{},\"instruction_bytes\":{instruction_bytes},\
\"artifact_bytes\":{artifact_bytes},\
\"preservation_saves\":{},\"preservation_loads\":{},\
\"belt_peak_logical_payload_bytes\":{belt_peak_payload_bytes},\
\"checker_peak_aux_payload_bytes\":{checker_peak_aux_bytes},\
\"compiled_retained_code_bytes\":{compiled_retained_bytes},\
\"compiled_workspace_scratch_bytes\":{compiled_workspace_bytes},\
\"compiled_peak_logical_payload_bytes\":{compiled_peak_payload_bytes},\"warm_runs\":{RUNS}}}",
        accepted.capacity_values(),
        accepted.save_slots(),
        median(belt_times),
        median(compiled_times),
        median(scalar_times),
        median(readback_times),
        accepted.instruction_count(),
        lowered.preservation_saves,
        lowered.preservation_loads,
    );
}

#[test]
#[ignore = "release-mode bounded-cost report; run the command documented in this module"]
fn cost_report() {
    report();
}
