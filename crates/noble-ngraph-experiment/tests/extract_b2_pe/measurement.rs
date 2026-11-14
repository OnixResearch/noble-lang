//! Source-bound runtime receipt for checked-graph specialization.

use super::*;

#[path = "timing.rs"]
mod timing;

fn measured_record(root: &str, inputs: usize, map: &[Option<i64>]) -> (serde_json::Value, bool) {
    let prepared = timing::prepare(root, inputs, map);
    let counts =
        pe::differential(&prepared.lowered, map, &prepared.rows).expect("measurement differential");
    assert!(
        counts.agree,
        "{root} {map:?}: {}",
        counts.first_mismatch.unwrap_or_default()
    );
    assert_eq!(counts.vectors, 1000);
    let (cold_eval_ns, warm_eval_ns_total) = timing::evaluation_times(&prepared);
    let report = &prepared.specialized.residual.report;
    let is_work_reduced =
        counts.l3_residual < counts.l3_original && counts.l4_residual < counts.l4_original;
    if root == "dup * swap 3 * +" && map.iter().all(Option::is_some) {
        assert!(is_work_reduced, "all-static fixture reduces L3 and L4 work");
    }
    let record = serde_json::json!({
        "root": root,
        "inputs": inputs,
        "static_inputs": map,
        "accounting": report.accounting,
        "original_graph": noble_ngraph::ir::hex(&report.original_graph),
        "residual_graph": noble_ngraph::ir::hex(&report.residual_graph),
        "readmitted_graph": noble_ngraph::ir::hex(&prepared.specialized.readmitted.id()),
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
        "residual_pipeline_ns": prepared.residual_pipeline_ns,
        "specialization_ns": prepared.specialized.specialization_ns,
        "cold_eval_ns": cold_eval_ns,
        "warm_eval_ns_total": warm_eval_ns_total,
        "warm_vectors": counts.vectors,
        "source_bytes": prepared.specialized.source.len(),
        "runtime_work_reduced": is_work_reduced,
        "agree": counts.agree,
    });
    (record, is_work_reduced)
}

fn empty_totals() -> serde_json::Value {
    serde_json::json!({
        "vectors": 0u64,
        "l3_original": 0u64, "l4_original": 0u64,
        "l3_residual": 0u64, "l4_residual": 0u64,
        "l3_readmitted": 0u64, "l4_readmitted": 0u64,
        "specialization_ns": 0u64,
        "residual_pipeline_ns": 0u64,
        "cold_eval_ns": {
            "l3_original": 0u64, "l4_original": 0u64,
            "l3_residual": 0u64, "l4_residual": 0u64,
            "l3_readmitted": 0u64, "l4_readmitted": 0u64,
        },
        "warm_eval_ns_total": {
            "l3_original": 0u64, "l4_original": 0u64,
            "l3_residual": 0u64, "l4_residual": 0u64,
            "l3_readmitted": 0u64, "l4_readmitted": 0u64,
        },
    })
}

fn add_total(totals: &mut serde_json::Value, key: &str, value: u64) {
    let current = totals[key].as_u64().expect("numeric total");
    totals[key] = serde_json::json!(current.checked_add(value).expect("total fits u64"));
}

fn accumulate_totals(totals: &mut serde_json::Value, record: &serde_json::Value) {
    for key in [
        "vectors",
        "l3_original",
        "l4_original",
        "l3_residual",
        "l4_residual",
        "l3_readmitted",
        "l4_readmitted",
        "specialization_ns",
        "residual_pipeline_ns",
    ] {
        add_total(
            totals,
            key,
            record[key].as_u64().expect("record count is numeric"),
        );
    }
    for key in [
        "l3_original",
        "l4_original",
        "l3_residual",
        "l4_residual",
        "l3_readmitted",
        "l4_readmitted",
    ] {
        for group in ["cold_eval_ns", "warm_eval_ns_total"] {
            let value = record[group][key].as_u64().expect("timing is numeric");
            let current = totals[group][key]
                .as_u64()
                .expect("timing total is numeric");
            totals[group][key] =
                serde_json::json!(current.checked_add(value).expect("timing total fits u64"));
        }
    }
}

fn clean_revision(root: &std::path::Path, revision: &str) {
    let head = std::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["rev-parse", "HEAD"])
        .output()
        .expect("read source commit");
    assert!(head.status.success(), "git rev-parse HEAD succeeded");
    let head = String::from_utf8(head.stdout).expect("HEAD is UTF-8");
    assert_eq!(revision, head.trim(), "receipt source revision is HEAD");
    let status = std::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["status", "--porcelain", "--untracked-files=all"])
        .output()
        .expect("check source-tree cleanliness");
    assert!(status.status.success(), "git status succeeded");
    assert!(status.stdout.is_empty(), "source tree is clean");
}

type FixtureCase = (&'static str, usize, &'static [&'static [Option<i64>]]);

const MEASUREMENT_CASES: &[FixtureCase] = &[
    (
        "dup * swap 3 * +",
        2,
        &[
            &[None, None],
            &[Some(5), None],
            &[None, Some(7)],
            &[Some(6), Some(7)],
        ],
    ),
    ("-", 2, &[&[None, None], &[Some(10), None]]),
    ("9223372036854775807 1 +", 0, &[&[]]),
    ("2 + 3 +", 1, &[&[None], &[Some(4)]]),
    ("3 =", 1, &[&[None], &[Some(3)]]),
];

#[test]
#[ignore = "writes the measurement receipt to the absent NOBLE_PE_RECEIPT path"]
fn source_bound_receipt() {
    let path = std::env::var("NOBLE_PE_RECEIPT").expect("NOBLE_PE_RECEIPT");
    let revision = std::env::var("NOBLE_PE_SOURCE_REV").expect("NOBLE_PE_SOURCE_REV");
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    clean_revision(root, &revision);
    assert!(
        !std::path::Path::new(&path).exists(),
        "receipt path absent: {path}"
    );
    let mut records = Vec::with_capacity(11);
    let mut totals = empty_totals();
    let mut is_observed_work_reduction = false;
    for &(root, inputs, maps) in MEASUREMENT_CASES {
        for &map in maps {
            let (record, is_work_reduced) = measured_record(root, inputs, map);
            is_observed_work_reduction |= is_work_reduced;
            accumulate_totals(&mut totals, &record);
            records.push(record);
        }
    }
    assert!(
        is_observed_work_reduction,
        "a fixture reduces evaluator work"
    );
    let receipt = serde_json::json!({
        "schema": "noble-pe-measurement/v1",
        "issue": 32,
        "change": "partial-evaluation-v1",
        "source_revision": revision,
        "clean_source": true,
        "accounting": noble_ngraph::specialize::ACCOUNTING_REVISION,
        "measurement_protocol": {
            "seed": "0x00000000140432a0",
            "vectors_per_record": 1000,
            "cold_eval_ns": "first single-vector evaluation after graph construction",
            "warm_eval_ns_total": "one 1,000-vector batch after one untimed warm-up",
            "residual_pipeline_ns": "specialization, source emission, ordinary re-admission, and residual evaluator construction",
            "specialization_ns": "checked-graph specialization time reported by the B2 harness",
            "runtime_work_reduced": "both L3 and L4 visit fewer nodes on a residual than the original",
        },
        "claims": {
            "model_proof": "specialize_sound covers packed-graph values for add/sub/mul; it does not cover I64Eq, pruning, Rust specialization or Programs",
            "rust_correspondence": "none claimed",
            "runtime": "measured node visits, evaluator times, retained bytes and residual source size; no threshold, normalized budget or equal-fuel claim",
            "finite_tests": "finite differential evidence only",
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
