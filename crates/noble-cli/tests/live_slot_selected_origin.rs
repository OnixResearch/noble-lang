#![feature(register_tool)]
#![register_tool(tigerstyle)]
#![allow(
    tigerstyle::no_unwrap,
    tigerstyle::underscore_in_module_filename,
    reason = "Owner: noble-maintainers; this integration harness constructs boundary scenarios and asserts their outcomes, so terse fixture names, unwraps and long setup legs are the readable form."
)]
//! The selected CLI binds saved Program origins to actual source, capture, and independent owners.
//! A changed root-input capture keeps the definition template and changes the Program value, while
//! a changed fixed literal changes the template (ID-05's capture change).

#[test]
fn selected_origin_owner_runs_with_independent_capture_and_releases() {
    let node = "/nix/store/sy0c7j0npsq33d9zhnnzvjnzc52f4y0p-nodejs-24.13.0/bin/node";
    let script = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/live_slot_selected_origin.mjs"
    );
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

#[test]
fn a_capture_change_keeps_the_template_and_changes_the_program_value() {
    let node = "/nix/store/sy0c7j0npsq33d9zhnnzvjnzc52f4y0p-nodejs-24.13.0/bin/node";
    let script = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/live_slot_capture_identity.mjs"
    );
    let output = std::process::Command::new(node)
        .arg(script)
        .arg(env!("CARGO_BIN_EXE_noble"))
        .output()
        .expect("launch selected live-slot capture identity child");
    assert!(
        output.status.success(),
        "capture identity failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}
