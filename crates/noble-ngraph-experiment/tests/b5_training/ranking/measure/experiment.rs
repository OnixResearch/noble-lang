use super::*;

#[test]
#[ignore = "runs the frozen 480-task training/evaluation experiment and writes retained evidence"]
pub(crate) fn frozen_trial_run() -> Result<(), String> {
    let provenance = RunProvenance {
        source_commit: command_text("git", &["rev-parse", "HEAD"])?,
        rustc_version: command_text("rustc", &["--version", "--verbose"])?,
        cargo_version: command_text("cargo", &["--version"])?,
    };
    let dirty = command_text("git", &["status", "--porcelain"])?;
    if !dirty.is_empty() {
        return Err(format!(
            "measurement requires a clean committed tree; status was {dirty}"
        ));
    }
    let output_parent = std::env::temp_dir().join("noble-tchunk-ranking");
    std::fs::create_dir_all(&output_parent).map_err(|error| error.to_string())?;
    let output_dir = output_parent.join(format!(
        "run-{}",
        provenance
            .source_commit
            .get(..12)
            .ok_or("short source commit")?
    ));
    let manifest = snapshot();
    let frozen: Value = serde_json::from_str(FROZEN_CORPUS)
        .map_err(|error| format!("frozen corpus JSON is invalid: {error}"))?;
    if manifest != frozen {
        return Err(String::from(
            "generator differs from the committed frozen corpus",
        ));
    }
    let session = noble_contracts::source::Session::without_test_hosts();
    let tasks = generate();
    let mut libraries = Vec::with_capacity(FAMILY_NAMES.len());
    for family in 0..FAMILY_NAMES.len() {
        libraries.push(family_library(&session, family)?);
    }
    debug_assert_eq!(tasks.len(), 480);
    debug_assert_eq!(libraries.len(), FAMILY_NAMES.len());

    let fit = train_and_select(&session, &libraries, &tasks)?;
    let artifact = model_artifact(&fit, &provenance)?;
    let results = run_results(&tasks, &libraries, &artifact)?;
    std::fs::create_dir(&output_dir)
        .map_err(|error| format!("measurement output must be fresh: {error}"))?;
    write_json_lines(
        &output_dir.join("training.jsonl"),
        &results.training_records,
    )?;
    write_json_lines(&output_dir.join("results.jsonl"), &results.result_records)?;
    write_json_pretty(&output_dir.join("model.json"), &model_json(&artifact)?)?;
    let selection = selection_json(&artifact);
    write_json_pretty(&output_dir.join("selection.json"), &selection)?;
    write_json_pretty(
        &output_dir.join("receipt.json"),
        &record(&artifact, &results, &selection),
    )?;
    println!(
        "tchunk ranking measurement retained: {}",
        output_dir.display()
    );
    Ok(())
}
