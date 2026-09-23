#![feature(register_tool)]
#![register_tool(tigerstyle)]
//! Real CLI checks for proof-required release refusal and argument decoding.

fn fixture(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../verification/mc1")
        .join(name)
}

struct FileInput<'a> {
    name: &'a str,
    contents: &'a str,
}

fn write_file(input: FileInput<'_>) -> std::io::Result<std::path::PathBuf> {
    let FileInput { name, contents } = input;
    let root = std::path::Path::new(env!("CARGO_TARGET_TMPDIR"));
    std::fs::create_dir_all(root)?;
    let path = root.join(format!("noble-build-{}-{name}", std::process::id()));
    std::fs::write(&path, contents)?;
    Ok(path)
}

struct Outcome {
    status: Option<i32>,
    stdout: String,
}

fn run(command: &mut std::process::Command) -> std::io::Result<Outcome> {
    let output = command.output()?;
    Ok(Outcome {
        status: output.status.code(),
        stdout: String::from_utf8(output.stdout).map_err(std::io::Error::other)?,
    })
}

fn build(arguments: &[&std::ffi::OsStr]) -> std::io::Result<Outcome> {
    run(std::process::Command::new(env!("CARGO_BIN_EXE_noble"))
        .arg("build")
        .args(arguments))
}

fn assert_blocked(outcome: &Outcome, expected: &str, exit: i32) {
    assert_eq!(outcome.status, Some(exit), "stdout: {}", outcome.stdout);
    assert!(
        outcome.stdout.contains("\"schema\":\"noble-mc2-build/v1\""),
        "{}",
        outcome.stdout
    );
    assert!(
        outcome
            .stdout
            .contains(&format!("\"outcome\":\"{expected}\"")),
        "{}",
        outcome.stdout
    );
    assert!(
        outcome.stdout.contains("\"allowed\":false"),
        "{}",
        outcome.stdout
    );
    assert!(
        outcome
            .stdout
            .contains("\"verified_backend_claimed\":false"),
        "{}",
        outcome.stdout
    );
}

#[test]
fn usage_failures_are_reported_as_errors() -> std::io::Result<()> {
    let destination = std::path::Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("noble-build-{}-invalid-output", std::process::id()));
    for arguments in [
        vec![],
        vec!["--require-proof", "[ 1 + ]"],
        vec!["--require-proof", "[ 1 + ]", "--contract"],
        vec![
            "--require-proof",
            "[ 1 + ]",
            "--contract",
            "a",
            "--contract",
            "b",
        ],
        vec![
            "--require-proof",
            "[ 1 + ]",
            "--contract",
            "a",
            "--wat",
            "x",
        ],
        vec![
            "--require-proof",
            "[ 1 + ]",
            "--contract",
            "a",
            "--proof",
            "p",
            "--refutation",
            "r",
        ],
    ] {
        let mut arguments_with_destination = vec!["--out".as_ref(), destination.as_os_str()];
        arguments_with_destination.extend(arguments.iter().map(std::ffi::OsStr::new));
        let outcome = build(&arguments_with_destination)?;
        assert_blocked(&outcome, "error", 2);
        assert!(
            outcome.stdout.contains("\"code\":\"usage\""),
            "{}",
            outcome.stdout
        );
        assert!(
            matches!(
                std::fs::symlink_metadata(&destination),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound
            ),
            "invalid arguments created an output destination: {}",
            destination.display()
        );
    }
    Ok(())
}

#[test]
fn required_proof_without_offer_does_not_run_and_blocks_release() -> std::io::Result<()> {
    let source = write_file(FileInput {
        name: "req-source.noble",
        contents: "[ 1 + ]\n",
    })?;
    let outcome = build(&[
        "--require-proof".as_ref(),
        source.as_os_str(),
        "--contract".as_ref(),
        fixture("increment.noble-contract").as_os_str(),
    ])?;
    assert_blocked(&outcome, "not-run", 0);
    std::fs::remove_file(source)
}

#[test]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; assert_blocked performs the shared assertions for exit status, report schema, exact unknown outcome, denied release and absence of a verified-backend claim."
)]
fn empty_offer_is_inconclusive_and_blocks_release() -> std::io::Result<()> {
    let source = write_file(FileInput {
        name: "empty-source.noble",
        contents: "[ 1 + ]\n",
    })?;
    let proof = write_file(FileInput {
        name: "empty-proof.lean",
        contents: "",
    })?;
    let outcome = build(&[
        "--require-proof".as_ref(),
        source.as_os_str(),
        "--contract".as_ref(),
        fixture("increment.noble-contract").as_os_str(),
        "--proof".as_ref(),
        proof.as_os_str(),
    ])?;
    assert_blocked(&outcome, "unknown", 3);
    std::fs::remove_file(source)?;
    std::fs::remove_file(proof)
}

#[test]
fn missing_sandbox_fails_closed_and_blocks_release() -> std::io::Result<()> {
    let source = write_file(FileInput {
        name: "sandbox-source.noble",
        contents: "[ 1 + ]\n",
    })?;
    let outcome = run(std::process::Command::new(env!("CARGO_BIN_EXE_noble"))
        .arg("build")
        .args([
            "--require-proof".as_ref(),
            source.as_os_str(),
            "--contract".as_ref(),
            fixture("increment.noble-contract").as_os_str(),
            "--proof".as_ref(),
            fixture("increment-proof.lean").as_os_str(),
        ])
        .env("NOBLE_BWRAP", "/nonexistent/missing-sandbox"))?;
    assert_blocked(&outcome, "unsupported", 4);
    std::fs::remove_file(source)
}

#[test]
fn rejected_contract_blocks_release_as_error() -> std::io::Result<()> {
    let source = write_file(FileInput {
        name: "reject-source.noble",
        contents: "[ 1 + ]\n",
    })?;
    let contract = write_file(FileInput {
        name: "malformed.noble-contract",
        contents: "(contract 1",
    })?;
    let outcome = build(&[
        "--require-proof".as_ref(),
        source.as_os_str(),
        "--contract".as_ref(),
        contract.as_os_str(),
    ])?;
    assert_blocked(&outcome, "error", 2);
    std::fs::remove_file(source)?;
    std::fs::remove_file(contract)
}
