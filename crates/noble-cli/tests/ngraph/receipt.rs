//! Whole-request ceilings, census check and receipt fields of
//! `noble ngraph eval --net` (`docs/i64-net-contract-v3.md` §9,
//! `.cairn/changes/net-request-limits-receipts`). Each ceiling is exercised
//! below, at and above its observed requirement; a refusal is one exhausted
//! line and never a truncated success.

use super::{
    check_envelope, eval_lines, expect_normal, expect_usage, extract_command, hex64, vector_file,
    Ngraph, Run, Scratch, TestResult, I1,
};
use serde_json::Value;
use std::path::{Path, PathBuf};

const THEOREMS: [&str; 3] = [
    "NobleFoundation.NetI64.i64_normal_form_unique",
    "NobleFoundation.NetI64.i64_equal_interaction_count",
    "NobleFoundation.NetI64.Certificate.accepted_exact_count",
];

/// An extracted I1 graph and a vector file with `rows`.
fn fixture(
    scratch: &Scratch,
    rows: &[&str],
) -> Result<(PathBuf, PathBuf), Box<dyn std::error::Error>> {
    let source = scratch.write("i1.noble", I1)?;
    let graph = scratch.path("i1.ngraph");
    expect_normal(
        &scratch.run(&extract_command(&source, "i64,i64", &graph))?,
        "extract",
    )?;
    let vectors = scratch.write("i1.vectors", &vector_file("i64 i64", rows))?;
    Ok((graph, vectors))
}

fn net(graph: &Path, vectors: &Path, flags: &[(&str, String)]) -> Ngraph {
    let mut command = Ngraph::new("eval").arg(graph).arg(vectors).arg("--net");
    for (flag, value) in flags {
        command = command.arg(*flag).arg(value);
    }
    command
}

fn capped(graph: &Path, vectors: &Path, flag: &str, cap: u64) -> Ngraph {
    net(graph, vectors, &[(flag, cap.to_string())])
}

/// The single exhausted refusal of `limit`, with its cap and observed amount.
fn refusal(run: &Run, stage: &str, limit: &str) -> Result<Value, Box<dyn std::error::Error>> {
    let mut lines = run.reports()?;
    assert_eq!(
        lines.len(),
        1,
        "no header or partial prefix\n{}",
        run.describe()
    );
    let line = lines.remove(0);
    check_envelope(&line, "eval");
    assert_eq!(run.exit, Some(5), "{}", run.describe());
    assert_eq!(line["outcome"], "exhausted", "{line}");
    assert_eq!(line["code"], format!("E-NET-BUDGET-{limit}"), "{line}");
    assert_eq!(line["stage"], stage, "{line}");
    assert_eq!(line["limit"], limit, "{line}");
    assert!(line["outputs"].is_null(), "{line}");
    Ok(line)
}

#[test]
fn net_header_binds_producer_contract_limits_and_semantic_count() -> TestResult {
    let scratch = Scratch::new("net-receipt")?;
    let (graph, vectors) = fixture(&scratch, &["v 5 4", "v 8 3", "v -9223372036854775808 -1"])?;
    let run = scratch.run(&net(&graph, &vectors, &[]))?;
    let (header, rows) = eval_lines(&run)?;
    assert_eq!(header["backend"], "net");

    let producer = &header["producer"];
    assert_eq!(producer["package"], "noble-cli");
    assert!(producer["version"].is_string(), "{producer}");
    assert!(
        producer["executable_digest"].is_null(),
        "no fabricated binary digest"
    );
    match producer["source_revision_status"].as_str() {
        Some("build-declared") => {
            let revision = producer["source_revision"].as_str().unwrap_or_default();
            assert!(
                revision.len() == 40 && revision.bytes().all(|b| b.is_ascii_hexdigit()),
                "{producer}"
            );
        }
        Some("unavailable") => assert!(producer["source_revision"].is_null(), "{producer}"),
        other => panic!("source revision status {other:?}"),
    }

    let contract = &header["net_contract"];
    for (key, value) in [
        ("contract", "ngraph-net-i64/3"),
        ("lowering", "ngraph-net-i64-lowering/0"),
        ("rule_table", "noble-net-i64-rules/3"),
        ("plan", "ngraph-net-plan/1"),
        ("representation", "noble-net-arena/1"),
        ("plan_checker", "noble-net-plan-checker/1"),
        ("plan_executor", "noble-net-plan-executor/1"),
        ("schedule", "lifo"),
        ("storage_policy", "reuse"),
        ("identity_scheme", "blake3"),
    ] {
        assert_eq!(contract[key], value, "{contract}");
    }

    let limits = &header["limits"];
    for (key, value) in [
        ("max_input_bytes", 134_217_728u64),
        ("max_retained_bytes", 268_435_456),
        ("max_output_bytes", 67_108_864),
        ("interaction_fuel", 2_097_152),
        ("max_agent_slots", 1_048_576),
        ("max_wires", 2_097_152),
        ("max_worklist_entries", 1_048_576),
    ] {
        assert_eq!(limits[key], value, "{limits}");
    }
    let input = std::fs::metadata(&graph)?.len() + std::fs::metadata(&vectors)?.len();
    assert_eq!(header["input_bytes"], input);

    let count = &header["semantic_count"];
    assert_eq!(count["unit"], "inets-principal-pair-interaction");
    assert_eq!(count["census"], "2*ops+dups+eras");
    assert_eq!(count["count_basis"], "census-checked");
    assert_eq!(count["assurance"], "model-only");
    assert_eq!(count["theorems"], Value::from(THEOREMS.to_vec()));
    assert_eq!(count["lean_system"], "NobleFoundation.NetI64.system");
    let source = &count["proof_source"];
    assert_eq!(
        source["inets_commit"],
        "98bbb01070664e4462a35efa5d62de6f27143572"
    );
    assert_eq!(source["lean_toolchain"], "leanprover/lean4:v4.35.0");
    for key in [
        "module_blake3",
        "module_sha256",
        "inets_blake3",
        "inets_sha256",
        "receipt_blake3",
    ] {
        assert!(hex64(&source[key]), "{key}: {source}");
    }
    let boundary = count["model_boundary"].as_str().unwrap_or_default();
    assert!(boundary.starts_with("model-only"), "{count}");
    assert!(boundary.contains("not a refinement proof"), "{count}");

    let reference = scratch.run(&Ngraph::new("eval").arg(&graph).arg(&vectors))?;
    let (_, reference_rows) = eval_lines(&reference)?;
    assert_eq!(rows.len(), 3);
    for (row, expected) in rows.iter().zip(&reference_rows) {
        assert_eq!(row["outputs"], expected["outputs"], "{row}");
        assert_eq!(row["count_basis"], "census-checked", "{row}");
        assert!(row["interactions"].is_u64(), "{row}");
        assert_eq!(row["interactions"], row["expected_interactions"], "{row}");
        let retained = row["retained_bytes"].as_u64().ok_or("no retained bytes")?;
        let high_water = row["retained_bytes_high_water"]
            .as_u64()
            .ok_or("no retained high water")?;
        assert!(
            high_water > retained,
            "the candidate plus reduction buffers: {row}"
        );
    }
    Ok(())
}

#[test]
fn net_input_ceiling_refuses_below_and_admits_at_and_above_the_input_size() -> TestResult {
    let scratch = Scratch::new("net-input-ceiling")?;
    let (graph, vectors) = fixture(&scratch, &["v 5 4"])?;
    let graph_bytes = std::fs::metadata(&graph)?.len();
    let total = graph_bytes + std::fs::metadata(&vectors)?.len();
    for cap in [total, total + 1] {
        let run = scratch.run(&net(
            &graph,
            &vectors,
            &[("--max-input-bytes", cap.to_string())],
        ))?;
        let (header, rows) = eval_lines(&run)?;
        assert_eq!(run.exit, Some(0), "{}", run.describe());
        assert_eq!(header["input_bytes"], total);
        assert_eq!(header["limits"]["max_input_bytes"], cap);
        assert_eq!(rows.len(), 1);
    }
    let below = scratch.run(&capped(&graph, &vectors, "--max-input-bytes", total - 1))?;
    let line = refusal(&below, "input", "input-bytes")?;
    assert_eq!(line["cap"], total - 1, "{line}");
    assert_eq!(
        line["observed"], total,
        "the vector file is refused before it is read: {line}"
    );
    let none = scratch.run(&net(
        &graph,
        &vectors,
        &[("--max-input-bytes", "0".to_owned())],
    ))?;
    let line = refusal(&none, "input", "input-bytes")?;
    assert_eq!(
        line["observed"], graph_bytes,
        "the graph file is refused first: {line}"
    );
    Ok(())
}

#[test]
fn net_retained_ceiling_refuses_below_and_admits_at_and_above_the_high_water() -> TestResult {
    let scratch = Scratch::new("net-retained-ceiling")?;
    let (graph, vectors) = fixture(&scratch, &["v 5 4"])?;
    let (_, rows) = eval_lines(&scratch.run(&net(&graph, &vectors, &[]))?)?;
    let high_water = rows[0]["retained_bytes_high_water"]
        .as_u64()
        .ok_or("no retained high water")?;
    assert!(high_water > 0);
    for cap in [high_water, high_water + 1] {
        let run = scratch.run(&capped(&graph, &vectors, "--max-retained-bytes", cap))?;
        let (header, bounded) = eval_lines(&run)?;
        assert_eq!(header["limits"]["max_retained_bytes"], cap);
        assert_eq!(bounded[0]["retained_bytes_high_water"], high_water);
        assert_eq!(bounded[0]["outputs"], rows[0]["outputs"]);
    }
    let below = scratch.run(&net(
        &graph,
        &vectors,
        &[("--max-retained-bytes", (high_water - 1).to_string())],
    ))?;
    let line = refusal(&below, "evaluation", "retained-bytes")?;
    assert_eq!(line["cap"], high_water - 1, "{line}");
    assert_eq!(line["observed"], high_water, "{line}");
    assert_eq!(line["vector"], 0, "{line}");
    assert_eq!(line["observation"], "exhausted", "{line}");
    let none = scratch.run(&net(
        &graph,
        &vectors,
        &[("--max-retained-bytes", "0".to_owned())],
    ))?;
    let line = refusal(&none, "evaluation", "retained-bytes")?;
    assert_eq!(
        line["vector"], 0,
        "lowering charges before allocation: {line}"
    );
    Ok(())
}

#[test]
fn net_output_ceiling_refuses_below_and_admits_at_and_above_the_report_size() -> TestResult {
    let scratch = Scratch::new("net-output-ceiling")?;
    let (graph, vectors) = fixture(&scratch, &["v 5 4", "v 8 3"])?;
    // Every cap below has four digits, so the header that reports the cap
    // keeps one length and the report size is the same under each.
    let probe = scratch.run(&net(
        &graph,
        &vectors,
        &[("--max-output-bytes", "9999".to_owned())],
    ))?;
    eval_lines(&probe)?;
    let report_bytes = u64::try_from(probe.stdout.len())?;
    assert!(
        (1001..9999).contains(&report_bytes),
        "fixture assumption: {report_bytes} report bytes"
    );
    for cap in [report_bytes, report_bytes + 1] {
        let run = scratch.run(&net(
            &graph,
            &vectors,
            &[("--max-output-bytes", cap.to_string())],
        ))?;
        let (_, rows) = eval_lines(&run)?;
        assert_eq!(
            u64::try_from(run.stdout.len())?,
            report_bytes,
            "{}",
            run.describe()
        );
        assert_eq!(rows.len(), 2);
    }
    let below = scratch.run(&capped(
        &graph,
        &vectors,
        "--max-output-bytes",
        report_bytes - 1,
    ))?;
    let line = refusal(&below, "output", "output-bytes")?;
    assert_eq!(line["cap"], report_bytes - 1, "{line}");
    assert_eq!(
        line["observed"], report_bytes,
        "the last line is charged before it is built: {line}"
    );
    let none = scratch.run(&net(
        &graph,
        &vectors,
        &[("--max-output-bytes", "0".to_owned())],
    ))?;
    let line = refusal(&none, "output", "output-bytes")?;
    assert!(
        line["observed"]
            .as_u64()
            .is_some_and(|observed| observed > 0),
        "{line}"
    );
    Ok(())
}

#[test]
fn net_rows_carry_the_census_and_stop_one_interaction_short() -> TestResult {
    let scratch = Scratch::new("net-census")?;
    let (graph, vectors) = fixture(&scratch, &["v 5 4"])?;
    let (_, rows) = eval_lines(&scratch.run(&net(&graph, &vectors, &[]))?)?;
    let census = rows[0]["expected_interactions"]
        .as_u64()
        .ok_or("no census")?;
    assert_eq!(rows[0]["interactions"], census);
    let exact = scratch.run(&net(&graph, &vectors, &[("--fuel", census.to_string())]))?;
    let (_, exact_rows) = eval_lines(&exact)?;
    assert_eq!(exact_rows[0]["count_basis"], "census-checked");
    let short = scratch.run(&net(
        &graph,
        &vectors,
        &[("--fuel", (census - 1).to_string())],
    ))?;
    let line = refusal(&short, "evaluation", "fuel")?;
    assert_eq!(line["expected_interactions"], census, "{line}");
    assert_eq!(line["interactions"], census - 1, "{line}");
    assert!(
        line.get("count_basis").is_none(),
        "no count basis without completion: {line}"
    );
    Ok(())
}

#[test]
fn net_ceilings_need_net_and_may_only_lower_their_defaults() -> TestResult {
    let scratch = Scratch::new("net-ceiling-usage")?;
    let (graph, vectors) = fixture(&scratch, &["v 5 4"])?;
    for flag in [
        "--max-input-bytes",
        "--max-retained-bytes",
        "--max-output-bytes",
    ] {
        expect_usage(
            &scratch.run(
                &Ngraph::new("eval")
                    .arg(&graph)
                    .arg(&vectors)
                    .arg(flag)
                    .arg("1"),
            )?,
        );
    }
    for (flag, default) in [
        ("--max-input-bytes", 134_217_728u64),
        ("--max-retained-bytes", 268_435_456),
        ("--max-output-bytes", 67_108_864),
    ] {
        let at = scratch.run(&net(&graph, &vectors, &[(flag, default.to_string())]))?;
        assert_eq!(at.exit, Some(0), "{}", at.describe());
        expect_usage(&scratch.run(&net(&graph, &vectors, &[(flag, (default + 1).to_string())]))?);
        expect_usage(&scratch.run(&net(&graph, &vectors, &[(flag, "-1".to_owned())]))?);
    }
    Ok(())
}
