#![feature(register_tool)]
#![register_tool(tigerstyle)]
#![allow(
    tigerstyle::no_unwrap,
    tigerstyle::underscore_in_module_filename,
    reason = "Owner: noble-maintainers; this integration harness constructs boundary scenarios and asserts their outcomes, so terse fixture names, unwraps and long setup legs are the readable form."
)]
//! Real selected-CLI coverage of transitive nested roots (canonical LSLOT-02):
//! one root keeps the map epoch it pinned while a nested parent/child chain
//! dispatches, and a replacement map is observed only by the next root. This
//! lane was previously run only by the ad hoc `verification/live-slot-current-source`
//! gate, which accepted a failing exit; a cargo test now requires it to pass.

#[test]
fn nested_slot_roots_keep_one_pinned_map_epoch() {
    let output = std::process::Command::new(
        "/nix/store/sy0c7j0npsq33d9zhnnzvjnzc52f4y0p-nodejs-24.13.0/bin/node",
    )
    .arg(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/live_slot_nested_roots.mjs"
    ))
    .arg(env!("CARGO_BIN_EXE_noble"))
    .output()
    .expect("launch selected Node/V8 nested-root child");
    assert!(
        output.status.success(),
        "checked CLI nested-root witness failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}
