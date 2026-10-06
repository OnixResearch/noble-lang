//! A selected CLI late control must not poison the session or leak into the next root.

#[test]
fn late_control_is_refused_without_changing_future_checkpoint_or_publication() {
    let node = "/nix/store/sy0c7j0npsq33d9zhnnzvjnzc52f4y0p-nodejs-24.13.0/bin/node";
    let script = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/live_slot_late_control.mjs");
    let output = std::process::Command::new(node)
        .arg(script)
        .arg(env!("CARGO_BIN_EXE_noble"))
        .output()
        .expect("launch selected late-control CLI child");
    assert!(
        output.status.success(),
        "late control failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}
