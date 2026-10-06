//! A direct typed quote must retain its actual boxed capture as a usable saved Program.

#[test]
fn direct_quote_owner_can_run_and_release_without_retaining_code() {
    let node = "/nix/store/sy0c7j0npsq33d9zhnnzvjnzc52f4y0p-nodejs-24.13.0/bin/node";
    let script = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/live_slot_direct_quote.mjs");
    let output = std::process::Command::new(node)
        .arg(script)
        .arg(env!("CARGO_BIN_EXE_noble"))
        .output()
        .expect("launch selected live-slot quote-owner child");
    assert!(
        output.status.success(),
        "selected direct quote failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}
