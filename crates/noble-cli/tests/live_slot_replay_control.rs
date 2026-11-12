#![feature(register_tool)]
#![register_tool(tigerstyle)]
#![allow(
    tigerstyle::no_unwrap,
    tigerstyle::underscore_in_module_filename,
    reason = "Owner: noble-maintainers; this integration harness constructs boundary scenarios and asserts their outcomes, so terse fixture names, unwraps and long setup legs are the readable form."
)]
//! Real selected-CLI coverage of a control that commits during a frozen replay: the change keeps
//! its operator receipt, the replay still matches its recorded trace, and the session stays live
//! because the host reads the control events from the replay reply itself. This is behavioral
//! evidence, not a live-slot theorem.

#[test]
fn a_control_that_commits_during_replay_keeps_its_receipt_and_the_session() {
    let output = std::process::Command::new(
        "/nix/store/sy0c7j0npsq33d9zhnnzvjnzc52f4y0p-nodejs-24.13.0/bin/node",
    )
    .arg(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/live_slot_replay_control.mjs"
    ))
    .arg(env!("CARGO_BIN_EXE_noble"))
    .output()
    .expect("launch selected Node/V8 replay-control child");
    assert!(
        output.status.success(),
        "checked CLI replay-control witness failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}
