//! Phase B B2 for `ngraph-i64/0`: the four differential legs over extracted
//! graphs.
//!
//! - L1 ordinary Noble Wasm/V8 through the real CLI ([`crate::l1`]), `--opt
//!   off` and `--opt on`, on the exact root source;
//! - L2 an independent BigInt modulo-2^64 oracle under a pinned Node
//!   ([`Oracle`]);
//! - L3 the reference interpreter ([`noble_ngraph::eval::reference`]);
//! - L4 the compiled layered evaluator ([`noble_ngraph::compiled::Compiled`]).
//!
//! Agreement is `≡obs` (for `I64`/`Bool`: equality of every output). It is
//! finite differential evidence; it proves neither extraction (NG-PO-A) nor
//! the evaluators (NG-PO-C).

mod l2;
pub mod random;

use crate::scheme::BLAKE3;
use std::path::PathBuf;

use serde_json::json;

pub use l2::{cell, Oracle, ORACLE_JS};

use crate::l1::{self, Cli, Opt};
use noble_ngraph::compiled::Compiled;
use noble_ngraph::eval::{self, Control, Obs};
use noble_ngraph::extract::{self, source, Budget, Extraction, Request};
use noble_ngraph::prng::SplitMix64;
use noble_ngraph::sem::{self, Value};

/// One extracted subject with its compiled evaluator.
#[derive(Debug)]
pub struct Lowered {
    /// `def` submissions.
    pub definitions: Vec<String>,
    /// Root body.
    pub root: String,
    /// `I64` inputs.
    pub inputs: usize,
    /// Extraction.
    pub extraction: Extraction,
    /// L4.
    pub compiled: Compiled,
}

impl Lowered {
    /// Prepares, admits, extracts and compiles a Core root over `inputs`
    /// `I64` values.
    ///
    /// # Errors
    /// A frontend or D6 refusal, rendered.
    pub fn new(definitions: &[String], root: &str, inputs: usize) -> Result<Self, String> {
        let defs: Vec<&str> = definitions.iter().map(String::as_str).collect();
        let p = source::prepare(&defs, root, &source::i64s(inputs))?;
        let admitted = extract::admit(
            &BLAKE3,
            &p.submission,
            &Request {
                profile: "ngraph-i64/0",
                claimed_closure: &p.claimed_closure,
                budget: Budget::default(),
            },
        )
        .map_err(|d| d.to_string())?;
        let extraction =
            extract::extract(&BLAKE3, &admitted, &noble_ngraph::check::Budgets::default())
                .map_err(|d| d.to_string())?;
        let compiled = Compiled::new(&BLAKE3, &extraction.checked);
        Ok(Self {
            definitions: definitions.to_vec(),
            root: root.to_owned(),
            inputs,
            extraction,
            compiled,
        })
    }

    /// Number of graph outputs (all roots here have scalar outputs).
    #[must_use]
    pub fn outputs(&self) -> usize {
        self.extraction.checked.graph().outputs.len()
    }

    /// L3 outputs.
    ///
    /// # Errors
    /// A non-normal observation.
    pub fn l3(&self, inputs: &[Value]) -> Result<Vec<Value>, String> {
        normal(eval::reference(&self.extraction.checked, inputs, None, &Control::default()).obs)
    }

    /// L4 outputs.
    ///
    /// # Errors
    /// A non-normal observation.
    pub fn l4(&self, inputs: &[Value]) -> Result<Vec<Value>, String> {
        normal(
            self.compiled
                .evaluate(inputs, None, &Control::default())
                .obs,
        )
    }
}

fn normal(obs: Obs) -> Result<Vec<Value>, String> {
    match obs {
        Obs::Normal(v) => Ok(v),
        other => Err(format!("{other:?}")),
    }
}

fn l1_values(values: &[Value]) -> Vec<l1::Value> {
    values
        .iter()
        .map(|v| match v {
            Value::I64(x) => l1::Value::I64(*x),
            Value::Bool(b) => l1::Value::Bool(*b),
            Value::F32(bits) => l1::Value::Other(format!("F32 {bits:08x}")),
        })
        .collect()
}

fn i64s(values: &[i64]) -> Vec<Value> {
    values.iter().map(|v| Value::I64(*v)).collect()
}

/// Campaign configuration.
#[derive(Clone, Debug)]
pub struct Config {
    /// Recorded `splitmix64/0` seed.
    pub seed: u64,
    /// Random programs.
    pub programs: u64,
    /// Input vectors per graph through L2–L4.
    pub vectors: usize,
    /// Pinned CLI for L1 (`None` skips L1).
    pub cli: Option<PathBuf>,
    /// Random programs also run through L1.
    pub l1_programs: u64,
    /// Vectors per program through L1 (named programs add their design vectors).
    pub l1_vectors: usize,
    /// I4 host steps.
    pub transition_steps: usize,
    /// Pinned Node for L2.
    pub node: PathBuf,
}

/// A named B2 subject with its design vector and expected outputs.
struct Named {
    name: &'static str,
    definitions: &'static [&'static str],
    root: &'static str,
    inputs: usize,
    design: &'static [i64],
    expected: &'static [i64],
}

const NAMED: &[Named] = &[
    Named {
        name: "I1",
        definitions: &[],
        root: "dup * swap 3 * + 7 -",
        inputs: 2,
        design: &[5, 4],
        expected: &[24],
    },
    Named {
        name: "I2",
        definitions: &[],
        root: "2 quote [ + ] compose dup compose run",
        inputs: 1,
        design: &[40],
        expected: &[44],
    },
    Named {
        name: "I3",
        definitions: &["def sq [ dup * ]"],
        root: "sq swap sq +",
        inputs: 2,
        design: &[3, 4],
        expected: &[25],
    },
    Named {
        name: "I4",
        definitions: &[],
        root: "swap [ * ] dip + dup",
        inputs: 3,
        design: &[2, 3, 5],
        expected: &[13, 13],
    },
    Named {
        name: "operand-order-literals",
        definitions: &[],
        root: "5 3 -",
        inputs: 0,
        design: &[],
        expected: &[2],
    },
    Named {
        name: "operand-order",
        definitions: &[],
        root: "-",
        inputs: 2,
        design: &[5, 3],
        expected: &[2],
    },
    Named {
        name: "overflow-add-literals",
        definitions: &[],
        root: "9223372036854775807 1 +",
        inputs: 0,
        design: &[],
        expected: &[i64::MIN],
    },
    Named {
        name: "overflow-add",
        definitions: &[],
        root: "+",
        inputs: 2,
        design: &[i64::MAX, 1],
        expected: &[i64::MIN],
    },
    Named {
        name: "overflow-mul-literals",
        definitions: &[],
        root: "-9223372036854775808 -1 *",
        inputs: 0,
        design: &[],
        expected: &[i64::MIN],
    },
    Named {
        name: "overflow-mul",
        definitions: &[],
        root: "*",
        inputs: 2,
        design: &[i64::MIN, -1],
        expected: &[i64::MIN],
    },
];

struct Tally {
    mismatches: Vec<String>,
}

impl Tally {
    fn check(&mut self, what: impl FnOnce() -> String, ok: bool) {
        if !ok && self.mismatches.len() < 32 {
            self.mismatches.push(what());
        }
    }
}

/// Runs L2–L4 on `vectors` (and L1 on the first `l1` of them, both `--opt`
/// modes); returns the per-graph record.
fn legs(
    cfg: &Config,
    t: &mut Tally,
    name: &str,
    lowered: &Lowered,
    vectors: &[Vec<Value>],
    l1: usize,
) -> Result<serde_json::Value, String> {
    let oracle = Oracle {
        node: cfg.node.clone(),
    };
    let l2 = oracle.evaluate(lowered.extraction.checked.graph(), vectors)?;
    let mut digest = blake3::Hasher::new();
    let mut l3_all = Vec::with_capacity(vectors.len());
    for (i, v) in vectors.iter().enumerate() {
        let l3 = lowered.l3(v)?;
        let l4 = lowered.l4(v)?;
        t.check(
            || format!("{name} v{i}: L3 {l3:?} ≢ L4 {l4:?}"),
            sem::obs_eq_all(&l3, &l4),
        );
        let l3_cells: Vec<String> = l3.iter().map(|x| cell(*x)).collect();
        t.check(
            || format!("{name} v{i}: L3 {l3_cells:?} ≢ L2 {:?}", l2[i]),
            l3_cells == l2[i],
        );
        for c in &l3_cells {
            digest.update(c.as_bytes());
            digest.update(b",");
        }
        digest.update(b";");
        l3_all.push(l3);
    }
    let mut l1_record = serde_json::Value::Null;
    if let (Some(bin), true) = (&cfg.cli, l1 > 0) {
        let n = l1.min(vectors.len());
        let inputs: Vec<Vec<l1::Value>> = vectors[..n].iter().map(|v| l1_values(v)).collect();
        let defs: Vec<&str> = lowered.definitions.iter().map(String::as_str).collect();
        let mut modes = serde_json::Map::new();
        for opt in [Opt::Off, Opt::On] {
            let cli = Cli::new(bin.clone(), opt);
            let run = cli
                .run_vectors(&defs, &lowered.root, lowered.outputs(), &inputs)
                .map_err(|e| format!("{name} L1 {}: {e}", opt.flag()))?;
            let mut agree = 0usize;
            let mut quota = Vec::new();
            for (i, r) in run.results.iter().enumerate() {
                // An ordinary-path quota (BE-LIMIT-01) is a distinct class,
                // never counted as agreement or as a disagreement.
                if l1::quota(r) {
                    quota.push(json!({ "vector": i, "outcome": r.outcome, "stage": r.stage, "diagnostic": r.diagnostic }));
                    continue;
                }
                let ok = r.outcome == "normal" && r.stack == l1_values(&l3_all[i]);
                t.check(
                    || {
                        format!(
                            "{name} v{i}: L1 --opt {} {} ≢ L3 {:?}",
                            opt.flag(),
                            r.raw,
                            l3_all[i]
                        )
                    },
                    ok,
                );
                agree += usize::from(ok);
            }
            modes.insert(
                opt.flag().to_owned(),
                json!({
                    "vectors": n,
                    "agree": agree,
                    "ordinary_quota_outcomes": quota,
                    "session_restarts_after_ordinary_quota": run.restarts,
                }),
            );
        }
        l1_record = serde_json::Value::Object(modes);
    }
    let g = lowered.extraction.checked.graph();
    Ok(json!({
        "name": name,
        "definitions": lowered.definitions,
        "root": lowered.root,
        "inputs": lowered.inputs,
        "graph_logical_id": noble_ngraph::ir::hex(&lowered.extraction.checked.graph().logical_id(&BLAKE3)),
        "live_nodes": g.nodes.len(),
        "outputs": g.outputs.len(),
        "expanded_nodes": lowered.extraction.stats.expanded_nodes,
        "pruned_nodes": lowered.extraction.stats.pruned_nodes,
        "work_units": lowered.extraction.stats.work_units,
        "max_inline_depth": lowered.extraction.stats.max_inline_depth,
        "l2_l3_l4_vectors": vectors.len(),
        "l1": l1_record,
        "l3_output_digest": digest.finalize().to_hex().to_string(),
    }))
}

fn seeded_vectors(
    rng: &mut SplitMix64,
    inputs: usize,
    count: usize,
    first: &[i64],
) -> Vec<Vec<Value>> {
    let mut out = Vec::with_capacity(count);
    if inputs == 0 {
        out.push(Vec::new());
        return out;
    }
    if first.len() == inputs {
        out.push(i64s(first));
    }
    while out.len() < count {
        out.push(
            (0..inputs)
                .map(|_| Value::I64(random::value(rng)))
                .collect(),
        );
    }
    out
}

/// Runs the whole campaign and returns the receipt body.
///
/// # Errors
/// A refusal of a subject that must extract, or a leg failure (Node/CLI).
pub fn campaign(cfg: &Config) -> Result<serde_json::Value, String> {
    let mut t = Tally {
        mismatches: Vec::new(),
    };
    let mut rng = SplitMix64::new(cfg.seed);
    let mut named = Vec::new();
    for n in NAMED {
        let defs: Vec<String> = n.definitions.iter().map(|d| (*d).to_owned()).collect();
        let lowered = Lowered::new(&defs, n.root, n.inputs)?;
        let design = lowered.l3(&i64s(n.design))?;
        t.check(
            || {
                format!(
                    "{} design vector: L3 {design:?} ≠ expected {:?}",
                    n.name, n.expected
                )
            },
            design == i64s(n.expected),
        );
        let vectors = seeded_vectors(&mut rng, n.inputs, cfg.vectors, n.design);
        let l1 = if n.inputs == 0 { 1 } else { cfg.l1_vectors + 1 };
        let mut record = legs(cfg, &mut t, n.name, &lowered, &vectors, l1)?;
        record["design_inputs"] = json!(n.design);
        record["design_expected"] = json!(n.expected);
        named.push(record);
    }
    let transition = transition(cfg, &mut t, &mut rng)?;
    let mut random_records = Vec::new();
    for i in 0..cfg.programs {
        let p = random::program(cfg.seed, i);
        let lowered = Lowered::new(&p.definitions, &p.root, p.inputs)
            .map_err(|e| format!("random program {i} {p:?}: {e}"))?;
        let vectors = seeded_vectors(&mut rng, p.inputs, cfg.vectors, &[]);
        let l1 = if i < cfg.l1_programs {
            cfg.l1_vectors
        } else {
            0
        };
        random_records.push(legs(
            cfg,
            &mut t,
            &format!("random-{i}"),
            &lowered,
            &vectors,
            l1,
        )?);
    }
    Ok(json!({
        "named": named,
        "i4_transition": transition,
        "l1_totals": l1_totals(named.iter().chain(&random_records)),
        "random": random_records,
        "mismatches": t.mismatches,
        "agree": t.mismatches.is_empty(),
    }))
}

/// Sums the per-graph L1 records per `--opt` mode.
fn l1_totals<'a>(records: impl Iterator<Item = &'a serde_json::Value>) -> serde_json::Value {
    let mut sums = [[0u64; 5]; 2];
    for r in records {
        for (m, opt) in [Opt::Off, Opt::On].iter().enumerate() {
            let mode = &r["l1"][opt.flag()];
            if mode.is_null() {
                continue;
            }
            let quota = mode["ordinary_quota_outcomes"]
                .as_array()
                .map_or(0, Vec::len) as u64;
            sums[m][0] += 1;
            sums[m][1] += mode["vectors"].as_u64().unwrap_or(0);
            sums[m][2] += mode["agree"].as_u64().unwrap_or(0);
            sums[m][3] += quota;
            sums[m][4] += mode["session_restarts_after_ordinary_quota"]
                .as_u64()
                .unwrap_or(0);
        }
    }
    let row = |s: [u64; 5]| {
        json!({
            "graphs": s[0],
            "vectors": s[1],
            "agree": s[2],
            "ordinary_quota_outcomes": s[3],
            "session_restarts_after_ordinary_quota": s[4],
        })
    };
    json!({ "off": row(sums[0]), "on": row(sums[1]) })
}

/// I4 as a host-driven transition for `transition_steps` steps.
fn transition(
    cfg: &Config,
    t: &mut Tally,
    rng: &mut SplitMix64,
) -> Result<serde_json::Value, String> {
    let lowered = Lowered::new(&[], "swap [ * ] dip + dup", 3)?;
    let steps: Vec<[i64; 2]> = (0..cfg.transition_steps)
        .map(|_| [random::value(rng), random::value(rng)])
        .collect();
    let s0 = 1i64;
    // L3 drives the reference chain; L4 runs its own chain; L2 checks every
    // step's (state, inputs) → outputs, which by induction fixes its chain.
    let mut s3 = s0;
    let mut s4 = s0;
    let mut l3_steps = Vec::with_capacity(steps.len());
    let mut l2_vectors = Vec::with_capacity(steps.len());
    for (j, [i, k]) in steps.iter().enumerate() {
        let v3 = i64s(&[s3, *i, *k]);
        let o3 = lowered.l3(&v3)?;
        let o4 = lowered.l4(&i64s(&[s4, *i, *k]))?;
        t.check(
            || format!("I4 step {j}: L3 {o3:?} ≢ L4 {o4:?}"),
            sem::obs_eq_all(&o3, &o4),
        );
        let expected = s3.wrapping_mul(*k).wrapping_add(*i);
        t.check(
            || format!("I4 step {j}: L3 {o3:?} ≠ s·k+i = {expected}"),
            o3 == i64s(&[expected, expected]),
        );
        l2_vectors.push(v3);
        let Value::I64(next3) = o3[0] else {
            return Err("I4 output is not I64".into());
        };
        let Value::I64(next4) = o4[0] else {
            return Err("I4 output is not I64".into());
        };
        s3 = next3;
        s4 = next4;
        l3_steps.push(o3);
    }
    let oracle = Oracle {
        node: cfg.node.clone(),
    };
    let l2 = oracle.evaluate(lowered.extraction.checked.graph(), &l2_vectors)?;
    for (j, o3) in l3_steps.iter().enumerate() {
        let cells: Vec<String> = o3.iter().map(|x| cell(*x)).collect();
        t.check(
            || format!("I4 step {j}: L3 {cells:?} ≢ L2 {:?}", l2[j]),
            cells == l2[j],
        );
    }
    let mut l1_record = serde_json::Value::Null;
    if let Some(bin) = &cfg.cli {
        let host: Vec<Vec<l1::Value>> = steps.iter().map(|s| l1_values(&i64s(s))).collect();
        let mut modes = serde_json::Map::new();
        for opt in [Opt::Off, Opt::On] {
            let run = Cli::new(bin.clone(), opt)
                .run_transition(&[], &lowered.root, 2, &[l1::Value::I64(s0)], &host)
                .map_err(|e| format!("I4 L1 {}: {e}", opt.flag()))?;
            let mut agree = 0usize;
            for (j, o) in run.results.iter().enumerate() {
                let ok = *o == l1_values(&l3_steps[j]);
                t.check(
                    || format!("I4 step {j}: L1 --opt {} {o:?} ≢ L3", opt.flag()),
                    ok,
                );
                agree += usize::from(ok);
            }
            modes.insert(
                opt.flag().to_owned(),
                json!({ "steps": run.results.len(), "agree": agree, "session_restarts_after_ordinary_quota": run.restarts }),
            );
        }
        l1_record = serde_json::Value::Object(modes);
    }
    Ok(json!({
        "root": lowered.root,
        "initial_state": s0,
        "steps": steps.len(),
        "final_state": s3,
        "graph_logical_id": noble_ngraph::ir::hex(&lowered.extraction.checked.graph().logical_id(&BLAKE3)),
        "l1": l1_record,
    }))
}

/// Evaluates the B3 controls and, with a CLI, their ordinary-path outcomes.
///
/// # Errors
/// A CLI failure.
pub fn b3_receipt(cli: Option<&PathBuf>) -> Result<serde_json::Value, String> {
    use crate::controls::{self, Ordinary};
    let mut cases = Vec::new();
    for c in controls::run() {
        let ordinary = match (&c.ordinary, cli) {
            (Ordinary::Session(lines), Some(bin)) => {
                let reports = Cli::new(bin.clone(), Opt::Off)
                    .session(lines)
                    .map_err(|e| format!("{}: {e}", c.name))?;
                let last = reports.last();
                let first_failure = reports.iter().position(|r| !r.ok());
                json!({
                    "submissions": lines,
                    "reports": reports.len(),
                    "first_non_normal": first_failure,
                    "first_non_normal_outcome": first_failure.map(|i| json!({
                        "outcome": reports[i].outcome,
                        "stage": reports[i].stage,
                        "diagnostic": reports[i].diagnostic,
                    })),
                    "last_outcome": last.map(|r| r.outcome.clone()),
                    "last_stack": last.map(|r| format!("{:?}", r.stack)),
                    "last_trace": last.map(|r| r.trace.clone()),
                })
            }
            (Ordinary::Session(lines), None) => {
                json!({ "submissions": lines, "not_run": "no CLI" })
            }
            (Ordinary::NotExpressible(why), _) => json!({ "not_expressible": why }),
            (Ordinary::LiveSlot { sources, requests }, Some(bin)) => {
                let authority = live_authority(sources)?;
                let path = std::env::temp_dir().join(format!(
                    "ngraph-b3-{}-{}.json",
                    std::process::id(),
                    c.name
                ));
                std::fs::write(&path, authority.to_string()).map_err(|e| e.to_string())?;
                let reports = l1::live_slot(bin, &path, requests);
                let _ = std::fs::remove_file(&path);
                let reports = reports.map_err(|e| format!("{}: {e}", c.name))?;
                json!({
                    "live_slot_requests": requests,
                    "outcomes": reports.iter().map(|r| r["outcome"].clone()).collect::<Vec<_>>(),
                    "last_stack": reports.last().map(|r| r["stack"].clone()),
                })
            }
            (Ordinary::LiveSlot { requests, .. }, None) => {
                json!({ "live_slot_requests": requests, "not_run": "no CLI" })
            }
        };
        cases.push(json!({
            "name": c.name,
            "subject": c.subject,
            "expected": c.expected,
            "observed": c.observed(),
            "detail": c.result.as_ref().err().map(|d| d.to_string()),
            "budget": c.result.as_ref().err().and_then(|d| d.budget),
            "graph_nodes_evaluated": 0,
            "ordinary": ordinary,
        }));
    }
    Ok(json!(cases))
}

fn sha256_text(text: &str) -> Result<String, String> {
    use std::io::Write as _;
    let mut child = std::process::Command::new("sha256sum")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("sha256sum: {e}"))?;
    child
        .stdin
        .take()
        .ok_or("no stdin")?
        .write_all(text.as_bytes())
        .map_err(|e| e.to_string())?;
    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    String::from_utf8_lossy(&out.stdout)
        .split_whitespace()
        .next()
        .map(str::to_owned)
        .ok_or_else(|| "sha256sum produced nothing".to_owned())
}

/// Host authority for the live-slot controls: one pure `I64 -- I64` slot
/// `inc`, dispatch and publish granted, exact source allowlist.
fn live_authority(sources: &[(&'static str, String)]) -> Result<serde_json::Value, String> {
    let allow = sources
        .iter()
        .map(|(id, s)| Ok(json!({ "id": id, "sha256": sha256_text(s)? })))
        .collect::<Result<Vec<_>, String>>()?;
    Ok(json!({
        "owner": "ngraph-b3", "quota": 8, "effects": ["live.dispatch"],
        "grants": [
            { "operation": "publish", "slotId": "inc", "allowed": true },
            { "operation": "dispatch", "slotId": "inc", "allowed": true },
        ],
        "resources": [],
        "slots": [{ "slotId": "inc", "input": ["I64"], "output": ["I64"], "effectCeiling": [], "proofRequired": false }],
        "sources": allow,
    }))
}
