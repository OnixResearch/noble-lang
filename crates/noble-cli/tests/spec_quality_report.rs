#![feature(register_tool)]
#![register_tool(tigerstyle)]
#![allow(
    tigerstyle::no_unwrap,
    tigerstyle::underscore_in_module_filename,
    reason = "Owner: noble-maintainers; this integration harness constructs report submissions and asserts their refusal classes, so terse fixture names and unwraps are the readable form."
)]
//! Real selected-CLI coverage of the specification-quality report producer (issue #65).
//! This is reporting evidence, not a correspondence or specification-quality proof.

#[test]
fn specification_quality_report_refuses_wrong_bindings_and_separates_obligations() {
    let output = std::process::Command::new(
        "/nix/store/sy0c7j0npsq33d9zhnnzvjnzc52f4y0p-nodejs-24.13.0/bin/node",
    )
    .arg(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/spec_quality_report.mjs"
    ))
    .arg(env!("CARGO_BIN_EXE_noble"))
    .output()
    .expect("launch selected Node/V8 specification-quality child");
    assert!(
        output.status.success(),
        "checked CLI specification-quality report failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}
