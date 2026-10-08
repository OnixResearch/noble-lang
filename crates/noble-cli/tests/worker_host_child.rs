use std::io::Write;
use std::process::{Command, Stdio};

const EMIT: &str = "module Ledger@1 [ require emit Text -- ! test.emit export plus def plus [ \"private-ledger-marker\" emit 1 + ] ]";
const EXPORT: &str = "module ledger@1 [ require code_export Text Text -- ! test.code-export export capture def capture [ \"private-export-marker\" \"hidden-capture-destination\" code_export ] ]";

fn ledger_denial_source() -> String {
    let mut source = String::with_capacity(52_554);
    source.push_str("module Ledger@1 [ require emit Text -- ! test.emit export plus def plus [ \"private-ledger-marker");
    source.extend(std::iter::repeat_n('x', 52_408));
    source.push_str("\" dup emit dup emit dup emit dup emit emit 1 + ] ]");
    source
}

fn child(mode: &str, source: &str) -> Result<(serde_json::Value, Vec<u8>, Vec<u8>),
    Box<dyn std::error::Error>> {
    child_with_exit(mode, source, if mode == "emit" || mode == "emit-wrong" { 0 } else { 2 })
}

fn child_with_exit(mode: &str, source: &str, expected_exit: i32)
    -> Result<(serde_json::Value, Vec<u8>, Vec<u8>), Box<dyn std::error::Error>> {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    let directory = std::env::temp_dir().join(format!("noble-worker-host-{}-{}",
        std::process::id(), NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)));
    std::fs::create_dir(&directory)?;
    let mut child = Command::new(env!("CARGO_BIN_EXE_noble"))
        .args(["worker", "program", "test-host", mode])
        .current_dir(&directory)
        .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped())
        .spawn()?;
    child.stdin.take().ok_or("missing child stdin")?.write_all(source.as_bytes())?;
    let result = child.wait_with_output()?;
    assert!(result.status.code() == Some(expected_exit),
        "Worker host child exit mismatch: expected {expected_exit}, actual {:?}",
        result.status.code());
    assert_eq!(std::fs::read_dir(&directory)?.count(), 0,
        "test host created an export or other output file");
    std::fs::remove_dir(&directory)?;
    let report = serde_json::from_slice(&result.stdout)
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidData,
            "Worker host report is not valid JSON"))?;
    Ok((report, result.stdout, result.stderr))
}

#[test]
fn selected_host_ledger_denial_and_failures_preserve_committed_effects()
    -> Result<(), Box<dyn std::error::Error>> {
    let changed = EMIT.replace("1 +", "2 +");
    let (unselected, stdout, stderr) = child_with_exit("emit", &changed, 2)?;
    assert!(unselected["outcome"] == "invalid-input");
    assert!(unselected["receipt"]["effect_requests"] == 0);
    assert!(unselected["receipt"]["protected_operations"] == 0);
    redacted(&stdout, &stderr, &changed, &["private-ledger-marker"]);

    let (normal, stdout, stderr) = child("emit-wrong", &changed)?;
    assert!(normal["issued_value"] == 43,
        "candidate result is not a host-selected authority requirement");
    assert!(normal["receipt"]["protected_operations"] == 1);
    assert!(normal["receipt"]["ledger_entries"] == 1);
    redacted(&stdout, &stderr, &changed, &["private-ledger-marker"]);

    // Four real copies fill 209,716 ledger bytes. The fifth would exceed
    // 262,144 bytes, so the guest fails before producing an I64 result.
    let failing = ledger_denial_source();
    let (failure, stdout, stderr) = child_with_exit("emit-ledger-denied", &failing, 2)?;
    assert!(failure["outcome"] == "guest-trapped");
    assert!(failure["operation"] == "test.emit");
    assert!(failure["guest_trapped"] == true);
    assert!(failure["guest_failure_kind"] == "source-effect-denied");
    assert!(failure["receipt"]["effect_requests"] == 5);
    assert!(failure["receipt"]["protected_operations"] == 4);
    assert!(failure["receipt"]["denied_requests"] == 1);
    assert!(failure["receipt"]["ledger_entries"] == 4);
    assert!(failure["receipt"]["ledger_bytes"] == 209_716);
    assert!(failure["receipt"]["unknown_after_attempt"] == 0);
    assert!(failure.get("issued_value").is_none(),
        "a denied fifth callback must not produce an I64 result");
    assert!(failure.get("protected_operations").is_none(),
        "failure must not invent a standalone zero effect count");
    redacted(&stdout, &stderr, &failing, &["private-ledger-marker"]);

    let (panic, stdout, stderr) = child_with_exit("emit-panic", EMIT, 2)?;
    assert!(panic["outcome"] == "execution-panicked");
    assert!(panic["receipt"]["effect_requests"] == 1);
    assert!(panic["receipt"]["protected_operations"] == 1);
    assert!(panic["receipt"]["ledger_entries"] == 1);
    assert!(panic["receipt"]["unknown_after_attempt"] == 0);
    assert!(String::from_utf8_lossy(&stderr).contains("Worker host panic (redacted)"),
        "host panic was not visibly reported");
    redacted(&stdout, &stderr, EMIT, &["private-ledger-marker"]);

    let (revoked, stdout, stderr) = child_with_exit("emit-revoked", EMIT, 2)?;
    assert!(revoked["outcome"] == "denied");
    assert!(revoked["issued_then_revoked_denied"] == true);
    assert!(revoked["receipt"]["effect_requests"] == 1);
    assert!(revoked["receipt"]["protected_operations"] == 0);
    assert!(revoked["receipt"]["denied_requests"] == 1);
    assert!(revoked["receipt"]["ledger_entries"] == 0);
    redacted(&stdout, &stderr, EMIT, &["private-ledger-marker"]);

    let (invalid, stdout, stderr) = child_with_exit("emit", "not a module", 2)?;
    assert!(invalid["outcome"] == "invalid-input");
    assert!(invalid["receipt"]["effect_requests"] == 0);
    assert!(invalid["receipt"]["protected_operations"] == 0);
    redacted(&stdout, &stderr, "not a module", &[]);
    Ok(())
}

#[test]
fn effect_request_quota_denies_257th_checked_callback_after_256_commits()
    -> Result<(), Box<dyn std::error::Error>> {
    let changed = EMIT.replace("1 +", "2 +");
    let (unselected, stdout, stderr) = child("emit-request-denied", &changed)?;
    assert!(unselected["outcome"] == "invalid-input");
    assert!(unselected["receipt"]["effect_requests"] == 0);
    assert!(unselected["receipt"]["protected_operations"] == 0);
    redacted(&stdout, &stderr, &changed, &["private-ledger-marker"]);

    let (failure, stdout, stderr) = child("emit-request-denied", EMIT)?;
    assert!(failure["outcome"] == "guest-trapped");
    assert!(failure["operation"] == "test.emit");
    assert!(failure["guest_trapped"] == true);
    assert!(failure["guest_failure_kind"] == "source-effect-denied");
    assert!(failure["receipt"]["effect_requests"] == 257);
    assert!(failure["receipt"]["protected_operations"] == 256);
    assert!(failure["receipt"]["denied_requests"] == 1);
    assert!(failure["receipt"]["succeeded"] == 256);
    assert!(failure["receipt"]["ledger_entries"] == 256);
    assert!(failure["receipt"]["ledger_bytes"] == 5_376);
    assert!(failure["receipt"]["unknown_after_attempt"] == 0);
    assert!(failure.get("issued_value").is_none(),
        "the denied callback must not deliver the prior successful I64");
    redacted(&stdout, &stderr, EMIT, &["private-ledger-marker"]);
    Ok(())
}

fn redacted(stdout: &[u8], stderr: &[u8], source: &str, markers: &[&str]) {
    let combined = [stdout, stderr].concat();
    let text = String::from_utf8(combined).unwrap_or_else(|_|
        panic!("Worker host output is not valid UTF-8"));
    assert!(!text.contains(source), "raw candidate source escaped");
    for marker in markers {
        assert!(!text.contains(marker), "private Text escaped");
    }
}

#[test]
fn checked_installed_emit_requires_a_fresh_issuer_and_revocation() -> Result<(), Box<dyn std::error::Error>> {
    let (report, stdout, stderr) = child("emit", EMIT)?;
    assert!(report["operation"] == "test.emit");
    assert!(report["subject"]["adapter_identity"] == "worker-test-emit@1.0.0");
    assert!(report["subject"]["adapter_slot"] == 4);
    assert!(report["subject"]["source_generation"] == 1);
    assert!(report["subject"]["policy_epoch"] == 1);
    assert!(report["subject"]["destination"] == "synthetic-ledger");
    assert_eq!(report["subject"]["source_sha256"].as_str().unwrap().len(), 64);
    assert!(report["subject"]["definition_identity"].as_str().unwrap().parse::<u64>()? > 0);
    assert!(report["issued_value"] == 42);
    assert!(report["cancelled_denied"] == true);
    assert!(report["unissued_denied"] == true);
    assert!(report["stale_install_denied"] == true);
    assert!(report["cross_session_denied"] == true);
    assert!(report["revoked_denied"] == true);
    assert!(report["receipt"]["effect_requests"] == 5);
    assert!(report["receipt"]["protected_operations"] == 1);
    assert!(report["receipt"]["denied_requests"] == 4);
    for key in ["source_sha256", "definition_identity", "source_generation",
        "destination", "policy_epoch", "adapter_identity", "adapter_slot"] {
        assert!(report["receipt"][key] == report["subject"][key],
            "receipt did not retain exact host-selected subject field");
    }
    redacted(&stdout, &stderr, EMIT, &["private-ledger-marker"]);
    Ok(())
}

#[test]
fn wit_owner_request_and_core_bound_effect_are_separately_denied()
    -> Result<(), Box<dyn std::error::Error>> {
    let (wit, stdout, stderr) = child("wit-export", EXPORT)?;
    assert!(wit["operation"] == "wit.request-code-export");
    assert!(wit["subject"]["adapter_identity"] == "worker-test-code-export@1.0.0");
    assert!(wit["denial"]["guest_requests"] == 1);
    assert!(wit["denial"]["candidate_body_requests"] == 0);
    assert!(wit["denial"]["protected_operations"] == 0);
    assert!(wit["denial"]["denial_recorded"] == true);
    assert!(wit["denial"]["capture_published"] == false);
    redacted(&stdout, &stderr, EXPORT, &["private-export-marker", "hidden-capture-destination",
        "blocked-destination"]);

    let (core, stdout, stderr) = child("core-export", EXPORT)?;
    assert!(core["operation"] == "core.test.code-export");
    assert!(core["denial"]["core_requests"] == 1);
    assert!(core["denial"]["core_denials"] == 1);
    assert!(core["denial"]["protected_operations"] == 0);
    assert!(core["denial"]["completed_invocations"] == 0);
    assert!(core["denial"]["capture_published"] == false);
    redacted(&stdout, &stderr, EXPORT, &["private-export-marker", "hidden-capture-destination"]);
    Ok(())
}
