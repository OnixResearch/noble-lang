//! Source-only C candidate controls. Native execution remains separately gated.

use std::path::{Path, PathBuf};
use std::process::Command;

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Result<Self, std::io::Error> {
        let path = std::env::temp_dir().join(format!(
            "noble-c-source-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        std::fs::create_dir(&path)?;
        Ok(Self(path))
    }
    fn file(&self, name: &str, bytes: &[u8]) -> Result<PathBuf, std::io::Error> {
        let path = self.0.join(name);
        std::fs::write(&path, bytes)?;
        Ok(path)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn build(
    source: &Path,
    contract: &Path,
    policy: &Path,
    out: &Path,
) -> Result<std::process::Output, std::io::Error> {
    Command::new(env!("CARGO_BIN_EXE_noble"))
        .arg("build")
        .arg(source)
        .args(["--target", "c"])
        .arg("--out")
        .arg(out)
        .arg("--contract")
        .arg(contract)
        .arg("--policy")
        .arg(policy)
        .output()
}

#[test]
fn source_bytes_change_build_identity_without_rebinding_numeric_recipe(
) -> Result<(), Box<dyn std::error::Error>> {
    let scratch = Scratch::new()?;
    let source_a = scratch.file("a.noble", b"9223372036854775807 1 +")?;
    let source_b = scratch.file("b.noble", b"9223372036854775807  1 +")?;
    let source_min = scratch.file("min.noble", b"-9223372036854775808")?;
    let contract = scratch.file(
        "contract.json",
        br#"{"expected_interface":"-- I64","resolved_host_contracts":{}}"#,
    )?;
    let policy = scratch.file("policy.json", br#"{"profile":"Core-Bootstrap","target":"x86_64-unknown-linux-gnu","allowed_effects":[],"limits":{"stack_slots":8,"steps":100,"output_bytes":1024}}"#)?;
    let a = build(&source_a, &contract, &policy, &scratch.0.join("out-a"))?;
    let b = build(&source_b, &contract, &policy, &scratch.0.join("out-b"))?;
    let minimum = build(&source_min, &contract, &policy, &scratch.0.join("out-min"))?;
    assert!(a.status.success(), "{}", String::from_utf8_lossy(&a.stderr));
    assert!(b.status.success(), "{}", String::from_utf8_lossy(&b.stderr));
    assert!(
        minimum.status.success(),
        "{}",
        String::from_utf8_lossy(&minimum.stderr)
    );
    let a: serde_json::Value = serde_json::from_slice(&a.stdout)?;
    let b: serde_json::Value = serde_json::from_slice(&b.stdout)?;
    let minimum: serde_json::Value = serde_json::from_slice(&minimum.stdout)?;
    assert_ne!(a["build_key"], b["build_key"]);
    assert_eq!(a["checked_candidate_sha256"], b["checked_candidate_sha256"]);
    assert_eq!(a["semantic_recipe_sha256"], b["semantic_recipe_sha256"]);
    assert_ne!(
        a["semantic_recipe_sha256"],
        minimum["semantic_recipe_sha256"]
    );
    Ok(())
}

#[test]
fn checked_boundary_arithmetic_requires_the_exact_result_interface(
) -> Result<(), Box<dyn std::error::Error>> {
    let scratch = Scratch::new()?;
    let source = scratch.file(
        "boundary.noble",
        b"9223372036854775807 1 + -1 * -9223372036854775808 =",
    )?;
    let wrong_result = scratch.file(
        "wrong-result.noble",
        b"9223372036854775807 1 + -1 * -9223372036854775808 +",
    )?;
    let contract = scratch.file(
        "contract.json",
        br#"{"expected_interface":"-- Bool","resolved_host_contracts":{}}"#,
    )?;
    let policy = scratch.file("policy.json", br#"{"profile":"Core-Bootstrap","target":"x86_64-unknown-linux-gnu","allowed_effects":[],"limits":{"stack_slots":8,"steps":100,"output_bytes":1024}}"#)?;
    let admitted = build(&source, &contract, &policy, &scratch.0.join("checked-c"))?;
    assert!(
        admitted.status.success(),
        "{}",
        String::from_utf8_lossy(&admitted.stderr)
    );
    let refused_dir = scratch.0.join("wrong-contract");
    let refused = build(&wrong_result, &contract, &policy, &refused_dir)?;
    assert_eq!(refused.status.code(), Some(2));
    assert!(!refused_dir.exists());
    Ok(())
}

#[test]
fn unsupported_target_effect_and_out_of_range_source_refuse_before_emission(
) -> Result<(), Box<dyn std::error::Error>> {
    let scratch = Scratch::new()?;
    let contract = scratch.file(
        "contract.json",
        br#"{"expected_interface":"-- I64","resolved_host_contracts":{}}"#,
    )?;
    let source = scratch.file("source.noble", b"1 2 +")?;
    let wrong_target = scratch.file("wrong-target.json", br#"{"profile":"Core-Bootstrap","target":"aarch64-unknown-linux-gnu","allowed_effects":[],"limits":{"stack_slots":8,"steps":100,"output_bytes":1024}}"#)?;
    let effects = scratch.file("effect-policy.json", br#"{"profile":"Core-Bootstrap","target":"x86_64-unknown-linux-gnu","allowed_effects":["test.emit"],"limits":{"stack_slots":8,"steps":100,"output_bytes":1024}}"#)?;
    let pure = scratch.file("pure-policy.json", br#"{"profile":"Core-Bootstrap","target":"x86_64-unknown-linux-gnu","allowed_effects":[],"limits":{"stack_slots":8,"steps":100,"output_bytes":1024}}"#)?;
    let tiny_output = scratch.file("tiny-output.json", br#"{"profile":"Core-Bootstrap","target":"x86_64-unknown-linux-gnu","allowed_effects":[],"limits":{"stack_slots":8,"steps":100,"output_bytes":128}}"#)?;
    let out_of_range = scratch.file("out-of-range.noble", b"9223372036854775808")?;
    let invalid_utf8 = scratch.file("invalid-utf8.noble", b"\xff")?;
    let bad_type = scratch.file("bad-type.noble", b"1 true +")?;
    let unsupported_value = scratch.file("unsupported-value.noble", b"unit drop 1")?;
    let host_effect = scratch.file("host-effect.noble", b"\"A\" test.emit 1")?;
    let named = scratch.file("named.noble", b"def twice [ dup compose ]\n20 [ 1 + ] twice run")?;
    let reflect = scratch.file("reflect.noble", b"[ 1 ] reflect drop 1")?;
    for (index, (source, policy)) in [
        (&source, &wrong_target),
        (&source, &effects),
        (&source, &tiny_output),
        (&out_of_range, &pure),
        (&invalid_utf8, &pure),
        (&bad_type, &pure),
        (&unsupported_value, &pure),
        (&host_effect, &pure),
        (&named, &pure),
        (&reflect, &pure),
    ]
    .into_iter()
    .enumerate()
    {
        let destination = scratch.0.join(format!("refused-{index}"));
        let output = build(source, &contract, policy, &destination)?;
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert!(!destination.exists());
    }
    let wrong_interface = scratch.file(
        "wrong-interface.json",
        br#"{"expected_interface":"-- Bool","resolved_host_contracts":{}}"#,
    )?;
    let mismatched = scratch.0.join("mismatched-interface");
    let refusal = build(&source, &wrong_interface, &pure, &mismatched)?;
    assert_eq!(refusal.status.code(), Some(2));
    assert!(!mismatched.exists());
    let native_destination = scratch.0.join("unavailable-native");
    let native = Command::new(env!("CARGO_BIN_EXE_noble"))
        .arg("build")
        .arg(&source)
        .args(["--target", "c"])
        .arg("--out")
        .arg(&native_destination)
        .arg("--contract")
        .arg(&contract)
        .arg("--policy")
        .arg(&pure)
        .arg("--native")
        .output()?;
    assert_eq!(native.status.code(), Some(2));
    assert!(native.stdout.is_empty());
    assert!(!native_destination.exists());
    let execute = Command::new(env!("CARGO_BIN_EXE_noble"))
        .args(["execute", "--target", "c", "--artifact", "untrusted"])
        .output()?;
    assert_eq!(execute.status.code(), Some(2));
    assert!(execute.stdout.is_empty());
    assert!(String::from_utf8_lossy(&execute.stderr).contains("native C execution is unavailable"));
    let occupied = scratch.0.join("occupied");
    std::fs::create_dir(&occupied)?;
    let prior = occupied.join("prior");
    std::fs::write(&prior, b"host-owned")?;
    let refusal = build(&source, &contract, &pure, &occupied)?;
    assert_eq!(refusal.status.code(), Some(2));
    assert_eq!(std::fs::read(&prior)?, b"host-owned");
    Ok(())
}
