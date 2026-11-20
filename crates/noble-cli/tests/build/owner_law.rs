//! Real CLI checks for the independent owner-law release consumer.
//!
//! These tests cover the law-record, comparison and route-separation refusals
//! that must hold before any Lean process could run; the source-bound
//! `verification/owner-law/gate.mjs` receipt owns the exact-checked variants.

fn root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

const LAW_V1_SHA256: &str = "9a8eda12c74356ca0d312be840411d4445c6f377c16ee7b96a680581b2e91eac";
const LAW_V2_SHA256: &str = "51f2a3158155077dcb29c8f354d356d31df3282b3c76f60c2bc8534c7932ee80";

fn law_v1() -> std::path::PathBuf {
    root().join("verification/owner-law/laws/owner-law-v1.json")
}

fn law_v2() -> std::path::PathBuf {
    root().join("verification/owner-law/laws/owner-law-v2.json")
}

fn fixture(name: &str) -> std::path::PathBuf {
    root().join("verification/owner-law/fixtures").join(name)
}

fn mc1(name: &str) -> std::path::PathBuf {
    root().join("verification/mc1").join(name)
}

fn mc2(name: &str) -> std::path::PathBuf {
    root().join("verification/mc2/contracts").join(name)
}

struct Outcome {
    status: Option<i32>,
    stdout: std::string::String,
}

fn run(command: &mut std::process::Command) -> std::io::Result<Outcome> {
    let output = command.output()?;
    Ok(Outcome {
        status: output.status.code(),
        stdout: std::string::String::from_utf8(output.stdout).map_err(std::io::Error::other)?,
    })
}

fn release(arguments: &[&std::ffi::OsStr]) -> std::io::Result<Outcome> {
    run(std::process::Command::new(env!("CARGO_BIN_EXE_noble"))
        .arg("release")
        .args(arguments))
}

fn report(outcome: &Outcome) -> serde_json::Value {
    serde_json::from_str(&outcome.stdout).expect("release prints one JSON report")
}

fn assert_refused(outcome: &Outcome, code: &str, exit: i32) {
    assert_eq!(outcome.status, Some(exit), "stdout: {}", outcome.stdout);
    let report = report(outcome);
    assert_eq!(report["schema"], "noble-owner-law-release/v1");
    assert_eq!(report["admission"], "rejected");
    assert_eq!(report["refusal"]["code"], code);
    assert_eq!(report["guest_requests"], 0);
    assert_eq!(report["protected_operations"], 0);
    assert_eq!(report["ordinary_admission"], "unchanged");
    assert_eq!(report["proof_author_status_is_authority"], false);
}

fn rewrite_law(
    source: &std::path::Path,
    mutate: impl Fn(&mut serde_json::Value),
) -> std::io::Result<std::path::PathBuf> {
    let text = std::fs::read_to_string(source)?;
    let mut value: serde_json::Value =
        serde_json::from_str(&text).map_err(std::io::Error::other)?;
    mutate(&mut value);
    let bytes = serde_json::to_vec(&value).map_err(std::io::Error::other)?;
    let root = std::path::Path::new(env!("CARGO_TARGET_TMPDIR"));
    std::fs::create_dir_all(root)?;
    let path = root.join(std::format!(
        "noble-release-{}-{:?}.json",
        std::process::id(),
        std::thread::current().id()
    ));
    std::fs::write(&path, bytes)?;
    Ok(path)
}

#[test]
fn usage_failures_are_reported_as_errors() -> std::io::Result<()> {
    for arguments in [
        vec![],
        vec!["--law".as_ref(), law_v1().as_os_str()],
        vec![
            "--law".as_ref(),
            law_v1().as_os_str(),
            "--law-digest".as_ref(),
            "not-hex".as_ref(),
        ],
    ] {
        let outcome = release(&arguments)?;
        assert_eq!(outcome.status, Some(2), "stdout: {}", outcome.stdout);
        let report = report(&outcome);
        assert_eq!(report["admission"], "error");
        assert_eq!(report["refusal"]["code"], "owner-law-argument");
    }
    Ok(())
}

#[test]
fn an_altered_law_byte_string_refuses_before_any_submission_read() -> std::io::Result<()> {
    let missing = std::path::Path::new("/nonexistent/release-submission");
    let outcome = release(&[
        "--law".as_ref(),
        law_v1().as_os_str(),
        "--law-digest".as_ref(),
        std::ffi::OsStr::new("0000000000000000000000000000000000000000000000000000000000000000"),
        "--source".as_ref(),
        missing.as_os_str(),
        "--contract".as_ref(),
        missing.as_os_str(),
        "--proof".as_ref(),
        missing.as_os_str(),
        "--artifact".as_ref(),
        missing.as_os_str(),
    ])?;
    assert_refused(&outcome, "owner-law-digest-mismatch", 1);
    assert_eq!(report(&outcome)["law_digest"]["computed"], LAW_V1_SHA256);
    Ok(())
}

#[test]
fn a_law_that_is_not_the_closed_record_refuses_after_its_digest_matches() -> std::io::Result<()> {
    let altered = rewrite_law(&law_v1(), |value| {
        value["candidate_author_can_edit_owner_law"] = serde_json::Value::Bool(true);
    })?;
    let missing = std::path::Path::new("/nonexistent/release-submission");
    let first = release(&[
        "--law".as_ref(),
        altered.as_os_str(),
        "--law-digest".as_ref(),
        std::ffi::OsStr::new("0000000000000000000000000000000000000000000000000000000000000000"),
        "--source".as_ref(),
        missing.as_os_str(),
        "--contract".as_ref(),
        missing.as_os_str(),
        "--proof".as_ref(),
        missing.as_os_str(),
        "--artifact".as_ref(),
        missing.as_os_str(),
    ])?;
    let computed = report(&first)["law_digest"]["computed"]
        .as_str()
        .expect("the refused run reports the computed digest")
        .to_owned();
    let outcome = release(&[
        "--law".as_ref(),
        altered.as_os_str(),
        "--law-digest".as_ref(),
        std::ffi::OsStr::new(&computed),
        "--source".as_ref(),
        missing.as_os_str(),
        "--contract".as_ref(),
        missing.as_os_str(),
        "--proof".as_ref(),
        missing.as_os_str(),
        "--artifact".as_ref(),
        missing.as_os_str(),
    ])?;
    assert_refused(&outcome, "owner-law-record-invalid", 1);
    std::fs::remove_file(altered)
}

fn preflight(arguments: &[&std::ffi::OsStr]) -> std::io::Result<Outcome> {
    run(std::process::Command::new(env!("CARGO_BIN_EXE_noble"))
        .arg("release")
        .args(arguments)
        .env("NOBLE_LEAN", "/nonexistent/no-selected-lean")
        .env("NOBLE_CONTRACT_LIBRARY", "/nonexistent/no-selected-library"))
}

#[test]
fn an_easier_expected_statement_is_refused_before_any_toolchain() -> std::io::Result<()> {
    let artifact = fixture("identity-statement.contract");
    let outcome = preflight(&[
        "--law".as_ref(),
        law_v1().as_os_str(),
        "--law-digest".as_ref(),
        std::ffi::OsStr::new(LAW_V1_SHA256),
        "--source".as_ref(),
        mc2("q-one.noble").as_os_str(),
        "--contract".as_ref(),
        fixture("identity-statement.contract").as_os_str(),
        "--proof".as_ref(),
        mc1("increment-proof.lean").as_os_str(),
        "--artifact".as_ref(),
        artifact.as_os_str(),
    ])?;
    assert_refused(&outcome, "owner-law-ensures-mismatch", 1);
    Ok(())
}

#[test]
fn a_weakened_precondition_is_refused() -> std::io::Result<()> {
    let artifact = fixture("weakened-precondition.contract");
    let outcome = preflight(&[
        "--law".as_ref(),
        law_v1().as_os_str(),
        "--law-digest".as_ref(),
        std::ffi::OsStr::new(LAW_V1_SHA256),
        "--source".as_ref(),
        mc2("q-one.noble").as_os_str(),
        "--contract".as_ref(),
        fixture("weakened-precondition.contract").as_os_str(),
        "--proof".as_ref(),
        mc2("nonnegative.proof.lean").as_os_str(),
        "--artifact".as_ref(),
        artifact.as_os_str(),
    ])?;
    assert_refused(&outcome, "owner-law-requires-mismatch", 1);
    Ok(())
}

#[test]
fn a_different_declaration_is_refused() -> std::io::Result<()> {
    let artifact = fixture("add-two.contract");
    let outcome = preflight(&[
        "--law".as_ref(),
        law_v1().as_os_str(),
        "--law-digest".as_ref(),
        std::ffi::OsStr::new(LAW_V1_SHA256),
        "--source".as_ref(),
        fixture("add-two.noble").as_os_str(),
        "--contract".as_ref(),
        fixture("add-two.contract").as_os_str(),
        "--proof".as_ref(),
        fixture("add-two.proof.lean").as_os_str(),
        "--artifact".as_ref(),
        artifact.as_os_str(),
    ])?;
    assert_refused(&outcome, "owner-law-declaration-mismatch", 1);
    Ok(())
}

#[test]
fn a_law_clear_submission_stops_at_the_missing_selected_toolchain() -> std::io::Result<()> {
    let artifact = fixture("add-two.contract");
    let outcome = preflight(&[
        "--law".as_ref(),
        law_v2().as_os_str(),
        "--law-digest".as_ref(),
        std::ffi::OsStr::new(LAW_V2_SHA256),
        "--source".as_ref(),
        fixture("add-two.noble").as_os_str(),
        "--contract".as_ref(),
        fixture("add-two.contract").as_os_str(),
        "--proof".as_ref(),
        fixture("add-two.proof.lean").as_os_str(),
        "--artifact".as_ref(),
        artifact.as_os_str(),
    ])?;
    assert_eq!(outcome.status, Some(4), "stdout: {}", outcome.stdout);
    let report = report(&outcome);
    assert_eq!(report["admission"], "unsupported");
    assert_eq!(
        report["refusal"]["code"],
        "owner-law-exact-check-unsupported"
    );
    // The law and every comparison matched before the toolchain selection.
    for check in report["checks"]
        .as_array()
        .expect("comparison checks are an array")
    {
        assert_eq!(check["status"], "matched", "{check}");
    }
    Ok(())
}

#[test]
fn ordinary_admission_without_the_proof_policy_is_unchanged() -> std::io::Result<()> {
    // The ordinary proof-free route on the same program is untouched: compile
    // admits the well-typed source and verify without evidence stays `unknown`
    // (not-run is not a release decision anywhere).
    let compiled = run(std::process::Command::new(env!("CARGO_BIN_EXE_noble"))
        .arg("compile")
        .arg(mc2("q-one.noble")))?;
    assert_eq!(compiled.status, Some(0), "stdout: {}", compiled.stdout);
    let verified = run(std::process::Command::new(env!("CARGO_BIN_EXE_noble"))
        .arg("verify")
        .arg(mc1("increment.noble-contract"))
        .env("NOBLE_LEAN", "/nonexistent/no-selected-lean")
        .env("NOBLE_CONTRACT_LIBRARY", "/nonexistent/no-selected-library"))?;
    assert_eq!(verified.status, Some(3), "stdout: {}", verified.stdout);
    assert!(
        verified.stdout.contains("\"outcome\":\"unknown\""),
        "{}",
        verified.stdout
    );
    assert!(
        verified
            .stdout
            .contains("\"schema\":\"noble-mc1-report/v1\""),
        "{}",
        verified.stdout
    );
    Ok(())
}
