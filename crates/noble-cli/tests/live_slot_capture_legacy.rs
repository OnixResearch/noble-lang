//! A saved checked Program and an opt-in slot caller observe distinct target versions.

#[test]
fn saved_capture_is_immutable_while_generic_dispatch_selects_fresh_target() {
    let node = "/nix/store/sy0c7j0npsq33d9zhnnzvjnzc52f4y0p-nodejs-24.13.0/bin/node";
    let script = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/live_slot_capture_legacy.mjs");
    let output = std::process::Command::new(node)
        .arg(script)
        .arg(env!("CARGO_BIN_EXE_noble"))
        .output()
        .expect("launch checked CLI capture-versus-generic child");
    assert!(
        output.status.success(),
        "saved capture/slot publication failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}
