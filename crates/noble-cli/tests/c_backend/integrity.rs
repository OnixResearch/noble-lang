//! Descriptor-anchored input and output tests for the nonselected source-only
//! C build. These are host-side integrity checks; none of them executes
//! emitted C or claims native admission.

use std::os::unix::fs::symlink;
use std::path::Path;

use super::{build, Scratch};

const POLICY: &[u8] = br#"{"profile":"Core-Bootstrap","target":"x86_64-unknown-linux-gnu","allowed_effects":[],"limits":{"stack_slots":8,"steps":100,"output_bytes":1024}}"#;
const CONTRACT: &[u8] = br#"{"expected_interface":"-- I64","resolved_host_contracts":{}}"#;

fn fixture(
    scratch: &Scratch,
) -> Result<(std::path::PathBuf, std::path::PathBuf, std::path::PathBuf), std::io::Error> {
    Ok((
        scratch.file("source.noble", b"1")?,
        scratch.file("contract.json", CONTRACT)?,
        scratch.file("policy.json", POLICY)?,
    ))
}

fn refusal(
    source: &Path,
    contract: &Path,
    policy: &Path,
    out: &Path,
) -> Result<(i32, String, bool), String> {
    let result = build(source, contract, policy, out).map_err(|error| error.to_string())?;
    Ok((
        result.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&result.stderr).into_owned(),
        out.exists(),
    ))
}

#[test]
fn relative_dotdot_and_unnormalized_inputs_refuse_before_emission(
) -> Result<(), Box<dyn std::error::Error>> {
    let scratch = Scratch::new()?;
    let (source, contract, policy) = fixture(&scratch)?;
    let cases = [
        (
            "relative",
            Path::new("source.noble").to_path_buf(),
            "must be lexically absolute",
        ),
        (
            "dotdot",
            scratch.0.join("..").join("source.noble"),
            "refuse `.` and `..` components",
        ),
        (
            "dot",
            scratch.0.join(".").join("source.noble"),
            "refuse `.` and `..` components",
        ),
        (
            "trailing",
            scratch.0.join("source.noble/"),
            "non-empty and normalized",
        ),
    ];
    for (name, path, diagnostic) in cases {
        let out = scratch.0.join(format!("{name}-out"));
        let (code, stderr, existed) = refusal(&path, &contract, &policy, &out)?;
        assert_eq!(code, 2, "{name}");
        assert!(stderr.contains(diagnostic), "{name}: {stderr}");
        assert!(!existed, "{name}: refused input emitted output");
    }
    assert_eq!(std::fs::read(&source)?, b"1");
    Ok(())
}

#[test]
fn symlinked_and_hard_linked_inputs_refuse_before_emission(
) -> Result<(), Box<dyn std::error::Error>> {
    let scratch = Scratch::new()?;
    let (source, contract, policy) = fixture(&scratch)?;
    let real = scratch.0.join("real");
    std::fs::create_dir(&real)?;
    std::fs::copy(&source, real.join("source.noble"))?;
    let ancestor = scratch.0.join("linked");
    symlink(&real, &ancestor)?;
    let leaf = scratch.0.join("leaf.noble");
    symlink(&source, &leaf)?;
    let hard = scratch.0.join("hard.noble");
    std::fs::hard_link(&source, &hard)?;
    for (name, path, diagnostic) in [
        (
            "ancestor-symlink",
            ancestor.join("source.noble"),
            "is a symlink",
        ),
        ("leaf-symlink", leaf, "is a symlink"),
        ("hard-link", hard, "hard-linked mutable input refused"),
    ] {
        let out = scratch.0.join(format!("{name}-out"));
        let (code, stderr, existed) = refusal(&path, &contract, &policy, &out)?;
        assert_eq!(code, 2, "{name}");
        assert!(stderr.contains(diagnostic), "{name}: {stderr}");
        assert!(!existed, "{name}: refused input emitted output");
    }
    Ok(())
}

#[test]
fn output_paths_are_bound_to_walked_directories() -> Result<(), Box<dyn std::error::Error>> {
    let scratch = Scratch::new()?;
    let (source, contract, policy) = fixture(&scratch)?;
    let parent = scratch.0.join("parent");
    std::fs::create_dir(&parent)?;
    let linked = scratch.0.join("linked-parent");
    symlink(&parent, &linked)?;
    let out = linked.join("out");
    let (code, stderr, _) = refusal(&source, &contract, &policy, &out)?;
    assert_eq!(code, 2);
    assert!(stderr.contains("is a symlink"), "{stderr}");
    assert!(!parent.join("out").exists());
    let existing = scratch.0.join("existing");
    std::fs::create_dir(&existing)?;
    let (code, stderr, _) = refusal(&source, &contract, &policy, &existing)?;
    assert_eq!(code, 2);
    assert!(stderr.contains("already exists"), "{stderr}");
    let target = scratch.0.join("target");
    std::fs::create_dir(&target)?;
    let leaf = scratch.0.join("output-leaf");
    symlink(&target, &leaf)?;
    let (code, stderr, _) = refusal(&source, &contract, &policy, &leaf)?;
    assert_eq!(code, 2);
    assert!(stderr.contains("already exists"), "{stderr}");
    assert_eq!(std::fs::read_dir(&target)?.count(), 0);
    Ok(())
}

#[test]
fn emitted_inputs_are_the_checked_bytes_under_one_new_directory(
) -> Result<(), Box<dyn std::error::Error>> {
    let scratch = Scratch::new()?;
    let source_bytes = b"1 true pair";
    let source = scratch.file("source.noble", source_bytes)?;
    let contract_bytes =
        br#"{"expected_interface":"-- Pair<I64,Bool>","resolved_host_contracts":{}}"#;
    let contract = scratch.file("contract.json", contract_bytes)?;
    let policy = scratch.file("policy.json", POLICY)?;
    let mut manifests = Vec::new();
    for name in ["first", "second"] {
        let out = scratch.0.join(name);
        let built = build(&source, &contract, &policy, &out)?;
        assert!(
            built.status.success(),
            "{name}: {}",
            String::from_utf8_lossy(&built.stderr)
        );
        assert_eq!(std::fs::read(out.join("source.noble"))?, source_bytes);
        assert_eq!(std::fs::read(out.join("contract.json"))?, contract_bytes);
        assert_eq!(std::fs::read(out.join("policy.json"))?, POLICY);
        let manifest: serde_json::Value =
            serde_json::from_slice(&std::fs::read(out.join("manifest.json"))?)?;
        let digest = manifest["source_sha256"]
            .as_str()
            .ok_or("missing source digest")?;
        assert_eq!(digest.len(), 64);
        assert!(digest.bytes().all(|byte| byte.is_ascii_hexdigit()));
        assert_eq!(digest, digest.to_ascii_lowercase());
        manifests.push(manifest);
    }
    assert_eq!(manifests[0]["build_key"], manifests[1]["build_key"]);
    assert_eq!(manifests[0]["c_sha256"], manifests[1]["c_sha256"]);
    Ok(())
}
