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
fn word_and_branch_failures_report_constraints_and_honest_value_origins(
) -> Result<(), Box<dyn std::error::Error>> {
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

fn padded_declared_module(name: &str, value: i64, length: usize) -> String {
    let mut source = format!("module {name}@1 [ export value def value [ {value} ] # ");
    let closing = "\n ]";
    source.extend(std::iter::repeat_n(
        'x',
        length - source.len() - closing.len(),
    ));
    source.push_str(closing);
    source
}

fn framed_declared_reports(
    flags: &[&str],
    sources: &[&[u8]],
) -> Result<(std::process::ExitStatus, Vec<serde_json::Value>), Box<dyn std::error::Error>> {
    use std::io::Write;
    let mut framed = Vec::new();
    for source in sources {
        writeln!(framed, "{}", source.len())?;
        framed.extend_from_slice(source);
    }
    framed_declared_input(flags, &framed)
}

fn framed_declared_input(
    flags: &[&str],
    input: &[u8],
) -> Result<(std::process::ExitStatus, Vec<serde_json::Value>), Box<dyn std::error::Error>> {
    use std::io::Write;
    let mut child = std::process::Command::new(env!("CARGO_BIN_EXE_noble"))
        .args([
            "session",
            "--framed",
            "--declared-modules",
            "--text-byte-cursor",
        ])
        .args(flags)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()?;
    {
        let stdin = child.stdin.as_mut().ok_or("missing framed session stdin")?;
        stdin.write_all(input)?;
    }
    drop(child.stdin.take());
    let output = child.wait_with_output()?;
    let reports = std::str::from_utf8(&output.stdout)?
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<Vec<_>, _>>()?;
    Ok((output.status, reports))
}

#[test]
fn declared_retained_source_opt_in_preserves_default_and_explicit_input_caps(
) -> Result<(), Box<dyn std::error::Error>> {
    let first = padded_declared_module("first", 1, 40_000);
    let second = padded_declared_module("second", 2, 30_000);
    for (flags, opted_in) in [
        (&[][..], false),
        (&["--source-bytes", "50000"][..], false),
        (&["--retained-source-bytes", "262144"][..], true),
        (
            &[
                "--source-bytes",
                "50000",
                "--retained-source-bytes",
                "262144",
            ][..],
            true,
        ),
    ] {
        let expression = if opted_in {
            b"second@1.value".as_slice()
        } else {
            b"first@1.value".as_slice()
        };
        let (status, reports) =
            framed_declared_reports(flags, &[first.as_bytes(), second.as_bytes(), expression])?;
        assert_eq!(reports.len(), 3);
        assert_eq!(reports[0]["outcome"], "linked");
        if opted_in {
            assert!(status.success());
            assert_eq!(reports[1]["outcome"], "linked");
        } else {
            assert_eq!(status.code(), Some(2));
            assert_eq!(reports[1]["stage"], "check");
            assert!(reports[1]["diagnostic"]
                .as_str()
                .ok_or("missing source retention refusal diagnostic")?
                .contains("retained module and definition byte limit"));
            assert_eq!(reports[1]["prior_namespace"], "unchanged");
        }
        assert_eq!(reports[2]["stage"], "wasm");
        assert_eq!(reports[2]["outcome"], "normal");
        assert_eq!(
            reports[2]["stack"][0]["value"],
            if opted_in { "2" } else { "1" }
        );
    }

    let oversized = padded_declared_module("oversized", 0, 65_537);
    let (status, reports) = framed_declared_reports(
        &["--retained-source-bytes", "262144"],
        &[first.as_bytes(), oversized.as_bytes(), b"first@1.value"],
    )?;
    assert_eq!(status.code(), Some(5));
    assert_eq!(reports.len(), 3);
    assert_eq!(reports[1]["submission"], 2);
    assert_eq!(reports[1]["stage"], "parse");
    assert_eq!(reports[1]["outcome"], "exhausted");
    assert_eq!(reports[1]["prior_stack"], "unchanged");
    assert_eq!(reports[1]["prior_namespace"], "unchanged");
    assert_eq!(reports[1]["guest_requests"], 0);
    assert_eq!(reports[2]["submission"], 3);
    assert_eq!(reports[2]["outcome"], "normal");
    assert_eq!(reports[2]["stack"][0]["value"], "1");

    Ok(())
}

#[test]
fn declared_frame_above_safe_drain_ceiling_and_truncated_bounded_frame_remain_terminal(
) -> Result<(), Box<dyn std::error::Error>> {
    use std::io::Write;
    let next = b"1\n1";
    let mut huge = b"4294967295\n".to_vec();
    huge.extend_from_slice(next);
    let (status, reports) = framed_declared_input(&["--retained-source-bytes", "262144"], &huge)?;
    assert_eq!(status.code(), Some(5));
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0]["stage"], "parse");
    assert_eq!(reports[0]["outcome"], "exhausted");
    assert_eq!(reports[0]["prior_namespace"], "unchanged");
    let (status, reports) =
        framed_declared_input(&["--retained-source-bytes", "262144"], b"65537\nshort")?;
    assert_eq!(status.code(), Some(2));
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0]["stage"], "shell");
    assert_eq!(reports[0]["outcome"], "internal-failure");
    assert_eq!(reports[0]["prior_namespace"], "unchanged");

    let mut ordinary = std::process::Command::new(env!("CARGO_BIN_EXE_noble"))
        .args(["session", "--framed"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()?;
    ordinary
        .stdin
        .as_mut()
        .ok_or("ordinary session stdin")?
        .write_all(b"65537\n1\n1")?;
    drop(ordinary.stdin.take());
    let output = ordinary.wait_with_output()?;
    assert_eq!(output.status.code(), Some(5));
    let ordinary_reports = std::str::from_utf8(&output.stdout)?
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<Vec<serde_json::Value>, _>>()?;
    assert_eq!(ordinary_reports.len(), 1);
    assert_eq!(ordinary_reports[0]["stage"], "parse");
    assert_eq!(ordinary_reports[0]["outcome"], "exhausted");
    Ok(())
}

#[test]
fn certified_signed_alias_rebind_keeps_captured_old_program_and_current_new_owner(
) -> Result<(), Box<dyn std::error::Error>> {
    let sources: &[&[u8]] = &[
        b"module m@1 [ signature addone forall<S:stack> [ S I64 -- S I64 ! pure ] export addone def addone [ dup 0 = [ 1 + ] [ 1 - addone 1 + ] if ] ]",
        b"0 m@1.addone drop",
        b"module m@2 [ signature addone forall<S:stack> [ S I64 -- S I64 ! pure ] export addone def addone [ dup 0 = [ 2 + ] [ 1 - addone 1 + ] if ] ]",
        b"0 m@2.addone drop",
        b"import m@1 as m",
        b"import m@1 as other",
        b"def once [ let x x m.addone ]",
        b"def twice [ once once ]",
        b"[ once ]",
        b"import m@2 as m",
        b"20 swap run",
        b"drop",
        b"20 other.addone",
        b"drop",
        b"20 twice",
    ];
    let (status, reports) = framed_declared_reports(
        &[
            "--signed-self-recursion",
            "--retained-source-bytes",
            "262144",
        ],
        sources,
    )?;
    assert!(status.success());
    assert_eq!(reports.len(), sources.len());
    assert_eq!(reports[0]["resolved_module"]["identity"], "1");
    assert_eq!(reports[1]["outcome"], "normal");
    assert_eq!(reports[2]["resolved_module"]["identity"], "2");
    assert_eq!(reports[3]["outcome"], "normal");
    assert_eq!(reports[4]["resolved_module"]["identity"], "1");
    assert_eq!(reports[5]["resolved_module"]["identity"], "1");
    assert_eq!(reports[8]["stack"][0]["type"], "Program");
    assert_eq!(reports[9]["resolved_module"]["identity"], "2");
    assert_eq!(reports[10]["stack"][0]["value"], "21");
    assert_eq!(reports[12]["stack"][0]["value"], "21");
    assert_eq!(reports[14]["stack"][0]["value"], "24");
    for report in &reports {
        assert_eq!(report["guest_requests"], 0);
        assert_eq!(report["host_requests"], 0);
        assert_eq!(report["protected_operations"], 0);
    }
    Ok(())
}

#[test]
fn retained_source_flag_rejects_nonopted_and_out_of_range_settings(
) -> Result<(), Box<dyn std::error::Error>> {
    for flags in [
        &["session", "--framed", "--retained-source-bytes", "262144"][..],
        &[
            "session",
            "--framed",
            "--declared-modules",
            "--text-byte-cursor",
            "--retained-source-bytes",
            "0",
        ][..],
        &[
            "session",
            "--framed",
            "--declared-modules",
            "--text-byte-cursor",
            "--retained-source-bytes",
            "262145",
        ][..],
    ] {
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_noble"))
            .args(flags)
            .output()?;
        assert_eq!(output.status.code(), Some(2));
        assert!(!std::str::from_utf8(&output.stdout)?.contains("\"outcome\":\"linked\""));
    }
    Ok(())
}
