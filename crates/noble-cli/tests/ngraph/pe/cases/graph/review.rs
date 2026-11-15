use super::*;

pub(crate) fn case_10() -> TestResult {
    let file = "verification/ngraph/pe-2026-11-13.json";
    let bytes = std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../")
            .join(file),
    )?;
    let retained: Value = serde_json::from_slice(&bytes)?;
    assert_eq!(retained["schema"], "noble-pe-measurement/v1");
    assert_eq!(retained["issue"], 32);
    assert_eq!(retained["clean_source"], true);
    let rows = retained["records"]
        .as_array()
        .ok_or("measurement records")?;
    assert!(!rows.is_empty());
    for row in rows {
        for field in [
            "original_nodes",
            "residual_nodes",
            "folded_nodes",
            "pruned_nodes",
            "retained_original",
            "retained_residual",
        ] {
            assert!(row[field].as_u64().is_some(), "{field}: {row}");
        }
        for side in ["l3_original", "l3_residual", "l4_original", "l4_residual"] {
            assert!(
                row[side].as_u64().is_some_and(|value| value > 0),
                "{side}: {row}"
            );
            for timer in ["cold_eval_ns", "warm_eval_ns_total"] {
                assert!(
                    row[timer][side].as_u64().is_some_and(|value| value > 0),
                    "{timer}.{side}: {row}"
                );
            }
        }
        assert!(row["vectors"].as_u64().is_some_and(|n| n > 0));
        assert!(row["warm_vectors"].as_u64().is_some_and(|n| n > 0));
    }
    // Retained historical timing is not comparable as equal fuel or as an
    // instrument-normalized performance figure on this later source commit.
    let fields = [
        "original_nodes",
        "residual_nodes",
        "folded_nodes",
        "pruned_nodes",
        "per-side-evaluator-visits",
        "per-side-retained-bytes",
        "per-side-cold-latency",
        "per-side-warm-batch-latency",
    ];
    let registered = case_row(10)?;
    assert_eq!(registered["expected"]["fields"], serde_json::json!(fields));
    assert_eq!(
        registered["expected"]["equal_fuel_or_normalized_figure"],
        false
    );
    case_observation(
        10,
        vec![serde_json::json!({
            "kind":"review", "name":"retained-measurement-record", "exit":null,
        })],
        serde_json::json!({
            "fields":fields, "equal_fuel_or_normalized_figure":false,
            "measurement_file":file, "measurement_records":rows.len(),
        }),
    )
}
