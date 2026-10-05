//! Host-owned slot transitions and checked CLI replay in selected Node/V8.
//! These tests do not claim an independently checked LSLOT05 proof.

#[test]
fn host_registry_pins_authorizes_and_retires_real_versions() {
    let node = "/nix/store/sy0c7j0npsq33d9zhnnzvjnzc52f4y0p-nodejs-24.13.0/bin/node";
    let script = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/live_slot_registry.mjs");
    let output = std::process::Command::new(node)
        .arg(script)
        .output()
        .expect("launch selected Node/V8 host registry child");
    assert!(
        output.status.success(),
        "host registry transitions failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}

#[test]
fn cli_scripted_effect_failure_counts_no_real_replay_operations() {
    let node = "/nix/store/sy0c7j0npsq33d9zhnnzvjnzc52f4y0p-nodejs-24.13.0/bin/node";
    let script = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/live_slot_replay_failure.mjs"
    );
    let output = std::process::Command::new(node)
        .arg(script)
        .arg(env!("CARGO_BIN_EXE_noble"))
        .output()
        .expect("launch selected CLI conditional replay child");
    assert!(
        output.status.success(),
        "checked CLI scripted replay failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}
