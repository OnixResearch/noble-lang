//! The selected CLI binds saved Program origins to actual source, capture, and independent owners.

#[test]
fn selected_origin_owner_runs_with_independent_capture_and_releases() {
    let node = "/nix/store/sy0c7j0npsq33d9zhnnzvjnzc52f4y0p-nodejs-24.13.0/bin/node";
    let script = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/live_slot_selected_origin.mjs");
    let output = std::process::Command::new(node)
        .arg(script)
        .arg(env!("CARGO_BIN_EXE_noble"))
        .output()
        .expect("launch selected live-slot source-origin child");
    assert!(
        output.status.success(),
        "selected source origin failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}
