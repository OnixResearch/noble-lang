use super::programs::{self, Program};
use crate::common;
use noble_ngraph::check::{self, CheckedGraph};
use noble_ngraph::compiled::Compiled;
use noble_ngraph::eval::{self, Control, Obs};
use noble_ngraph::ir::Graph;
use noble_ngraph::sem::Value;
use noble_ngraph::snapshot::ParamSnapshot;
use std::time::Instant;

const CYCLES: u64 = 100;
const STEPS: usize = 4;
const REPS: usize = 5;

struct Lane {
    checked: CheckedGraph,
    compiled: Compiled,
    weights: ParamSnapshot,
    reference: bool,
    graph_bytes: usize,
}

fn lane(graph: Graph, reference: bool) -> Lane {
    let checked = check::check(&common::IDS, graph).expect("measurement graph checks");
    let values = if checked.param_slots().is_empty() {
        vec![]
    } else {
        vec![0.0, 1.0]
    };
    let weights = ParamSnapshot::new(&common::IDS, &checked, &values).expect("snapshot");
    let compiled = Compiled::new(&common::IDS, &checked);
    let graph_bytes = noble_ngraph::format::graph::print(checked.graph()).len();
    Lane {
        checked,
        compiled,
        weights,
        reference,
        graph_bytes,
    }
}

fn encode(word: u64) -> [Value; 64] {
    core::array::from_fn(|bit| Value::f32(((word >> bit) & 1) as f32))
}

fn decode(values: &[Value]) -> u64 {
    super::checker::decode_words(values).expect("target decoder")[0]
}

fn rss_bytes() -> u64 {
    std::fs::read_to_string("/proc/self/status")
        .expect("proc status available")
        .lines()
        .find_map(|line| {
            line.strip_prefix("VmRSS:")
                .and_then(|value| value.split_whitespace().next())
        })
        .and_then(|kb| kb.parse::<u64>().ok())
        .expect("VmRSS field")
        * 1024
}

fn eval(lane: &Lane, inputs: &[Value]) -> (Vec<Value>, u64) {
    let mut control = Control::default();
    let report = if lane.reference {
        eval::reference(&lane.checked, inputs, Some(&lane.weights), &mut control)
    } else {
        lane.compiled
            .evaluate(inputs, Some(&lane.weights), &mut control)
    };
    let Obs::Normal(outputs) = report.obs else {
        panic!("valid benchmark input refused")
    };
    (outputs, report.evaluated_nodes)
}

fn interleave(state: u64, input: u64) -> Vec<Value> {
    encode(state)
        .into_iter()
        .zip(encode(input))
        .flat_map(|(a, b)| [a, b])
        .collect()
}

fn flatten(state: u64, inputs: &[u64]) -> Vec<Value> {
    core::iter::once(state)
        .chain(inputs.iter().copied())
        .flat_map(encode)
        .collect()
}

fn flatten_words(words: &[u64]) -> Vec<Value> {
    words.iter().flat_map(|word| encode(*word)).collect()
}

fn decode_pair(values: &[Value]) -> [u64; 2] {
    let words = super::checker::decode_words(values).expect("two-word decoder");
    [words[0], words[1]]
}

fn host_cycle(lane: &Lane, mut state: u64) -> (u64, u64, u64) {
    let mut nodes = 0;
    let mut host_ops = 0;
    for step in 0..STEPS {
        let (output, evaluated) = eval(lane, &interleave(state, step as u64 + 1));
        nodes += evaluated;
        state = decode(&output);
        host_ops += 194; // 128 bit-cell encodes, 64 decodes, one dispatch, and one handoff
    }
    (state, nodes, host_ops)
}

fn unrolled_cycle(lane: &Lane, state: u64) -> (u64, u64, u64) {
    let (output, nodes) = eval(
        lane,
        &flatten(state, &(1..=STEPS as u64).collect::<Vec<_>>()),
    );
    (decode(&output), nodes, 64 * (STEPS as u64 + 1) + 66)
}

fn cycles(lane: &Lane, unrolled: bool) -> (u64, u64, u64, u64, u64) {
    let (mut state, mut nodes, mut host_ops, mut calls) = (0, 0, 0, 0);
    let mut peak_rss = rss_bytes();
    for _ in 0..CYCLES {
        let (next, visited, ops) = if unrolled {
            unrolled_cycle(lane, state)
        } else {
            host_cycle(lane, state)
        };
        state = next;
        nodes += visited;
        host_ops += ops;
        calls += if unrolled { 1 } else { STEPS as u64 };
        peak_rss = peak_rss.max(rss_bytes());
    }
    (state, nodes, host_ops, calls, peak_rss)
}

fn median(mut values: Vec<u128>) -> u128 {
    values.sort_unstable();
    values[values.len() / 2]
}

fn native_cycle(mut state: u64) -> u64 {
    for step in 0..STEPS {
        state = state.wrapping_add(step as u64 + 1);
    }
    state
}

fn expected_accumulator() -> u64 {
    let mut state = 0;
    for _ in 0..CYCLES {
        for input in 1..=STEPS as u64 {
            state = programs::source_transition(Program::Accumulator, &[state], &[input])
                .expect("bounded source transition")[0];
        }
    }
    state
}

fn swap_host_cycle(lane: &Lane, mut state: [u64; 2]) -> ([u64; 2], u64, u64) {
    let mut nodes = 0;
    let mut host_ops = 0;
    for _ in 0..STEPS {
        let (output, evaluated) = eval(lane, &flatten_words(&state));
        nodes += evaluated;
        state = decode_pair(&output);
        host_ops += 1 + 1 + 128 + 128;
    }
    (state, nodes, host_ops)
}

fn swap_unrolled_cycle(lane: &Lane, state: [u64; 2]) -> ([u64; 2], u64, u64) {
    let (output, nodes) = eval(lane, &flatten_words(&state));
    (decode_pair(&output), nodes, 1 + 1 + 128 + 128)
}

fn swap_cycles(lane: &Lane, unrolled: bool) -> ([u64; 2], u64, u64, u64, u64) {
    let (mut state, mut nodes, mut host_ops, mut calls) = ([1, 2], 0, 0, 0);
    let mut peak_rss = rss_bytes();
    for _ in 0..CYCLES {
        let (next, visited, ops) = if unrolled {
            swap_unrolled_cycle(lane, state)
        } else {
            swap_host_cycle(lane, state)
        };
        state = next;
        nodes += visited;
        host_ops += ops;
        calls += if unrolled { 1 } else { STEPS as u64 };
        peak_rss = peak_rss.max(rss_bytes());
    }
    (state, nodes, host_ops, calls, peak_rss)
}

fn measure_swap(name: &str, lane: &Lane, unrolled: bool, compile_load_ns: u128) -> (u128, u64) {
    let mut times = Vec::with_capacity(REPS);
    let mut growth = Vec::with_capacity(REPS);
    for rep in 0..REPS {
        let before = rss_bytes();
        let start = Instant::now();
        let (state, nodes, host_ops, calls, peak) = swap_cycles(lane, unrolled);
        let elapsed = start.elapsed().as_nanos();
        assert_eq!(state, [1, 2]);
        times.push(elapsed);
        growth.push(peak.saturating_sub(before));
        println!("RECURRENT_COST\t{{\"lane\":\"{name}\",\"rep\":{rep},\"transitions\":{},\"eval_calls\":{calls},\"microsteps\":{nodes},\"host_ops\":{host_ops},\"emitted_tokens\":0,\"skipped_work\":0,\"elapsed_ns\":{elapsed},\"ns_per_transition\":{:.1},\"compile_load_ns\":{compile_load_ns},\"graph_bytes\":{},\"weight_bytes\":{},\"state_bytes\":16,\"rss_samples\":{},\"rss_before_bytes\":{before},\"rss_peak_bytes\":{peak},\"peak_scratch_rss_bytes\":{peak},\"rss_growth_bytes\":{}}}", CYCLES * STEPS as u64, elapsed as f64 / (CYCLES * STEPS as u64) as f64, lane.graph_bytes, lane.weights.bytes().len(), CYCLES + 1, growth[rep]);
    }
    (median(times), *growth.iter().max().expect("five reps"))
}

fn native_swap_cycle(mut state: [u64; 2]) -> [u64; 2] {
    for _ in 0..STEPS {
        state.swap(0, 1);
    }
    state
}

fn measure(
    name: &str,
    lane: &Lane,
    unrolled: bool,
    state_bytes: u64,
    compile_load_ns: u128,
) -> (u128, u64) {
    let mut times = Vec::with_capacity(REPS);
    let mut growth = Vec::with_capacity(REPS);
    for rep in 0..REPS {
        let before = rss_bytes();
        let start = Instant::now();
        let (state, nodes, host_ops, calls, peak) = cycles(lane, unrolled);
        let elapsed = start.elapsed().as_nanos();
        assert_eq!(state, expected_accumulator(), "measured source result");
        times.push(elapsed);
        growth.push(peak.saturating_sub(before));
        println!("RECURRENT_COST\t{{\"lane\":\"{name}\",\"rep\":{rep},\"transitions\":{},\"eval_calls\":{calls},\"microsteps\":{nodes},\"host_ops\":{host_ops},\"emitted_tokens\":0,\"skipped_work\":0,\"elapsed_ns\":{elapsed},\"ns_per_transition\":{:.1},\"compile_load_ns\":{compile_load_ns},\"graph_bytes\":{},\"weight_bytes\":{},\"state_bytes\":{state_bytes},\"rss_samples\":{},\"rss_before_bytes\":{before},\"rss_peak_bytes\":{peak},\"peak_scratch_rss_bytes\":{peak},\"rss_growth_bytes\":{}}}", CYCLES * STEPS as u64, elapsed as f64 / (CYCLES * STEPS as u64) as f64, lane.graph_bytes, lane.weights.bytes().len(), CYCLES + 1, growth[rep]);
    }
    let max_growth = *growth.iter().max().expect("five reps");
    (median(times), max_growth)
}

#[test]
#[ignore = "five-repetition reporting-only timing/RSS lane; run with run.mjs"]
fn recurrent_unrolled_cost_protocol() {
    let baseline_build = Instant::now();
    let baseline = lane(programs::one_step_adder(), false);
    let baseline_build_ns = baseline_build.elapsed().as_nanos();
    let unrolled_build = Instant::now();
    let unrolled = lane(programs::unrolled_adder(STEPS), false);
    let unrolled_build_ns = unrolled_build.elapsed().as_nanos();

    for steps in 1..=STEPS {
        let artifact = lane(programs::unrolled_adder(steps), false);
        let inputs = (1..=steps as u64).collect::<Vec<_>>();
        let mut host = 0;
        for input in &inputs {
            host = decode(&eval(&baseline, &interleave(host, *input)).0);
        }
        let result = decode(&eval(&artifact, &flatten(0, &inputs)).0);
        let expected = inputs
            .iter()
            .fold(0u64, |sum, value| sum.wrapping_add(*value));
        assert_eq!(host, result);
        assert_eq!(result, expected);

        let swap_host = lane(programs::swap_graph(), false);
        let swap_artifact = lane(programs::unrolled_swap(steps), false);
        let mut swap_state = [0x0123_4567_89ab_cdef, 0xfedc_ba98_7654_3210];
        let mut source_state = swap_state.to_vec();
        for _ in 0..steps {
            swap_state = decode_pair(&eval(&swap_host, &flatten_words(&swap_state)).0);
            source_state = programs::source_transition(Program::StackSwap, &source_state, &[])
                .expect("stack-swap source transition");
        }
        let swap_result = decode_pair(
            &eval(
                &swap_artifact,
                &flatten_words(&[0x0123_4567_89ab_cdef, 0xfedc_ba98_7654_3210]),
            )
            .0,
        );
        assert_eq!(
            swap_state, swap_result,
            "unrolled stack swap at {steps} steps"
        );
        assert_eq!(swap_result.as_slice(), source_state);
    }

    let _ = cycles(&baseline, false);
    let _ = cycles(&unrolled, true);
    let (baseline_ns, baseline_rss) =
        measure("host_loop_compiled", &baseline, false, 8, baseline_build_ns);
    let (unrolled_ns, unrolled_rss) =
        measure("unrolled_compiled", &unrolled, true, 8, unrolled_build_ns);
    let reference = lane(programs::one_step_adder(), true);
    let (reference_ns, reference_rss) = measure("host_loop_reference", &reference, false, 8, 0);
    let swap_host_build = Instant::now();
    let swap_host = lane(programs::swap_graph(), false);
    let swap_host_build_ns = swap_host_build.elapsed().as_nanos();
    let swap_unrolled_build = Instant::now();
    let swap_unrolled = lane(programs::unrolled_swap(STEPS), false);
    let swap_unrolled_build_ns = swap_unrolled_build.elapsed().as_nanos();
    let _ = swap_cycles(&swap_host, false);
    let _ = swap_cycles(&swap_unrolled, true);
    let (swap_host_ns, swap_host_rss) = measure_swap(
        "stack_swap_host_loop_compiled",
        &swap_host,
        false,
        swap_host_build_ns,
    );
    let (swap_unrolled_ns, swap_unrolled_rss) = measure_swap(
        "stack_swap_unrolled_compiled",
        &swap_unrolled,
        true,
        swap_unrolled_build_ns,
    );
    let swap_reference = lane(programs::swap_graph(), true);
    let (swap_reference_ns, swap_reference_rss) =
        measure_swap("stack_swap_host_loop_reference", &swap_reference, false, 0);
    let mut native_times = Vec::with_capacity(REPS);
    for rep in 0..REPS {
        let before = rss_bytes();
        let start = Instant::now();
        let state = (0..CYCLES).fold(0u64, |state, _| native_cycle(state));
        let elapsed = start.elapsed().as_nanos();
        let peak = rss_bytes();
        assert_eq!(state, 1_000, "native accumulator/source result");
        native_times.push(elapsed);
        println!("RECURRENT_COST\t{{\"lane\":\"native_u64\",\"rep\":{rep},\"transitions\":{},\"eval_calls\":0,\"microsteps\":0,\"host_ops\":{},\"emitted_tokens\":0,\"skipped_work\":0,\"elapsed_ns\":{elapsed},\"ns_per_transition\":{:.1},\"compile_load_ns\":0,\"graph_bytes\":0,\"weight_bytes\":0,\"state_bytes\":8,\"rss_samples\":2,\"rss_before_bytes\":{before},\"rss_peak_bytes\":{peak},\"peak_scratch_rss_bytes\":{peak},\"rss_growth_bytes\":{}}}", CYCLES * STEPS as u64, CYCLES * STEPS as u64, elapsed as f64 / (CYCLES * STEPS as u64) as f64, peak.saturating_sub(before));
    }
    let native_ns = median(native_times);
    let mut native_swap_times = Vec::with_capacity(REPS);
    for rep in 0..REPS {
        let before = rss_bytes();
        let start = Instant::now();
        let state = (0..CYCLES).fold([1, 2], |state, _| native_swap_cycle(state));
        let elapsed = start.elapsed().as_nanos();
        let peak = rss_bytes();
        assert_eq!(state, [1, 2]);
        native_swap_times.push(elapsed);
        println!("RECURRENT_COST\t{{\"lane\":\"stack_swap_native\",\"rep\":{rep},\"transitions\":{},\"eval_calls\":0,\"microsteps\":0,\"host_ops\":{},\"emitted_tokens\":0,\"skipped_work\":0,\"elapsed_ns\":{elapsed},\"ns_per_transition\":{:.1},\"compile_load_ns\":0,\"graph_bytes\":0,\"weight_bytes\":0,\"state_bytes\":16,\"rss_samples\":2,\"rss_before_bytes\":{before},\"rss_peak_bytes\":{peak},\"peak_scratch_rss_bytes\":{peak},\"rss_growth_bytes\":{}}}", CYCLES * STEPS as u64, CYCLES * STEPS as u64, elapsed as f64 / (CYCLES * STEPS as u64) as f64, peak.saturating_sub(before));
    }
    let native_swap_ns = median(native_swap_times);
    println!("RECURRENT_COST_SUMMARY\t{{\"program\":\"wrapping-accumulator\",\"repetitions\":5,\"transitions_per_rep\":{},\"host_median_ns\":{baseline_ns},\"unrolled_median_ns\":{unrolled_ns},\"reference_median_ns\":{reference_ns},\"native_median_ns\":{native_ns},\"unrolled_over_host\":{:.5},\"host_rss_growth_bytes\":{baseline_rss},\"unrolled_rss_growth_bytes\":{unrolled_rss},\"reference_rss_growth_bytes\":{reference_rss},\"host_compile_load_ns\":{baseline_build_ns},\"unrolled_compile_load_ns\":{unrolled_build_ns},\"host_weight_bytes\":{},\"unrolled_weight_bytes\":{},\"host_graph_bytes\":{},\"unrolled_graph_bytes\":{},\"host_graph_nodes\":{},\"unrolled_graph_nodes\":{},\"state_bytes\":8,\"rss_sampling_overhead_included\":true,\"external_runtime\":\"none\"}}", CYCLES * STEPS as u64, unrolled_ns as f64 / baseline_ns as f64, baseline.weights.bytes().len(), unrolled.weights.bytes().len(), baseline.graph_bytes, unrolled.graph_bytes, baseline.checked.node_count(), unrolled.checked.node_count());
    println!("RECURRENT_COST_SUMMARY\t{{\"program\":\"stack-swap\",\"repetitions\":5,\"transitions_per_rep\":{},\"host_median_ns\":{swap_host_ns},\"unrolled_median_ns\":{swap_unrolled_ns},\"reference_median_ns\":{swap_reference_ns},\"native_median_ns\":{native_swap_ns},\"unrolled_over_host\":{:.5},\"host_rss_growth_bytes\":{swap_host_rss},\"unrolled_rss_growth_bytes\":{swap_unrolled_rss},\"reference_rss_growth_bytes\":{swap_reference_rss},\"host_compile_load_ns\":{swap_host_build_ns},\"unrolled_compile_load_ns\":{swap_unrolled_build_ns},\"host_weight_bytes\":{},\"unrolled_weight_bytes\":{},\"host_graph_bytes\":{},\"unrolled_graph_bytes\":{},\"host_graph_nodes\":{},\"unrolled_graph_nodes\":{},\"state_bytes\":16,\"rss_sampling_overhead_included\":true,\"external_runtime\":\"none\"}}", CYCLES * STEPS as u64, swap_unrolled_ns as f64 / swap_host_ns as f64, swap_host.weights.bytes().len(), swap_unrolled.weights.bytes().len(), swap_host.graph_bytes, swap_unrolled.graph_bytes, swap_host.checked.node_count(), swap_unrolled.checked.node_count());
}
