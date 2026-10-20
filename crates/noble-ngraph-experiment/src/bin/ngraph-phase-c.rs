#![feature(register_tool)]
#![register_tool(tigerstyle)]
#![allow(
    tigerstyle::ambiguous_params,
    tigerstyle::assertion_density,
    tigerstyle::missing_const_fn,
    tigerstyle::raw_arithmetic_overflow,
    tigerstyle::ambient_clock,
    tigerstyle::ambient_env,
    tigerstyle::bool_naming,
    tigerstyle::excessive_file_length,
    tigerstyle::explicit_defaults,
    tigerstyle::function_length,
    tigerstyle::no_panic,
    tigerstyle::no_unwrap,
    tigerstyle::non_trait_imports,
    tigerstyle::unbounded_collection_growth,
    tigerstyle::unchecked_division,
    reason = "Owner: noble-ngraph-maintainers; this Issue #14 evidence receipt binary measures wall-clock and environment effects and writes its JSON receipt directly, so harness terse names, unwraps and long campaign legs are the intended form."
)]
//! Issue #14 Phase C measurement harness (C1–C3, core legs). Release only;
//! not part of the default test suite.
//!
//! ```text
//! cargo build --release -p noble-ngraph-experiment --bin ngraph-phase-c
//! taskset -c 2 target/release/ngraph-phase-c --out verification/ngraph/phase-c-core.json \
//!   --source-rev "$(git rev-parse HEAD)" --toolchain "$(rustc -V)" --pinned-cpu 2
//! ```
//!
//! Every timed evaluator's observations are first compared with L3 (native
//! baselines bitwise under `≡obs`). Warm latency is per evaluation: each of
//! N ≥ 1,000 samples times a calibrated batch of evaluations (≥ 20 µs) and
//! divides by the batch size.

use noble_ngraph_experiment::scheme::BLAKE3;
use std::hint::black_box;
use std::time::{Duration, Instant};

use noble_ngraph::check::retained_bytes;
use noble_ngraph::compiled::{Compiled, COMPILER_REVISION};
use noble_ngraph::eval::{f32_inputs, i64_inputs, reference, reference_scratch_bytes, Control};
use noble_ngraph::ir::{hex, FACT_RULE_REVISION, IR_VERSION, SEMANTIC_REVISION};
use noble_ngraph::prng::{SplitMix64, PRNG};
use noble_ngraph::snapshot::ParamSnapshot;
use noble_ngraph::train::{self, init_params, Adam, Point, TrainControl};
use noble_ngraph::{check, CheckedGraph, Graph, Obs, Value};
use noble_ngraph_experiment::bench::{cpu_model, ns, read, stats, warm};
use noble_ngraph_experiment::json::Json;
use noble_ngraph_experiment::oracle::APFLOAT_ORACLE;
use noble_ngraph_experiment::random::random_f32;
use noble_ngraph_experiment::workloads::{self, NativeTrainer, ReGluNet, W3_SIZES, W4_TABLE};

struct Args {
    out: Option<String>,
    source_rev: String,
    toolchain: String,
    pinned_cpu: String,
    samples: usize,
    cold_reps: usize,
    phase_profile: bool,
}

fn args() -> Args {
    let mut a = Args {
        out: None,
        source_rev: "unrecorded".into(),
        toolchain: "unrecorded".into(),
        pinned_cpu: "unpinned".into(),
        samples: 2_000,
        cold_reps: 21,
        phase_profile: false,
    };
    let v: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i + 1 < v.len() {
        let val = v[i + 1].clone();
        match v[i].as_str() {
            "--out" => a.out = Some(val),
            "--source-rev" => a.source_rev = val,
            "--toolchain" => a.toolchain = val,
            "--pinned-cpu" => a.pinned_cpu = val,
            "--samples" => a.samples = val.parse().unwrap_or(a.samples),
            "--cold-reps" => a.cold_reps = val.parse().unwrap_or(a.cold_reps),
            "--phase-profile" => a.phase_profile = val.parse().expect("true or false"),
            other => panic!("unknown argument {other}"),
        }
        i += 2;
    }
    a
}

/// Repeated-evaluation throughput over ~0.3 s.
fn throughput(mut f: impl FnMut()) -> f64 {
    let t = Instant::now();
    let mut n = 0u64;
    while t.elapsed() < Duration::from_millis(300) {
        for _ in 0..64 {
            f();
        }
        n += 64;
    }
    // Issue #14 F32 evidence: diagnostics deliberately widen exact f32 values.
    #[allow(
        clippy::cast_precision_loss,
        reason = "Issue #14 F32 evidence: diagnostics deliberately widen exact f32 values."
    )]
    let r = n as f64 / t.elapsed().as_secs_f64();
    r
}

/// Cold path: admission (check), build (compile) and first evaluation,
/// repeated `reps` times from a cloned graph; medians in ns.
fn cold(reps: usize, g: &Graph, first: impl Fn(&CheckedGraph, &Compiled) -> Obs) -> (Json, f64) {
    let (mut adm, mut build, mut ev, mut tot) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for _ in 0..reps {
        let gg = g.clone();
        let t0 = Instant::now();
        let c = check(&BLAKE3, gg).expect("admitted");
        let t1 = Instant::now();
        let comp = Compiled::new(&BLAKE3, &c);
        let t2 = Instant::now();
        black_box(first(&c, &comp));
        let t3 = Instant::now();
        adm.push(ns(t1 - t0));
        build.push(ns(t2 - t1));
        ev.push(ns(t3 - t2));
        tot.push(ns(t3 - t0));
    }
    let (aj, am) = stats(adm);
    (
        Json::obj([
            ("admission_check", aj),
            ("build_compile", stats(build).0),
            ("first_evaluation_l4", stats(ev).0),
            ("cold_total", stats(tot).0),
        ]),
        am,
    )
}

fn bytes_json(c: &CheckedGraph, comp: &Compiled, params: usize) -> Json {
    Json::obj([
        ("param_bytes", Json::U(params as u64 * 4)),
        ("activation_bytes_l3", Json::U(reference_scratch_bytes(c))),
        ("activation_bytes_l4_register_file", Json::U(comp.scratch_bytes())),
        ("packed_layout_key", Json::s(hex(&comp.layout_key()))),
        ("retained_graph_bytes_estimate", Json::U(retained_bytes(c.graph()))),
        ("retained_compiled_code_bytes_estimate", Json::U(comp.retained_bytes())),
        ("measurement", Json::s("estimates from element counts × element sizes; allocator overhead and RSS: unsupported")),
    ])
}

fn shape_json(c: &CheckedGraph) -> Json {
    Json::obj([
        ("graph_logical_id", Json::s(hex(&c.id()))),
        ("live_nodes", Json::U(c.node_count() as u64)),
        ("depth_layers", Json::U(u64::from(c.layer_count()))),
    ])
}

fn speed(native: f64, l4: f64, l3: f64) -> Json {
    Json::obj([
        ("l4_speedup_over_native", Json::F(native / l4)),
        ("l4_speedup_over_l3_reference", Json::F(l3 / l4)),
        ("native_ns_per_eval", Json::F(native)),
        ("l4_ns_per_eval", Json::F(l4)),
        ("l3_ns_per_eval", Json::F(l3)),
    ])
}

struct W3Out {
    json: Json,
    l4_over_native: f64,
    admission_ms: f64,
}

/// Inclusive wall-clock function timings, not sampling/hardware/allocation
/// profiling. Reuses the exact timed workload, snapshot, input and data.
fn phase_profile(a: &Args, c: &train::Contract, s: &ParamSnapshot, xi: &[Value]) -> Json {
    if !a.phase_profile {
        return Json::s("not requested (--phase-profile true)");
    }
    let model = &c.model;
    let slots: Vec<f32> = noble_ngraph::eval::preflight(model, xi, Some(s))
        .expect("slots")
        .iter()
        .map(|v| match v {
            Value::F32(b) => f32::from_bits(*b),
            _ => unreachable!("W3 is F32"),
        })
        .collect();
    let vals = noble_ngraph::grad::forward(model, &slots);
    let seeds = vec![1.0; model.graph().outputs.len()];
    let measure = |f: &mut dyn FnMut()| warm(a.samples, f).0;
    Json::obj([
        ("kind", Json::s("inclusive function wall-clock; nested phases overlap; no allocation or hardware counts")),
        ("data_digest", Json::s(hex(&train::data_digest(&BLAKE3, &c.data)))),
        ("snapshot", Json::s(hex(&s.id()))),
        ("snapshot_check_use", measure(&mut || { black_box(s.check_use(black_box(model))).expect("bound"); })),
        ("eval_preflight", measure(&mut || { black_box(noble_ngraph::eval::preflight(model, black_box(xi), Some(s))).expect("slots"); })),
        ("grad_forward", measure(&mut || { black_box(noble_ngraph::grad::forward(model, black_box(&slots))); })),
        ("first_failure", measure(&mut || { black_box(noble_ngraph::eval::first_failure(model, black_box(&vals))); })),
        ("grad_backward", measure(&mut || { black_box(noble_ngraph::grad::backward(model, black_box(&vals), &seeds, &c.deriv)); })),
        ("objective_checked", measure(&mut || { black_box(noble_ngraph::grad::objective_checked(model, &c.loss, black_box(&slots), &c.data[0].targets, &c.deriv)).expect("objective"); })),
        ("train_admit", measure(&mut || { train::admit(model, &c.loss, &c.params, &c.deriv).expect("admitted"); })),
        ("train_one_step", measure(&mut || { black_box(train::train(&BLAKE3, c, None, 1, &mut TrainControl::default())).expect("trains"); })),
    ])
}

// Issue #14 campaign leg; its linear measurement order is the review subject.
#[allow(
    clippy::too_many_lines,
    reason = "Issue #14 campaign leg; its linear measurement order is the review subject."
)]
fn w3(a: &Args, name: &str, net: ReGluNet) -> W3Out {
    let t = Instant::now();
    let g = net.graph(name);
    let authoring_ns = ns(t.elapsed());
    let c = check(&BLAKE3, g.clone()).expect("admitted");
    let comp = Compiled::new(&BLAKE3, &c);
    let p = init_params(0x00c3_0003, net.params());
    let s = ParamSnapshot::new(&BLAKE3, &c, &p).expect("snapshot");
    let mut rng = SplitMix64::new(0x00c1_0003);
    // Issue #14 evidence: the value is range-checked before this bounded diagnostic cast.
    #[allow(
        clippy::cast_possible_truncation,
        reason = "Issue #14 evidence: the value is range-checked before this bounded diagnostic cast."
    )]
    let x: Vec<f32> = (0..net.inputs())
        .map(|_| (rng.next_f64() * 2.0 - 1.0) as f32)
        .collect();
    let xi = f32_inputs(&x);
    // Agreement before timing (plus random special inputs).
    let mut agree = true;
    for k in 0..200 {
        let xs: Vec<f32> = if k == 0 {
            x.clone()
        } else {
            (0..net.inputs()).map(|_| random_f32(&mut rng)).collect()
        };
        let mut out = vec![0f32; net.batch];
        net.native(&p, &xs, &mut out);
        let native = Obs::Normal(out.iter().map(|v| Value::f32(*v)).collect());
        let l3 = reference(&c, &f32_inputs(&xs), Some(&s), &mut Control::default()).obs;
        let l4 = comp
            .evaluate(&f32_inputs(&xs), Some(&s), &mut Control::default())
            .obs;
        agree &= l3.obs_eq(&native) && l4.obs_eq(&native);
    }
    let (cold, cold_adm_ns) = cold(a.cold_reps, &g, |c, comp| {
        let s = ParamSnapshot::new(&BLAKE3, c, &p).expect("snapshot");
        comp.evaluate(&xi, Some(&s), &mut Control::default()).obs
    });
    let mut ws = comp.workspace();
    let mut ctl = Control::default();
    let (l3j, l3m, _) = warm(a.samples, || {
        black_box(reference(&c, black_box(&xi), Some(&s), &mut ctl));
    });
    let (l4j, l4m, _) = warm(a.samples, || {
        black_box(comp.evaluate_in(&mut ws, black_box(&xi), Some(&s), &mut ctl));
    });
    let mut out = vec![0f32; net.batch];
    let (nj, nm, _) = warm(a.samples, || {
        net.native(black_box(&p), black_box(&x), &mut out);
        black_box(&out);
    });
    let tp_l3 = throughput(|| {
        black_box(reference(&c, black_box(&xi), Some(&s), &mut ctl));
    });
    let tp_l4 = throughput(|| {
        black_box(comp.evaluate_in(&mut ws, black_box(&xi), Some(&s), &mut ctl));
    });
    let tp_n = throughput(|| {
        net.native(black_box(&p), black_box(&x), &mut out);
        black_box(&out);
    });
    // Training step cost: 15 points per step.
    let loss = check(&BLAKE3, workloads::sse_loss(net.batch)).expect("loss");
    let steps = 15usize.checked_div(net.batch).unwrap_or_default();
    let points: Vec<(Vec<f32>, Vec<f32>)> = (0..steps)
        .map(|_| {
            // Issue #14 evidence: the value is range-checked before this bounded diagnostic cast.
            #[allow(clippy::cast_possible_truncation, reason = "Issue #14 evidence: the value is range-checked before this bounded diagnostic cast.")]
            let xs: Vec<f32> = (0..net.inputs())
                .map(|_| (rng.next_f64() * 2.0 - 1.0) as f32)
                .collect();
            // Issue #14 evidence: the value is range-checked before this bounded diagnostic cast.
            #[allow(clippy::cast_possible_truncation, reason = "Issue #14 evidence: the value is range-checked before this bounded diagnostic cast.")]
            let ts: Vec<f32> = (0..net.batch)
                .map(|_| (rng.next_f64() * 2.0 - 1.0) as f32)
                .collect();
            (xs, ts)
        })
        .collect();
    let contract = train::Contract {
        model: c.clone(),
        loss,
        params: c.graph().param_slots(),
        deriv: noble_ngraph::grad::DerivOptions::default(),
        adam: Adam::default(),
        seed: 0x00c3_0003,
        data: points
            .iter()
            .map(|(xs, ts)| Point {
                inputs: xs.clone(),
                targets: ts.clone(),
            })
            .collect(),
    };
    let steps = 100u64;
    let phases = phase_profile(a, &contract, &s, &xi);
    let mut ours = Vec::new();
    let mut loss_ratio = 0.0;
    let mut training_result = Json::Null;
    for _ in 0..5 {
        let t = Instant::now();
        let r = train::train(
            &BLAKE3,
            &contract,
            None,
            steps,
            &mut TrainControl::default(),
        )
        .expect("trains");
        // Issue #14 F32 evidence: diagnostics deliberately widen exact f32 values.
        #[allow(
            clippy::cast_precision_loss,
            reason = "Issue #14 F32 evidence: diagnostics deliberately widen exact f32 values."
        )]
        ours.push(ns(t.elapsed()) / steps as f64);
        loss_ratio = r.losses[r.losses.len() - 1] / r.losses[0];
        training_result = Json::obj([
            ("initial_snapshot", Json::s(hex(&r.initial.id()))),
            ("final_snapshot", Json::s(hex(&r.snapshot.id()))),
            ("optimizer_state", Json::s(hex(&r.state.digest(&BLAKE3)))),
            (
                "loss_f64_bits",
                Json::A(
                    r.losses
                        .iter()
                        .map(|v| Json::s(format!("0x{:016x}", v.to_bits())))
                        .collect(),
                ),
            ),
        ]);
    }
    let flat_x: Vec<f32> = points.iter().flat_map(|(xs, _)| xs.clone()).collect();
    let flat_t: Vec<f32> = points.iter().flat_map(|(_, ts)| ts.clone()).collect();
    let native_net = ReGluNet { batch: 15, ..net };
    let mut nt = NativeTrainer::new(native_net, init_params(0x00c3_0003, net.params()));
    let adam = Adam::default();
    let (ntj, ntm, _) = warm(1000, || {
        black_box(nt.step(black_box(&flat_x), black_box(&flat_t), &adam));
    });
    let (oursj, oursm) = stats(ours);
    let admission_ms = cold_adm_ns / 1e6;
    W3Out {
        l4_over_native: nm / l4m,
        admission_ms,
        json: Json::obj([
            ("workload", Json::s(name)),
            ("shape", Json::s(format!("{net:?}: {} ops, {} params, {} inputs", net.ops(), net.params(), net.inputs()))),
            ("ordinary_noble_path", Json::s("not applicable: ngraph-f32/0 graphs are authored IR (B0(b)); Noble's ordinary path has no F32")),
            ("extraction", Json::s("not applicable (authored IR, B0(b))")),
            ("authoring_build_graph_ns", Json::F(authoring_ns)),
            ("graph", shape_json(&c)),
            ("bytes", bytes_json(&c, &comp, net.params())),
            ("agreement_before_timing", Json::s(format!("200 input vectors: native, L3, L4 agree under ≡obs = {agree}"))),
            ("cold", cold),
            ("function_wall_profile", phases),
            ("reproducibility", Json::obj([
                ("param_seed", Json::s("0x00c30003")),
                ("input_seed", Json::s("0x00c10003")),
                ("input_f32_bits", Json::A(x.iter().map(|v| Json::s(format!("0x{:08x}", v.to_bits()))).collect())),
                ("param_f32_bits", Json::A(p.iter().map(|v| Json::s(format!("0x{:08x}", v.to_bits()))).collect())),
                ("data_digest", Json::s(hex(&train::data_digest(&BLAKE3, &contract.data)))),
                ("steps", Json::U(steps)),
                ("training_result", training_result),
            ])),
            ("warm_l3_reference", l3j),
            ("warm_l4_compiled", l4j),
            ("warm_native_rust", nj),
            ("throughput_evals_per_s", Json::obj([("l3", Json::F(tp_l3)), ("l4", Json::F(tp_l4)), ("native", Json::F(tp_n))])),
            ("speed", speed(nm, l4m, l3m)),
            (
                "training_step_15_points",
                Json::obj([
                    ("ours", Json::s("reference FP32 backward (grad::objective<f32>) + adam/0 + immutable snapshot publication, amortized over 100 steps, 5 runs")),
                    ("ours_ns_per_step", oursj),
                    ("ours_loss_ratio_after_100_steps", Json::F(loss_ratio)),
                    ("native", Json::s("hand-written FP32 forward/backward + binary64 Adam, in-place update, no snapshot (handwired style)")),
                    ("native_ns_per_step", ntj),
                    ("native_over_ours_speed_ratio", Json::F(oursm / ntm)),
                    ("backward_scratch_bytes_estimate", Json::U(((c.node_count() + contract.loss.node_count()) * 4 * 2) as u64)),
                ]),
            ),
        ]),
    }
}

// Issue #14 campaign leg; its linear measurement order is the review subject.
#[allow(
    clippy::too_many_lines,
    reason = "Issue #14 campaign leg; its linear measurement order is the review subject."
)]
fn w4(a: &Args) -> (Json, f64) {
    let t = Instant::now();
    let g = workloads::w4_graph();
    let authoring_ns = ns(t.elapsed());
    let c = check(&BLAKE3, g.clone()).expect("admitted");
    let comp = Compiled::new(&BLAKE3, &c);
    let table: Vec<f32> = (0..W4_TABLE).map(workloads::w4_entry).collect();
    let mut rng = SplitMix64::new(0x00c1_0004);
    // Issue #14 F32 evidence: diagnostics deliberately widen exact f32 values.
    #[allow(
        clippy::cast_precision_loss,
        reason = "Issue #14 F32 evidence: diagnostics deliberately widen exact f32 values."
    )]
    let idx: Vec<f32> = (0..workloads::W4_READS)
        .map(|_| rng.below(W4_TABLE as u64) as f32)
        .collect();
    let ii = f32_inputs(&idx);
    let mut agree = true;
    for _ in 0..200 {
        // Issue #14 F32 evidence: diagnostics deliberately widen exact f32 values.
        #[allow(
            clippy::cast_precision_loss,
            reason = "Issue #14 F32 evidence: diagnostics deliberately widen exact f32 values."
        )]
        let ix: Vec<f32> = (0..workloads::W4_READS)
            .map(|_| rng.below(W4_TABLE as u64 + 2) as f32)
            .collect();
        let l3 = reference(&c, &f32_inputs(&ix), None, &mut Control::default()).obs;
        let l4 = comp
            .evaluate(&f32_inputs(&ix), None, &mut Control::default())
            .obs;
        let n = workloads::w4_native(&table, &ix);
        let nat_ok = match (&l3, n) {
            (Obs::Normal(v), Some(x)) => v[..] == [Value::f32(x)],
            (Obs::Refused(_), None) => true,
            _ => false,
        };
        agree &= nat_ok && l3.obs_eq(&l4);
    }
    let (cold, _) = cold(a.cold_reps, &g, |_, comp| {
        comp.evaluate(&ii, None, &mut Control::default()).obs
    });
    let mut ws = comp.workspace();
    let mut ctl = Control::default();
    let (l3j, l3m, _) = warm(a.samples, || {
        black_box(reference(&c, black_box(&ii), None, &mut ctl));
    });
    let (l4j, l4m, _) = warm(a.samples, || {
        black_box(comp.evaluate_in(&mut ws, black_box(&ii), None, &mut ctl));
    });
    let (nj, nm, _) = warm(a.samples, || {
        black_box(workloads::w4_native(black_box(&table), black_box(&idx)));
    });
    let tp = |f: &mut dyn FnMut()| throughput(f);
    let tp_l3 = tp(&mut || {
        black_box(reference(&c, black_box(&ii), None, &mut ctl));
    });
    let tp_l4 = tp(&mut || {
        black_box(comp.evaluate_in(&mut ws, black_box(&ii), None, &mut ctl));
    });
    let tp_n = tp(&mut || {
        black_box(workloads::w4_native(black_box(&table), black_box(&idx)));
    });
    (
        Json::obj([
            ("workload", Json::s("W4")),
            ("shape", Json::s(format!("{} reads of one {}-entry constant table (refuse/refuse), summed by affine(-0)", workloads::W4_READS, W4_TABLE))),
            ("ordinary_noble_path", Json::s("not applicable: F32 authored IR (B0(b))")),
            ("extraction", Json::s("not applicable (authored IR, B0(b))")),
            ("authoring_build_graph_ns", Json::F(authoring_ns)),
            ("graph", shape_json(&c)),
            ("bytes", bytes_json(&c, &comp, 0)),
            ("agreement_before_timing", Json::s(format!("200 index vectors incl. out-of-range (refusal): native, L3, L4 agree = {agree}"))),
            ("cold", cold),
            ("warm_l3_reference", l3j),
            ("warm_l4_compiled", l4j),
            ("warm_native_rust", nj),
            ("throughput_evals_per_s", Json::obj([("l3", Json::F(tp_l3)), ("l4", Json::F(tp_l4)), ("native", Json::F(tp_n))])),
            ("speed", speed(nm, l4m, l3m)),
            ("training_step", Json::s("not applicable (no params)")),
        ]),
        nm / l4m,
    )
}

// Issue #14 campaign leg; its linear measurement order is the review subject.
#[allow(
    clippy::too_many_lines,
    reason = "Issue #14 campaign leg; its linear measurement order is the review subject."
)]
fn w12(a: &Args) -> Json {
    let g1 = workloads::w1_graph();
    let c1 = check(&BLAKE3, g1.clone()).expect("W1");
    let comp1 = Compiled::new(&BLAKE3, &c1);
    let x1 = i64_inputs(&[5, 4]);
    let mut ctl = Control::default();
    let mut ws1 = comp1.workspace();
    let (l3j, l3m, _) = warm(a.samples, || {
        black_box(reference(&c1, black_box(&x1), None, &mut ctl));
    });
    let (l4j, l4m, _) = warm(a.samples, || {
        black_box(comp1.evaluate_in(&mut ws1, black_box(&x1), None, &mut ctl));
    });
    let (nj, nm, _) = warm(a.samples, || {
        black_box(workloads::w1_native(black_box(5), black_box(4)));
    });
    let (cold1, _) = cold(a.cold_reps, &g1, |_, comp| {
        comp.evaluate(&x1, None, &mut Control::default()).obs
    });
    let w1 = Json::obj([
        ("workload", Json::s("W1 = I1 (x=5, y=4 → 24)")),
        ("graph_source", Json::s("hand-built ngraph-i64/0 IR equal to the I1 lowering; extraction time and the ordinary Wasm/V8 leg come from the extraction leg's L1 harness")),
        ("graph", shape_json(&c1)),
        ("bytes", bytes_json(&c1, &comp1, 0)),
        ("cold", cold1),
        ("warm_l3_reference", l3j),
        ("warm_l4_compiled", l4j),
        ("warm_native_rust", nj),
        ("speed", speed(nm, l4m, l3m)),
        ("training_step", Json::s("not applicable (E-DERIV-I64)")),
    ]);
    let g2 = workloads::w2_graph();
    let c2 = check(&BLAKE3, g2.clone()).expect("W2");
    let comp2 = Compiled::new(&BLAKE3, &c2);
    let (s0, i, k) = (11i64, 3i64, -7i64);
    let steps = 100_000u64;
    let mut ws2 = comp2.workspace();
    let mut run_l = |l4: bool, ws: &mut noble_ngraph::compiled::Workspace| {
        let mut s = s0;
        for _ in 0..steps {
            let r = if l4 {
                comp2.evaluate_in(ws, &i64_inputs(&[s, i, k]), None, &mut ctl)
            } else {
                reference(&c2, &i64_inputs(&[s, i, k]), None, &mut ctl)
            };
            if let Some([Value::I64(n), _]) = r.obs.outputs() {
                s = *n;
            }
        }
        s
    };
    let want = workloads::w2_native(s0, i, k, steps);
    let tot = |f: &mut dyn FnMut() -> i64| {
        let mut v = Vec::new();
        let mut last = 0;
        for _ in 0..7 {
            let t = Instant::now();
            last = f();
            v.push(ns(t.elapsed()));
        }
        (stats(v).0, last)
    };
    let (l3t, l3s) = tot(&mut || run_l(false, &mut ws2.clone()));
    let (l4t, l4s) = tot(&mut || run_l(true, &mut ws2));
    let (nt, ns_) = tot(&mut || {
        workloads::w2_native(black_box(s0), black_box(i), black_box(k), black_box(steps))
    });
    let w2 = Json::obj([
        ("workload", Json::s("W2 = I4 transition (s,i,k) → (s·k+i, s·k+i), 10^5 host steps from s=11, i=3, k=-7")),
        ("graph_source", Json::s("hand-built ngraph-i64/0 IR equal to the I4 lowering; extraction time and the ordinary Wasm/V8 leg come from the extraction leg's L1 harness")),
        ("graph", shape_json(&c2)),
        ("bytes", bytes_json(&c2, &comp2, 0)),
        ("final_state_agreement", Json::s(format!("L3 {l3s}, L4 {l4s}, native {ns_}, expected {want}: equal = {}", l3s == want && l4s == want && ns_ == want))),
        ("total_10^5_steps_l3_ns", l3t),
        ("total_10^5_steps_l4_ns", l4t),
        ("total_10^5_steps_native_ns", nt),
        ("training_step", Json::s("not applicable (E-DERIV-I64)")),
    ]);
    Json::A(vec![w1, w2])
}

// Issue #14 campaign leg; its linear measurement order is the review subject.
#[allow(
    clippy::too_many_lines,
    reason = "Issue #14 campaign leg; its linear measurement order is the review subject."
)]
fn main() {
    let a = args();
    let load_start = read("/proc/loadavg");
    let started = Instant::now();
    let w3s: Vec<W3Out> = W3_SIZES.iter().map(|(n, net)| w3(&a, n, *net)).collect();
    let (w4j, w4_ratio) = w4(&a);
    let w12j = w12(&a);
    let large = &w3s[2];
    let speedup = large.l4_over_native;
    let admission_ok = large.admission_ms <= 100.0;
    let cpu_spec = speedup >= 2.0 && admission_ok;
    let recommendation = if cpu_spec {
        "cpu-specialization"
    } else {
        "stop"
    };
    let c3 = Json::obj([
        ("thresholds", Json::s("kept at approval: stop unless compiled CPU graph ≥2× faster than both baselines on warm W3; admission ≤100 ms at W3 scale; ≥2× margin over compiled CPU before any scoped #2 GPU experiment")),
        (
            "both_baselines_interpretation",
            Json::s("W3 is ngraph-f32/0 authored IR (B0(b)); Noble's ordinary Wasm/V8 path has no F32, so that baseline does not exist for W3. The test is applied to the one existing baseline (host-native Rust) and cannot pass on the missing one: the missing baseline never makes the outcome more favourable. The I64 workloads W1/W2, where the ordinary path exists, are reported separately and do not substitute for W3."),
        ),
        ("w3_large_l4_speedup_over_native", Json::F(speedup)),
        ("w3_large_admission_ms_median", Json::F(large.admission_ms)),
        ("admission_within_100ms", Json::Bool(admission_ok)),
        ("training_value_demonstrated", Json::s("B5 passed (loss ≤10% of initial on F1); W3 training cost measured")),
        ("w4_l4_speedup_over_native", Json::F(w4_ratio)),
        ("recommendation", Json::s(recommendation)),
        ("gpu_experiment", Json::s("blocked: no measured or explicitly modelled transfer and launch costs; and the compiled CPU graph is not the comparison point that would need a 2× GPU margin unless CPU specialization were recommended")),
    ]);
    let po = |id: &str, what: &str| {
        Json::obj([
            ("id", Json::s(id)),
            ("statement", Json::s(what)),
            ("status", Json::s("open")),
        ])
    };
    let c2 = Json::obj([
        (
            "reference_agreement",
            Json::s("Per workload, before timing: host-native, L3 and L4 agree under ≡obs on 200 input vectors (W3, W4) and on the W1 value and the W2 final state; see each workload's agreement field. Phase B: L2 (rustc_apfloat), B2-ORACLE, L3, L4 agree on all vectors (phase-b-core.json)."),
        ),
        (
            "finite_coverage",
            Json::s("Finite only: Phase B boundary vectors, 8 seeded random graphs x 20,000 vectors, 10^6 operand pairs per operation, 2,000 gradient samples, the 35 core rejection controls; Phase C agreement samples above. No exhaustive or symbolic coverage."),
        ),
        (
            "theorem_obligations",
            Json::A(vec![
                po("NG-PO-A", "extraction preserves Obs on the I64 overlap"),
                po("NG-PO-B", "graph-checker soundness"),
                po("NG-PO-C", "compiled evaluator refines the reference"),
                po("NG-PO-D", "reference model corresponds to IEEE binary32"),
                po("NG-PO-E", "fact-transfer soundness"),
                po("NG-PO-F", "ngraph-deriv-real/0 computes the real derivative away from corners"),
                po("NG-PO-G", "snapshot and provenance binding"),
                po("NG-PO-H", "executed compiled-graph bytes correspond to the admitted graph"),
            ]),
        ),
        (
            "trusted_boundaries",
            Json::A(vec![
                Json::s("rustc/LLVM release codegen of the pinned nightly (no contraction; default x86-64 features)"),
                Json::s("host CPU IEEE binary32/64 arithmetic, no FTZ/DAZ"),
                Json::s("rustc_apfloat =0.2.3+llvm-462a31f5a5ab as L2 oracle (Phase B)"),
                Json::s("blake3 =1.8.7 identities (bytes only)"),
                Json::s("this crate's checker, L3, L4, trainer and the hand-written native baselines"),
                Json::s("std::time::Instant and a shared, unisolated host for timings"),
            ]),
        ),
        (
            "unsupported_instrumentation",
            Json::s("hardware performance counters, RSS/allocator peak, cache misses, energy: unsupported (not zero); byte figures are element-count estimates"),
        ),
        (
            "not_run",
            Json::s("pinned handwired under pinned Bun (optional, informational): not run"),
        ),
    ]);
    let run = Json::obj([
        ("source_revision", Json::s(a.source_rev.clone())),
        ("executable_sha256", Json::s(std::env::current_exe().map_or_else(|_| "unsupported".into(), |p| noble_ngraph_experiment::bench::sha256(&p)))),
        ("toolchain", Json::s(a.toolchain.clone())),
        ("profiles", Json::s(format!("{IR_VERSION}; {SEMANTIC_REVISION}; {FACT_RULE_REVISION}; {COMPILER_REVISION}; ngraph-deriv-real/0; adam/0"))),
        ("oracle", Json::s(APFLOAT_ORACLE)),
        ("prng", Json::s(PRNG)),
        ("target", Json::s(format!("{}-{} (default x86-64 target features: no FMA instructions; rustc does not contract)", std::env::consts::ARCH, std::env::consts::OS))),
        ("optimization_mode", Json::s(if cfg!(debug_assertions) { "debug" } else { "release (overflow-checks=true, panic=unwind per workspace profile)" })),
        ("cpu", Json::s(cpu_model())),
        ("pinned_cpu", Json::s(a.pinned_cpu.clone())),
        ("governor", Json::s(read("/sys/devices/system/cpu/cpu0/cpufreq/scaling_governor"))),
        ("cpufreq_driver", Json::s(read("/sys/devices/system/cpu/cpu0/cpufreq/scaling_driver"))),
        ("load_average_at_start", Json::s(load_start)),
        ("load_average_at_end", Json::s(read("/proc/loadavg"))),
        ("host_shared", Json::Bool(true)),
        ("warm_samples", Json::U(a.samples as u64)),
        ("cold_reps", Json::U(a.cold_reps as u64)),
        ("timer", Json::s("std::time::Instant (CLOCK_MONOTONIC); hardware counters: unsupported")),
        ("wasm_features", Json::s("not applicable to these legs (no Wasm executed); the ordinary Wasm/V8 leg is the extraction leg's L1 harness")),
        ("wall_seconds", Json::F(started.elapsed().as_secs_f64())),
    ]);
    let doc = Json::obj([
        ("schema", Json::s("noble-ngraph-phase-c-core-evidence/v1")),
        ("issue", Json::s("OnixResearch/noble-lang#14")),
        ("run", run),
        ("w3", Json::A(w3s.into_iter().map(|w| w.json).collect())),
        ("w4", w4j),
        ("w1_w2_cpu_legs", w12j),
        ("c2", c2),
        ("c3", c3),
    ]);
    let text = doc.pretty();
    match &a.out {
        Some(p) => std::fs::write(p, text).unwrap_or_else(|e| panic!("write {p}: {e}")),
        None => print!("{text}"),
    }
}
