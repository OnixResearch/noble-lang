//! Descriptor-anchored input and output tests for the nonselected source-only
//! C build. These are host-side integrity checks; none of them executes
//! emitted C or claims native admission.

use std::ffi::OsString;
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::os::unix::fs::{symlink, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::Command;

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
        ("hard-link", hard, "multiply-linked input refused"),
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
    let source_bytes = b"1";
    let source = scratch.file("source.noble", source_bytes)?;
    let contract_bytes = CONTRACT;
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

#[test]
fn non_regular_oversized_and_deep_inputs_refuse() -> Result<(), Box<dyn std::error::Error>> {
    let scratch = Scratch::new()?;
    let (_, contract, policy) = fixture(&scratch)?;
    let directory = scratch.0.join("as-directory");
    std::fs::create_dir(&directory)?;
    let (code, stderr, existed) =
        refusal(&directory, &contract, &policy, &scratch.0.join("dir-out"))?;
    assert_eq!(code, 2);
    assert!(stderr.contains("bounded regular file"), "{stderr}");
    assert!(!existed);
    let oversized = scratch.file("oversized.noble", &vec![b'1'; 4097])?;
    let (code, stderr, existed) =
        refusal(&oversized, &contract, &policy, &scratch.0.join("big-out"))?;
    assert_eq!(code, 2);
    assert!(stderr.contains("exceeds 4096 bytes"), "{stderr}");
    assert!(!existed);
    let mut deep = scratch.0.clone();
    for _ in 0..129 {
        deep.push("a");
    }
    let (code, stderr, existed) = refusal(&deep, &contract, &policy, &scratch.0.join("deep-out"))?;
    assert_eq!(code, 2);
    assert!(stderr.contains("depth exceeds 128 components"), "{stderr}");
    assert!(!existed);
    Ok(())
}

struct RestoreMode(PathBuf);

impl Drop for RestoreMode {
    fn drop(&mut self) {
        let _ = std::fs::set_permissions(&self.0, std::fs::Permissions::from_mode(0o755));
    }
}

#[test]
fn search_only_ancestor_permits_o_path_anchor_and_writes_output(
) -> Result<(), Box<dyn std::error::Error>> {
    let scratch = Scratch::new()?;
    let (_, contract, policy) = fixture(&scratch)?;
    let search_only = scratch.0.join("search-only");
    let sub = search_only.join("sub");
    std::fs::create_dir_all(&sub)?;
    let source = sub.join("source.noble");
    std::fs::write(&source, b"1")?;
    std::fs::set_permissions(&search_only, std::fs::Permissions::from_mode(0o111))?;
    let _restore = RestoreMode(search_only);
    let out = scratch.0.join("search-only-out");
    let built = build(&source, &contract, &policy, &out)?;
    assert!(
        built.status.success(),
        "O_PATH search-only ancestor should succeed: {}",
        String::from_utf8_lossy(&built.stderr)
    );
    assert_eq!(std::fs::read(out.join("source.noble"))?, b"1");
    Ok(())
}

#[test]
fn writable_non_sticky_ancestors_refuse_for_inputs_and_outputs(
) -> Result<(), Box<dyn std::error::Error>> {
    let scratch = Scratch::new()?;
    let (source, contract, policy) = fixture(&scratch)?;
    let open_parent = scratch.0.join("open-parent");
    let group_parent = scratch.0.join("group-parent");
    let sticky_parent = scratch.0.join("sticky-parent");
    for dir in [&open_parent, &group_parent, &sticky_parent] {
        std::fs::create_dir(dir)?;
    }
    let open_source = open_parent.join("source.noble");
    let group_source = group_parent.join("source.noble");
    let sticky_source = sticky_parent.join("source.noble");
    for file in [&open_source, &group_source, &sticky_source] {
        std::fs::write(file, b"1")?;
    }
    std::fs::set_permissions(&open_parent, std::fs::Permissions::from_mode(0o777))?;
    std::fs::set_permissions(&group_parent, std::fs::Permissions::from_mode(0o775))?;
    std::fs::set_permissions(&sticky_parent, std::fs::Permissions::from_mode(0o1777))?;

    for (name, src, out_dir) in [
        (
            "world-writable-input",
            &open_source,
            scratch.0.join("out-1"),
        ),
        (
            "group-writable-input",
            &group_source,
            scratch.0.join("out-2"),
        ),
        ("world-writable-output", &source, open_parent.join("out")),
        ("group-writable-output", &source, group_parent.join("out")),
    ] {
        let (code, stderr, existed) = refusal(src, &contract, &policy, &out_dir)?;
        assert_eq!(code, 2, "{name}: {stderr}");
        assert!(
            stderr.contains("writable without the sticky bit"),
            "{name}: {stderr}"
        );
        assert!(!existed, "{name}: refused ancestor emitted output");
    }

    let sticky_out = sticky_parent.join("out");
    let built = build(&sticky_source, &contract, &policy, &sticky_out)?;
    assert!(
        built.status.success(),
        "sticky-bit ancestor owned by current user should succeed: {}",
        String::from_utf8_lossy(&built.stderr)
    );
    Ok(())
}

#[test]
fn magic_links_and_non_directory_ancestors_refuse_before_emission(
) -> Result<(), Box<dyn std::error::Error>> {
    let scratch = Scratch::new()?;
    let (source, contract, policy) = fixture(&scratch)?;
    let pid = std::process::id();
    let stripped = source.strip_prefix("/")?;
    let proc_root = PathBuf::from(format!("/proc/{pid}/root")).join(stripped);
    let proc_fd = PathBuf::from(format!("/proc/{pid}/fd/0"));
    let non_dir = source.join("child");

    for (name, path, diagnostic) in [
        ("proc-root-magic", &proc_root, "is a symlink"),
        ("proc-fd-magic", &proc_fd, ""),
        ("non-directory-ancestor", &non_dir, "is not a directory"),
    ] {
        let out = scratch.0.join(format!("{name}-out"));
        let (code, stderr, existed) = refusal(path, &contract, &policy, &out)?;
        assert_eq!(code, 2, "{name}: {stderr}");
        if !diagnostic.is_empty() {
            assert!(stderr.contains(diagnostic), "{name}: {stderr}");
        }
        assert!(!existed, "{name}: refused input emitted output");
    }
    Ok(())
}

#[test]
fn non_utf8_paths_fifo_inputs_and_exact_128_component_boundary(
) -> Result<(), Box<dyn std::error::Error>> {
    let scratch = Scratch::new()?;
    let (source, contract, policy) = fixture(&scratch)?;

    let mut bad_bytes = scratch.0.as_os_str().as_bytes().to_vec();
    bad_bytes.extend_from_slice(b"/bad-\xff");
    let bad_path = OsString::from_vec(bad_bytes);

    for (flag, value) in [
        ("--contract", &bad_path),
        ("--policy", &bad_path),
        ("--out", &bad_path),
    ] {
        let out = scratch.0.join("non-utf8-out");
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_noble"));
        cmd.arg("build").arg(&source).args(["--target", "c"]);
        if flag == "--contract" {
            cmd.arg("--contract").arg(value);
        } else {
            cmd.arg("--contract").arg(&contract);
        }
        if flag == "--policy" {
            cmd.arg("--policy").arg(value);
        } else {
            cmd.arg("--policy").arg(&policy);
        }
        if flag == "--out" {
            cmd.arg("--out").arg(value);
        } else {
            cmd.arg("--out").arg(&out);
        }
        let result = cmd.output()?;
        assert_eq!(result.status.code(), Some(2), "{flag}");
        let stderr = String::from_utf8_lossy(&result.stderr);
        assert!(stderr.contains("must be UTF-8"), "{flag}: {stderr}");
        assert!(!out.exists(), "{flag}: non-UTF-8 path emitted output");
    }

    let dev_null = Path::new("/dev/null");
    if dev_null.exists() && !dev_null.is_symlink() {
        let out = scratch.0.join("devnull-out");
        let (code, stderr, existed) = refusal(dev_null, &contract, &policy, &out)?;
        assert_eq!(code, 2);
        assert!(stderr.contains("bounded regular file"), "{stderr}");
        assert!(!existed);
    }

    let fifo = scratch.0.join("input.fifo");
    let mkfifo_status = Command::new("mkfifo").arg(&fifo).status()?;
    assert!(mkfifo_status.success(), "mkfifo failed");
    let fifo_out = scratch.0.join("fifo-out");
    let (code, stderr, existed) = refusal(&fifo, &contract, &policy, &fifo_out)?;
    assert_eq!(code, 2);
    assert!(stderr.contains("bounded regular file"), "{stderr}");
    assert!(!existed);

    let base_components = scratch
        .0
        .components()
        .filter(|component| matches!(component, std::path::Component::Normal(_)))
        .count();
    let mut exact_dir = scratch.0.clone();
    for _ in 0..128usize.saturating_sub(base_components).saturating_sub(1) {
        exact_dir.push("d");
    }
    std::fs::create_dir_all(&exact_dir)?;
    let exact_source = exact_dir.join("s.noble");
    std::fs::write(&exact_source, b"1")?;
    let exact_out = scratch.0.join("exact-128-out");
    let built = build(&exact_source, &contract, &policy, &exact_out)?;
    assert!(
        built.status.success(),
        "exact 128-component path should succeed: {}",
        String::from_utf8_lossy(&built.stderr)
    );

    let too_deep_dir = exact_dir.join("d");
    std::fs::create_dir_all(&too_deep_dir)?;
    let too_deep = too_deep_dir.join("s.noble");
    std::fs::write(&too_deep, b"1")?;
    let deep_out = scratch.0.join("too-deep-129-out");
    let (code, stderr, existed) = refusal(&too_deep, &contract, &policy, &deep_out)?;
    assert_eq!(code, 2);
    assert!(stderr.contains("depth exceeds 128 components"), "{stderr}");
    assert!(!existed);
    Ok(())
}
