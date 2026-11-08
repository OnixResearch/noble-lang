#![feature(register_tool)]
#![register_tool(tigerstyle)]
#![allow(
    tigerstyle::no_unwrap,
    tigerstyle::underscore_in_module_filename,
    reason = "Owner: noble-maintainers; this integration harness constructs boundary scenarios and asserts their outcomes, so terse fixture names, unwraps and long setup legs are the readable form."
)]
//! Saved Program owners against the real selected CLI. A direct typed quote must retain its actual
//! boxed capture as a usable saved Program. A saved Program whose compiled body reads a named
//! definition through its module's global must survive that module's supersession, and the module
//! retires only after the owner is released (OWN-CASE-06). Behavioral evidence, not a proof.

#[test]
fn direct_quote_owner_can_run_and_release_without_retaining_code() {
    let node = "/nix/store/sy0c7j0npsq33d9zhnnzvjnzc52f4y0p-nodejs-24.13.0/bin/node";
    let script = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/live_slot_direct_quote.mjs"
    );
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

#[test]
fn a_saved_named_caller_survives_module_root_retirement() {
    let output = std::process::Command::new(
        "/nix/store/sy0c7j0npsq33d9zhnnzvjnzc52f4y0p-nodejs-24.13.0/bin/node",
    )
    .arg(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/live_slot_module_root_retention.mjs"
    ))
    .arg(env!("CARGO_BIN_EXE_noble"))
    .output()
    .expect("launch selected Node/V8 module-root retention child");
    assert!(
        output.status.success(),
        "checked CLI module-root retention witness failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}
