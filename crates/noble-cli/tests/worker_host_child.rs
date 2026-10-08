use std::io::Write;
use std::process::{Command, Stdio};

const EMIT: &str = "module Ledger@1 [ require emit Text -- ! test.emit export plus def plus [ \"private-ledger-marker\" emit 1 + ] ]";
const EXPORT: &str = "module ledger@1 [ require code_export Text Text -- ! test.code-export export capture def capture [ \"private-export-marker\" \"hidden-capture-destination\" code_export ] ]";

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
    assert_eq!(result.status.code(), Some(expected_exit),
        "child exit mismatch: stdout={} stderr={}",
        String::from_utf8_lossy(&result.stdout), String::from_utf8_lossy(&result.stderr));
    assert_eq!(std::fs::read_dir(&directory)?.count(), 0,
        "test host created an export or other output file");
    std::fs::remove_dir(&directory)?;
    let report = serde_json::from_slice(&result.stdout)?;
    Ok((report, result.stdout, result.stderr))
}

#[test]
fn post_effect_result_and_callback_failure_never_erase_committed_work()
    -> Result<(), Box<dyn std::error::Error>> {
    let changed = EMIT.replace("1 +", "2 +");
    let (unselected, stdout, stderr) = child_with_exit("emit", &changed, 2)?;
    assert_eq!(unselected["outcome"], "invalid-input");
    assert_eq!(unselected["receipt"]["effect_requests"], 0);
    assert_eq!(unselected["receipt"]["protected_operations"], 0);
    redacted(&stdout, &stderr, &changed, &["private-ledger-marker"]);

    let (normal, stdout, stderr) = child("emit-wrong", &changed)?;
    assert_eq!(normal["issued_value"], 43,
        "candidate result is not a host-selected authority requirement");
    assert_eq!(normal["receipt"]["protected_operations"], 1);
    assert_eq!(normal["receipt"]["ledger_entries"], 1);
    redacted(&stdout, &stderr, &changed, &["private-ledger-marker"]);

    // The 257th callback exceeds the selected per-request quota only after
    // 256 actual appends. Its guest trap must not mint an all-zero receipt.
    let failing = format!(
        "module Ledger@1 [ require emit Text -- ! test.emit export plus def plus [ {} 1 + ] ]",
        "\"private-ledger-marker\" emit ".repeat(257));
    let (failure, stdout, stderr) = child_with_exit("emit-trap", &failing, 2)?;
    assert_eq!(failure["outcome"], "guest-trapped");
    assert_eq!(failure["operation"], "test.emit");
    assert_eq!(failure["guest_trapped"], true);
    assert_eq!(failure["guest_failure_kind"], "source-effect-denied");
    assert_eq!(failure["receipt"]["effect_requests"], 257);
    assert_eq!(failure["receipt"]["protected_operations"], 256);
    assert_eq!(failure["receipt"]["denied_requests"], 1);
    assert_eq!(failure["receipt"]["ledger_entries"], 256);
    assert_eq!(failure["receipt"]["unknown_after_attempt"], 0);
    assert!(failure.get("protected_operations").is_none(),
        "failure must not invent a standalone zero effect count");
    redacted(&stdout, &stderr, &failing, &["private-ledger-marker"]);

    let (panic, stdout, stderr) = child_with_exit("emit-panic", EMIT, 2)?;
    assert_eq!(panic["outcome"], "execution-panicked");
    assert_eq!(panic["receipt"]["effect_requests"], 1);
    assert_eq!(panic["receipt"]["protected_operations"], 1);
    assert_eq!(panic["receipt"]["ledger_entries"], 1);
    assert_eq!(panic["receipt"]["unknown_after_attempt"], 0);
    assert!(String::from_utf8_lossy(&stderr).contains("Worker host panic (redacted)"),
        "host panic was not visibly reported");
    redacted(&stdout, &stderr, EMIT, &["private-ledger-marker"]);

    let (revoked, stdout, stderr) = child_with_exit("emit-revoked", EMIT, 2)?;
    assert_eq!(revoked["outcome"], "denied");
    assert_eq!(revoked["issued_then_revoked_denied"], true);
    assert_eq!(revoked["receipt"]["effect_requests"], 1);
    assert_eq!(revoked["receipt"]["protected_operations"], 0);
    assert_eq!(revoked["receipt"]["denied_requests"], 1);
    assert_eq!(revoked["receipt"]["ledger_entries"], 0);
    redacted(&stdout, &stderr, EMIT, &["private-ledger-marker"]);

    let (invalid, stdout, stderr) = child_with_exit("emit", "not a module", 2)?;
    assert_eq!(invalid["outcome"], "invalid-input");
    assert_eq!(invalid["receipt"]["effect_requests"], 0);
    assert_eq!(invalid["receipt"]["protected_operations"], 0);
    redacted(&stdout, &stderr, "not a module", &[]);
    Ok(())
}

fn redacted(stdout: &[u8], stderr: &[u8], source: &str, markers: &[&str]) {
    let combined = [stdout, stderr].concat();
    let text = String::from_utf8(combined).unwrap();
    assert!(!text.contains(source), "raw candidate source escaped");
    for marker in markers {
        assert!(!text.contains(marker), "private Text escaped: {marker}");
    }
}

#[test]
fn checked_installed_emit_requires_a_fresh_issuer_and_revocation() -> Result<(), Box<dyn std::error::Error>> {
    let (report, stdout, stderr) = child("emit", EMIT)?;
    assert_eq!(report["operation"], "test.emit");
    assert_eq!(report["subject"]["adapter_identity"], "worker-test-emit@1.0.0");
    assert_eq!(report["subject"]["adapter_slot"], 4);
    assert_eq!(report["subject"]["source_generation"], 1);
    assert_eq!(report["subject"]["policy_epoch"], 1);
    assert_eq!(report["subject"]["destination"], "synthetic-ledger");
    assert_eq!(report["subject"]["source_sha256"].as_str().unwrap().len(), 64);
    assert!(report["subject"]["definition_identity"].as_str().unwrap().parse::<u64>()? > 0);
    assert_eq!(report["issued_value"], 42);
    assert_eq!(report["cancelled_denied"], true);
    assert_eq!(report["unissued_denied"], true);
    assert_eq!(report["stale_install_denied"], true);
    assert_eq!(report["cross_session_denied"], true);
    assert_eq!(report["revoked_denied"], true);
    assert_eq!(report["receipt"]["effect_requests"], 5);
    assert_eq!(report["receipt"]["protected_operations"], 1);
    assert_eq!(report["receipt"]["denied_requests"], 4);
    for key in ["source_sha256", "definition_identity", "source_generation",
        "destination", "policy_epoch", "adapter_identity", "adapter_slot"] {
        assert_eq!(report["receipt"][key], report["subject"][key],
            "receipt did not retain exact host-selected {key}");
    }
    redacted(&stdout, &stderr, EMIT, &["private-ledger-marker"]);
    Ok(())
}

#[test]
fn wit_owner_request_and_core_bound_effect_are_separately_denied()
    -> Result<(), Box<dyn std::error::Error>> {
    let (wit, stdout, stderr) = child("wit-export", EXPORT)?;
    assert_eq!(wit["operation"], "wit.request-code-export");
    assert_eq!(wit["subject"]["adapter_identity"], "worker-test-code-export@1.0.0");
    assert_eq!(wit["denial"]["guest_requests"], 1);
    assert_eq!(wit["denial"]["candidate_body_requests"], 0);
    assert_eq!(wit["denial"]["protected_operations"], 0);
    assert_eq!(wit["denial"]["denial_recorded"], true);
    assert_eq!(wit["denial"]["capture_published"], false);
    redacted(&stdout, &stderr, EXPORT, &["private-export-marker", "hidden-capture-destination",
        "blocked-destination"]);

    let (core, stdout, stderr) = child("core-export", EXPORT)?;
    assert_eq!(core["operation"], "core.test.code-export");
    assert_eq!(core["denial"]["core_requests"], 1);
    assert_eq!(core["denial"]["core_denials"], 1);
    assert_eq!(core["denial"]["protected_operations"], 0);
    assert_eq!(core["denial"]["completed_invocations"], 0);
    assert_eq!(core["denial"]["capture_published"], false);
    redacted(&stdout, &stderr, EXPORT, &["private-export-marker", "hidden-capture-destination"]);
    Ok(())
}
