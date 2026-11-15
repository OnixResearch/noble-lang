//! Stage-clock boundaries and reusable execution samples.

use super::*;
use noble_ngraph::compiled::Compiled;
use noble_ngraph::specialize::cache::{Invocation, Session, Stage, StageEvent};
use std::hint::black_box;
use std::time::Instant;

type Json = serde_json::Value;

// Only this receipt boundary may consult the ambient monotonic clock.
#[allow(
    tigerstyle::ambient_clock,
    reason = "the source-bound benchmark samples elapsed stage time"
)]
fn now() -> Instant {
    Instant::now()
}

pub(super) fn timed<T>(run: impl FnOnce() -> T) -> (u64, T) {
    let start = now();
    let result = run();
    (
        u64::try_from(start.elapsed().as_nanos()).expect("elapsed nanoseconds fit u64"),
        result,
    )
}

pub(super) fn stats(mut samples: Vec<u64>) -> Json {
    assert!(!samples.is_empty());
    samples.sort_unstable();
    let upper = samples.len().saturating_sub(1);
    serde_json::json!({
        "n": samples.len(), "min_ns": samples[0],
        "p25_ns": samples[upper / 4], "median_ns": samples[upper / 2],
        "p75_ns": samples[upper.saturating_mul(3) / 4], "max_ns": samples[upper],
    })
}

pub(super) fn byte_stats(mut samples: Vec<u64>) -> Json {
    assert!(!samples.is_empty());
    samples.sort_unstable();
    let upper = samples.len().saturating_sub(1);
    serde_json::json!({
        "n": samples.len(), "min_bytes": samples[0],
        "p25_bytes": samples[upper / 4], "median_bytes": samples[upper / 2],
        "p75_bytes": samples[upper.saturating_mul(3) / 4], "max_bytes": samples[upper],
    })
}

fn stage_stats(samples: Vec<u64>) -> Json {
    if samples.is_empty() {
        serde_json::json!({"status": "not-run", "reason": "the verified cache hit bypasses this stage"})
    } else {
        stats(samples)
    }
}

pub(super) fn median(report: &Json) -> u64 {
    report["median_ns"].as_u64().expect("nonempty stats")
}

#[derive(Default)]
pub(super) struct StageSamples {
    lookup: Vec<u64>,
    specialization: Vec<u64>,
    emission: Vec<u64>,
    admission: Vec<u64>,
    publication: Vec<u64>,
    total: Vec<u64>,
}

impl StageSamples {
    fn record(&mut self, stage: Stage, nanos: u64) {
        match stage {
            Stage::Lookup => &mut self.lookup,
            Stage::Specialization => &mut self.specialization,
            Stage::SourceEmission => &mut self.emission,
            Stage::Admission => &mut self.admission,
            Stage::Publication => &mut self.publication,
        }
        .push(nanos);
    }

    pub(super) fn report(self) -> Json {
        serde_json::json!({
            "lookup_and_validation": stage_stats(self.lookup),
            "specialization": stage_stats(self.specialization),
            "source_emission": stage_stats(self.emission),
            "independent_admission": stage_stats(self.admission),
            "publication": stage_stats(self.publication),
            "request_total": stats(self.total),
        })
    }
}

pub(super) fn cache_request(
    session: &mut Session,
    lowered: &Interpreted,
    map: &[Option<i64>],
    stages: &mut StageSamples,
) -> noble_ngraph::specialize::cache::Result {
    let budgets = Budgets::default();
    let invocation = Invocation::new(
        &BLAKE3,
        &lowered.checked,
        INTERPRETER.as_bytes(),
        Request {
            static_inputs: map,
            budgets: &budgets,
        },
        noble_contracts::Limits::default(),
    );
    let mut active = None;
    let (total, response) = timed(|| {
        session.specialize_with_observer(invocation, |event| match event {
            StageEvent::Begin(stage) => assert!(active.replace((stage, now())).is_none()),
            StageEvent::End(stage) => {
                let (old, started) = active.take().expect("stage began");
                assert_eq!(stage, old, "properly nested cache stages");
                stages.record(stage, u64::try_from(started.elapsed().as_nanos()).unwrap());
            }
        })
    });
    assert!(active.is_none());
    stages.total.push(total);
    response.expect("cache request succeeds")
}

fn compiled_batch(
    compiled: &Compiled,
    rows: &[Vec<Value>],
    workspace: &mut noble_ngraph::compiled::Workspace,
) {
    for row in rows {
        let observation = compiled
            .evaluate_in(workspace, black_box(row), None, &mut Control::default())
            .obs;
        black_box(observation);
    }
}

fn sampled(mut batch: impl FnMut()) -> Json {
    for _ in 0..2 {
        batch();
    }
    let mut samples = Vec::with_capacity(SAMPLES);
    for _ in 0..SAMPLES {
        let (elapsed, ()) = timed(&mut batch);
        samples.push(elapsed);
    }
    stats(samples)
}

pub(super) fn warm_costs(
    original: &Compiled,
    residual: &Compiled,
    direct: &Compiled,
    full_rows: &[Vec<Value>],
    rows: &[Vec<Value>],
    ops: [Opcode; 2],
    scalar_rows: &[(i64, i64)],
) -> Json {
    let mut original_workspace = original.workspace();
    let mut residual_workspace = residual.workspace();
    let mut direct_workspace = direct.workspace();
    let interpreted = sampled(|| compiled_batch(original, full_rows, &mut original_workspace));
    let specialized = sampled(|| compiled_batch(residual, rows, &mut residual_workspace));
    let ordinary = sampled(|| compiled_batch(direct, rows, &mut direct_workspace));
    let host = sampled(|| {
        for &(x, y) in scalar_rows {
            let accumulator = ops[0].apply(Operands {
                left: black_box(x),
                right: black_box(y),
            });
            black_box(ops[1].apply(Operands {
                left: accumulator,
                right: y,
            }));
        }
    });
    serde_json::json!({"sample_size": BATCH, "samples_per_path": SAMPLES,
        "boundary": "already-built evaluator, reusable preallocated workspace and row vectors; every returned observation consumed; host-native arithmetic has no graph output allocation",
        "interpreter": interpreted, "residual": specialized,
        "direct_ordinary_graph": ordinary, "host_native": host})
}

pub(super) fn original_rows(map: &[Option<i64>; 8], pairs: &[(i64, i64)]) -> Vec<Vec<Value>> {
    pairs
        .iter()
        .map(|&(x, y)| {
            map[..6]
                .iter()
                .map(|v| Value::I64(v.expect("static coefficient")))
                .chain([Value::I64(x), Value::I64(y)])
                .collect()
        })
        .collect()
}
