#![feature(register_tool)]
#![register_tool(tigerstyle)]
#![allow(
    tigerstyle::compound_assertion,
    tigerstyle::no_unwrap,
    tigerstyle::underscore_in_module_filename,
    reason = "Owner: noble-maintainers; this integration harness constructs boundary scenarios and asserts their outcomes, so terse fixture names, unwraps and long setup legs are the readable form."
)]
//! The calculator engine's Node tests (arena commit/restore), run with the
//! selected Node/V8 and the built `noble` binary as the WAT assembler.

#[test]
fn calculator_engine_node_tests() {
    let assembler =
        std::fs::canonicalize(env!("CARGO_BIN_EXE_noble")).expect("canonical noble assembler path");
    let output = std::process::Command::new(
        "/nix/store/sy0c7j0npsq33d9zhnnzvjnzc52f4y0p-nodejs-24.13.0/bin/node",
    )
    .arg("--test")
    .arg(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/core/runtime/engine-calculator.test.mjs"
    ))
    .env("NOBLE_CALCULATOR_ASSEMBLER", assembler)
    .output()
    .expect("launch selected Node/V8 engine tests");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success() && stdout.contains("skipped 0"),
        "calculator engine tests failed: stdout={stdout} stderr={}",
        String::from_utf8_lossy(&output.stderr),
    );
}
