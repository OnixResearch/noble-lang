//! Issue #14 Phase B evidence harness for the `I64` extraction leg: B1
//! (I1–I4 lowering), B2 (L1 real CLI `--opt off|on`, L2 Node BigInt oracle,
//! L3, L4) and B3 (source-level controls with their ordinary-path runs).
//! Writes one JSON receipt with EV-STATE-01 fields per record and the
//! BE-CONFIG-01 run fields.
//!
//! ```text
//! cargo run --release -p noble-ngraph-experiment --bin ngraph-phase-b-extract -- \
//!   --cli /abs/noble --node /abs/node --source-rev <git sha> \
//!   --toolchain "$(rustc -V)" --out verification/ngraph/phase-b-extract.json \
//!   [--seed N] [--programs N] [--vectors N] [--l1-programs N] [--l1-vectors N] [--steps N]
//! ```
//!
//! `--out` must not exist. Exit status 1 if any record failed.

use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Instant;

use serde_json::{json, Value as J};

use noble_ngraph_experiment::b2i64::{self, Config, Lowered};
use noble_ngraph_experiment::bench::{cpu_model, read, sha256};
use noble_ngraph_experiment::compiled::COMPILER_REVISION;
use noble_ngraph_experiment::extract::Budget;
use noble_ngraph_experiment::ir::{hex, Op, IR_VERSION, SEMANTIC_REVISION};
use noble_ngraph_experiment::prng::PRNG;
use noble_ngraph_experiment::sem::Value;

const TRUSTED: [&str; 7] = [
    "noble_kernel::acceptance::check as the independent subject checker (PO-09/10/11 accepted for bootstrap fragment v1 only)",
    "the extractor's unpruned compose/run expansion accounting (inside the trusted extraction boundary until NG-PO-A)",
    "this crate's graph checker and L3/L4 evaluators (NG-PO-B/C open)",
    "the pinned noble release CLI, its Node/V8 runtime and wasm-opt (--opt on) as the ordinary path (PO-17/18 open)",
    "the pinned Node BigInt implementation as the L2 oracle runtime",
    "rustc/LLVM of the pinned nightly toolchain",
    "blake3 =1.8.7 (pure) and sha256sum for identities: a hash identifies bytes, it proves nothing",
];

#[allow(clippy::too_many_arguments)]
fn record(
    id: &str,
    requirements: &[&str],
    claim: &str,
    passed: bool,
    obligations: &[&str],
    assumptions: &[&str],
    result: J,
    cfg: &J,
) -> J {
    json!({
        "id": id,
        "requirements": requirements,
        "claim": claim,
        "implementation": "implemented",
        "execution": if passed { "passed" } else { "failed" },
        "proof": "open",
        "open_obligations": obligations,
        "trust": "explicit",
        "trusted_boundaries": TRUSTED,
        "evidence": {
            "kind": "test",
            "subject": id,
            "revision": cfg["source_revision"],
            "configuration": "see run (BE-CONFIG-01)",
            "claim": claim,
            "assumptions": assumptions,
            "result": result,
        },
    })
}

struct Args {
    out: PathBuf,
    cli: PathBuf,
    node: PathBuf,
    source_rev: String,
    toolchain: String,
    seed: u64,
    programs: u64,
    vectors: usize,
    l1_programs: u64,
    l1_vectors: usize,
    steps: usize,
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
        node: need("--node")?.into(),
        source_rev: flag("--source-rev").unwrap_or_else(|| "unrecorded".into()),
        toolchain: flag("--toolchain").unwrap_or_else(|| "unrecorded".into()),
        seed: num("--seed", 0x1404_b201)?,
        programs: num("--programs", 64)?,
        vectors: size("--vectors", 10_000)?,
        l1_programs: num("--l1-programs", 16)?,
        l1_vectors: size("--l1-vectors", 24)?,
        steps: size("--steps", 1_000)?,
    })
}

fn main() -> ExitCode {
    match run() {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::from(1),
        Err(e) => {
            eprintln!("ngraph-phase-b-extract: {e}");
            ExitCode::from(2)
        }
    }
}

fn i64s(v: &[i64]) -> Vec<Value> {
    v.iter().map(|x| Value::I64(*x)).collect()
}

/// B1: the design's I1–I4 lowered, with their structural properties.
fn b1(cfg: &J) -> Result<(J, bool), String> {
    let lower = |defs: &[&str], root: &str, inputs: usize| {
        let defs: Vec<String> = defs.iter().map(|d| (*d).to_owned()).collect();
        Lowered::new(&defs, root, inputs)
    };
    let summary =
        |name: &str, x: &Lowered, design: &[i64], expected: &[i64]| -> Result<(J, bool), String> {
            let l3 = x.l3(&i64s(design))?;
            let l4 = x.l4(&i64s(design))?;
            let ok = l3 == i64s(expected) && l4 == l3;
            let g = x.extraction.checked.graph();
            Ok((
                json!({
                    "name": name,
                    "definitions": x.definitions,
                    "root": x.root,
                    "input_types": vec!["I64"; x.inputs],
                    "graph_logical_id": hex(&g.logical_id()),
                    "subject_closure": g.subject.closure,
                    "live_nodes": g.nodes.len(),
                    "expanded_nodes": x.extraction.stats.expanded_nodes,
                    "pruned_nodes": x.extraction.stats.pruned_nodes,
                    "max_inline_depth": x.extraction.stats.max_inline_depth,
                    "design_inputs": design,
                    "expected": expected,
                    "l3": format!("{l3:?}"),
                    "l4": format!("{l4:?}"),
                    "ir_ops": g.nodes.iter().map(|n| format!("{:?}", n.op)).collect::<Vec<_>>(),
                }),
                ok,
            ))
        };
    let i1 = lower(&[], "dup * swap 3 * + 7 -", 2)?;
    let i2 = lower(&[], "2 quote [ + ] compose dup compose run", 1)?;
    let i3 = lower(&["def sq [ dup * ]"], "sq swap sq +", 2)?;
    let i4 = lower(&[], "swap [ * ] dip + dup", 3)?;
    let (r1, k1) = summary("I1", &i1, &[5, 4], &[24])?;
    let (r2, k2) = summary("I2", &i2, &[40], &[44])?;
    let (r3, k3) = summary("I3", &i3, &[3, 4], &[25])?;
    let (r4, k4) = summary("I4", &i4, &[2, 3, 5], &[13, 13])?;
    // I2: one `const 2` shared by both additions (capture id reused).
    let g2 = i2.extraction.checked.graph();
    let consts: Vec<u32> = (0..g2.nodes.len())
        .filter(|&i| matches!(g2.nodes[i].op, Op::Const(_)))
        .map(|i| u32::try_from(i).unwrap_or(u32::MAX))
        .collect();
    let shared_uses = consts.first().map_or(0, |c| {
        g2.nodes
            .iter()
            .filter(|n| matches!(n.op, Op::I64Add(_, b) if b.0 == *c))
            .count()
    });
    let i2_shared = consts.len() == 1 && shared_uses == 2;
    // I3: both calls resolve to one identity.
    let g3 = i3.extraction.checked.graph();
    let i3_one_identity = g3.subject.closure.len() == 1;
    // I4: 1,000 host steps of (s, i, k) → (s·k+i, s·k+i) on L3 and L4.
    let mut s = 1i64;
    let mut i4_ok = true;
    let mut rng = noble_ngraph_experiment::prng::SplitMix64::new(0x1404_b104);
    for _ in 0..1000 {
        let (i, k) = (
            b2i64::random::value(&mut rng),
            b2i64::random::value(&mut rng),
        );
        let v = i64s(&[s, i, k]);
        let (a, b) = (i4.l3(&v)?, i4.l4(&v)?);
        let next = s.wrapping_mul(k).wrapping_add(i);
        i4_ok &= a == i64s(&[next, next]) && a == b;
        s = next;
    }
    let ok = k1 && k2 && k3 && k4 && i2_shared && i3_one_identity && i4_ok;
    let result = json!({
        "surface_syntax": "Core frontend: the design's (block IN OUT [ body ]) is the body prepared with the block's input types (what `noble session` submits with those values on its stack); `sq = [ dup * ]` is spelled `def sq [ dup * ]`",
        "programs": [r1, r2, r3, r4],
        "i2_one_shared_capture_used_twice": i2_shared,
        "i3_repeated_call_one_identity": i3_one_identity,
        "i4_host_steps": { "steps": 1000, "seed": format!("{:#x}", 0x1404_b104u64), "l3_l4_agree_with_s_k_plus_i": i4_ok, "final_state": s },
    });
    Ok((
        record(
            "B1-I64-lowering",
            &["NGRAPH-EXTRACT-01"],
            "I1–I4 lower through D7 admission and symbolic extraction to checked ngraph-i64/0 graphs that give the design's results (I1 24, I2 44 with one shared capture, I3 25 via one identity, I4 s·k+i for 1,000 host steps)",
            ok,
            &["NG-PO-A", "NG-PO-B", "NG-PO-C"],
            &["subjects come from the ordinary Core frontend and are re-accepted by the kernel"],
            result,
            cfg,
        ),
        ok,
    ))
}

fn run() -> Result<bool, String> {
    let a = args()?;
    if a.out.exists() {
        return Err(format!(
            "{} exists; receipts are never overwritten",
            a.out.display()
        ));
    }
    let load_start = read("/proc/loadavg").trim().to_owned();
    let started = Instant::now();
    let oracle = b2i64::Oracle {
        node: a.node.clone(),
    };
    let budget = Budget::default();
    let cfg = json!({
        "source_revision": a.source_rev,
        "toolchain": a.toolchain,
        "crate": format!("noble-ngraph-experiment {}", env!("CARGO_PKG_VERSION")),
        "profiles": format!("{IR_VERSION}; ngraph-i64/0 (extraction); {SEMANTIC_REVISION}; {COMPILER_REVISION}"),
        "ordinary_path": {
            "cli": a.cli,
            "cli_sha256": sha256(&a.cli),
            "modes": ["noble session --framed --opt off", "noble session --framed --opt on", "noble live slot --engine v8 (B3 live-slot controls)"],
        },
        "oracle": {
            "kind": "L2 BigInt modulo-2^64 (explicit reduction), Node -e",
            "node": a.node,
            "node_sha256": sha256(&a.node),
            "node_version": oracle.version()?,
            "oracle_blake3": blake3::hash(b2i64::ORACLE_JS.as_bytes()).to_hex().to_string(),
        },
        "prng": PRNG,
        "seed": format!("{:#x}", a.seed),
        "target": format!("{}-{}", std::env::consts::ARCH, std::env::consts::OS),
        "optimization_mode": if cfg!(debug_assertions) { "debug" } else { "release (overflow-checks=true, panic=unwind per workspace profile)" },
        "cpu": cpu_model(),
        "logical_cpus": std::thread::available_parallelism().map_or(0, std::num::NonZero::get),
        "load_average_at_start": load_start,
        "workload": format!(
            "B1 I1-I4; B2 {} named graphs + {} random Core programs x {} vectors (L2-L4), L1 on named design vectors + {} vectors per named graph and {} random programs x {} vectors, I4 {} host steps; B3 source controls",
            10, a.programs, a.vectors, a.l1_vectors, a.l1_programs, a.l1_vectors, a.steps
        ),
        "resource_limits": format!(
            "extraction /0 defaults: work units {}, expanded nodes {}, inline depth {}; checker /0 defaults; CLI Core session limits (65,536 bytes, 16,384 nodes, depth 64, work 2,000,000)",
            budget.work_units, budget.expanded_nodes, budget.inline_depth
        ),
        "wasm_features": "as fixed by the pinned CLI runtime (managed-linear-memory backend; --opt on runs wasm-opt --enable-gc --enable-reference-types --enable-bulk-memory -O2 --strip-debug)",
        "instrumentation": "wall-clock seconds via std::time::Instant; extractor counters (work units, expanded/pruned nodes, inline depth); no hardware counters (unsupported, not zero)",
    });
    let mut records = Vec::new();
    let mut ok = true;

    let (r, k) = b1(&cfg)?;
    records.push(r);
    ok &= k;

    let config = Config {
        seed: a.seed,
        programs: a.programs,
        vectors: a.vectors,
        cli: Some(a.cli.clone()),
        l1_programs: a.l1_programs,
        l1_vectors: a.l1_vectors,
        transition_steps: a.steps,
        node: a.node.clone(),
    };
    let b2 = b2i64::campaign(&config)?;
    let k = b2["agree"].as_bool().unwrap_or(false);
    records.push(record(
        "B2-I64-differential",
        &["NGRAPH-EXTRACT-01", "NGRAPH-PROOF-01"],
        "L1 (real CLI, --opt off and on), L2 (Node BigInt mod 2^64), L3 and L4 agree under ≡obs on the named vectors (5 3 - = 2, MAX 1 + = MIN, MIN -1 * = MIN, I1–I4), the I4 host-step chain and seeded random Core I64 programs",
        k,
        &["NG-PO-A", "NG-PO-B", "NG-PO-C", "PO-17", "PO-18"],
        &[
            "L2 evaluates the extracted graph, so L1 is the only leg that observes the source independently of extraction",
            "L1 covers a recorded sample; L2–L4 cover every vector",
        ],
        b2,
        &cfg,
    ));
    ok &= k;

    let b3 = b2i64::b3_receipt(Some(&a.cli))?;
    let k = b3
        .as_array()
        .is_some_and(|c| c.iter().all(|x| x["expected"] == x["observed"]));
    records.push(record(
        "B3-I64-source-controls",
        &["NGRAPH-EXTRACT-01", "NGRAPH-SCOPE-01"],
        "every forbidden source construct is refused by admission/extraction with its D6 code before any graph exists (0 graph nodes evaluated), while the ordinary path's own behavior on the same source is unchanged",
        k,
        &["NG-PO-A"],
        &[
            "recursion and the k=20 chain cannot be written in Core source (the frontend refuses recursion and exhausts at d10); they are minimally forged, kernel-accepted submissions",
        ],
        json!({ "cases": b3 }),
        &cfg,
    ));
    ok &= k;

    let mut run = cfg;
    run["load_average_at_end"] = json!(read("/proc/loadavg").trim());
    run["wall_seconds"] = json!(started.elapsed().as_secs_f64());
    let doc = json!({
        "schema": "noble-ngraph-phase-b-extract-evidence/v1",
        "issue": "OnixResearch/noble-lang#14",
        "change": ".cairn/changes/numeric-graph-profile",
        "scope": "Phase B I64 extraction leg: D7 admission/extraction from Core source (B0(b): I64 only), B1 I1-I4, B2 L1/L2/L3/L4 for I64, B3 source-level controls",
        "non_claims": [
            "Finite differential agreement does not prove extraction (NG-PO-A), the checker (NG-PO-B) or the evaluators (NG-PO-C).",
            "The subject digest is a producer digest over the kernel's Debug rendering; it is not independent validation.",
            "NG-PO-A..H remain open; no obligation is added to obligations.json. No performance claim (Phase C).",
        ],
        "run": run,
        "record_count": records.len(),
        "records": records,
        "result": if ok { "passed" } else { "failed" },
    });
    let text = serde_json::to_string_pretty(&doc).map_err(|e| e.to_string())?;
    std::fs::write(&a.out, format!("{text}\n")).map_err(|e| format!("{}: {e}", a.out.display()))?;
    println!(
        "{} {}",
        a.out.display(),
        if ok { "passed" } else { "failed" }
    );
    Ok(ok)
}
