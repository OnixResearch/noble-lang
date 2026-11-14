//! Timing boundary and prepared inputs for the issue #32 receipt.

use super::*;
use std::time::Instant;

pub(super) struct Prepared {
    pub(super) lowered: Lowered,
    pub(super) specialized: pe::Specialized,
    pub(super) rows: Vec<Vec<i64>>,
    pub(super) dynamic_values: Vec<Vec<Value>>,
    pub(super) original_values: Vec<Vec<Value>>,
    pub(super) residual_pipeline_ns: u64,
}

// justification: the dedicated measurement shell samples elapsed evaluator time.
#[allow(
    tigerstyle::ambient_clock,
    reason = "the benchmark timing boundary measures elapsed runtime"
)]
fn timed<T>(run: impl FnOnce() -> T) -> (u64, T) {
    let started = Instant::now();
    let result = run();
    let elapsed =
        u64::try_from(started.elapsed().as_nanos()).expect("elapsed nanoseconds fit in u64");
    (elapsed, result)
}

pub(super) fn prepare(root: &str, inputs: usize, map: &[Option<i64>]) -> Prepared {
    let lowered = Lowered::new(&[], root, inputs).expect("measurement fixture extracts");
    let dynamic = map.iter().filter(|entry| entry.is_none()).count();
    let rows = vectors(VectorSpec {
        seed: fixture_seed(inputs),
        dynamic_inputs: dynamic,
        count: 1000,
    });
    let (residual_pipeline_ns, specialized) = timed(|| {
        lowered
            .specialized(map)
            .expect("measurement fixture specializes")
    });
    let dynamic_values: Vec<Vec<Value>> = rows
        .iter()
        .map(|row| row.iter().copied().map(Value::I64).collect())
        .collect();
    let original_values: Vec<Vec<Value>> = rows
        .iter()
        .map(|row| specialized.merge(row).expect("merged input vector"))
        .collect();
    Prepared {
        lowered,
        specialized,
        rows,
        dynamic_values,
        original_values,
        residual_pipeline_ns,
    }
}

fn warm_l3(checked: &noble_ngraph::check::CheckedGraph, vectors: &[Vec<Value>]) -> u64 {
    timed(|| {
        for values in vectors {
            pe::observe_l3(checked, values).expect("warm L3 evaluation");
        }
    })
    .0
}

fn warm_l4(compiled: &noble_ngraph::compiled::Compiled, vectors: &[Vec<Value>]) -> u64 {
    timed(|| {
        for values in vectors {
            pe::observe_l4(compiled, values).expect("warm L4 evaluation");
        }
    })
    .0
}

pub(super) fn evaluation_times(prepared: &Prepared) -> (serde_json::Value, serde_json::Value) {
    let dynamic_first = prepared.dynamic_values.first().expect("measurement vector");
    let original_first = prepared
        .original_values
        .first()
        .expect("measurement vector");
    let specialized = &prepared.specialized;
    let cold = serde_json::json!({
        "l3_original": timed(|| pe::observe_l3(&prepared.lowered.extraction.checked, original_first).expect("cold original L3")).0,
        "l4_original": timed(|| pe::observe_l4(&prepared.lowered.compiled, original_first).expect("cold original L4")).0,
        "l3_residual": timed(|| specialized.l3(dynamic_first).expect("cold residual L3")).0,
        "l4_residual": timed(|| specialized.l4(dynamic_first).expect("cold residual L4")).0,
        "l3_readmitted": timed(|| specialized.readmitted_l3(dynamic_first).expect("cold readmitted L3")).0,
        "l4_readmitted": timed(|| specialized.readmitted_l4(dynamic_first).expect("cold readmitted L4")).0,
    });
    pe::observe_l3(&prepared.lowered.extraction.checked, original_first)
        .expect("warmup original L3");
    pe::observe_l4(&prepared.lowered.compiled, original_first).expect("warmup original L4");
    specialized.l3(dynamic_first).expect("warmup residual L3");
    specialized.l4(dynamic_first).expect("warmup residual L4");
    specialized
        .readmitted_l3(dynamic_first)
        .expect("warmup readmitted L3");
    specialized
        .readmitted_l4(dynamic_first)
        .expect("warmup readmitted L4");
    let warm = serde_json::json!({
        "l3_original": warm_l3(&prepared.lowered.extraction.checked, &prepared.original_values),
        "l4_original": warm_l4(&prepared.lowered.compiled, &prepared.original_values),
        "l3_residual": warm_l3(&specialized.residual.checked, &prepared.dynamic_values),
        "l4_residual": warm_l4(&specialized.compiled, &prepared.dynamic_values),
        "l3_readmitted": warm_l3(&specialized.readmitted, &prepared.dynamic_values),
        "l4_readmitted": warm_l4(&specialized.readmitted_compiled, &prepared.dynamic_values),
    });
    (cold, warm)
}
