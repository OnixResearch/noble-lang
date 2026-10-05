//! Real selected-CLI coverage of independent saved-Program quota ownership and
//! source-bound frozen replay receipts. This is behavioral evidence, not LSLOT05 proof.

#[test]
fn pinned_saved_program_quota_and_frozen_replay_boundaries() {
    let output = std::process::Command::new(
        "/nix/store/sy0c7j0npsq33d9zhnnzvjnzc52f4y0p-nodejs-24.13.0/bin/node",
    )
    .arg(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/live_slot_quota_replay.mjs"))
    .arg(env!("CARGO_BIN_EXE_noble"))
    .output()
    .expect("launch selected Node/V8 quota and replay child");
    assert!(
        output.status.success(),
        "checked CLI quota/replay failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}
