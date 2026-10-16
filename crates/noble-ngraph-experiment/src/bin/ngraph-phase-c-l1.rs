//! Issue #14 Phase C C1 for the `I64` workloads W1 (`I1`) and W2 (`I4`, 10^5
//! host steps): extraction from Core source, the extracted graph on L3/L4,
//! the host-native baseline, and the ordinary Noble Wasm/V8 path (L1, the
//! real CLI with `--opt off` and `--opt on`). Release only; not part of the
//! default test suite.
//!
//! ```text
//! cargo build --release -p noble-ngraph-experiment --bin ngraph-phase-c-l1
//! target/release/ngraph-phase-c-l1 --cli /abs/noble --out verification/ngraph/phase-c-l1.json \
//!   --source-rev "$(git rev-parse HEAD)" --toolchain "$(rustc -V)" \
//!   [--w1-vectors N] [--w2-steps N] [--seed N] [--samples N] [--cold-reps N] [--w2-sequential]
//! ```
//!
//! Every L1 observation is compared with L4 on the extracted graph (and the
//! W2 final state with the native recurrence) before any timing is reported.
//! L1 times come from report arrival ([`l1::Timing`]): the CLI prepares,
//! compiles, instantiates and runs each submission, so an L1 "evaluation" is
//! one root submission, not a call into an already instantiated module.
//! `--out` must not exist. Exit status 1 on any disagreement.

use std::hint::black_box;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::{Duration, Instant};

use noble_ngraph_experiment::bench::{cpu_model, ns, read, sha256, stats, warm};
use noble_ngraph_experiment::compiled::{Compiled, COMPILER_REVISION};
use noble_ngraph_experiment::eval::{i64_inputs, reference, Control};
use noble_ngraph_experiment::extract::{self, source, Budget, Extraction, Request};
use noble_ngraph_experiment::ir::{hex, Graph, IR_VERSION, SEMANTIC_REVISION};
use noble_ngraph_experiment::json::Json;
use noble_ngraph_experiment::l1::{self, Cli, Opt, Timing};
use noble_ngraph_experiment::prng::{SplitMix64, PRNG};
use noble_ngraph_experiment::workloads;
use noble_ngraph_experiment::Value;

const I1_ROOT: &str = "dup * swap 3 * + 7 -";
const I4_ROOT: &str = "swap [ * ] dip + dup";
/// W2 initial state and constant host inputs (as in `ngraph-phase-c`).
const W2_S0: i64 = 11;
const W2_I: i64 = 3;
const W2_K: i64 = -7;
/// Steps per `run_transition` call (100 sessions of 120 steps); progress is
/// reported on stderr after each.
const W2_BATCH: usize = 12_000;

struct Args {
    out: PathBuf,
    cli: PathBuf,
    source_rev: String,
    toolchain: String,
    w1_vectors: usize,
    w2_steps: usize,
    seed: u64,
    samples: usize,
    cold_reps: usize,
    w2_concurrent: bool,
}

fn args() -> Result<Args, String> {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let flag = |name: &str| {
        argv.iter()
            .position(|a| a == name)
            .and_then(|i| argv.get(i + 1))
            .cloned()
    };
    let need = |name: &str| flag(name).ok_or(format!("{name} is required"));
    let num = |name: &str, default: u64| -> Result<u64, String> {
        flag(name).map_or(Ok(default), |v| {
            match v.strip_prefix("0x") {
                Some(hex) => u64::from_str_radix(hex, 16),
                None => v.parse(),
            }
            .map_err(|e| format!("{name}: {e}"))
        })
    };
    let size = |name: &str, default: u64| -> Result<usize, String> {
        usize::try_from(num(name, default)?).map_err(|e| e.to_string())
    };
    Ok(Args {
        out: need("--out")?.into(),
        cli: need("--cli")?.into(),
        source_rev: flag("--source-rev").unwrap_or_else(|| "unrecorded".into()),
        toolchain: flag("--toolchain").unwrap_or_else(|| "unrecorded".into()),
        w1_vectors: size("--w1-vectors", 1_000)?,
        w2_steps: size("--w2-steps", 100_000)?,
        seed: num("--seed", 0x1414_c101)?,
        samples: size("--samples", 2_000)?,
        cold_reps: size("--cold-reps", 21)?,
        w2_concurrent: !argv.iter().any(|a| a == "--w2-sequential"),
    })
}

fn obj(pairs: Vec<(&str, Json)>) -> Json {
    Json::O(pairs.into_iter().map(|(k, v)| (k.to_owned(), v)).collect())
}

/// Extraction from Core source, timed per D7 stage over `reps` repetitions.
/// Returns the stage timings and the last extraction with its L4 program.
fn extraction(
    root: &str,
    inputs: usize,
    first: &[Value],
    reps: usize,
) -> (Json, Extraction, Compiled) {
    let (mut fe, mut adm, mut ext, mut build, mut ev, mut tot) = (
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
    );
    let mut last = None;
    for _ in 0..reps.max(1) {
        let t0 = Instant::now();
        let p = source::prepare(&[], root, &source::i64s(inputs)).expect("Core frontend");
        let t1 = Instant::now();
        let admitted = extract::admit(
            &p.submission,
            &Request {
                profile: "ngraph-i64/0",
                claimed_closure: &p.claimed_closure,
                budget: Budget::default(),
            },
        )
        .expect("admitted");
        let t2 = Instant::now();
        let e = extract::extract(&admitted).expect("extracted");
        let t3 = Instant::now();
        let comp = Compiled::new(&e.checked);
        let t4 = Instant::now();
        black_box(comp.evaluate(first, None, &Control::default()));
        let t5 = Instant::now();
        fe.push(ns(t1 - t0));
        adm.push(ns(t2 - t1));
        ext.push(ns(t3 - t2));
        build.push(ns(t4 - t3));
        ev.push(ns(t5 - t4));
        tot.push(ns(t5 - t0));
        last = Some((e, comp));
    }
    let (e, comp) = last.expect("at least one repetition");
    let s = e.stats;
    let j = Json::obj([
        ("repetitions", Json::U(reps.max(1) as u64)),
        ("frontend_prepare", stats(fe).0),
        ("admission_d7_stage1_kernel_recheck", stats(adm).0),
        (
            "extraction_d7_stages2_5_including_graph_checker",
            stats(ext).0,
        ),
        ("build_compile_l4", stats(build).0),
        ("first_evaluation_l4", stats(ev).0),
        ("cold_total_source_to_first_result", stats(tot).0),
        (
            "extractor_counters",
            Json::obj([
                ("work_units", Json::U(s.work_units)),
                ("expanded_nodes", Json::U(s.expanded_nodes)),
                ("pruned_nodes", Json::U(s.pruned_nodes)),
                ("max_inline_depth", Json::U(s.max_inline_depth)),
            ]),
        ),
    ]);
    (j, e, comp)
}

fn graph_json(e: &Extraction, hand_built: &Graph) -> Json {
    let g = e.checked.graph();
    // Operations and outputs; node origins and the subject are producer
    // metadata and differ by construction.
    let same = g.outputs == hand_built.outputs
        && g.nodes.len() == hand_built.nodes.len()
        && g.nodes
            .iter()
            .zip(&hand_built.nodes)
            .all(|(a, b)| a.op == b.op);
    Json::obj([
        ("graph_logical_id", Json::s(hex(&e.checked.id()))),
        ("live_nodes", Json::U(e.checked.node_count() as u64)),
        ("depth_layers", Json::U(u64::from(e.checked.layer_count()))),
        (
            "ops_and_outputs_equal_ngraph_phase_c_hand_built_graph",
            Json::Bool(same),
        ),
    ])
}

fn l4_outputs(comp: &Compiled, inputs: &[i64]) -> Vec<i64> {
    comp.evaluate(&i64_inputs(inputs), None, &Control::default())
        .obs
        .outputs()
        .expect("normal")
        .iter()
        .map(|v| match v {
            Value::I64(x) => *x,
            other => panic!("non-I64 output {other:?}"),
        })
        .collect()
}

fn metrics(raw: &str) -> Json {
    let v: serde_json::Value = serde_json::from_str(raw).unwrap_or_default();
    let m = &v["metrics"];
    let field = |k: &str| {
        m.get(k)
            .and_then(serde_json::Value::as_u64)
            .map_or(Json::s("unsupported"), Json::U)
    };
    Json::obj([
        ("allocated_total_bytes", field("allocated_total_bytes")),
        ("allocation_peak_bytes", field("allocation_peak_bytes")),
        ("operand_peak_bytes", field("operand_peak_bytes")),
        ("continuation_peak_bytes", field("continuation_peak_bytes")),
        ("linear_memory_bytes", field("memory_bytes")),
        ("execution_steps", field("execution_steps")),
        ("source", Json::s("the root report's noble-core-report/v1 `metrics` (guest-side counters); Wasm module bytes, V8 heap and process RSS: unsupported (the report carries module hashes only)")),
    ])
}

struct L1Summary {
    json: Json,
    root_warm_median_ns: f64,
}

/// L1 timing summary over one run (`wall` = whole run, all sessions).
fn l1_timing(timing: &[Timing], wall: Duration) -> L1Summary {
    let warm_of = |f: fn(&Timing) -> Duration| -> Vec<f64> {
        timing
            .iter()
            .filter(|t| !t.first_in_session)
            .map(|t| ns(f(t)))
            .collect()
    };
    let roots = warm_of(|t| t.root);
    let pushes = warm_of(|t| t.push);
    let steps = warm_of(|t| t.push + t.root);
    let cold: Vec<f64> = timing
        .iter()
        .filter(|t| t.first_in_session)
        .map(|t| ns(t.since_spawn))
        .collect();
    let (rj, rm) = stats(roots);
    #[allow(clippy::cast_precision_loss)]
    let rate = timing.len() as f64 / wall.as_secs_f64();
    L1Summary {
        json: Json::obj([
            ("sessions", Json::U(cold.len() as u64)),
            ("warm_root_submission_latency", rj),
            ("warm_input_submission_latency", stats(pushes).0),
            ("warm_host_step_input_plus_root", stats(steps).0),
            ("cold_spawn_to_first_root_report", stats(cold).0),
            ("wall_ns_all_sessions", Json::F(ns(wall))),
            ("host_loop_evaluations_per_s", Json::F(rate)),
            ("timing_source", Json::s("report-line arrival times (CLI flushes one line per finished submission); warm = every vector/step that is not the first of its session process; cold = process spawn to the first root report of each session")),
        ]),
        root_warm_median_ns: rm,
    }
}

struct W1Mode {
    json: Json,
    agree: bool,
    root_ns: f64,
}

fn w1_mode(cli: &Cli, vectors: &[[i64; 2]], expected: &[i64]) -> Result<W1Mode, String> {
    let l1v: Vec<Vec<l1::Value>> = vectors
        .iter()
        .map(|v| v.iter().map(|x| l1::Value::I64(*x)).collect())
        .collect();
    let t = Instant::now();
    let run = cli
        .run_vectors(&[], I1_ROOT, 1, &l1v)
        .map_err(|e| e.to_string())?;
    let wall = t.elapsed();
    let mut agree = 0u64;
    let mut quota = 0u64;
    let mut mismatches = Vec::new();
    for (i, r) in run.results.iter().enumerate() {
        if r.ok() && r.stack == [l1::Value::I64(expected[i])] {
            agree += 1;
        } else if l1::quota(r) {
            quota += 1;
        } else if mismatches.len() < 10 {
            mismatches.push(Json::s(format!(
                "vector {i} {:?}: expected {} got {}",
                vectors[i], expected[i], r.raw
            )));
        }
    }
    let complete = run.results.len() == vectors.len();
    let ok = complete && agree as usize == vectors.len();
    let s = l1_timing(&run.timing, wall);
    Ok(W1Mode {
        json: Json::obj([
            ("vectors", Json::U(vectors.len() as u64)),
            ("observed", Json::U(run.results.len() as u64)),
            ("agree_with_l4", Json::U(agree)),
            ("ordinary_quota_outcomes", Json::U(quota)),
            (
                "session_restarts_after_ordinary_quota",
                Json::U(run.restarts as u64),
            ),
            ("mismatches", Json::A(mismatches)),
            ("timing", s.json),
            (
                "design_vector_root_metrics",
                run.results.first().map_or(Json::Null, |r| metrics(&r.raw)),
            ),
        ]),
        agree: ok,
        root_ns: s.root_warm_median_ns,
    })
}

struct W2Mode {
    json: Json,
    agree: bool,
    total_ns: f64,
}

fn w2_mode(cli: &Cli, expected: &[i64]) -> Result<W2Mode, String> {
    let steps = expected.len();
    let inputs = vec![l1::Value::I64(W2_I), l1::Value::I64(W2_K)];
    let mut state = W2_S0;
    let mut timing = Vec::with_capacity(steps);
    let mut restarts = 0usize;
    let mut agree = 0u64;
    let mut mismatches = Vec::new();
    let t = Instant::now();
    let mut at = 0;
    while at < steps {
        let n = W2_BATCH.min(steps - at);
        let run = cli
            .run_transition(
                &[],
                I4_ROOT,
                2,
                &[l1::Value::I64(state)],
                &vec![inputs.clone(); n],
            )
            .map_err(|e| format!("step {at}: {e}"))?;
        for (j, out) in run.results.iter().enumerate() {
            let want = expected[at + j];
            if out == &[l1::Value::I64(want), l1::Value::I64(want)] {
                agree += 1;
            } else if mismatches.len() < 10 {
                mismatches.push(Json::s(format!(
                    "step {}: expected ({want}, {want}) got {out:?}",
                    at + j
                )));
            }
        }
        state = match run.results.last() {
            Some(v) => match v.first() {
                Some(l1::Value::I64(s)) => *s,
                other => return Err(format!("step {at}: state {other:?}")),
            },
            None => return Err(format!("step {at}: no results")),
        };
        restarts += run.restarts;
        timing.extend(run.timing);
        at += n;
        eprintln!(
            "[{}] W2 L1 --opt {}: {at}/{steps} steps, {:.0} s",
            cli.opt.flag(),
            cli.opt.flag(),
            t.elapsed().as_secs_f64()
        );
    }
    let wall = t.elapsed();
    let ok = agree as usize == steps && state == expected[steps - 1];
    let s = l1_timing(&timing, wall);
    Ok(W2Mode {
        json: Json::obj([
            ("steps", Json::U(steps as u64)),
            ("agree_with_l4_per_step", Json::U(agree)),
            ("final_state", Json::I(state)),
            (
                "session_restarts_after_ordinary_quota",
                Json::U(restarts as u64),
            ),
            ("mismatches", Json::A(mismatches)),
            ("total_10^5_steps_wall_ns", Json::F(ns(wall))),
            ("timing", s.json),
        ]),
        agree: ok,
        total_ns: ns(wall),
    })
}

fn total_stats(reps: usize, mut f: impl FnMut() -> i64) -> (Json, f64, i64) {
    let mut v = Vec::with_capacity(reps);
    let mut last = 0;
    for _ in 0..reps {
        let t = Instant::now();
        last = f();
        v.push(ns(t.elapsed()));
    }
    let (j, m) = stats(v);
    (j, m, last)
}

#[allow(clippy::too_many_lines)]
fn main() -> ExitCode {
    let a = match args() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(2);
        }
    };
    if a.out.exists() {
        eprintln!("{} exists", a.out.display());
        return ExitCode::from(2);
    }
    let load_start = read("/proc/loadavg");
    let started = Instant::now();
    let ctl = Control::default();

    // W1: extraction, L3/L4 on the extracted graph, native.
    let (w1_ext, e1, comp1) = extraction(I1_ROOT, 2, &i64_inputs(&[5, 4]), a.cold_reps);
    let x1 = i64_inputs(&[5, 4]);
    let mut ws1 = comp1.workspace();
    let (l3j, l3m, _) = warm(a.samples, || {
        black_box(reference(&e1.checked, black_box(&x1), None, &ctl));
    });
    let (l4j, l4m, _) = warm(a.samples, || {
        black_box(comp1.evaluate_in(&mut ws1, black_box(&x1), None, &ctl));
    });
    let (nj, nm, _) = warm(a.samples, || {
        black_box(workloads::w1_native(black_box(5), black_box(4)));
    });
    let mut rng = SplitMix64::new(a.seed);
    let mut vectors = vec![[5i64, 4i64]];
    #[allow(clippy::cast_possible_wrap)]
    while vectors.len() < a.w1_vectors {
        vectors.push([rng.next_u64() as i64, rng.next_u64() as i64]);
    }
    let expected1: Vec<i64> = vectors.iter().map(|v| l4_outputs(&comp1, v)[0]).collect();
    let native_agree = vectors
        .iter()
        .zip(&expected1)
        .all(|(v, e)| workloads::w1_native(v[0], v[1]) == *e);

    // W2: extraction, L3/L4 on the extracted graph, native.
    let (w2_ext, e2, comp2) =
        extraction(I4_ROOT, 3, &i64_inputs(&[W2_S0, W2_I, W2_K]), a.cold_reps);
    let mut expected2 = Vec::with_capacity(a.w2_steps);
    let mut s = W2_S0;
    for _ in 0..a.w2_steps {
        let out = l4_outputs(&comp2, &[s, W2_I, W2_K]);
        assert_eq!(out[0], out[1], "I4 outputs are (n, n)");
        s = out[0];
        expected2.push(s);
    }
    let steps_u64 = a.w2_steps as u64;
    let want2 = workloads::w2_native(W2_S0, W2_I, W2_K, steps_u64);
    let mut ws2 = comp2.workspace();
    let run_l = |l4: bool, ws: &mut noble_ngraph_experiment::compiled::Workspace| {
        let mut s = W2_S0;
        for _ in 0..a.w2_steps {
            let r = if l4 {
                comp2.evaluate_in(ws, &i64_inputs(&[s, W2_I, W2_K]), None, &ctl)
            } else {
                reference(&e2.checked, &i64_inputs(&[s, W2_I, W2_K]), None, &ctl)
            };
            if let Some([Value::I64(n), _]) = r.obs.outputs() {
                s = *n;
            }
        }
        s
    };
    let (l3t, _, l3s) = total_stats(7, || run_l(false, &mut ws2.clone()));
    let (l4t, l4tm, l4s) = total_stats(7, || run_l(true, &mut ws2));
    let (nt, ntm, nts) = total_stats(7, || {
        workloads::w2_native(
            black_box(W2_S0),
            black_box(W2_I),
            black_box(W2_K),
            black_box(steps_u64),
        )
    });
    let cpu_agree2 =
        l3s == want2 && l4s == want2 && nts == want2 && expected2.last() == Some(&want2);

    // L1: W1 both modes, then W2 both modes (concurrently unless --w2-sequential).
    let mut failures = Vec::new();
    let mut w1_modes = Vec::new();
    let mut w1_root = Vec::new();
    for opt in [Opt::Off, Opt::On] {
        eprintln!("W1 L1 --opt {}", opt.flag());
        match w1_mode(&Cli::new(a.cli.clone(), opt), &vectors, &expected1) {
            Ok(m) => {
                if !m.agree {
                    failures.push(format!("W1 --opt {} disagrees", opt.flag()));
                }
                w1_root.push((opt, m.root_ns));
                w1_modes.push((opt.flag(), m.json));
            }
            Err(e) => {
                failures.push(format!("W1 --opt {}: {e}", opt.flag()));
                w1_modes.push((opt.flag(), Json::obj([("error", Json::s(e))])));
            }
        }
    }
    let w2_started = Instant::now();
    let w2_results: Vec<(Opt, Result<W2Mode, String>)> = if a.w2_concurrent {
        std::thread::scope(|sc| {
            let hs: Vec<_> = [Opt::Off, Opt::On]
                .into_iter()
                .map(|opt| {
                    let cli = Cli::new(a.cli.clone(), opt);
                    let exp = &expected2;
                    (opt, sc.spawn(move || w2_mode(&cli, exp)))
                })
                .collect();
            hs.into_iter()
                .map(|(opt, h)| {
                    (
                        opt,
                        h.join().unwrap_or_else(|_| Err("thread panicked".into())),
                    )
                })
                .collect()
        })
    } else {
        [Opt::Off, Opt::On]
            .into_iter()
            .map(|opt| (opt, w2_mode(&Cli::new(a.cli.clone(), opt), &expected2)))
            .collect()
    };
    let w2_wall = w2_started.elapsed();
    let mut w2_modes = Vec::new();
    let mut w2_total = Vec::new();
    for (opt, r) in w2_results {
        match r {
            Ok(m) => {
                if !m.agree {
                    failures.push(format!("W2 --opt {} disagrees", opt.flag()));
                }
                w2_total.push((opt, m.total_ns));
                w2_modes.push((opt.flag(), m.json));
            }
            Err(e) => {
                failures.push(format!("W2 --opt {}: {e}", opt.flag()));
                w2_modes.push((opt.flag(), Json::obj([("error", Json::s(e))])));
            }
        }
    }
    if !native_agree {
        failures.push("W1 native disagrees with L4".into());
    }
    if !cpu_agree2 {
        failures.push(format!(
            "W2 final states: L3 {l3s}, L4 {l4s}, native {nts}, expected {want2}"
        ));
    }

    // Speed ratios (> 1 means the first-named leg is faster).
    let ratios = |l1: &[(Opt, f64)], l4: f64, native: f64| -> Json {
        let mut pairs: Vec<(&str, Json)> = vec![("l4_speedup_over_native", Json::F(native / l4))];
        for (opt, v) in l1 {
            let (k4, kn) = match opt {
                Opt::Off => (
                    "l4_speedup_over_l1_opt_off",
                    "native_speedup_over_l1_opt_off",
                ),
                Opt::On => ("l4_speedup_over_l1_opt_on", "native_speedup_over_l1_opt_on"),
            };
            pairs.push((k4, Json::F(v / l4)));
            pairs.push((kn, Json::F(v / native)));
        }
        let both = native / l4 >= 2.0 && l1.len() == 2 && l1.iter().all(|(_, v)| v / l4 >= 2.0);
        pairs.push((
            "l4_at_least_2x_faster_than_both_baselines",
            Json::Bool(both),
        ));
        obj(pairs)
    };
    let w1 = Json::obj([
        ("workload", Json::s(format!("W1 = I1 `{I1_ROOT}` over (I64 I64); design vector (5, 4) → 24 plus {} splitmix64/0 vectors", a.w1_vectors.saturating_sub(1)))),
        ("graph_source", Json::s("extracted from Core source (D7) in this run")),
        ("graph", graph_json(&e1, &workloads::w1_graph())),
        ("extraction", w1_ext),
        ("native_agrees_with_l4_on_all_vectors", Json::Bool(native_agree)),
        ("warm_l3_reference", l3j),
        ("warm_l4_compiled", l4j),
        ("warm_native_rust", nj),
        ("l1_ordinary_wasm_v8", obj(w1_modes)),
        ("speed_warm_per_evaluation", ratios(&w1_root, l4m, nm)),
        ("l3_ns_per_eval", Json::F(l3m)),
        ("training_step", Json::s("not applicable (E-DERIV-I64)")),
    ]);
    let w2 = Json::obj([
        ("workload", Json::s(format!("W2 = I4 `{I4_ROOT}` transition (s,i,k) → (s·k+i, s·k+i), {} host steps from s={W2_S0}, i={W2_I}, k={W2_K}", a.w2_steps))),
        ("graph_source", Json::s("extracted from Core source (D7) in this run")),
        ("graph", graph_json(&e2, &workloads::w2_graph())),
        ("extraction", w2_ext),
        ("final_state_agreement_cpu_legs", Json::s(format!("L3 {l3s}, L4 {l4s}, native {nts}, expected {want2}: equal = {cpu_agree2}"))),
        ("total_steps_l3_ns", l3t),
        ("total_steps_l4_ns", l4t),
        ("total_steps_native_ns", nt),
        ("l1_ordinary_wasm_v8", obj(w2_modes)),
        ("l1_modes_concurrent", Json::Bool(a.w2_concurrent)),
        ("l1_wall_ns_both_modes", Json::F(ns(w2_wall))),
        ("speed_total_steps", ratios(&w2_total, l4tm, ntm)),
        ("training_step", Json::s("not applicable (E-DERIV-I64)")),
    ]);
    let run = Json::obj([
        ("source_revision", Json::s(a.source_rev.clone())),
        ("toolchain", Json::s(a.toolchain.clone())),
        ("profiles", Json::s(format!("{IR_VERSION}; ngraph-i64/0 (extraction); {SEMANTIC_REVISION}; {COMPILER_REVISION}"))),
        ("prng", Json::s(PRNG)),
        ("seed", Json::s(format!("{:#x}", a.seed))),
        ("target", Json::s(format!("{}-{}", std::env::consts::ARCH, std::env::consts::OS))),
        ("optimization_mode", Json::s(if cfg!(debug_assertions) { "debug" } else { "release (overflow-checks=true, panic=unwind per workspace profile)" })),
        ("cpu", Json::s(cpu_model())),
        ("pinned_cpu", Json::s("unpinned: L1 runs multi-threaded Node/V8 (and wasm-opt) child processes")),
        ("governor", Json::s(read("/sys/devices/system/cpu/cpu0/cpufreq/scaling_governor"))),
        ("cpufreq_driver", Json::s(read("/sys/devices/system/cpu/cpu0/cpufreq/scaling_driver"))),
        ("load_average_at_start", Json::s(load_start)),
        ("load_average_at_end", Json::s(read("/proc/loadavg"))),
        ("host_shared", Json::Bool(true)),
        ("warm_samples", Json::U(a.samples as u64)),
        ("cold_reps", Json::U(a.cold_reps as u64)),
        (
            "ordinary_path",
            Json::obj([
                ("cli", Json::s(a.cli.display().to_string())),
                ("cli_sha256", Json::s(sha256(&a.cli))),
                ("modes", Json::s("noble session --framed --opt off; noble session --framed --opt on")),
                ("session_chunking", Json::s(format!("≤ {} submissions per session process (l1::MAX_SUBMISSIONS): 120 vectors/steps per session", l1::MAX_SUBMISSIONS))),
            ]),
        ),
        ("wasm_features", Json::s("as fixed by the pinned CLI runtime (managed-linear-memory backend; --opt on runs wasm-opt --enable-gc --enable-reference-types --enable-bulk-memory -O2 --strip-debug)")),
        ("resource_limits", Json::s("extraction /0 defaults; checker /0 defaults; CLI Core session limits")),
        ("timer", Json::s("std::time::Instant (CLOCK_MONOTONIC); hardware counters: unsupported")),
        ("wall_seconds", Json::F(started.elapsed().as_secs_f64())),
    ]);
    let doc = Json::obj([
        ("schema", Json::s("noble-ngraph-phase-c-l1-evidence/v1")),
        ("issue", Json::s("OnixResearch/noble-lang#14")),
        ("scope", Json::s("Phase C C1 for W1 (I1) and W2 (I4): extraction time and the ordinary Noble Wasm/V8 baseline (L1), next to L3/L4 on the extracted graph and the host-native baseline")),
        ("result", Json::s(if failures.is_empty() { "passed" } else { "failed" })),
        ("failures", Json::A(failures.iter().cloned().map(Json::s).collect())),
        ("run", run),
        ("w1", w1),
        ("w2", w2),
        (
            "interpretation",
            Json::s("The ordinary path's host interface (`noble session`) prepares, compiles, instantiates and runs every submission, so an L1 evaluation is one root submission and a W2 host step is an input submission plus a root submission. These are the costs a host pays through that interface; they are not the cost of calling an already instantiated Wasm function, which the CLI does not expose."),
        ),
        (
            "trusted_boundaries",
            Json::A(vec![
                Json::s("the pinned noble release CLI, its Node/V8 runtime and wasm-opt (--opt on) as the ordinary path (PO-17/18 open)"),
                Json::s("noble_kernel::acceptance::check as the independent subject checker; the extractor (NG-PO-A open); this crate's checker, L3 and L4 (NG-PO-B/C open)"),
                Json::s("rustc/LLVM of the pinned nightly toolchain; the hand-written native baselines"),
                Json::s("std::time::Instant, report-line arrival timing and a shared, unisolated host"),
                Json::s("sha256sum and blake3 identities: a hash identifies bytes, it proves nothing"),
            ]),
        ),
        (
            "non_claims",
            Json::A(vec![
                Json::s("Finite agreement proves neither extraction (NG-PO-A) nor the evaluators (NG-PO-C); NG-PO-A..H stay open."),
                Json::s("Timings are from a shared host and are informational at the stated precision."),
            ]),
        ),
    ]);
    if let Err(e) = std::fs::write(&a.out, doc.pretty()) {
        eprintln!("write {}: {e}", a.out.display());
        return ExitCode::from(2);
    }
    if failures.is_empty() {
        ExitCode::SUCCESS
    } else {
        for f in &failures {
            eprintln!("{f}");
        }
        ExitCode::FAILURE
    }
}
