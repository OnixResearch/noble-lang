//! Clean-commit producer for the preregistered first-projection receipt.

use super::*;
use noble_ngraph::specialize::cache::{Invocation, Limits, Session, Stage, StageEvent, Status};
use serde_json::{json, Value as Json};
use std::hint::black_box;
use std::process::Command;
use std::time::Instant;

const REPEATS: usize = 11;
const SAMPLES: usize = 31;
const BATCH: usize = 1_024;

fn timed<T>(run: impl FnOnce() -> T) -> (u64, T) {
    let start = Instant::now();
    let result = run();
    (
        u64::try_from(start.elapsed().as_nanos()).expect("elapsed nanoseconds fit u64"),
        result,
    )
}

fn stats(mut samples: Vec<u64>) -> Json {
    assert!(!samples.is_empty());
    samples.sort_unstable();
    json!({
        "n": samples.len(), "min_ns": samples[0],
        "p25_ns": samples[(samples.len() - 1) / 4],
        "median_ns": samples[(samples.len() - 1) / 2],
        "p75_ns": samples[3 * (samples.len() - 1) / 4],
        "max_ns": samples[samples.len() - 1],
    })
}

fn stage_stats(samples: Vec<u64>) -> Json {
    if samples.is_empty() {
        json!({"status": "not-run", "reason": "the verified cache hit bypasses this stage"})
    } else {
        stats(samples)
    }
}

fn byte_stats(mut samples: Vec<u64>) -> Json {
    assert!(!samples.is_empty());
    samples.sort_unstable();
    json!({
        "n": samples.len(), "min_bytes": samples[0],
        "p25_bytes": samples[(samples.len() - 1) / 4],
        "median_bytes": samples[(samples.len() - 1) / 2],
        "p75_bytes": samples[3 * (samples.len() - 1) / 4],
        "max_bytes": samples[samples.len() - 1],
    })
}

fn median(report: &Json) -> u64 {
    report["median_ns"].as_u64().expect("nonempty stats")
}

#[derive(Default)]
struct Stages {
    lookup: Vec<u64>,
    specialization: Vec<u64>,
    emission: Vec<u64>,
    admission: Vec<u64>,
    publication: Vec<u64>,
    total: Vec<u64>,
}

impl Stages {
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

    fn report(self) -> Json {
        json!({
            "lookup_and_validation": stage_stats(self.lookup),
            "specialization": stage_stats(self.specialization),
            "source_emission": stage_stats(self.emission),
            "independent_admission": stage_stats(self.admission),
            "publication": stage_stats(self.publication),
            "request_total": stats(self.total),
        })
    }
}

fn cache_request(
    session: &mut Session,
    lowered: &Interpreted,
    map: &[Option<i64>],
    stages: &mut Stages,
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
            StageEvent::Begin(stage) => assert!(active.replace((stage, Instant::now())).is_none()),
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

fn warm_costs(
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
            black_box(ops[1].apply(ops[0].apply(black_box(x), black_box(y)), y));
        }
    });
    json!({"sample_size": BATCH, "samples_per_path": SAMPLES,
        "boundary": "already-built evaluator, reusable preallocated workspace and row vectors; every returned observation consumed; host-native arithmetic has no graph output allocation",
        "interpreter": interpreted, "residual": specialized,
        "direct_ordinary_graph": ordinary, "host_native": host})
}

fn original_rows(map: &[Option<i64>; 8], pairs: &[(i64, i64)]) -> Vec<Vec<Value>> {
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

fn stage_record(lowered: &Interpreted, ops: [Opcode; 2], pairs: &[(i64, i64)]) -> Json {
    let source = [ops[0].byte(), ops[1].byte()];
    let map = static_map(ops);
    let direct_source = direct_source(ops);
    let mut specialization = Vec::with_capacity(REPEATS);
    let mut emission = Vec::with_capacity(REPEATS);
    let mut admission = Vec::with_capacity(REPEATS);
    let mut build = Vec::with_capacity(REPEATS);
    let mut direct_build = Vec::with_capacity(REPEATS);
    let mut cold = Vec::with_capacity(REPEATS);
    let mut miss = Stages::default();
    let mut hit = Stages::default();
    let mut cache_bytes = Vec::with_capacity(REPEATS);
    let mut compiled = None;
    let mut direct = None;
    let mut last_residual = None;
    let mut last_source = None;
    let mut last_graph = None;
    for _ in 0..REPEATS {
        let (elapsed, new_direct) = timed(|| super::direct(ops));
        direct_build.push(elapsed);
        direct = Some(new_direct);
        let budgets = Budgets::default();
        let (elapsed, residual) = timed(|| {
            specialize::specialize(
                &BLAKE3,
                &lowered.checked,
                &Request {
                    static_inputs: &map,
                    budgets: &budgets,
                },
            )
            .expect("specialization")
        });
        specialization.push(elapsed);
        super::assert_dispatch_removed(&residual, ops);
        let (elapsed, text) =
            timed(|| specialize::emit_source(&residual.checked).expect("source emission"));
        emission.push(elapsed);
        let (elapsed, checked) = timed(|| {
            specialize::readmit(
                &BLAKE3,
                &text,
                &specialize::interface_types(&residual.checked).expect("interface"),
                noble_contracts::Limits::default(),
                &budgets,
            )
            .expect("independent admission")
        });
        admission.push(elapsed);
        let (elapsed, new_compiled) = timed(|| Compiled::new(&BLAKE3, &checked));
        build.push(elapsed);
        let mut workspace = new_compiled.workspace();
        let (elapsed, first) = timed(|| {
            new_compiled
                .evaluate_in(
                    &mut workspace,
                    &eval::i64_inputs(&[7, 2]),
                    None,
                    &mut Control::default(),
                )
                .obs
        });
        assert_eq!(
            first,
            Obs::Normal(vec![Value::I64(reference(&source, 7, 2).unwrap())])
        );
        cold.push(elapsed);
        let mut session = Session::new(Limits::default()).expect("bounded default session");
        let cached_miss = cache_request(&mut session, lowered, &map, &mut miss);
        assert_eq!(cached_miss.status, Status::MissStored);
        let cached_hit = cache_request(&mut session, lowered, &map, &mut hit);
        assert_eq!(cached_hit.status, Status::Hit);
        assert_eq!(cached_hit.source, text);
        assert_eq!(cached_hit.readmitted.id(), checked.id());
        assert_eq!(cached_miss.residual.checked.id(), residual.checked.id());
        assert_eq!(cached_hit.lookup_entries, 1);
        cache_bytes.push(session.retained_bytes() as u64);
        compiled = Some(new_compiled);
        last_residual = Some(residual);
        last_source = Some(text);
        last_graph = Some(checked);
    }
    let compiled = compiled.expect("measured compiled graph");
    let direct = direct.expect("measured direct graph");
    let last_residual = last_residual.expect("measured residual");
    let emitted = last_source.expect("measured emitted source");
    let checked = last_graph.expect("measured admitted graph");
    let dynamic_rows: Vec<Vec<Value>> = pairs
        .iter()
        .map(|&(x, y)| eval::i64_inputs(&[x, y]))
        .collect();
    let full_rows = original_rows(&map, pairs);
    for (&(x, y), (original, dynamic)) in pairs.iter().zip(full_rows.iter().zip(&dynamic_rows)) {
        let expected = Obs::Normal(vec![Value::I64(reference(&source, x, y).unwrap())]);
        let compiled_obs = compiled
            .evaluate(dynamic, None, &mut Control::default())
            .obs;
        let interpreter_obs = lowered
            .compiled
            .evaluate(original, None, &mut Control::default())
            .obs;
        let direct_obs = direct
            .compiled
            .evaluate(dynamic, None, &mut Control::default())
            .obs;
        assert_eq!(compiled_obs, expected);
        assert_eq!(interpreter_obs, expected);
        assert_eq!(direct_obs, expected);
    }
    let warm = warm_costs(
        &lowered.compiled,
        &compiled,
        &direct.compiled,
        &full_rows,
        &dynamic_rows,
        ops,
        pairs,
    );
    let saving = i128::from(median(&warm["interpreter"])) - i128::from(median(&warm["residual"]));
    let setup_stats = json!({"specialization": stats(specialization), "source_emission": stats(emission),
        "independent_admission": stats(admission), "backend_build": stats(build),
        "direct_ordinary_frontend_extraction_and_build": stats(direct_build),
        "cold_first_residual_execution": stats(cold)});
    let setup_sum = [
        "specialization",
        "source_emission",
        "independent_admission",
        "backend_build",
    ]
    .into_iter()
    .map(|stage| u128::from(median(&setup_stats[stage])))
    .sum::<u128>();
    let direct_setup = u128::from(median(
        &setup_stats["direct_ordinary_frontend_extraction_and_build"],
    ));
    // The interpreter itself is a common built artifact; the direct compiler
    // is a separate comparator, not a cost offset against interpretation.
    let incremental = setup_sum;
    let break_even_calls = if saving > 0 {
        let saving = u128::try_from(saving).expect("positive batch savings");
        Some(
            (incremental
                .saturating_mul(BATCH as u128)
                .saturating_add(saving - 1))
                / saving,
        )
    } else {
        None
    };
    json!({
        "source_tags": source, "one_hot_static_map": map,
        "interpreter_graph": noble_ngraph::ir::hex(&lowered.checked.id()),
        "residual_graph": noble_ngraph::ir::hex(&last_residual.checked.id()),
        "residual_graph_text": noble_ngraph::format::graph::print(last_residual.checked.graph()),
        "readmitted_graph": noble_ngraph::ir::hex(&checked.id()),
        "direct_graph": noble_ngraph::ir::hex(&direct.checked.id()),
        "residual_source": emitted, "direct_source": direct_source,
        "residual_source_bytes": emitted.len(), "original_source_bytes": INTERPRETER.len(),
        "original_nodes": lowered.checked.graph().nodes.len(),
        "residual_nodes": last_residual.report.residual_nodes,
        "variants": last_residual.report.variants,
        "cache_retained_bytes": byte_stats(cache_bytes),
        "compiled_retained_bytes": {"interpreter": lowered.compiled.retained_bytes(),
            "residual": compiled.retained_bytes(), "direct": direct.compiled.retained_bytes()},
        "workspace_scratch_bytes": {"interpreter": lowered.compiled.scratch_bytes(),
            "residual": compiled.scratch_bytes(), "direct": direct.compiled.scratch_bytes()},
        "stage_ns": setup_stats, "cache_miss_ns": miss.report(), "cache_hit_ns": hit.report(),
        "warm_batch_ns": warm, "batch_savings_ns": saving.to_string(),
        "incremental_setup_ns": incremental, "direct_setup_ns_not_offset_against_interpretation": direct_setup,
        "break_even_calls": break_even_calls,
        "break_even_reason_if_null": if saving <= 0 { Some("no positive warmed per-batch savings") } else { None },
        "differential_vectors": pairs.len(), "differential_agreement": true,
    })
}

fn version(command: &str, arg: &str) -> String {
    let output = Command::new(command)
        .arg(arg)
        .output()
        .expect("tool version executable");
    assert!(output.status.success(), "tool version command: {command}");
    String::from_utf8(output.stdout)
        .expect("version UTF-8")
        .trim()
        .to_owned()
}

fn clean_revision() -> String {
    let root = env!("CARGO_MANIFEST_DIR");
    let output = Command::new("git")
        .args(["-C", root, "status", "--porcelain", "--untracked-files=all"])
        .output()
        .expect("check source cleanliness");
    assert!(
        output.status.success() && output.stdout.is_empty(),
        "receipt requires clean committed tree"
    );
    let revision = Command::new("git")
        .args(["-C", root, "rev-parse", "HEAD"])
        .output()
        .expect("read source revision");
    assert!(revision.status.success());
    let revision = String::from_utf8(revision.stdout)
        .expect("revision UTF-8")
        .trim()
        .to_owned();
    assert_eq!(
        revision,
        std::env::var("NOBLE_INTERP_SOURCE_REV").expect("frozen source revision")
    );
    revision
}

#[test]
#[ignore = "writes a receipt to the absent NOBLE_INTERP_RECEIPT path after a clean commit"]
fn source_bound_first_projection_receipt() {
    let revision = clean_revision();
    let path =
        std::env::var("NOBLE_INTERP_RECEIPT").expect("receipt destination outside repository");
    assert!(
        !std::path::Path::new(&path).exists(),
        "receipt destination must not exist"
    );
    let (build_ns, base) = timed(interpreter);
    let pairs = program_rows();
    let scalar_rows: Vec<(i64, i64)> = (0..BATCH).map(|i| pairs[i % pairs.len()]).collect();
    let mut records = Vec::with_capacity(9);
    for first in Opcode::ALL {
        for second in Opcode::ALL {
            records.push(stage_record(&base, [first, second], &scalar_rows));
        }
    }
    let host = std::fs::read_to_string("/proc/cpuinfo").expect("host CPU model");
    let cpu = host
        .lines()
        .find(|line| line.starts_with("model name"))
        .unwrap_or("model name: unsupported")
        .to_owned();
    let receipt = json!({
        "schema": "noble-interpreter-first-projection/v1", "issue": 34,
        "source_revision": revision, "clean_source": true,
        "criteria": "verification/interpreter-specialization/criteria.md",
        "profile": "ngraph-i64/0", "specializer": noble_ngraph::specialize::ACCOUNTING_REVISION,
        "graph_semantics": noble_ngraph::ir::SEMANTIC_REVISION,
        "compiled_backend": noble_ngraph::compiled::COMPILER_REVISION,
        "specializer_revision": "ngraph-pe-specializer/2",
        "frontend_limits": {
            "interpreter": {"bytes": 65_536, "nodes": 524_288, "depth": 64, "work": 512_000_000},
            "residual_and_direct": {
                "bytes": noble_contracts::Limits::default().bytes,
                "nodes": noble_contracts::Limits::default().nodes,
                "depth": noble_contracts::Limits::default().depth,
                "work": noble_contracts::Limits::default().work,
            },
            "graph_budgets": format!("{:?}", Budgets::default()),
        },
        "optimization_flags": {"rust_debug_assertions": cfg!(debug_assertions), "source": "unoptimized ordinary Noble graph; no Wasm or wasm-opt path used"},
        "hardware": {"cpu": cpu, "os": version("uname", "-a")},
        "toolchain": {"rustc": version("rustc", "--version"), "cargo": version("cargo", "--version")},
        "dataset": {"seed": format!("0x{SEED:016x}"), "boundary_and_seeded_rows": pairs.len(),
            "timing_batch": BATCH, "timing_rows": "the same deterministic cyclic batch of boundary and seeded rows on all paths"},
        "repetitions": {"stage": REPEATS, "warm_samples": SAMPLES, "warm_batch": BATCH},
        "cache": {"limits": {"entries": Limits::default().entries,
            "variants_per_subject": Limits::default().variants_per_subject,
            "manifest_bytes": Limits::default().manifest_bytes,
            "retained_bytes": Limits::default().retained_bytes,
            "lookup_work": Limits::default().lookup_work,
            "eviction_work": Limits::default().eviction_work},
            "state": "new empty session for each setup repetition, miss then verified hit; no persisted executable"},
        "initial_interpreter_ordinary_admission_extraction_and_build_ns": build_ns,
        "unsupported": {"actual_peak_scratch_heap": "no allocator peak instrumentation; compiled workspace bytes are measured structurally",
            "end_to_end_cli_prepare_compile_instantiate": "no CLI Wasm process in this in-process graph experiment; cannot turn graph latency into CLI cost",
            "Wasm_code_bytes": "compiled backend is the in-memory layered graph evaluator, not a Wasm code emitter",
            "second_or_third_projection_cost": "no accepted representation of the Rust specializer as a Noble subject"},
        "proof_boundary": "existing FirstProjection/OperandForwarding Lean graph model, no Rust correspondence or Program refinement",
        "records": records,
    });
    std::fs::write(path, serde_json::to_string_pretty(&receipt).unwrap() + "\n")
        .expect("write receipt");
}
