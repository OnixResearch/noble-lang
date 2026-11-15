use super::*;

struct ExpectedResidual<'a> {
    name: &'a str,
    original: &'a Path,
    static_map: &'a str,
    expected_map: Value,
    inputs: &'a [noble_kernel::types::Ty],
}

fn checked_residual(
    scratch: &Scratch,
    case: ExpectedResidual<'_>,
) -> Result<Value, Box<dyn std::error::Error>> {
    let ExpectedResidual {
        name,
        original,
        static_map,
        expected_map,
        inputs,
    } = case;
    let source = scratch.path(&format!("{name}.residual"));
    let residual_graph = scratch.path(&format!("{name}.ngraph"));
    let run = scratch.run(
        &specialize(original, static_map, &source)
            .arg("--graph-out")
            .arg(&residual_graph),
    )?;
    let report = expect_normal(&run, "specialize")?;
    assert_eq!(
        report["accounting"],
        noble_ngraph::specialize::ACCOUNTING_REVISION
    );
    assert_eq!(report["static_inputs"], expected_map);
    for key in [
        "graph",
        "residual_graph",
        "readmitted_graph",
        "source_blake3",
    ] {
        assert!(hex64(&report[key]), "{name}: missing {key}: {report}");
    }
    for key in [
        "original_nodes",
        "folded_nodes",
        "pruned_nodes",
        "residual_nodes",
        "readmitted_nodes",
        "variants",
    ] {
        assert!(
            report[key].as_u64().is_some(),
            "{name}: missing {key}: {report}"
        );
    }
    assert_eq!(report["variants"], 1);
    assert_eq!(report["residual_nodes"], report["readmitted_nodes"]);
    let original_checked =
        expect_normal(&scratch.run(&Ngraph::new("check").arg(original))?, "check")?;
    assert_eq!(report["graph"], original_checked["graph"]);
    let residual_checked = expect_normal(
        &scratch.run(&Ngraph::new("check").arg(&residual_graph))?,
        "check",
    )?;
    assert_eq!(report["residual_graph"], residual_checked["graph"]);
    let published_source = std::fs::read_to_string(&source)?;
    assert_eq!(
        report["source_blake3"],
        blake3::hash(published_source.as_bytes())
            .to_hex()
            .to_string()
    );
    let readmitted = noble_ngraph::specialize::readmit(
        &TestIds,
        &published_source,
        inputs,
        noble_contracts::Limits::default(),
        &noble_ngraph::check::Budgets::default(),
    )?;
    assert_eq!(
        report["readmitted_graph"],
        blake3::Hash::from(readmitted.id()).to_hex().to_string()
    );
    assert_published(&source);
    cli_observation(name, &run, &source)
}

pub(super) fn case_01() -> TestResult {
    specialize_publishes_a_re_admitted_residual_program()?;
    let scratch = Scratch::new("pe-case01")?;
    let authored = scratch.write("authored.ngraph", PE_GRAPH)?;
    let authored_run = checked_residual(
        &scratch,
        ExpectedResidual {
            name: "authored-flat-i64-controls",
            original: &authored,
            static_map: "0=5",
            expected_map: serde_json::json!([5, null]),
            inputs: &[noble_kernel::types::Ty::I64],
        },
    )?;
    let core = scratch.write("core.noble", "dup +")?;
    let extracted = scratch.path("extracted.ngraph");
    expect_normal(
        &scratch.run(&extract_command(&core, "i64", &extracted))?,
        "extract",
    )?;
    let extracted_run = checked_residual(
        &scratch,
        ExpectedResidual {
            name: "extracted-flat-i64-Core-roots",
            original: &extracted,
            static_map: "0=5",
            expected_map: serde_json::json!([5]),
            inputs: &[],
        },
    )?;
    let fields = [
        "accounting_revision",
        "original_identity",
        "residual_identity",
        "static_map",
        "node_counts",
    ];
    let canonical = case_row(1)?;
    assert_eq!(
        canonical["expected"]["report_fields"],
        serde_json::json!(fields)
    );
    case_observation(
        1,
        vec![authored_run, extracted_run],
        serde_json::json!({
            "checked_residuals":2, "report_fields":fields, "static_map_verified":true,
        }),
    )
}
