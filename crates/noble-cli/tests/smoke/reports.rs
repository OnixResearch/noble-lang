static NEXT_FILE_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

struct TemporaryDirectory(std::path::PathBuf);

impl TemporaryDirectory {
    fn new() -> Result<Self, std::io::Error> {
        loop {
            let id = NEXT_FILE_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let path =
                std::env::temp_dir().join(format!("noble-report-json-{}-{id}", std::process::id()));
            match std::fs::create_dir(&path) {
                Ok(()) => return Ok(Self(path)),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error),
            }
        }
    }

    fn file(&self, extension: &str, bytes: &[u8]) -> Result<std::path::PathBuf, std::io::Error> {
        let id = NEXT_FILE_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = self.0.join(format!("{id}.{extension}"));
        std::fs::write(&path, bytes)?;
        Ok(path)
    }
}

impl Drop for TemporaryDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn admit_artifact(
    temporary: &TemporaryDirectory,
    artifact: &[u8],
    manifest: &[u8],
    host_source: Option<&[u8]>,
    allowed_effects: Option<&str>,
) -> Result<std::process::Output, Box<dyn std::error::Error>> {
    let artifact_path = temporary.file("wasm", artifact)?;
    let manifest_path = temporary.file("json", manifest)?;
    let mut command = std::process::Command::new(env!("CARGO_BIN_EXE_noble"));
    command
        .arg("admit-artifact")
        .arg(artifact_path)
        .arg("--effects")
        .arg(manifest_path);
    if let Some(source) = host_source {
        command
            .arg("--source")
            .arg(temporary.file("noble", source)?);
    }
    if let Some(effects) = allowed_effects {
        command.arg("--allow-effects").arg(effects);
    }
    Ok(command.output()?)
}

fn assert_refusal(
    output: &std::process::Output,
    outcome: &str,
    expected_exit: i32,
) -> Result<serde_json::Value, serde_json::Error> {
    assert_eq!(output.status.code(), Some(expected_exit));
    let report = serde_json::from_slice::<serde_json::Value>(&output.stdout)?;
    assert_eq!(report["outcome"], outcome);
    assert_eq!(report["guest_requests"].as_u64(), Some(0));
    assert_eq!(report["protected_operations"].as_u64(), Some(0));
    Ok(report)
}

#[test]
fn malformed_artifact_claim_json_refuses_before_guest_or_protected_operations(
) -> Result<(), Box<dyn std::error::Error>> {
    const EMPTY_MODULE: &[u8] = b"\0asm\x01\0\0\0";
    let temporary = TemporaryDirectory::new()?;
    let manifests: &[&[u8]] = &[
        b"{\"claimed_effects\":[],\"source\":\"raw\nline\"}",
        b"{\"claimed_effects\":[],\"source\":\"raw\0nul\"}",
        br#"{"claimed_effects":[],"source":"\uD800"}"#,
        br#"{"claimed_effects":[],"source":"\uDC00"}"#,
        b"{\"claimed_effects\":[] }\xff",
    ];
    for manifest in manifests {
        let output = admit_artifact(&temporary, EMPTY_MODULE, manifest, None, None)?;
        assert_refusal(&output, "invalid-manifest", 2)?;
    }
    Ok(())
}

#[test]
fn artifact_claim_file_limit_accepts_exactly_1024_bytes_then_refuses(
) -> Result<(), Box<dyn std::error::Error>> {
    const EMPTY_MODULE: &[u8] = b"\0asm\x01\0\0\0";
    let temporary = TemporaryDirectory::new()?;
    let mut manifest = [b' '; 1025];
    let prefix = br#"{"claimed_effects":[]}"#;
    manifest[..prefix.len()].copy_from_slice(prefix);
    let exact = admit_artifact(&temporary, EMPTY_MODULE, &manifest[..1024], None, None)?;
    assert_refusal(&exact, "correspondence-reject", 2)?;
    let over = admit_artifact(&temporary, EMPTY_MODULE, &manifest, None, None)?;
    assert_refusal(&over, "exhausted", 5)?;
    Ok(())
}

#[test]
fn escaped_claims_reach_effect_policy_but_manifest_flags_do_not_grant_authority(
) -> Result<(), Box<dyn std::error::Error>> {
    const EMIT_IMPORT_MODULE: &[u8] = &[
        0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, // Wasm header
        0x01, 0x07, 0x01, 0x60, 0x02, 0x7f, 0x7f, 0x01, 0x7f, // (i32, i32) -> i32
        0x02, 0x13, 0x01, 0x05, b'n', b'o', b'b', b'l', b'e', 0x09, b't', b'e', b's', b't', b'_',
        b'e', b'm', b'i', b't', 0x00, 0x00, // noble.test_emit import
    ];
    let temporary = TemporaryDirectory::new()?;
    let manifest = br#"{"claimed_effects":["test.\u0065mit"],"trusted_correspondence":true,"allowed":true,"source":"\u0063andidate-\uD83D\uDE00","digest":"\u0030"}"#;
    let output = admit_artifact(&temporary, EMIT_IMPORT_MODULE, manifest, None, None)?;
    let report = assert_refusal(&output, "effect-policy-reject", 2)?;
    assert_eq!(report["claimed_effects"][0], "test.emit");
    assert_eq!(report["allowed_effects"], serde_json::json!([]));
    Ok(())
}

#[test]
fn manifest_source_claim_cannot_replace_host_selected_source(
) -> Result<(), Box<dyn std::error::Error>> {
    const EMPTY_MODULE: &[u8] = b"\0asm\x01\0\0\0";
    let temporary = TemporaryDirectory::new()?;
    let manifest = br#"{"claimed_effects":[],"trusted_correspondence":true,"allowed":true,"source":"candidate-source","digest":"candidate-digest"}"#;
    let output = admit_artifact(&temporary, EMPTY_MODULE, manifest, Some(b""), None)?;
    assert_refusal(&output, "correspondence-reject", 2)?;
    Ok(())
}

#[test]
fn editor_integer_remains_i64_only() -> Result<(), Box<dyn std::error::Error>> {
    let temporary = TemporaryDirectory::new()?;
    let maximum = temporary.file(
        "json",
        br#"{"format":1,"nodes":[{"kind":"integer","value":9223372036854775807}]}"#,
    )?;
    let accepted = std::process::Command::new(env!("CARGO_BIN_EXE_noble"))
        .args(["editor", "analyze"])
        .arg(maximum)
        .output()?;
    assert!(accepted.status.success());
    let report = serde_json::from_slice::<serde_json::Value>(&accepted.stdout)?;
    assert_eq!(report["outcome"], "constraints");

    let above = temporary.file(
        "json",
        br#"{"format":1,"nodes":[{"kind":"integer","value":9223372036854775808}]}"#,
    )?;
    let rejected = std::process::Command::new(env!("CARGO_BIN_EXE_noble"))
        .args(["editor", "analyze"])
        .arg(above)
        .output()?;
    assert_eq!(rejected.status.code(), Some(2));
    let report = serde_json::from_slice::<serde_json::Value>(&rejected.stdout)?;
    assert_eq!(report["outcome"], "reject");
    assert_eq!(report["guest_requests"].as_u64(), Some(0));
    assert_eq!(report["protected_operations"].as_u64(), Some(0));
    Ok(())
}
