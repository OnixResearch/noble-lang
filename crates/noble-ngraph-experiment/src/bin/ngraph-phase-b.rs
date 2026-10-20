#![feature(register_tool)]
#![register_tool(tigerstyle)]
#![allow(
    tigerstyle::allocation_in_loop,
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
    tigerstyle::negated_predicate,
    tigerstyle::no_panic,
    tigerstyle::no_unwrap,
    tigerstyle::non_trait_imports,
    tigerstyle::too_many_parameters,
    tigerstyle::unbounded_collection_growth,
    reason = "Owner: noble-ngraph-maintainers; this Issue #14 evidence receipt binary measures wall-clock and environment effects and writes its JSON receipt directly, so harness terse names, unwraps and long campaign legs are the intended form."
)]
//! Issue #14 Phase B evidence harness (core legs). Runs the B1 (F1–F3),
//! B2 (F32 legs), B3 (core rejections), B4, B5 and B6 campaigns at full size
//! and writes one JSON receipt with EV-STATE-01 fields per record and the
//! BE-CONFIG-01 run fields.
//!
//! ```text
//! cargo run --release -p noble-ngraph-experiment --bin ngraph-phase-b -- \
//!   --out verification/ngraph/phase-b-core.json --source-rev <git sha> \
//!   --toolchain "$(rustc -V)"
//! ```

use noble_ngraph_experiment::scheme::BLAKE3;
use std::rc::Rc;
use std::time::Instant;

use noble_ngraph::compiled::{Compiled, COMPILER_REVISION};
use noble_ngraph::eval::{f32_inputs, reference, Control, ScratchCounters};
use noble_ngraph::ir::{hex, FACT_RULE_REVISION, IR_VERSION, SEMANTIC_REVISION};
use noble_ngraph::prng::{SplitMix64, PRNG};
use noble_ngraph::snapshot::{layout_key, ParamSnapshot};
use noble_ngraph::train::{self, data_digest, TrainControl};
use noble_ngraph::{check, BudgetKind, Budgets, CheckedGraph, Code, Obs, Value};
use noble_ngraph_experiment::differential::{campaign, run_legs};
use noble_ngraph_experiment::evidence::{capture, EvidenceRecord, NotApplicable};
use noble_ngraph_experiment::fixtures::{self, F1_FIXED, RULES};
use noble_ngraph_experiment::json::Json;
use noble_ngraph_experiment::oracle::{ap_add, ap_mul, ap_sub, f64_op, APFLOAT_ORACLE};
use noble_ngraph_experiment::random::random_f32;
use noble_ngraph_experiment::rejections;
use noble_ngraph_experiment::suites::{
    b5_contract, g1_tolerance, gradient_campaign, step_size, B5_DATA_SEED, B5_SEED, B5_STEPS,
};

struct Args {
    out: Option<String>,
    source_rev: String,
    toolchain: String,
    vectors: u64,
    graphs: u64,
    ops: usize,
    grad_samples: u64,
    pairs: u64,
}

fn args() -> Args {
    let mut a = Args {
        out: None,
        source_rev: "unrecorded".into(),
        toolchain: "unrecorded".into(),
        vectors: 20_000,
        graphs: 8,
        ops: 48,
        grad_samples: 2_000,
        pairs: 1_000_000,
    };
    let v: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i + 1 < v.len() {
        let val = v[i + 1].clone();
        match v[i].as_str() {
            "--out" => a.out = Some(val),
            "--source-rev" => a.source_rev = val,
            "--toolchain" => a.toolchain = val,
            "--vectors" => a.vectors = val.parse().unwrap_or(a.vectors),
            "--graphs" => a.graphs = val.parse().unwrap_or(a.graphs),
            "--ops" => a.ops = val.parse().unwrap_or(a.ops),
            "--grad-samples" => a.grad_samples = val.parse().unwrap_or(a.grad_samples),
            "--pairs" => a.pairs = val.parse().unwrap_or(a.pairs),
            other => panic!("unknown argument {other}"),
        }
        i += 2;
    }
    a
}

fn read(path: &str) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|_| "unsupported".into())
}

fn cpu_model() -> String {
    read("/proc/cpuinfo")
        .lines()
        .find(|l| l.starts_with("model name"))
        .and_then(|l| l.split(':').nth(1))
        .map_or_else(|| "unsupported".into(), |s| s.trim().to_owned())
}

fn obs_text(o: &Obs) -> String {
    match o {
        Obs::Normal(vs) => {
            let items: Vec<String> = vs
                .iter()
                .map(|v| match v {
                    Value::F32(b) => format!("f32:0x{b:08x}"),
                    Value::I64(x) => format!("i64:{x}"),
                    Value::Bool(x) => format!("bool:{x}"),
                })
                .collect();
            format!("Normal[{}]", items.join(","))
        }
        Obs::Refused(d) => format!("Refused({})", d.code_string()),
        Obs::GuardFailed { node, fact } => format!("GuardFailed(%{},{})", node.0, fact.kind_name()),
        Obs::Exhausted(k) => format!("Exhausted({})", k.name()),
        Obs::Cancelled => "Cancelled".into(),
    }
}

const TRUSTED: [&str; 5] = [
    "rustc/LLVM codegen of the pinned nightly toolchain (no contraction; IEEE binary32 SSE arithmetic on x86_64)",
    "host CPU IEEE 754 behaviour (no FTZ/DAZ set by this process)",
    "rustc_apfloat =0.2.3+llvm-462a31f5a5ab as the independent binary32 oracle (L2)",
    "blake3 =1.8.7 (pure) for identities: a hash identifies bytes, it proves nothing",
    "this crate's checker, evaluators and trainer (NG-PO-B/C/E/F/G open)",
];

// Issue #14 evidence record builder; the ordered parameters are one receipt row.
#[allow(clippy::too_many_arguments, reason = "Issue #14 evidence record builder; the ordered parameters are one receipt row.")]
fn record(
    id: &str,
    requirements: &[&str],
    claim: &str,
    passed: bool,
    obligations: &[&str],
    assumptions: &[&str],
    result: Json,
    cfg: &Json,
) -> Json {
    Json::obj([
        ("id", Json::s(id)),
        (
            "requirements",
            Json::A(requirements.iter().map(|r| Json::s(*r)).collect()),
        ),
        ("claim", Json::s(claim)),
        ("implementation", Json::s("implemented")),
        (
            "execution",
            Json::s(if passed { "passed" } else { "failed" }),
        ),
        ("proof", Json::s("open")),
        (
            "open_obligations",
            Json::A(obligations.iter().map(|r| Json::s(*r)).collect()),
        ),
        ("trust", Json::s("explicit")),
        (
            "trusted_boundaries",
            Json::A(TRUSTED.iter().map(|t| Json::s(*t)).collect()),
        ),
        (
            "evidence",
            Json::obj([
                ("kind", Json::s("test")),
                ("subject", Json::s(id)),
                ("revision", cfg_get(cfg, "source_revision")),
                ("configuration", Json::s("see run.config (BE-CONFIG-01)")),
                ("claim", Json::s(claim)),
                (
                    "assumptions",
                    Json::A(assumptions.iter().map(|r| Json::s(*r)).collect()),
                ),
                ("result", result),
            ]),
        ),
    ])
}

fn cfg_get(cfg: &Json, k: &str) -> Json {
    match cfg {
        Json::O(o) => o
            .iter()
            .find(|(kk, _)| kk == k)
            .map_or(Json::Null, |(_, v)| v.clone()),
        _ => Json::Null,
    }
}

fn checked(g: noble_ngraph::Graph) -> CheckedGraph {
    check(&BLAKE3, g).unwrap_or_else(|d| panic!("fixture refused: {d}"))
}

fn graph_summary(name: &str, g: &CheckedGraph) -> Json {
    let text = noble_ngraph::format::graph::print(g.graph());
    Json::obj([
        ("name", Json::s(name)),
        ("graph_logical_id", Json::s(hex(&g.id()))),
        ("nodes", Json::U(g.node_count() as u64)),
        ("layers", Json::U(u64::from(g.layer_count()))),
        (
            "ir_text_blake3",
            Json::s(hex(blake3::hash(text.as_bytes()).as_bytes())),
        ),
    ])
}

fn b1(cfg: &Json) -> (Json, bool) {
    let f1 = checked(fixtures::f1());
    let s1 = ParamSnapshot::new(&BLAKE3, &f1, &F1_FIXED).expect("snapshot");
    let c1 = Compiled::new(&BLAKE3, &f1);
    let f1_out = run_legs(&c1, &f32_inputs(&[0.5, -0.25]), Some(&s1));
    let f2 = checked(fixtures::f2());
    let m = [0.5f32, -1.25, 3.0, 0.75];
    let s2 = ParamSnapshot::new(&BLAKE3, &f2, &m).expect("snapshot");
    let c2 = Compiled::new(&BLAKE3, &f2);
    let mut state = [1.0f32, -2.0];
    let mut agree = f1_out.agree();
    for _ in 0..1000 {
        let legs = run_legs(&c2, &f32_inputs(&state), Some(&s2));
        agree &= legs.agree();
        if let Some([Value::F32(a), Value::F32(b)]) = legs.l3_reference.outputs() {
            state = [f32::from_bits(*a), f32::from_bits(*b)];
        }
    }
    let mut f3 = Vec::new();
    for below in RULES {
        for above in RULES {
            let g = checked(fixtures::f3(below, above));
            let c = Compiled::new(&BLAKE3, &g);
            let mut row = Vec::new();
            for i in [-1.0f32, 0.0, 1.0, 2.0, 3.0, 4.0] {
                let legs = run_legs(&c, &f32_inputs(&[i, 2.5, -7.0]), None);
                agree &= legs.agree();
                row.push(Json::s(format!("i={i}: {}", obs_text(&legs.l3_reference))));
            }
            f3.push(Json::obj([
                ("below", Json::s(below.name())),
                ("above", Json::s(above.name())),
                ("table", Json::s("[a, a, b, -0] with a=2.5, b=-7")),
                ("observations", Json::A(row)),
            ]));
        }
    }
    let result = Json::obj([
        ("authoring", Json::s("B0(b): F32 graphs authored directly in ngraph-ir/0; checked only by the graph checker")),
        (
            "graphs",
            Json::A(vec![
                graph_summary("F1 2-3-1 ReGLU (22 params)", &f1),
                graph_summary("F2 transition (4 params)", &f2),
                graph_summary("MSE point loss", &checked(fixtures::mse_loss())),
            ]),
        ),
        ("f1_fixed_params_on_(0.5,-0.25)", Json::s(obs_text(&f1_out.l3_reference))),
        ("f2_state_after_1000_host_steps", Json::s(format!("[0x{:08x}, 0x{:08x}]", state[0].to_bits(), state[1].to_bits()))),
        ("f3", Json::A(f3)),
        ("all_legs_agree", Json::Bool(agree)),
    ]);
    (
        record(
            "B1-core F1-F3",
            &["NGRAPH-EXTRACT-01", "NGRAPH-F32-01"],
            "F1 (2-3-1 ReGLU), F2 (transition, 1,000 host steps) and F3 (index over [a,a,b,-0], every rule pair) are admitted by the graph checker and evaluate identically on L2/B2-ORACLE/L3/L4",
            agree,
            &["NG-PO-B", "NG-PO-C", "NG-PO-D"],
            &["F32 graphs are authored IR (no source extraction under B0(b)); I1-I4 are the extraction leg's records"],
            result,
            cfg,
        ),
        agree,
    )
}

// Issue #14 campaign leg; its linear measurement order is the review subject.
#[allow(clippy::too_many_lines, reason = "Issue #14 campaign leg; its linear measurement order is the review subject.")]
fn b2(cfg: &Json, a: &Args) -> (Vec<Json>, bool) {
    let mut out = Vec::new();
    let mut all = true;
    // Boundary vectors.
    let mut rows = Vec::new();
    let mut ok = true;
    for v in fixtures::b2_vectors() {
        let c = checked(v.graph.clone());
        let legs = run_legs(&Compiled::new(&BLAKE3, &c), &f32_inputs(&v.inputs), None);
        let expected = Obs::Normal(v.expected.iter().map(|b| Value::F32(*b)).collect());
        let pass = legs.agree() && legs.l3_reference.obs_eq(&expected);
        ok &= pass;
        rows.push(Json::obj([
            ("name", Json::s(v.name)),
            ("expected", Json::s(obs_text(&expected))),
            ("l2_apfloat", Json::s(obs_text(&legs.l2_apfloat))),
            (
                "b2_oracle_f64_per_op",
                Json::s(obs_text(&legs.b2_oracle_f64)),
            ),
            ("l3_reference", Json::s(obs_text(&legs.l3_reference))),
            ("l4_compiled", Json::s(obs_text(&legs.l4_compiled))),
            ("passed", Json::Bool(pass)),
        ]));
    }
    // Index boundaries ±(2^24-1), -1, 0, n-1, n under every rule pair.
    let lim = fixtures::INDEX_LIMIT;
    let mut idx_rows = Vec::new();
    for below in RULES {
        for above in RULES {
            let g = checked(fixtures::f3(below, above));
            let c = Compiled::new(&BLAKE3, &g);
            for i in [-lim, -1.0, 0.0, 3.0, 4.0, lim] {
                let legs = run_legs(&c, &f32_inputs(&[i, 2.5, -7.0]), None);
                ok &= legs.agree();
                idx_rows.push(Json::s(format!(
                    "{}/{} i={i}: {}",
                    below.name(),
                    above.name(),
                    obs_text(&legs.l3_reference)
                )));
            }
        }
    }
    // Detection controls.
    let ftz = fixtures::ftz_mul(f32::MIN_POSITIVE, 0.5).to_bits();
    let a1 = 1.0 + f32::from_bits(0x3980_0000);
    let c1 = -(1.0 + f32::from_bits(0x3a00_0000));
    let fused = a1.mul_add(a1, c1).to_bits();
    let detect = ftz != 0x0040_0000 && fused != 0;
    ok &= detect;
    all &= ok;
    out.push(record(
        "B2-core boundary vectors",
        &["NGRAPH-F32-01"],
        "L2 (rustc_apfloat), B2-ORACLE (binary64 per op), L3 and L4 agree under ≡obs with the expected bits on the reassociation, FMA, signed-zero, nonfinite, subnormal, repeated-read and boundary-index vectors; simulated FTZ and fused evaluation are detected",
        ok,
        &["NG-PO-C", "NG-PO-D"],
        &["≡obs: bitwise binary32 with one NaN class", "FTZ and FMA are simulated (no unsafe MXCSR access); the vectors detect them"],
        Json::obj([
            ("vectors", Json::A(rows)),
            ("index_boundaries", Json::A(idx_rows)),
            ("simulated_ftz_result_for_2^-126*0.5", Json::s(format!("0x{ftz:08x} (conforming: 0x00400000)"))),
            ("fused_result_for_fma_vector", Json::s(format!("0x{fused:08x} (conforming unfused: 0x00000000)"))),
        ]),
        cfg,
    ));
    // Per-operation double rounding.
    let mut rng = SplitMix64::new(0xd0b1_e000_0014);
    let mut mism = [0u64; 3];
    let start = Instant::now();
    for _ in 0..a.pairs {
        let (x, y) = (
            random_f32(&mut rng).to_bits(),
            random_f32(&mut rng).to_bits(),
        );
        let same =
            |p: u32, q: u32| p == q || (f32::from_bits(p).is_nan() && f32::from_bits(q).is_nan());
        let host = [
            (f32::from_bits(x) + f32::from_bits(y)).to_bits(),
            (f32::from_bits(x) - f32::from_bits(y)).to_bits(),
            (f32::from_bits(x) * f32::from_bits(y)).to_bits(),
        ];
        let ap = [ap_add(x, y), ap_sub(x, y), ap_mul(x, y)];
        for k in 0..3 {
            let d = f64_op(k as u8, x, y);
            if !(same(ap[k], d) && same(ap[k], host[k])) {
                mism[k] += 1;
            }
        }
    }
    let ok2 = mism == [0, 0, 0];
    all &= ok2;
    out.push(record(
        "B2-ORACLE double rounding",
        &["NGRAPH-F32-01"],
        "For random binary32 operand pairs, rn32(x op64 y) equals the rustc_apfloat binary32 result and the host binary32 result for +, -, × (the D3 double-rounding inference, checked on finite samples)",
        ok2,
        &["NG-PO-D"],
        &["operands: 20% specials, 30% raw bit patterns, 50% moderate values (splitmix64/0, seed 0xd0b1e0000014)", "a finite sample is not a proof of the inference"],
        Json::obj([
            ("pairs_per_operation", Json::U(a.pairs)),
            ("mismatches_add_sub_mul", Json::A(mism.iter().map(|m| Json::U(*m)).collect())),
            ("seconds", Json::F(start.elapsed().as_secs_f64())),
        ]),
        cfg,
    ));
    // Seeded random graphs.
    let mut camps = Vec::new();
    let mut ok3 = true;
    let start = Instant::now();
    for seed in 1..=a.graphs {
        let c = campaign(seed, a.ops, a.vectors);
        ok3 &= c.checker_refusal.is_none() && c.disagreements == 0 && c.vectors >= a.vectors;
        camps.push(Json::obj([
            ("graph_seed", Json::U(c.seed)),
            ("input_seed", Json::U(c.input_seed)),
            ("ops_requested", Json::U(c.ops as u64)),
            ("nodes", Json::U(c.nodes as u64)),
            ("layers", Json::U(u64::from(c.layers))),
            ("graph_logical_id", Json::s(c.graph_id.clone())),
            ("vectors", Json::U(c.vectors)),
            ("normal", Json::U(c.normal)),
            ("guard_failed", Json::U(c.guard_failed)),
            ("refused_input_fact", Json::U(c.refused)),
            ("nan_outputs", Json::U(c.nan_outputs)),
            ("signed_zero_outputs", Json::U(c.zero_outputs)),
            ("subnormal_outputs", Json::U(c.subnormal_outputs)),
            ("infinite_outputs", Json::U(c.inf_outputs)),
            ("disagreements", Json::U(c.disagreements)),
            (
                "first_disagreement",
                c.first_disagreement.clone().map_or(Json::Null, Json::S),
            ),
            (
                "checker_refusal",
                c.checker_refusal.clone().map_or(Json::Null, Json::S),
            ),
        ]));
    }
    all &= ok3;
    out.push(record(
        "B2-core seeded random F32 graphs",
        &["NGRAPH-F32-01"],
        "Seeded random ngraph-f32/0 graphs (all whitelist nodes incl. guards, step, select, index under every rule) agree on L2/B2-ORACLE/L3/L4 under ≡obs for ≥10,000 input vectors each, including Normal, GuardFailed and E-FACT-INPUT observations",
        ok3,
        &["NG-PO-B", "NG-PO-C", "NG-PO-D"],
        &["generator and inputs: splitmix64/0, seeds recorded per graph", "finite coverage only"],
        Json::obj([("campaigns", Json::A(camps)), ("seconds", Json::F(start.elapsed().as_secs_f64()))]),
        cfg,
    ));
    (out, all)
}

fn b3(cfg: &Json) -> (Json, bool) {
    let all = rejections::run_all();
    let ok = all.iter().all(|r| {
        r.passed() && (r.observed == "G-FACT-FALSE" || (r.l3_evaluated == 0 && r.l4_evaluated == 0))
    });
    let rows = all
        .iter()
        .map(|r| {
            Json::obj([
                ("name", Json::s(r.name)),
                ("group", Json::s(r.group)),
                ("expected", Json::s(r.expected)),
                ("observed", Json::s(r.observed.clone())),
                ("stage", Json::s(r.stage)),
                ("l3_evaluated_nodes", Json::U(r.l3_evaluated)),
                ("l4_evaluated_nodes", Json::U(r.l4_evaluated)),
                ("legs_agree", Json::Bool(r.legs_agree)),
                ("passed", Json::Bool(r.passed())),
            ])
        })
        .collect();
    (
        record(
            "B3-core rejections on authored IR",
            &["NGRAPH-FACT-01", "NGRAPH-EXTRACT-01"],
            "Every core control (caller-asserted/underivable/evidence facts, violating inputs and intermediates, bad indices, malformed IR, budgets) yields its D6 code; static and preflight refusals evaluate 0 nodes; guard failures evaluate no dependent node; the ordinary Noble path is not involved",
            ok,
            &["NG-PO-B", "NG-PO-E"],
            &["source-level controls (effects, resources, dynamic run, recursion, reflect, control, Program output, the k=20 expansion chain) are the extraction leg's records", "evidence facts excluded by approval P5"],
            Json::obj([("cases", Json::A(rows))]),
            cfg,
        ),
        ok,
    )
}

fn b4(cfg: &Json, a: &Args) -> (Json, bool) {
    let start = Instant::now();
    let st = gradient_campaign(0x00b4_0001, a.grad_samples);
    let ok = st.g1_failures == 0 && st.g2_failures == 0 && st.used > 0;
    (
        record(
            "B4 gradients (F1 + MSE)",
            &["NGRAPH-DERIV-01"],
            "G1 binary64 reverse mode matches binary64 central differences within 1e-6·max(1,|∂|) with h = 2^-20·max(1,|p|); G2 FP32 reference backward matches G1 within 4·depth·2^-24 relative to the absolute-value magnitude of the computation; corner, boolean and index policies are tested separately (tests/b4_gradients.rs)",
            ok,
            &["NG-PO-F"],
            &[
                "samples farther than 64·4h from every ReLU/ReGLU corner, step threshold and index boundary",
                "G2 scale: the reverse pass over |values| and |local derivatives| (first-order running-error magnitude); a first attempt scaled by |forward values| alone failed 20/1440 slot checks from cancellation in d = y - t and was investigated, not loosened (README)",
                "policy cases (corner at +0/-0, E-DERIV-BOOLEAN, straight-through and clamped-linear surrogates, candidate accumulation a <- 2g, index reject/zero/backward-difference surrogate, E-DERIV-I64, E-PARAM-UNDECLARED) are cargo tests recorded in test-log.txt",
            ],
            Json::obj([
                ("seed", Json::U(st.seed)),
                ("samples_requested", Json::U(st.requested)),
                ("samples_used", Json::U(st.used)),
                ("skipped_near_corner", Json::U(st.skipped_near_corner)),
                ("composed_depth", Json::U(u64::from(st.depth))),
                ("h_rule", Json::s(format!("2^-20·max(1,|p|) (h(1)={})", step_size(1.0)))),
                ("g1_tolerance_rule", Json::s(format!("1e-6·max(1,|∂|) (tol(0)={})", g1_tolerance(0.0)))),
                ("g1_checks", Json::U(st.g1_checks)),
                ("g1_failures", Json::U(st.g1_failures)),
                ("g1_worst_error_over_tolerance", Json::F(st.g1_worst_ratio)),
                ("g2_checks", Json::U(st.g2_checks)),
                ("g2_failures", Json::U(st.g2_failures)),
                ("g2_worst_error_over_bound", Json::F(st.g2_worst_ratio)),
                ("g2_checks_also_within_plain_|g1|_relative_bound", Json::U(st.g2_plain_relative_ok)),
                ("first_failure", st.first_failure.map_or(Json::Null, Json::S)),
                ("seconds", Json::F(start.elapsed().as_secs_f64())),
            ]),
            cfg,
        ),
        ok,
    )
}

// Issue #14 campaign leg; its linear measurement order is the review subject.
#[allow(clippy::too_many_lines, reason = "Issue #14 campaign leg; its linear measurement order is the review subject.")]
fn b5(cfg: &Json) -> (Json, bool) {
    let c = b5_contract(B5_SEED, B5_DATA_SEED);
    let id_before = c.model.id();
    let text_before = noble_ngraph::format::graph::print(c.model.graph());
    let start = Instant::now();
    let t =
        train::train(&BLAKE3, &c, None, B5_STEPS, &mut TrainControl::default()).expect("training");
    let secs = start.elapsed().as_secs_f64();
    let (first, last) = (t.losses[0], *t.losses.last().expect("loss"));
    let ratio = last / first;
    let x = f32_inputs(&[0.25, -0.5]);
    let s0_out_before: Vec<Obs> = c
        .data
        .iter()
        .map(|p| {
            reference(
                &c.model,
                &f32_inputs(&p.inputs),
                Some(&t.initial),
                &mut Control::default(),
            )
            .obs
        })
        .collect();
    let replay = train::replay(
        &BLAKE3,
        &c,
        &t.provenance,
        None,
        &mut TrainControl::default(),
    )
    .expect("replay")
    .snapshot;
    let s0_after: Vec<Obs> = c
        .data
        .iter()
        .map(|p| {
            reference(
                &c.model,
                &f32_inputs(&p.inputs),
                Some(&t.initial),
                &mut Control::default(),
            )
            .obs
        })
        .collect();
    let refused =
        |s: &ParamSnapshot| match reference(&c.model, &x, Some(s), &mut Control::default()).obs {
            Obs::Refused(d) => d.code_string(),
            o => obs_text(&o),
        };
    let f2 = checked(fixtures::f2());
    let wrong_graph =
        refused(&ParamSnapshot::new(&BLAKE3, &f2, &[1.0, 2.0, 3.0, 4.0]).expect("s2"));
    let mut slots = t.snapshot.slots().to_vec();
    slots.swap(0, 1);
    let wrong_layout = refused(&ParamSnapshot::from_parts(
        &BLAKE3,
        t.snapshot.graph(),
        layout_key(&BLAKE3, c.model.graph()),
        slots,
        t.snapshot.bytes().to_vec(),
    ));
    let mut g_old = fixtures::f1();
    g_old.subject = noble_ngraph::ir::Subject::authored(&BLAKE3, "issue14/F1 previous revision");
    let old = checked(g_old);
    let stale = refused(&ParamSnapshot::new(&BLAKE3, &old, &F1_FIXED).expect("old"));
    let ev = EvidenceRecord::new("B5 final snapshot agreement", &c.model, Some(&t.snapshot));
    let captured = checked(capture(&c.model, &t.snapshot).expect("capture"));
    let ev_new = ev.applies(&captured, None);
    let mut bad = b5_contract(B5_SEED, B5_DATA_SEED);
    bad.data[3].targets[0] = f32::INFINITY;
    let nonfinite =
        train::train(&BLAKE3, &bad, None, 5, &mut TrainControl::default()).expect_err("nonfinite");
    let ok = ratio <= 0.1
        && c.model.id() == id_before
        && noble_ngraph::format::graph::print(c.model.graph()) == text_before
        && s0_out_before == s0_after
        && replay.bytes() == t.snapshot.bytes()
        && wrong_graph == "E-SNAPSHOT-STALE"
        && wrong_layout == "E-SNAPSHOT-LAYOUT"
        && stale == "E-SNAPSHOT-STALE"
        && ev_new == Err(NotApplicable::OtherSubject)
        && nonfinite.diagnostic.code == Code::NonfiniteUpdate
        && nonfinite.published_steps == 0;
    let p = &t.provenance;
    (
        record(
            "B5 training F1 with adam/0",
            &["NGRAPH-SNAP-01", "NGRAPH-DERIV-01"],
            "500 adam/0 steps on F1 against the fixed 64-point target set reduce the MSE to ≤10% of its initial value; GraphLogicalId and every non-param byte are unchanged; S0 evaluates bitwise-identically afterwards; replay reproduces the final snapshot bytes; wrong graph, wrong layout and stale snapshots are refused; old-subject evidence does not apply to the captured subject; a non-finite update publishes nothing",
            ok,
            &["NG-PO-F", "NG-PO-G"],
            &["smoke threshold, not a convergence claim", "replay on the same pinned platform only; cross-platform reproducibility is not claimed", "targets are F1 with F1_FIXED params (teacher) evaluated by L3"],
            Json::obj([
                ("seed", Json::U(B5_SEED)),
                ("data_seed", Json::U(B5_DATA_SEED)),
                ("prng", Json::s(PRNG)),
                ("data_points", Json::U(c.data.len() as u64)),
                ("data_digest", Json::s(hex(&data_digest(&BLAKE3, &c.data)))),
                ("steps", Json::U(B5_STEPS)),
                ("initial_loss", Json::F(first)),
                ("final_loss", Json::F(last)),
                ("final_over_initial", Json::F(ratio)),
                ("graph_logical_id_before", Json::s(hex(&id_before))),
                ("graph_logical_id_after", Json::s(hex(&c.model.id()))),
                ("s0_snapshot_id", Json::s(hex(&t.initial.id()))),
                ("final_snapshot_id", Json::s(hex(&t.snapshot.id()))),
                ("replayed_snapshot_id", Json::s(hex(&replay.id()))),
                ("optimizer_state_digest", Json::s(hex(&p.optimizer_state))),
                ("s0_outputs_unchanged_on_64_points", Json::Bool(s0_out_before == s0_after)),
                ("wrong_graph", Json::s(wrong_graph)),
                ("wrong_layout", Json::s(wrong_layout)),
                ("stale_snapshot", Json::s(stale)),
                ("old_subject_evidence_on_captured_subject", Json::s(format!("{ev_new:?}"))),
                ("captured_graph_logical_id", Json::s(hex(&captured.id()))),
                ("nonfinite_update", Json::s(format!("{} after {} published steps", nonfinite.diagnostic.code_string(), nonfinite.published_steps))),
                ("training_seconds", Json::F(secs)),
                (
                    "provenance",
                    Json::obj([
                        ("parent", Json::s(hex(&p.parent))),
                        ("subject", Json::s(hex(&p.subject.identity))),
                        ("graph", Json::s(hex(&p.graph))),
                        ("profile", Json::s(format!("{} {SEMANTIC_REVISION}", p.profile.name()))),
                        ("deriv", Json::s(p.deriv.describe())),
                        ("facts", Json::s(hex(&p.facts))),
                        ("loss", Json::s(hex(&p.loss))),
                        ("data", Json::s(hex(&p.data))),
                        ("seed", Json::U(p.seed)),
                        ("prng", Json::s(PRNG)),
                        ("optimizer", Json::s(p.optimizer.describe())),
                        ("steps", Json::U(p.steps)),
                        ("tools", Json::s(p.tools.clone())),
                        ("result", Json::s(hex(&p.result))),
                    ]),
                ),
            ]),
            cfg,
        ),
        ok,
    )
}

// Issue #14 campaign leg; its linear measurement order is the review subject.
#[allow(clippy::too_many_lines, reason = "Issue #14 campaign leg; its linear measurement order is the review subject.")]
fn b6(cfg: &Json) -> (Json, bool) {
    let g = checked(fixtures::f1());
    let s = ParamSnapshot::new(&BLAKE3, &g, &F1_FIXED).expect("snapshot");
    let (gid, sid) = (g.id(), s.id());
    let comp = Compiled::new(&BLAKE3, &g);
    let x = f32_inputs(&[0.5, 1.5]);
    let mut ok = true;
    let mut rows = Vec::new();
    let mut run =
        |label: String, ctl: &dyn Fn(&mut ScratchCounters) -> Obs, want: &dyn Fn(&Obs) -> bool| {
            let mut counters = ScratchCounters::default();
            let o = ctl(&mut counters);
            let (acq, rel, live) = counters.snapshot();
            let pass = want(&o) && acq == rel && live == 0 && acq <= 1;
            ok &= pass;
            rows.push(Json::obj([
                ("case", Json::s(label)),
                ("observation", Json::s(obs_text(&o))),
                ("scratch_acquired", Json::U(acq)),
                ("scratch_released", Json::U(rel)),
                ("scratch_live_bytes_after", Json::U(live)),
                ("passed", Json::Bool(pass)),
            ]));
        };
    for k in 0..g.layer_count() {
        for compiled in [false, true] {
            let cancel = move |l: u32| l == k;
            run(
                format!(
                    "cancel at layer {k} ({})",
                    if compiled { "L4" } else { "L3" }
                ),
                &|c| {
                    let mut control = Control {
                        cancel: Some(&cancel),
                        scratch: Some(c),
                        ..Control::default()
                    };
                    if compiled {
                        comp.evaluate(&x, Some(&s), &mut control).obs
                    } else {
                        reference(&g, &x, Some(&s), &mut control).obs
                    }
                },
                &|o| *o == Obs::Cancelled,
            );
        }
    }
    for compiled in [false, true] {
        run(
            format!(
                "fuel exhaustion after 20 nodes ({})",
                if compiled { "L4" } else { "L3" }
            ),
            &|c| {
                let mut control = Control {
                    fuel: Some(20),
                    scratch: Some(c),
                    ..Control::default()
                };
                if compiled {
                    comp.evaluate(&x, Some(&s), &mut control).obs
                } else {
                    reference(&g, &x, Some(&s), &mut control).obs
                }
            },
            &|o| *o == Obs::Exhausted(BudgetKind::Fuel),
        );
    }
    let guarded = checked(
        noble_ngraph::format::graph::parse(
            &BLAKE3,
            "ngraph-ir/0 ngraph-f32/0\nsubject authored-name b6 guard\nslot input f32\n%0 = input 0\n%1 = f32.add %0 %0\n%2 = guard %1 bit\n%3 = f32.select %1 %0 %1\nout %3\n",
        )
        .expect("parses"),
    );
    let gc = Compiled::new(&BLAKE3, &guarded);
    for compiled in [false, true] {
        let tag = if compiled { "L4" } else { "L3" };
        run(
            format!("guard failure G-FACT-FALSE ({tag})"),
            &|c| {
                let mut control = Control {
                    scratch: Some(c),
                    ..Control::default()
                };
                let i = f32_inputs(&[0.25]);
                if compiled {
                    gc.evaluate(&i, None, &mut control).obs
                } else {
                    reference(&guarded, &i, None, &mut control).obs
                }
            },
            &|o| matches!(o, Obs::GuardFailed { .. }),
        );
        run(
            format!("preflight refusal E-SNAPSHOT-STALE, no scratch acquired ({tag})"),
            &|c| {
                let mut control = Control {
                    scratch: Some(c),
                    ..Control::default()
                };
                let other = checked(fixtures::f2());
                let s2 = ParamSnapshot::new(&BLAKE3, &other, &[1.0, 2.0, 3.0, 4.0]).expect("s2");
                if compiled {
                    comp.evaluate(&x, Some(&s2), &mut control).obs
                } else {
                    reference(&g, &x, Some(&s2), &mut control).obs
                }
            },
            &|o| matches!(o, Obs::Refused(d) if d.code == Code::SnapshotStale),
        );
        run(
            format!("normal return ({tag})"),
            &|c| {
                let mut control = Control {
                    scratch: Some(c),
                    ..Control::default()
                };
                if compiled {
                    comp.evaluate(&x, Some(&s), &mut control).obs
                } else {
                    reference(&g, &x, Some(&s), &mut control).obs
                }
            },
            &|o| matches!(o, Obs::Normal(_)),
        );
    }
    // Owners released in both orders.
    let mut owners_ok = true;
    for order in [[0usize, 1], [1, 0]] {
        let snap = ParamSnapshot::new(&BLAKE3, &g, &F1_FIXED).expect("snapshot");
        let weak = Rc::downgrade(&snap.bytes_rc());
        let mut owners = [Some(snap.clone()), Some(snap)];
        owners[order[0]] = None;
        owners_ok &= weak.upgrade().is_some();
        owners[order[1]] = None;
        owners_ok &= weak.upgrade().is_none();
    }
    // Training cancelled before step 4: last published = after step 3.
    let c = b5_contract(B5_SEED, B5_DATA_SEED);
    let three = train::train(&BLAKE3, &c, None, 3, &mut TrainControl::default()).expect("3 steps");
    let mut counters = ScratchCounters::default();
    let cancel = |t: u64| t == 4;
    let e = train::train(
        &BLAKE3,
        &c,
        None,
        10,
        &mut TrainControl {
            cancel_before_step: Some(&cancel),
            scratch: Some(&mut counters),
            ..TrainControl::default()
        },
    )
    .expect_err("cancelled");
    let has_partial = e.diagnostic.code != Code::Cancelled
        || e.published_steps != 3
        || e.last.as_ref().map(ParamSnapshot::bytes) != Some(three.snapshot.bytes());
    let tc = counters.snapshot();
    let retained = g.id() == gid
        && ParamSnapshot::from_parts(
            &BLAKE3,
            s.graph(),
            s.layout(),
            s.slots().to_vec(),
            s.bytes().to_vec(),
        )
        .id()
            == sid;
    ok &= owners_ok && !has_partial && tc.0 == tc.1 && tc.2 == 0 && retained;
    (
        record(
            "B6 ownership and cleanup",
            &["NGRAPH-OWN-01"],
            "Scratch leases are released exactly once on normal return, guard failure, exhaustion and cancellation at every layer (L3 and L4); preflight refusals acquire none; duplicate snapshot owners release in both orders; cancelled training publishes no partial snapshot; retained graph and snapshot digests are unchanged; a retained snapshot is used after the training invocation exits (tests/b6_ownership.rs)",
            ok,
            &["NG-PO-C", "NG-PO-G"],
            &["instrumentation: the crate's scratch-lease counters, not a global allocator (a counting allocator needs unsafe, forbidden by workspace policy)", "#3 OWN-SCRATCH-01 is proposed, not canonical (P6)"],
            Json::obj([
                ("evaluation_cases", Json::A(rows)),
                ("owners_released_in_both_orders", Json::Bool(owners_ok)),
                ("training_cancel_before_step_4", Json::s(format!("{} published_steps={} last_equals_3_step_snapshot={}", e.diagnostic.code_string(), e.published_steps, !has_partial))),
                ("training_scratch_acquired_released_live", Json::A(vec![Json::U(tc.0), Json::U(tc.1), Json::U(tc.2)])),
                ("retained_digests_unchanged", Json::Bool(retained)),
            ]),
            cfg,
        ),
        ok,
    )
}

fn main() {
    let a = args();
    let load_start = read("/proc/loadavg").trim().to_owned();
    let started = Instant::now();
    let b = Budgets::default();
    let cfg = Json::obj([
        ("source_revision", Json::s(a.source_rev.clone())),
        ("toolchain", Json::s(a.toolchain.clone())),
        ("crate", Json::s(format!("noble-ngraph-experiment {}", env!("CARGO_PKG_VERSION")))),
        ("profiles", Json::s(format!("{IR_VERSION}; ngraph-i64/0 + ngraph-f32/0; {SEMANTIC_REVISION}; {FACT_RULE_REVISION}; {COMPILER_REVISION}"))),
        ("oracle", Json::s(APFLOAT_ORACLE)),
        ("prng", Json::s(PRNG)),
        ("target", Json::s(format!("{}-{}", std::env::consts::ARCH, std::env::consts::OS))),
        ("optimization_mode", Json::s(if cfg!(debug_assertions) { "debug" } else { "release (overflow-checks=true, panic=unwind per workspace profile)" })),
        ("cpu", Json::s(cpu_model())),
        ("logical_cpus", Json::U(std::thread::available_parallelism().map_or(0, |n| n.get() as u64))),
        ("load_average_at_start", Json::s(load_start)),
        ("workload", Json::s(format!("B1 F1-F3; B2 vectors + {} random graphs x {} vectors ({} ops) + {} op pairs; B3 core controls; B4 {} gradient samples; B5 {} steps; B6 cleanup matrix", a.graphs, a.vectors, a.ops, a.pairs, a.grad_samples, B5_STEPS))),
        (
            "resource_limits",
            Json::s(format!("/0 defaults: live nodes {}, layers {}, affine terms {}, table {} / {}, facts {}, graph bytes {}, snapshot bytes {}, forward scratch {}, backward scratch {}, optimizer steps {}", b.live_nodes, b.layers, b.affine_terms, b.table_length_entries, b.table_entries, b.facts, b.graph_bytes, b.snapshot_bytes, b.forward_scratch, b.backward_scratch, b.optimizer_steps)),
        ),
        (
            "wasm_features",
            Json::obj([
                ("gc", Json::s("not-applicable: CPU-only Rust legs; no Wasm executed by this harness")),
                ("function_references", Json::s("not-applicable")),
                ("multi_value", Json::s("not-applicable")),
                ("tail_call", Json::s("not-applicable")),
                ("component_model", Json::s("not-applicable")),
            ]),
        ),
        ("instrumentation", Json::s("scratch-lease counters (crate); wall-clock seconds via std::time::Instant; no hardware counters (unsupported, not zero)")),
    ]);
    let mut records = Vec::new();
    let mut ok = true;
    let (r, k) = b1(&cfg);
    records.push(r);
    ok &= k;
    let (rs, k) = b2(&cfg, &a);
    records.extend(rs);
    ok &= k;
    for f in [b3 as fn(&Json) -> (Json, bool), b5, b6] {
        let (r, k) = f(&cfg);
        records.push(r);
        ok &= k;
    }
    let (r, k) = b4(&cfg, &a);
    records.push(r);
    ok &= k;
    let mut run = cfg;
    run.push("load_average_at_end", Json::s(read("/proc/loadavg").trim()));
    run.push("wall_seconds", Json::F(started.elapsed().as_secs_f64()));
    let doc = Json::obj([
        ("schema", Json::s("noble-ngraph-phase-b-core-evidence/v1")),
        ("issue", Json::s("OnixResearch/noble-lang#14")),
        ("change", Json::s(".cairn/archive/2026-10-16-numeric-graph-profile")),
        ("scope", Json::s("Phase B core legs: authored F32 IR (B0(b)), graph checker, L2/B2-ORACLE/L3/L4, B3 core controls, B4, B5, B6. I64 extraction, L1 Wasm/V8 and the BigInt oracle are the extraction leg's evidence.")),
        ("non_claims", Json::A(vec![
            Json::s("Finite tests and evaluator agreement do not prove the compiler, the graph checker (NG-PO-B) or the evaluators (NG-PO-C) correct."),
            Json::s("NG-PO-A..H remain open; no obligation is added to obligations.json."),
            Json::s("No performance claim (Phase C)."),
        ])),
        ("run", run),
        ("record_count", Json::U(records.len() as u64)),
        ("records", Json::A(records)),
        ("result", Json::s(if ok { "passed" } else { "failed" })),
    ]);
    let text = doc.pretty();
    match &a.out {
        Some(p) => std::fs::write(p, text).unwrap_or_else(|e| panic!("write {p}: {e}")),
        None => print!("{text}"),
    }
    if !ok {
        std::process::exit(1);
    }
}
