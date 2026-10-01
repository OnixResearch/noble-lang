// r[verify VT-M1-02]
#[test]
fn shell_calls_the_real_transition() -> Result<(), std::io::Error> {
    for (input, expected) in [
        ("0", "exhausted\n"),
        ("1", "remaining:0\n"),
        ("2", "remaining:1\n"),
        ("42", "remaining:41\n"),
        ("4294967295", "remaining:4294967294\n"),
    ] {
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_noble"))
            .args(["--internal-budget-smoke", input])
            .output()?;
        assert!(output.status.success());
        assert_eq!(output.stdout, expected.as_bytes());
        assert!(output.stderr.is_empty());
    }
    Ok(())
}

// r[verify VT-M1-02]
#[test]
fn shell_rejects_invalid_input_without_transition_output() -> Result<(), std::io::Error> {
    let cases: &[&[&str]] = &[
        &[],
        &["41 [ 1 + ] run"],
        &["--unknown", "42"],
        &["--internal-budget-smoke"],
        &["--internal-budget-smoke", ""],
        &["--internal-budget-smoke", "-1"],
        &["--internal-budget-smoke", "4294967296"],
        &["--internal-budget-smoke", "1.5"],
        &["--internal-budget-smoke", " 42"],
        &["--internal-budget-smoke", "42", "extra"],
    ];
    for args in cases {
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_noble"))
            .args(*args)
            .output()?;
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
    }
    Ok(())
}

#[test]
fn conditional_join_reports_are_located_and_never_execute() -> Result<(), Box<dyn std::error::Error>>
{
    let mut child = std::process::Command::new(env!("CARGO_BIN_EXE_noble"))
        .arg("session")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()?;
    std::io::Write::write_all(
        child.stdin.as_mut().ok_or("missing session stdin")?,
        b"true [ 1 ] [ \"x\" ] if\ntrue [ 1 ] [ 1 2 ] if\ntrue [ 1 \"x\" ] [ \"x\" 1 ] if\n",
    )?;
    drop(child.stdin.take());
    let output = child.wait_with_output()?;
    assert_eq!(output.status.code(), Some(2));
    let reports = std::str::from_utf8(&output.stdout)?
        .lines()
        .collect::<Vec<_>>();
    assert_eq!(reports.len(), 3);
    for (report, span) in reports.iter().zip([
        "\"source_location\":{\"start\":19,\"end\":21}",
        "\"source_location\":{\"start\":19,\"end\":21}",
        "\"source_location\":{\"start\":25,\"end\":27}",
    ]) {
        assert!(report.contains("\"stage\":\"check\""));
        assert!(report.contains("\"outcome\":\"type-reject\""));
        assert!(report.contains("\"join\":\"if\""));
        assert!(report.contains("\"expected_stack\":\""));
        assert!(report.contains("\"actual_stack\":\""));
        assert!(report.contains(span));
        assert!(report.contains("\"guest_requests\":0"));
        assert!(report.contains("\"protected_operations\":0"));
    }

    let mut valid = std::process::Command::new(env!("CARGO_BIN_EXE_noble"))
        .arg("session")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()?;
    std::io::Write::write_all(
        valid.stdin.as_mut().ok_or("missing valid session stdin")?,
        b"true [ 1 ] [ 2 ] if\n",
    )?;
    drop(valid.stdin.take());
    let valid = valid.wait_with_output()?;
    assert!(valid.status.success());
    let report = std::str::from_utf8(&valid.stdout)?;
    assert!(report.contains("\"outcome\":\"normal\""));
    assert!(report.contains("\"type\":\"I64\",\"value\":\"1\""));
    Ok(())
}

#[test]
fn word_and_branch_failures_report_constraints_and_honest_value_origins()
-> Result<(), Box<dyn std::error::Error>> {
    let mut child = std::process::Command::new(env!("CARGO_BIN_EXE_noble"))
        .arg("session")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()?;
    std::io::Write::write_all(
        child.stdin.as_mut().ok_or("missing session stdin")?,
        b"1 true +\ntrue [ 1 ] [ false ] if\n",
    )?;
    drop(child.stdin.take());
    let output = child.wait_with_output()?;
    assert_eq!(output.status.code(), Some(2));
    let reports = std::str::from_utf8(&output.stdout)?
        .lines()
        .collect::<Vec<_>>();
    assert_eq!(reports.len(), 2);
    for report in &reports {
        assert!(report.contains("\"stage\":\"check\""));
        assert!(report.contains("\"outcome\":\"type-reject\""));
        assert!(report.contains("\"required_stack\":\""));
        assert!(report.contains("\"actual_stack\":\""));
        assert!(report.contains("\"guest_requests\":0"));
        assert!(report.contains("\"protected_operations\":0"));
    }
    assert!(reports[0].contains("\"word_or_join\":\"+\""));
    assert!(reports[0].contains("\"constraint\":\"stack-type\""));
    assert!(reports[0].contains("\"required_stack\":\"?stack I64 I64\""));
    assert!(reports[0].contains("\"actual_stack\":\"[] I64 Bool\""));
    assert!(reports[0].contains("\"source_span\":{\"start\":7,\"end\":8}"));
    assert!(reports[0].contains("\"value_origin_or_unavailable\":{\"start\":2,\"end\":6}"));
    assert!(reports[1].contains("\"word_or_join\":\"if\""));
    assert!(reports[1].contains("\"constraint\":\"branch-join\""));
    assert!(reports[1].contains("\"required_stack\":\"?stack Bool Program<"));
    assert!(reports[1].contains("\"actual_stack\":\"[] Bool Program<"));
    assert!(reports[1].contains("\"source_span\":{\"start\":21,\"end\":23}"));
    assert!(reports[1].contains("\"value_origin_or_unavailable\":\"unavailable\""));
    Ok(())
}

// The selected M1 target is Unix. This is an OS argument, not invalid Rust text.
// r[verify VT-M1-02]
#[cfg(unix)]
#[test]
fn shell_rejects_non_utf8_budget() -> Result<(), std::io::Error> {
    const INVALID_UTF8_BYTE: u8 = 0xff;
    let argument =
        <std::ffi::OsString as std::os::unix::ffi::OsStringExt>::from_vec(vec![INVALID_UTF8_BYTE]);
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_noble"))
        .arg("--internal-budget-smoke")
        .arg(argument)
        .output()?;
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    Ok(())
}
