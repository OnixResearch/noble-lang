#![feature(register_tool)]
#![register_tool(tigerstyle)]
#![allow(
    tigerstyle::no_unwrap,
    tigerstyle::underscore_in_module_filename,
    reason = "Owner: noble-maintainers; this integration harness constructs boundary scenarios and asserts their outcomes, so terse fixture names, unwraps and long setup legs are the readable form."
)]
//! Real selected-CLI coverage of authority grant scopes: a nominal-scoped `effect` grant can never
//! authorize — or re-enable — a protected effect that a slot-scoped grant denied, through either the
//! authority file or a runtime policy request. This is behavioral evidence, not a live-slot theorem.

#[test]
fn an_effect_grant_is_never_widened_from_the_nominal_scope() {
    let output = std::process::Command::new(
        "/nix/store/sy0c7j0npsq33d9zhnnzvjnzc52f4y0p-nodejs-24.13.0/bin/node",
    )
    .arg(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/live_slot_grant_scope.mjs"
    ))
    .arg(env!("CARGO_BIN_EXE_noble"))
    .output()
    .expect("launch selected Node/V8 grant-scope child");
    assert!(
        output.status.success(),
        "checked CLI grant-scope witness failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}
