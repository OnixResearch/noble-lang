use std::time::Instant;

use noble_ngraph::eval;
use noble_ngraph::net::{self, InteractionGraph, Plan, Report, StoragePolicy};
use noble_ngraph::sem::Value;

fn run_plan(
    checked: &noble_ngraph::CheckedGraph,
    candidate: &InteractionGraph,
    inputs: &[Value],
    storage_policy: StoragePolicy,
) -> (Report, u128) {
    let limits = net::Limits::default();
    let started = start_clock();
    let report = net::reduce(
        &super::common::IDS,
        net::ReduceRequest {
            checked,
            candidate,
            inputs,
            limits: &limits,
            plan: Plan {
                storage_policy: Some(storage_policy),
                ..Plan::default()
            },
        },
    );
    (report, started.elapsed().as_nanos())
}

#[expect(
    tigerstyle::ambient_clock,
    reason = "Owner: noble-maintainers; this paired test reads monotonic time only at its measurement boundary."
)]
fn start_clock() -> Instant {
    Instant::now()
}

fn digest_hex(digest: &noble_ngraph::ir::Digest) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut result = String::with_capacity(64);
    for byte in digest {
        result.push(char::from(HEX[usize::from(byte >> 4)]));
        result.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    result
}

fn arm_json(label: &str, report: &Report, times_ns: &[u128]) -> String {
    let mut samples = times_ns.to_vec();
    samples.sort_unstable();
    let median = samples[samples.len() / 2];
    let sample_text = times_ns
        .iter()
        .map(|sample| format!("{sample}"))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"storage\":\"{label}\",\"median_ns\":\"{median}\",\"samples_ns\":[{sample_text}],\"observation\":\"{:?}\",\"interactions\":{},\"allocated_slots\":{},\"reused_slots\":{},\"freed_slots\":{},\"committed_live_nodes\":{},\"transient_slots\":{},\"arena_slots_high_water\":{},\"queue_high_water\":{},\"scratch_bytes_high_water\":{},\"emitted_plan_bytes\":{},\"microsteps\":{},\"plan_digest\":\"{}\",\"replay_digest\":\"{}\"}}",
        report.obs,
        report.interactions,
        report.plan_stats.allocated_slots,
        report.plan_stats.reused_slots,
        report.plan_stats.freed_slots,
        report.plan_stats.committed_live_nodes,
        report.plan_stats.transient_slots,
        report.plan_stats.arena_slots_high_water,
        report.plan_stats.queue_high_water,
        report.plan_stats.scratch_bytes_high_water,
        report.plan_stats.emitted_plan_bytes,
        report.plan_stats.microsteps,
        digest_hex(&report.plan_stats.plan_digest),
        digest_hex(&report.plan_stats.replay_digest),
    )
}

struct Expected {
    obs: net::Obs,
    interactions: u64,
}

fn expected_from_warmups(
    family: &str,
    checked: &noble_ngraph::CheckedGraph,
    candidate: &InteractionGraph,
    inputs: &[Value],
) -> Expected {
    let reference = eval::reference(checked, inputs, None, &mut eval::Control::default());
    let expected_values = match reference.obs {
        eval::Obs::Normal(values) => values,
        other => panic!("{family} reference did not normalize: {other:?}"),
    };
    let (reuse_warm, _) = run_plan(checked, candidate, inputs, StoragePolicy::Reuse);
    let (allocate_warm, _) = run_plan(checked, candidate, inputs, StoragePolicy::Allocate);
    assert_eq!(
        reuse_warm.obs,
        net::Obs::Normal(expected_values.clone()),
        "{family} reuse differs from reference"
    );
    assert_eq!(
        allocate_warm.obs,
        net::Obs::Normal(expected_values),
        "{family} allocation differs from reference"
    );
    assert_eq!(
        reuse_warm.obs, allocate_warm.obs,
        "{family} warm-up outputs differ"
    );
    assert_eq!(
        reuse_warm.interactions, allocate_warm.interactions,
        "{family} warm-up counts differ"
    );
    Expected {
        obs: reuse_warm.obs,
        interactions: reuse_warm.interactions,
    }
}

fn measure_paired(
    family: &str,
    checked: &noble_ngraph::CheckedGraph,
    candidate: &InteractionGraph,
    inputs: &[Value],
    expected: &Expected,
) -> ([Report; 2], [Vec<u128>; 2]) {
    let mut reports: [Option<Report>; 2] = [None, None];
    let mut times: [Vec<u128>; 2] = [Vec::with_capacity(31), Vec::with_capacity(31)];
    for pair_index in 0..31 {
        let order = if pair_index % 2 == 0 {
            [StoragePolicy::Reuse, StoragePolicy::Allocate]
        } else {
            [StoragePolicy::Allocate, StoragePolicy::Reuse]
        };
        for storage_policy in order {
            let index = usize::from(storage_policy == StoragePolicy::Allocate);
            let (report, elapsed) = run_plan(checked, candidate, inputs, storage_policy);
            assert_eq!(
                &report.obs, &expected.obs,
                "{family}/{storage_policy:?} output differs"
            );
            assert_eq!(
                report.interactions, expected.interactions,
                "{family}/{storage_policy:?} count differs"
            );
            if let Some(previous) = &reports[index] {
                assert_eq!(
                    report.plan_stats, previous.plan_stats,
                    "{family}/{storage_policy:?} counters drifted"
                );
            } else {
                reports[index] = Some(report);
            }
            times[index].push(elapsed);
        }
    }
    let [Some(reuse), Some(allocate)] = reports else {
        panic!("{family}: both arms must run")
    };
    ([reuse, allocate], times)
}

fn assert_storage_counts(family: &str, reuse: &Report, allocate: &Report) {
    assert_eq!(reuse.obs, allocate.obs, "{family} measured outputs differ");
    assert_eq!(
        reuse.interactions, allocate.interactions,
        "{family} measured counts differ"
    );
    assert_eq!(
        reuse.plan_stats.allocated_slots, 0,
        "{family} reuse arm allocated a slot"
    );
    if family == "erasure" {
        assert_eq!(
            reuse.plan_stats.reused_slots, 0,
            "erasure reused a replacement slot"
        );
        assert_eq!(
            allocate.plan_stats.allocated_slots, 0,
            "erasure allocated a replacement slot"
        );
        assert_eq!(
            reuse.plan_stats.freed_slots,
            allocate.plan_stats.freed_slots
        );
    } else {
        assert!(
            reuse.plan_stats.reused_slots > 0,
            "{family} reuse arm reused no slots"
        );
        assert!(
            allocate.plan_stats.allocated_slots > 0,
            "{family} allocation arm allocated no slots"
        );
    }
    assert_ne!(
        reuse.plan_stats.plan_digest, allocate.plan_stats.plan_digest,
        "{family} plan identities collided"
    );
}

fn compare_storage(family: &str, checked: noble_ngraph::CheckedGraph, inputs: &[Value]) {
    let limits = net::Limits::default();
    let candidate = match net::lower(&super::common::IDS, &checked, inputs, &limits) {
        Ok(candidate) => candidate,
        Err(error) => panic!("{family} net refused: {error:?}"),
    };
    let expected = expected_from_warmups(family, &checked, &candidate, inputs);
    let ([reuse, allocate], times) =
        measure_paired(family, &checked, &candidate, inputs, &expected);
    assert_storage_counts(family, &reuse, &allocate);
    println!(
        "NOBLE_NET_PLAN_SAMPLE:{{\"family\":\"{family}\",\"graph_id\":\"{}\",\"net_key\":\"{}\",\"input_count\":{},\"rule_revision\":\"{}\",\"plan_revision\":\"{}\",\"representation_revision\":\"{}\",\"checker_revision\":\"{}\",\"executor_revision\":\"{}\",\"same_observation\":true,\"same_interactions\":true,\"reuse\":{},\"allocate\":{}}}",
        digest_hex(&checked.id()),
        digest_hex(&candidate.build_key),
        inputs.len(),
        net::PLAN_RULES_REVISION,
        net::PLAN_REVISION,
        net::PLAN_REPRESENTATION_REVISION,
        net::PLAN_CHECKER_REVISION,
        net::PLAN_EXECUTOR_REVISION,
        arm_json("reuse", &reuse, &times[0]),
        arm_json("allocate", &allocate, &times[1]),
    );
}

#[test]
fn plan_storage_comparison_covers_required_families() {
    compare_storage(
        "chain",
        super::plancomparison_fixtures::chain_graph(),
        &[Value::I64(3), Value::I64(5), Value::I64(7), Value::I64(11)],
    );
    compare_storage(
        "fanout",
        super::plancomparison_fixtures::fanout_graph(),
        &[Value::I64(13), Value::I64(4)],
    );
    compare_storage(
        "erasure",
        super::plancomparison_fixtures::erasure_graph(),
        &[Value::I64(23), Value::I64(8)],
    );
    let subtraction = super::plancomparison_fixtures::subtraction_graph();
    compare_storage(
        "asymmetric-subtraction",
        subtraction.clone(),
        &[Value::I64(2), Value::I64(8)],
    );
    compare_storage(
        "wrapping-subtraction",
        subtraction,
        &[Value::I64(i64::MIN), Value::I64(1)],
    );
}
