use std::time::Instant;

use noble_ngraph::eval::{self, Obs as EvalObs};
use noble_ngraph::ir::{GraphBuilder, Op, ScalarType, SubProfile, Subject};
use noble_ngraph::net::{self, InteractionGraph, Plan, Report, StoragePolicy};
use noble_ngraph::sem::Value;

fn checked(builder: GraphBuilder, label: &str) -> noble_ngraph::CheckedGraph {
    match noble_ngraph::check::check(&super::common::IDS, builder.finish()) {
        Ok(checked) => checked,
        Err(error) => panic!("{label} graph rejected: {error}"),
    }
}

fn chain_graph() -> noble_ngraph::CheckedGraph {
    let mut builder = GraphBuilder::new(
        SubProfile::I64,
        Subject::authored(&super::common::IDS, "net-plan-chain"),
    );
    let a = builder.input(ScalarType::I64);
    let b = builder.input(ScalarType::I64);
    let c = builder.input(ScalarType::I64);
    let d = builder.input(ScalarType::I64);
    let ab = builder.op(Op::I64Add(a, b));
    let abc = builder.op(Op::I64Mul(ab, c));
    let result = builder.op(Op::I64Sub(abc, d));
    builder.output(result);
    checked(builder, "chain")
}

fn fanout_graph() -> noble_ngraph::CheckedGraph {
    let mut builder = GraphBuilder::new(
        SubProfile::I64,
        Subject::authored(&super::common::IDS, "net-plan-fanout"),
    );
    let a = builder.input(ScalarType::I64);
    let b = builder.input(ScalarType::I64);
    let difference = builder.op(Op::I64Sub(a, b));
    let square = builder.op(Op::I64Mul(difference, difference));
    builder.output(difference);
    builder.output(square);
    checked(builder, "fanout")
}

fn erasure_graph() -> noble_ngraph::CheckedGraph {
    let mut builder = GraphBuilder::new(
        SubProfile::I64,
        Subject::authored(&super::common::IDS, "net-plan-erasure"),
    );
    let _erased = builder.input(ScalarType::I64);
    let observed = builder.input(ScalarType::I64);
    builder.output(observed);
    checked(builder, "erasure")
}

fn subtraction_graph() -> noble_ngraph::CheckedGraph {
    let mut builder = GraphBuilder::new(
        SubProfile::I64,
        Subject::authored(&super::common::IDS, "net-plan-subtraction"),
    );
    let a = builder.input(ScalarType::I64);
    let b = builder.input(ScalarType::I64);
    let difference = builder.op(Op::I64Sub(a, b));
    builder.output(difference);
    checked(builder, "subtraction")
}

fn run_plan(
    checked: &noble_ngraph::CheckedGraph,
    candidate: &InteractionGraph,
    inputs: &[Value],
    storage_policy: StoragePolicy,
) -> (Report, u128) {
    let limits = net::Limits::default();
    let started = Instant::now();
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

fn compare_storage(family: &str, checked: noble_ngraph::CheckedGraph, inputs: &[Value]) {
    let limits = net::Limits::default();
    let candidate = match net::lower(&super::common::IDS, &checked, inputs, &limits) {
        Ok(candidate) => candidate,
        Err(error) => panic!("{family} net refused: {error:?}"),
    };
    let reference = eval::reference(&checked, inputs, None, &mut eval::Control::default());
    let expected_values = match reference.obs {
        EvalObs::Normal(values) => values,
        other => panic!("{family} reference did not normalize: {other:?}"),
    };
    let (reuse_warm, _) = run_plan(&checked, &candidate, inputs, StoragePolicy::Reuse);
    let (allocate_warm, _) = run_plan(&checked, &candidate, inputs, StoragePolicy::Allocate);
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
    let expected_obs = reuse_warm.obs;
    let expected_interactions = reuse_warm.interactions;
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
            let (report, elapsed) = run_plan(&checked, &candidate, inputs, storage_policy);
            assert_eq!(
                report.obs, expected_obs,
                "{family}/{storage_policy:?} output differs"
            );
            assert_eq!(
                report.interactions, expected_interactions,
                "{family}/{storage_policy:?} count differs"
            );
            if let Some(previous) = &reports[index] {
                assert_eq!(
                    report.plan_stats, previous.plan_stats,
                    "{family}/{storage_policy:?} counters drifted"
                );
            } else {
                reports[index] = Some(report.clone());
            }
            times[index].push(elapsed);
        }
    }
    let reuse = reports[0].as_ref().expect("reuse arm ran");
    let allocate = reports[1].as_ref().expect("allocation arm ran");
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
        arm_json("reuse", reuse, &times[0]),
        arm_json("allocate", allocate, &times[1]),
    );
}

#[test]
fn plan_storage_comparison_covers_required_families() {
    compare_storage(
        "chain",
        chain_graph(),
        &[Value::I64(3), Value::I64(5), Value::I64(7), Value::I64(11)],
    );
    compare_storage("fanout", fanout_graph(), &[Value::I64(13), Value::I64(4)]);
    compare_storage("erasure", erasure_graph(), &[Value::I64(23), Value::I64(8)]);
    let subtraction = subtraction_graph();
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
