#![feature(register_tool)]
#![register_tool(tigerstyle)]
#![allow(
    tigerstyle::no_unwrap,
    tigerstyle::underscore_in_module_filename,
    reason = "Owner: noble-maintainers; this integration harness constructs boundary scenarios and asserts their outcomes, so terse fixture names, unwraps and long setup legs are the readable form."
)]
//! Real selected-CLI coverage of coherent entry maps: a pinned root dispatches once and performs
//! the effect exactly once, both rows report one pinned map epoch, and a publication that tries to
//! land after the pin is refused rather than duplicated. This is behavioral evidence, not a
//! VC-LIVE-03 proof.

#[test]
fn coherent_entry_maps_dispatch_and_emit_exactly_once() {
    let output = std::process::Command::new(
        "/nix/store/sy0c7j0npsq33d9zhnnzvjnzc52f4y0p-nodejs-24.13.0/bin/node",
    )
    .arg(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/live_slot_coherent_maps.mjs"
    ))
    .arg(env!("CARGO_BIN_EXE_noble"))
    .output()
    .expect("launch selected Node/V8 coherent-map witness child");
    assert!(
        output.status.success(),
        "checked CLI coherent-map witness failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}
