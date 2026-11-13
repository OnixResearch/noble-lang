#![feature(register_tool)]
#![register_tool(tigerstyle)]
#![allow(
    tigerstyle::assertion_density,
    tigerstyle::ambient_env,
    tigerstyle::no_panic,
    tigerstyle::no_unwrap,
    tigerstyle::non_trait_imports,
    tigerstyle::underscore_in_module_filename,
    reason = "Owner: noble-maintainers; this integration harness constructs boundary scenarios and asserts their outcomes, so terse fixture names, unwraps and long setup legs are the readable form."
)]
//! Phase B B2 (default tier): L3 ≡obs L4 over extracted `I64` graphs —
//! the design's named vectors and seeded random Core programs.
//!
//! The full campaign (L1 real CLI `--opt off|on`, L2 Node BigInt oracle,
//! ≥10,000 vectors per graph) is the `ngraph-phase-b-extract` harness; its
//! receipt is `verification/ngraph/phase-b-extract.json`. The ignored test below
//! runs L2 when `NOBLE_NODE` names a Node installation:
//! `NOBLE_NODE=… cargo test -p noble-ngraph-experiment --test extract_b2 -- --ignored`.

use noble_ngraph::prng::SplitMix64;
use noble_ngraph::sem::{obs_eq_all, Value};
use noble_ngraph_experiment::b2i64::{random, Lowered, Oracle};

/// The L1 adapter's attribution and completion checks against fixture children (issue #75).
/// The module is test-only; `cfg(test)` states that scope, as Octet's test-context
/// rules read it from the attribute rather than from the harness-expanded `#[test]`.
#[cfg(test)]
#[path = "attribution/mod.rs"]
mod attribution;

fn i64s(v: &[i64]) -> Vec<Value> {
    v.iter().map(|x| Value::I64(*x)).collect()
}

#[test]
fn design_vectors_agree_on_l3_and_l4() {
    type Case = (
        &'static [&'static str],
        &'static str,
        usize,
        &'static [i64],
        &'static [i64],
    );
    let cases: &[Case] = &[
        (&[], "-", 2, &[5, 3], &[2]),
        (&[], "5 3 -", 0, &[], &[2]),
        (&[], "+", 2, &[i64::MAX, 1], &[i64::MIN]),
        (&[], "9223372036854775807 1 +", 0, &[], &[i64::MIN]),
        (&[], "*", 2, &[i64::MIN, -1], &[i64::MIN]),
        (&[], "-9223372036854775808 -1 *", 0, &[], &[i64::MIN]),
        (&[], "swap [ * ] dip + dup", 3, &[2, 3, 5], &[13, 13]),
    ];
    for (defs, root, inputs, v, expected) in cases {
        let defs: Vec<String> = defs.iter().map(|d| (*d).to_owned()).collect();
        let x = Lowered::new(&defs, root, *inputs).expect("extracts");
        assert_eq!(x.l3(&i64s(v)).expect("L3"), i64s(expected), "{root}");
        assert_eq!(x.l4(&i64s(v)).expect("L4"), i64s(expected), "{root}");
    }
}

#[test]
fn seeded_random_programs_extract_and_agree_on_l3_and_l4() {
    let seed = 0x1404_b201;
    let mut rng = SplitMix64::new(seed);
    for i in 0..48 {
        let p = random::program(seed, i);
        let x = Lowered::new(&p.definitions, &p.root, p.inputs)
            .unwrap_or_else(|e| panic!("program {i} {p:?}: {e}"));
        for _ in 0..200 {
            let v: Vec<Value> = (0..p.inputs)
                .map(|_| Value::I64(random::value(&mut rng)))
                .collect();
            let (a, b) = (x.l3(&v).expect("L3"), x.l4(&v).expect("L4"));
            assert!(
                obs_eq_all(&a, &b),
                "program {i} {p:?} on {v:?}: {a:?} vs {b:?}"
            );
        }
    }
}

#[test]
#[ignore = "needs NOBLE_NODE (pinned Node) for the L2 BigInt oracle"]
fn l2_node_bigint_oracle_agrees_with_l3() {
    let node = std::env::var("NOBLE_NODE").expect("NOBLE_NODE");
    let oracle = Oracle {
        node: format!("{node}/bin/node").into(),
    };
    let seed = 0x1404_b202;
    let mut rng = SplitMix64::new(seed);
    for i in 0..8 {
        let p = random::program(seed, i);
        let x = Lowered::new(&p.definitions, &p.root, p.inputs).expect("extracts");
        let vectors: Vec<Vec<Value>> = (0..1000)
            .map(|_| {
                (0..p.inputs)
                    .map(|_| Value::I64(random::value(&mut rng)))
                    .collect()
            })
            .collect();
        let l2 = oracle
            .evaluate(x.extraction.checked.graph(), &vectors)
            .expect("oracle");
        for (v, cells) in vectors.iter().zip(&l2) {
            let l3: Vec<String> = x
                .l3(v)
                .expect("L3")
                .into_iter()
                .map(noble_ngraph_experiment::b2i64::cell)
                .collect();
            assert_eq!(&l3, cells, "program {i} on {v:?}");
        }
    }
}

// ---------------------------------------------------------------------------
// Issue #32: bounded static-input specialization (#partial-evaluation-v1).
// The differential compares the original under merged inputs, the checked
// residual, and the graph re-admitted from the residual's emitted source, on
// both evaluators. Agreement is finite differential evidence; it is not a
// proof of `specialize_sound`, of Rust/model correspondence or of Program
// refinement, and it claims no equal budget between the two sides.
// ---------------------------------------------------------------------------

fn pe_vectors(seed: u64, dynamic: usize, count: usize) -> Vec<Vec<i64>> {
    let mut rng = SplitMix64::new(seed);
    let edges = [0i64, 1, -1, i64::MAX, i64::MIN, 2, -3, 7];
    let mut rows = Vec::with_capacity(count);
    for i in 0..count {
        let mut row = Vec::with_capacity(dynamic);
        for _ in 0..dynamic {
            row.push(if i % 8 == 0 {
                edges[(i / 8) % edges.len()]
            } else {
                random::value(&mut rng)
            });
        }
        rows.push(row);
    }
    rows
}

#[test]
fn pe_model_fixtures_agree_across_residual_and_readmitted_graph() {
    type Map = Vec<Option<i64>>;
    let cases: Vec<(&str, usize, Vec<Map>)> = vec![
        (
            "dup * swap 3 * +",
            2,
            vec![
                vec![None, None],
                vec![Some(5), None],
                vec![None, Some(7)],
                vec![Some(6), Some(7)],
            ],
        ),
        (
            "-",
            2,
            vec![vec![None, None], vec![Some(10), None], vec![None, Some(-4)]],
        ),
        ("9223372036854775807 1 +", 0, vec![vec![]]),
        ("2 + 3 +", 1, vec![vec![None], vec![Some(4)]]),
        ("3 =", 1, vec![vec![None], vec![Some(3)], vec![Some(4)]]),
        ("dup dup * *", 1, vec![vec![Some(2)], vec![None]]),
    ];
    for (root, inputs, maps) in cases {
        let lowered = Lowered::new(&[], root, inputs).expect("extracts");
        for map in &maps {
            let dynamic = map.iter().filter(|entry| entry.is_none()).count();
            let rows = pe_vectors(0x3200 + inputs as u64, dynamic, 1000);
            let counts = pe::differential(&lowered, map, &rows)
                .unwrap_or_else(|e| panic!("{root} {map:?}: {e}"));
            assert!(
                counts.agree,
                "{root} {map:?}: {}",
                counts.first_mismatch.unwrap_or_default()
            );
            assert_eq!(counts.vectors, 1000, "{root} {map:?}");
            // The original consumes one input per leaf and the residual one per
            // dynamic leaf; both are reported, neither is normalized.
            assert!(counts.l3_original >= counts.l3_residual, "{root} {map:?}");
        }
    }
}

#[test]
fn pe_seeded_core_programs_agree_across_residual_and_readmitted_graph() {
    let seed = 0x1404_32a0;
    let mut rng = SplitMix64::new(seed);
    let mut compared = 0usize;
    for i in 0..16 {
        let program = random::program(seed, i);
        let lowered = Lowered::new(&program.definitions, &program.root, program.inputs)
            .unwrap_or_else(|e| panic!("program {i} {program:?}: {e}"));
        let identity = vec![None; program.inputs];
        let mut interleaved = vec![None; program.inputs];
        for leaf in (0..program.inputs).step_by(2) {
            interleaved[leaf] = Some(random::value(&mut rng));
        }
        for map in [identity, interleaved] {
            let dynamic = map.iter().filter(|entry| entry.is_none()).count();
            let rows = pe_vectors(seed ^ i as u64, dynamic, 200);
            let counts = pe::differential(&lowered, &map, &rows)
                .unwrap_or_else(|e| panic!("program {i} {program:?} {map:?}: {e}"));
            assert!(
                counts.agree,
                "program {i} {program:?} {map:?}: {}",
                counts.first_mismatch.unwrap_or_default()
            );
            compared += 1;
        }
    }
    assert_eq!(compared, 32, "sixteen programs, two static maps each");
}

// NGRAPH-PE-06: an effectful, resource-interface or recursive subject is
// refused before any graph exists, so `specialize` — whose only entry takes a
// checked graph — is unreachable for it.
#[test]
fn pe_refused_subjects_never_reach_specialization() {
    let cases = noble_ngraph_experiment::controls::run();
    for (name, code) in [
        ("test.emit", "E-EFFECT"),
        ("resource-input", "E-RESOURCE"),
        ("self-recursion", "E-RECURSION"),
    ] {
        let case = cases
            .iter()
            .find(|case| case.name == name)
            .unwrap_or_else(|| panic!("missing control {name}"));
        assert_eq!(case.observed(), code, "{name}: {case:?}");
        assert!(case.result.is_err(), "{name} produced a graph");
    }
    // The same effectful and control sources cannot be lowered at all.
    assert!(Lowered::new(&[], "1 + \"hi\" test.emit", 1).is_err());
    assert!(Lowered::new(&[], "drop true [ 1 ] [ 2 ] if", 1).is_err());
    assert!(Lowered::new(&[], "drop [ 1 + ]", 1).is_err());
}

// #32 measurement receipt. Run from a clean committed tree with the output
// path outside the repository:
//   NOBLE_PE_RECEIPT=<absent path> NOBLE_PE_SOURCE_REV=<commit> \
//     cargo test -p noble-ngraph-experiment --test extract_b2 -- --ignored \
//     pe_measurement_receipt
#[test]
#[ignore = "writes the issue #32 measurement receipt when NOBLE_PE_RECEIPT names an absent path"]
fn pe_measurement_receipt() {
    let path = std::env::var("NOBLE_PE_RECEIPT").expect("NOBLE_PE_RECEIPT");
    let revision = std::env::var("NOBLE_PE_SOURCE_REV").expect("NOBLE_PE_SOURCE_REV");
    assert!(
        !std::path::Path::new(&path).exists(),
        "the receipt path must be absent: {path}"
    );
    let seed = 0x1404_32a0;
    let mut rng = SplitMix64::new(seed);
    let mut records = Vec::new();
    let mut totals = serde_json::json!({
        "vectors": 0u64,
        "l3_original": 0u64, "l4_original": 0u64,
        "l3_residual": 0u64, "l4_residual": 0u64,
        "l3_readmitted": 0u64, "l4_readmitted": 0u64,
        "specialize_ns": 0u64,
    });
    let cases: Vec<(&str, usize, Vec<Vec<Option<i64>>>)> = vec![
        (
            "dup * swap 3 * +",
            2,
            vec![
                vec![None, None],
                vec![Some(5), None],
                vec![None, Some(7)],
                vec![Some(6), Some(7)],
            ],
        ),
        ("-", 2, vec![vec![None, None], vec![Some(10), None]]),
        ("9223372036854775807 1 +", 0, vec![vec![]]),
        ("2 + 3 +", 1, vec![vec![None], vec![Some(4)]]),
        ("3 =", 1, vec![vec![None], vec![Some(3)]]),
    ];
    for (root, inputs, maps) in cases {
        let lowered = Lowered::new(&[], root, inputs).expect("extracts");
        for map in maps {
            let dynamic = map.iter().filter(|entry| entry.is_none()).count();
            let rows = pe_vectors(0x3200 + inputs as u64, dynamic, 1000);
            let started = std::time::Instant::now();
            let specialized = lowered.specialized(&map).expect("specialized");
            let specialize_ns = u64::try_from(started.elapsed().as_nanos()).unwrap_or(u64::MAX);
            let counts = pe::differential(&lowered, &map, &rows).expect("differential");
            assert!(
                counts.agree,
                "{root} {map:?}: {}",
                counts.first_mismatch.unwrap_or_default()
            );
            assert_eq!(counts.vectors, 1000);
            let report = &specialized.residual.report;
            records.push(serde_json::json!({
                "root": root,
                "inputs": inputs,
                "static_inputs": map,
                "accounting": report.accounting,
                "original_graph": noble_ngraph::ir::hex(&report.original_graph),
                "residual_graph": noble_ngraph::ir::hex(&report.residual_graph),
                "readmitted_graph": noble_ngraph::ir::hex(&specialized.readmitted.id()),
                "original_nodes": report.original_nodes,
                "folded_nodes": report.folded_nodes,
                "pruned_nodes": report.pruned_nodes,
                "residual_nodes": report.residual_nodes,
                "variants": report.variants,
                "vectors": counts.vectors,
                "l3_original": counts.l3_original,
                "l4_original": counts.l4_original,
                "l3_residual": counts.l3_residual,
                "l4_residual": counts.l4_residual,
                "l3_readmitted": counts.l3_readmitted,
                "l4_readmitted": counts.l4_readmitted,
                "retained_original": counts.retained_original,
                "retained_residual": counts.retained_residual,
                "retained_readmitted": counts.retained_readmitted,
                "specialize_ns": specialize_ns,
                "source_bytes": specialized.source.len(),
                "agree": counts.agree,
            }));
            for (key, value) in [
                ("vectors", counts.vectors),
                ("l3_original", counts.l3_original),
                ("l4_original", counts.l4_original),
                ("l3_residual", counts.l3_residual),
                ("l4_residual", counts.l4_residual),
                ("l3_readmitted", counts.l3_readmitted),
                ("l4_readmitted", counts.l4_readmitted),
            ] {
                let current = totals[key].as_u64().unwrap_or(0);
                totals[key] = serde_json::json!(current.saturating_add(value));
            }
            let timed = totals["specialize_ns"].as_u64().unwrap_or(0);
            totals["specialize_ns"] = serde_json::json!(timed.saturating_add(specialize_ns));
        }
    }
    let receipt = serde_json::json!({
        "schema": "noble-pe-measurement/v1",
        "issue": 32,
        "change": "partial-evaluation-v1",
        "source_revision": revision,
        "accounting": noble_ngraph::specialize::ACCOUNTING_REVISION,
        "claims": {
            "model_proof": "specialize_sound in proofs/foundation/NobleFoundation/Specialize.lean covers packed-graph values for add/sub/mul; it does not cover the I64Eq fold, pruning, this Rust implementation or any Program",
            "rust_correspondence": "none claimed",
            "runtime": "measured counts below; no threshold, no normalized budget and no equal-fuel claim",
            "finite_tests": "these records are finite differential evidence only",
        },
        "records": records,
        "totals": totals,
    });
    std::fs::write(
        &path,
        serde_json::to_string_pretty(&receipt).expect("encode receipt") + "\n",
    )
    .expect("write receipt");
}
