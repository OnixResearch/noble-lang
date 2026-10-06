//! Canonical LSLOT-03/06 selected-CLI cases, with per-attempt raw JSONL under /tmp.
//! Unsupported source effects and unrepresentable ref serialization are reported
//! as partial/non-run, never silently counted as successful negative executions.

const NODE: &str = "/nix/store/sy0c7j0npsq33d9zhnnzvjnzc52f4y0p-nodejs-24.13.0/bin/node";

fn run_selected_cases(script: &str) {
    let script = format!("{}/tests/{script}", env!("CARGO_MANIFEST_DIR"));
    let output = std::process::Command::new(NODE)
        .arg(script)
        .arg(env!("CARGO_BIN_EXE_noble"))
        .output()
        .expect("launch pinned Node over genuine noble CLI child cases");
    assert!(output.status.success(),
        "selected CLI cases failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr));
}

#[test]
fn canonical_ordered_nominal_resource_admission_uses_real_cli_and_epoch_four() {
    run_selected_cases("live_slot_resource_ref_admission.mjs");
}

#[test]
fn canonical_borrowed_reference_static_and_invocation_boundaries_use_real_cli() {
    run_selected_cases("live_slot_resource_ref_static.mjs");
}
