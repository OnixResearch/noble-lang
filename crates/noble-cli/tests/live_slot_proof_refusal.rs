#![feature(register_tool)]
#![register_tool(tigerstyle)]
#![allow(
    tigerstyle::no_unwrap,
    tigerstyle::underscore_in_module_filename,
    reason = "Owner: noble-maintainers; this integration harness constructs boundary scenarios and asserts their outcomes, so terse fixture names, unwraps and long setup legs are the readable form."
)]
//! Real selected-CLI coverage of the canonical LSLOT-05 proof-required refusal:
//! a slot demanding independently checked target evidence refuses a publication
//! that lacks it, while the same candidate publishes into an ordinary control
//! slot. This lane was previously run only by the ad hoc
//! `verification/live-slot-current-source` gate, which accepted a failing exit; a
//! cargo test now requires it to pass.

#[test]
fn a_proof_required_slot_refuses_a_publication_without_target_evidence() {
    let output = std::process::Command::new(
        "/nix/store/sy0c7j0npsq33d9zhnnzvjnzc52f4y0p-nodejs-24.13.0/bin/node",
    )
    .arg(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/live_slot_proof_refusal.mjs"
    ))
    .arg(env!("CARGO_BIN_EXE_noble"))
    .output()
    .expect("launch selected Node/V8 proof-required refusal child");
    assert!(
        output.status.success(),
        "checked CLI proof-required refusal witness failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}
