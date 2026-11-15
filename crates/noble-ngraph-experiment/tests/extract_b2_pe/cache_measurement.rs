//! Source-bound cache-on/cache-off differential and stage-cost receipt.

use super::*;
use noble_ngraph::compiled::Compiled;
use noble_ngraph::eval::Obs;
use noble_ngraph::sem::obs_eq_all;
use noble_ngraph::specialize::cache::{self, Session, Stage, StageEvent};
use noble_ngraph_experiment::scheme::BLAKE3;
use std::time::Instant;

#[path = "cache_measurement/case.rs"]
mod case;
#[path = "cache_measurement/receipt.rs"]
mod receipt;

#[derive(Default)]
pub(super) struct StageCosts {
    pub(super) lookup_ns: u64,
    pub(super) source_emission_ns: u64,
    pub(super) admission_ns: u64,
    pub(super) specialization_ns: u64,
    pub(super) publication_ns: u64,
}

pub(super) struct Uncached {
    pub(super) residual: noble_ngraph::specialize::Residual,
    pub(super) readmitted: noble_ngraph::check::CheckedGraph,
    pub(super) compiled: Compiled,
}

#[derive(Default)]
pub(super) struct UncachedCosts {
    pub(super) specialization_ns: u64,
    pub(super) source_emission_ns: u64,
    pub(super) admission_ns: u64,
    pub(super) backend_build_ns: u64,
}

#[derive(Clone, Copy)]
pub(super) struct CaseSpec {
    pub(super) root: &'static str,
    pub(super) input_leaves: usize,
    pub(super) static_inputs: &'static [Option<i64>],
}

pub(super) const CASES: &[CaseSpec] = &[
    CaseSpec {
        root: "dup * swap 3 * +",
        input_leaves: 2,
        static_inputs: &[Some(5), None],
    },
    CaseSpec {
        root: "2 + 3 +",
        input_leaves: 1,
        static_inputs: &[None],
    },
    CaseSpec {
        root: "dup dup * *",
        input_leaves: 1,
        static_inputs: &[None],
    },
];

const fn check_budgets() -> noble_ngraph::check::Budgets {
    noble_ngraph::check::Budgets {
        work_units: 4_194_304,
        expanded_nodes: 262_144,
        inline_depth: 64,
        live_nodes: 262_144,
        layers: 16_384,
        inputs: 65_536,
        outputs: 65_536,
        affine_terms: 4_096,
        table_length_entries: 65_536,
        table_entries: 1_048_576,
        facts: 262_144,
        graph_bytes: 33_554_432,
        snapshot_bytes: 4_194_304,
        forward_scratch: 67_108_864,
        backward_scratch: 134_217_728,
        optimizer_steps: 1_000_000,
        residual_nodes: 262_144,
        variants: 1,
    }
}

const fn frontend_limits() -> noble_contracts::Limits {
    noble_contracts::Limits {
        bytes: 65_536,
        nodes: 16_384,
        depth: 64,
        work: 2_000_000,
    }
}

fn evaluation_control() -> noble_ngraph::eval::Control<'static> {
    noble_ngraph::eval::Control {
        fuel: None,
        cancel: None,
        scratch: None,
        forward_scratch: None,
        evaluated_nodes: None,
    }
}

fn new_session() -> Session {
    Session::new(cache::Limits {
        entries: 64,
        variants_per_subject: 8,
        manifest_bytes: 4_194_304,
        retained_bytes: 67_108_864,
        lookup_work: 64,
        eviction_work: 4096,
    })
    .expect("explicit cache limits satisfy bounded-work constraints")
}

// justification: this receipt records elapsed stage and evaluator costs.
#[allow(
    tigerstyle::ambient_clock,
    reason = "the dedicated receipt producer measures elapsed runtime"
)]
pub(super) fn timed<T>(run: impl FnOnce() -> T) -> (u64, T) {
    let started = Instant::now();
    let result = run();
    let elapsed = u64::try_from(started.elapsed().as_nanos())
        .expect("elapsed receipt nanoseconds fit in u64");
    (elapsed, result)
}

// justification: this harness samples the measured cache stage boundaries.
#[allow(
    tigerstyle::ambient_clock,
    reason = "the dedicated receipt producer measures elapsed runtime"
)]
pub(super) fn request(
    session: &mut Session,
    lowered: &Lowered,
    root: &str,
    map: &[Option<i64>],
) -> (u64, StageCosts, Result<cache::Result, String>) {
    let mut costs = StageCosts::default();
    let mut active: Option<(Stage, Instant)> = None;
    let budgets = check_budgets();
    let invocation = noble_ngraph::specialize::cache::Invocation {
        ids: &BLAKE3,
        original: &lowered.extraction.checked,
        recipe_bytes: root.as_bytes(),
        request: noble_ngraph::specialize::Request {
            static_inputs: map,
            budgets: &budgets,
        },
        frontend_limits: frontend_limits(),
    };
    let (elapsed, result) = timed(|| {
        session.specialize_with_observer(invocation, |event| match event {
            StageEvent::Begin(stage) => {
                assert!(active.replace((stage, Instant::now())).is_none());
            }
            StageEvent::End(stage) => {
                let (active_stage, started) = active.take().expect("stage was started");
                assert_eq!(active_stage, stage, "stage boundary is paired");
                let elapsed = u64::try_from(started.elapsed().as_nanos())
                    .expect("stage nanoseconds fit in u64");
                let total = match stage {
                    Stage::Lookup => &mut costs.lookup_ns,
                    Stage::SourceEmission => &mut costs.source_emission_ns,
                    Stage::Specialization => &mut costs.specialization_ns,
                    Stage::Admission => &mut costs.admission_ns,
                    Stage::Publication => &mut costs.publication_ns,
                };
                *total = total.saturating_add(elapsed);
            }
        })
    });
    assert!(active.is_none(), "all cache stages completed");
    (
        elapsed,
        costs,
        result.map_err(|diagnostic| diagnostic.to_string()),
    )
}

pub(super) fn execute(compiled: &Compiled, inputs: &[Vec<Value>]) -> (u64, Vec<Vec<Value>>) {
    let mut workspace = compiled.workspace();
    timed(|| {
        inputs
            .iter()
            .map(|row| {
                match compiled
                    .evaluate_in(&mut workspace, row, None, &mut evaluation_control())
                    .obs
                {
                    Obs::Normal(values) => values,
                    other => {
                        panic!("cache measurement produced a non-normal observation: {other:?}")
                    }
                }
            })
            .collect()
    })
}

pub(super) fn prepare_uncached(
    lowered: &Lowered,
    map: &[Option<i64>],
) -> (UncachedCosts, Uncached) {
    let budgets = check_budgets();
    let (specialization_ns, residual) = timed(|| {
        noble_ngraph::specialize::specialize(
            &BLAKE3,
            &lowered.extraction.checked,
            &noble_ngraph::specialize::Request {
                static_inputs: map,
                budgets: &budgets,
            },
        )
        .expect("cache-disabled specialization")
    });
    let (source_emission_ns, source) = timed(|| {
        noble_ngraph::specialize::emit_source(&residual.checked)
            .expect("cache-disabled source emission")
    });
    let (admission_ns, readmitted) = timed(|| {
        let types = noble_ngraph::specialize::interface_types(&residual.checked)
            .expect("cache-disabled residual interface");
        noble_ngraph::specialize::readmit(&BLAKE3, &source, &types, frontend_limits(), &budgets)
            .expect("cache-disabled admission")
    });
    let (backend_build_ns, compiled) = timed(|| Compiled::new(&BLAKE3, &readmitted));
    (
        UncachedCosts {
            specialization_ns,
            source_emission_ns,
            admission_ns,
            backend_build_ns,
        },
        Uncached {
            residual,
            readmitted,
            compiled,
        },
    )
}
