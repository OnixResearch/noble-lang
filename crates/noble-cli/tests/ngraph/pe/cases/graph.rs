use super::*;

#[path = "graph/fold.rs"]
mod fold;
#[path = "graph/identity.rs"]
mod identity;
#[path = "graph/review.rs"]
mod review;

pub(super) use fold::{case_05, case_06};
pub(super) use identity::{case_08, case_13, case_14};
pub(super) use review::case_10;

fn evaluate_outputs(
    scratch: &Scratch,
    name: &str,
    typed_graph: (&Path, &str),
    vectors: &[&str],
    reference: bool,
) -> Result<Vec<Value>, Box<dyn std::error::Error>> {
    let (graph, ty) = typed_graph;
    let file = scratch.write(&format!("{name}.vectors"), &vector_file(ty, vectors))?;
    let mut command = Ngraph::new("eval").arg(graph).arg(&file);
    if reference {
        command = command.arg("--reference");
    }
    let run = scratch.run(&command)?;
    let (_header, rows) = eval_lines(&run)?;
    assert_eq!(run.exit, Some(0), "{}", run.describe());
    assert_eq!(rows.len(), vectors.len());
    Ok(rows.into_iter().map(|row| row["outputs"].clone()).collect())
}

pub(super) fn case_03() -> TestResult {
    let scratch = Scratch::new("pe-case03")?;
    let source = scratch.write("original.ngraph", PE_GRAPH)?;
    let original = evaluate_outputs(
        &scratch,
        "original",
        (&source, "i64 i64"),
        &["v 3 5"],
        false,
    )?;
    let residual = scratch.path("residual.noble");
    let graph = scratch.path("residual.ngraph");
    let run = scratch.run(
        &specialize(&source, "1=5", &residual)
            .arg("--graph-out")
            .arg(&graph),
    )?;
    let line = expect_normal(&run, "specialize")?;
    assert_eq!(line["static_inputs"], serde_json::json!([null, 5]));
    assert_eq!(line["inputs"], serde_json::json!(["i64"]));
    assert_published(&graph);
    let dynamic = evaluate_outputs(&scratch, "dynamic", (&graph, "i64"), &["v 3"], false)?;
    assert_eq!(original, dynamic);
    let wrong_source = scratch.path("prefix.noble");
    let wrong_graph = scratch.path("prefix.ngraph");
    expect_normal(
        &scratch.run(
            &specialize(&source, "0=5", &wrong_source)
                .arg("--graph-out")
                .arg(&wrong_graph),
        )?,
        "specialize",
    )?;
    let prefix = evaluate_outputs(&scratch, "prefix", (&wrong_graph, "i64"), &["v 3"], false)?;
    assert_eq!(original, vec![serde_json::json!(["i64:18"])]);
    assert_eq!(prefix, vec![serde_json::json!(["i64:20"])]);
    case_observation(
        3,
        vec![cli_observation("non-prefix-input", &run, &residual)?],
        serde_json::json!({"dynamic_rank_correct": true, "prefix_control_disagrees": true}),
    )
}

struct Compared {
    runs: Vec<Value>,
    vectors: usize,
    graphs: usize,
}

fn compare_root(
    scratch: &Scratch,
    name: &str,
    original: &Path,
) -> Result<Value, Box<dyn std::error::Error>> {
    let source = scratch.path(&format!("{name}.noble"));
    let residual = scratch.path(&format!("{name}.ngraph"));
    let run = scratch.run(
        &specialize(original, "1=4", &source)
            .arg("--graph-out")
            .arg(&residual),
    )?;
    let report = expect_normal(&run, "specialize")?;
    assert_published(&source);
    assert_published(&residual);
    let readmitted = scratch.path(&format!("{name}.readmitted.ngraph"));
    let rechecked = noble_ngraph::specialize::readmit(
        &TestIds,
        &std::fs::read_to_string(&source)?,
        &[noble_kernel::types::Ty::I64],
        noble_contracts::Limits::default(),
        &noble_ngraph::check::Budgets::default(),
    )?;
    assert_eq!(
        report["readmitted_graph"],
        blake3::Hash::from(rechecked.id()).to_hex().to_string()
    );
    scratch.write(
        &format!("{name}.readmitted.ngraph"),
        &noble_ngraph::format::graph::print(rechecked.graph()),
    )?;
    let inputs = ["v 0 4", "v 3 4", "v -7 4"];
    let dynamics = ["v 0", "v 3", "v -7"];
    let mut expected = None;
    for reference in [false, true] {
        let initial = evaluate_outputs(
            scratch,
            &format!("{name}-original-{reference}"),
            (original, "i64 i64"),
            &inputs,
            reference,
        )?;
        let specialized = evaluate_outputs(
            scratch,
            &format!("{name}-residual-{reference}"),
            (&residual, "i64"),
            &dynamics,
            reference,
        )?;
        let rechecked = evaluate_outputs(
            scratch,
            &format!("{name}-readmitted-{reference}"),
            (&readmitted, "i64"),
            &dynamics,
            reference,
        )?;
        assert_eq!(specialized, initial, "{name}: graph residual differs");
        assert_eq!(rechecked, initial, "{name}: ordinary source differs");
        if let Some(prior) = &expected {
            assert_eq!(&initial, prior, "{name}: L3 and L4 disagree");
        }
        expected = Some(initial);
    }
    cli_observation(name, &run, &source)
}

fn compare_both() -> Result<Compared, Box<dyn std::error::Error>> {
    let scratch = Scratch::new("pe-case07-11")?;
    let authored = scratch.write("authored.ngraph", PE_GRAPH)?;
    let core = scratch.write("seeded-core.noble", "swap 3 * +")?;
    let extracted = scratch.path("extracted.ngraph");
    expect_normal(
        &scratch.run(&extract_command(&core, "i64,i64", &extracted))?,
        "extract",
    )?;
    // The runner additionally executes the complete frozen fixture/seeded corpus.
    // These two roots make ordinary readmission and both evaluators directly
    // observable inside each selected CLI case, not only through that corpus.
    let a = compare_root(&scratch, "fixture-graphs", &authored)?;
    let b = compare_root(&scratch, "seeded-Core-programs", &extracted)?;
    Ok(Compared {
        runs: vec![a, b],
        vectors: 6,
        graphs: 2,
    })
}

pub(super) fn case_07() -> TestResult {
    let compared = compare_both()?;
    case_observation(
        7,
        compared.runs,
        serde_json::json!({
            "evaluators": ["L3-reference", "L4-compiled"],
            "compared_vectors": compared.vectors,
            "compared_graphs": compared.graphs,
            "record_compared_vectors_and_graphs": true,
            "observations_agree": true,
        }),
    )
}

pub(super) fn case_11() -> TestResult {
    let compared = compare_both()?;
    case_observation(
        11,
        compared.runs,
        serde_json::json!({
            "readmitted_checked": true,
            "observations_agree": true,
        }),
    )
}
