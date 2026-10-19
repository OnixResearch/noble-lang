#![feature(register_tool)]
#![register_tool(tigerstyle)]
#![allow(
    tigerstyle::no_unwrap,
    tigerstyle::underscore_in_module_filename,
    reason = "Owner: noble-maintainers; this integration harness constructs boundary scenarios and asserts their outcomes, so terse fixture names, unwraps and long setup legs are the readable form."
)]
#[test]
fn checked_cli_slot_cas_and_current_policy_boundaries() {
    let script = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/live_slot_cas_policy.mjs"
    );
    let output = std::process::Command::new(
        "/nix/store/sy0c7j0npsq33d9zhnnzvjnzc52f4y0p-nodejs-24.13.0/bin/node",
    )
    .arg(script)
    .arg(env!("CARGO_BIN_EXE_noble"))
    .output()
    .expect("launch selected real-CLI slot CAS and policy cases");
    assert!(
        output.status.success(),
        "checked CLI slot CAS/policy cases failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}
