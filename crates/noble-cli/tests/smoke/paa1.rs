//! The `admit-proof-artifact` route through the real binary: malformed
//! envelopes are typed refusals with zero guest requests and protected
//! operations, and the release-owner pin preparation route is separate from
//! admission.

use std::process::{Command, Output};

fn run(arguments: &[&str]) -> Result<Output, std::io::Error> {
    Command::new(env!("CARGO_BIN_EXE_noble"))
        .arg("admit-proof-artifact")
        .args(arguments)
        .output()
}

fn assert_refusal(output: &Output, code: &str, exit: i32) {
    assert_eq!(
        output.status.code(),
        Some(exit),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("one JSON envelope per run");
    assert_eq!(report["schema"], "noble-proof-artifact-admission/v1");
    assert_eq!(report["admission"], "refused");
    assert_eq!(report["outcome"], code, "{report}");
    assert_eq!(report["guest_requests"], 0);
    assert_eq!(report["protected_operations"], 0);
}

#[test]
fn the_admission_route_requires_every_operand() -> Result<(), Box<dyn std::error::Error>> {
    // No candidate and no other operand at all.
    let output = run(&[])?;
    assert_refusal(&output, "invalid-envelope", 2);
    // A candidate with no law.
    let output = run(&["candidate.wasm"])?;
    assert_refusal(&output, "invalid-envelope", 2);
    // A candidate and a law without the owner-supplied digest.
    let output = run(&["candidate.wasm", "--law", "law.json"])?;
    assert_refusal(&output, "invalid-envelope", 2);
    Ok(())
}

#[test]
fn malformed_operands_are_refused_before_any_read() -> Result<(), Box<dyn std::error::Error>> {
    let digest = "9a8eda12c74356ca0d312be840411d4445c6f377c16ee7b96a680581b2e91eac";
    let base = [
        "candidate.wasm",
        "--law",
        "law.json",
        "--source",
        "source.noble",
        "--contract",
        "contract.json",
        "--proof",
        "proof.lean",
        "--pins",
        "pins.json",
        "--call",
        "call.wasm",
        "--argument",
        "41",
    ];
    // A short digest.
    let mut short: Vec<&str> = base.to_vec();
    short.extend(["--law-digest", "9a8eda"]);
    assert_refusal(&run(&short)?, "invalid-envelope", 2);
    // A duplicate operand.
    let mut duplicate: Vec<&str> = base.to_vec();
    duplicate.extend(["--law-digest", digest, "--law", "second.json"]);
    assert_refusal(&run(&duplicate)?, "invalid-envelope", 2);
    // A non-integer argument.
    let mut argument: Vec<&str> = base.to_vec();
    let last = argument.len() - 1;
    argument[last] = "forty-one";
    argument.extend(["--law-digest", digest]);
    assert_refusal(&run(&argument)?, "invalid-envelope", 2);
    // A zero policy revision.
    let mut revision: Vec<&str> = base.to_vec();
    revision.extend(["--law-digest", digest, "--policy-revision", "0"]);
    assert_refusal(&run(&revision)?, "invalid-envelope", 2);
    Ok(())
}

#[test]
fn pin_preparation_is_a_separate_route() -> Result<(), Box<dyn std::error::Error>> {
    // Pin preparation takes no admission operand; passing one is a usage error
    // rather than authority the preparation would ignore.
    let output = run(&["--emit-pins", "pins.json", "--law", "law.json"])?;
    assert_refusal(&output, "invalid-envelope", 2);
    // And admission cannot write pins.
    let output = run(&["candidate.wasm", "--emit-pins", "pins.json"])?;
    assert_refusal(&output, "invalid-envelope", 2);
    Ok(())
}
