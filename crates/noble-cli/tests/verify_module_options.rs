#![feature(register_tool)]
#![register_tool(tigerstyle)]
#![allow(
    tigerstyle::explicit_defaults,
    tigerstyle::no_unwrap,
    tigerstyle::non_trait_imports,
    tigerstyle::underscore_in_module_filename,
    reason = "Owner: noble-maintainers; this integration harness constructs boundary scenarios and asserts their outcomes, so terse fixture names and unwraps are the readable form."
)]
//! `verify-module` option boundaries that refuse before any source or Lean work.

use std::process::Command;

/// A repeated `--timeout-ms` used to silently keep the last value. It is now a
/// usage refusal, so a later flag cannot widen the caller's earlier deadline,
/// and the refusal happens before any module is read.
#[test]
fn verify_module_refuses_a_repeated_proof_deadline() -> Result<(), Box<dyn std::error::Error>> {
    let base = std::env::temp_dir().join(format!(
        "noble-verify-module-options-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&base)?;
    let source = base.join("subject.noble");
    std::fs::write(
        &source,
        b"module Subject@1 [ export value def value [ 1 ] ]",
    )?;
    let output = Command::new(env!("CARGO_BIN_EXE_noble"))
        .arg("verify-module")
        .arg(&source)
        .args(["--timeout-ms", "1000", "--timeout-ms", "2000"])
        .output()?;
    let stdout = String::from_utf8(output.stdout)?;
    std::fs::remove_dir_all(&base)?;
    assert_eq!(
        output.status.code(),
        Some(2),
        "a repeated deadline is a usage error: {stdout}"
    );
    assert!(
        stdout.contains("repeated proof deadline"),
        "the refusal must name the repeated deadline: {stdout}"
    );
    Ok(())
}
