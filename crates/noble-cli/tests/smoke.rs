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
    let mut stdin = child.stdin.take().ok_or("missing framed session stdin")?;
    let frame_bytes = input.to_vec();
    let writer = std::thread::spawn(move || {
        std::io::Write::write_all(&mut stdin, &frame_bytes)
    });
    // Drain reports while writing: a framed input larger than both pipe
    // capacities must not deadlock behind its own successful reports.
    let output = child.wait_with_output()?;
    writer.join().map_err(|_| std::io::Error::other("framed input writer panicked"))??;
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
fn source_module_import_rebind_keeps_old_signed_owner_and_selects_new_alias(
) -> Result<(), Box<dyn std::error::Error>> {
    let sources: &[&[u8]] = &[
        b"module value@1 [ signature get forall<S:stack> [ S -- S I64 ! pure ] export get def get [ 7 ] ]",
        b"module value@2 [ signature get forall<S:stack> [ S -- S I64 ! pure ] export get def get [ 8 ] ]",
        b"import value@1 as current",
        b"module caller@1 [ signature run forall<S:stack> [ S -- S I64 ! pure ] export run def run [ current.get ] ]",
        b"caller@1.run",
        b"drop",
        b"import value@2 as current",
        b"module caller@2 [ signature run forall<S:stack> [ S -- S I64 ! pure ] export run def run [ current.get ] ]",
        b"caller@2.run",
        b"drop",
        b"caller@1.run",
    ];
    let (status, reports) = framed_declared_reports(
        &["--signed-self-recursion", "--retained-source-bytes", "262144"],
        sources,
    )?;
    assert!(status.success());
    assert_eq!(reports.len(), sources.len());
    for index in [0, 1, 2, 3, 6, 7] {
        assert_eq!(reports[index]["outcome"], "linked");
    }
    for (index, expected) in [(4, "7"), (8, "8"), (10, "7")] {
        assert_eq!(reports[index]["stage"], "wasm");
        assert_eq!(reports[index]["outcome"], "normal");
        assert_eq!(reports[index]["stack"][0]["value"], expected);
        assert_eq!(reports[index]["guest_requests"], 0);
        assert_eq!(reports[index]["protected_operations"], 0);
    }
    Ok(())
}

#[test]
fn reverse_alias_namespace_does_not_exhaust_an_unrelated_literal_module(
) -> Result<(), Box<dyn std::error::Error>> {
    let mut exports = String::new();
    for ordinal in 0..20 {
        exports.push_str(&format!(
            " signature w{ordinal} forall<S:stack> [ S -- S I64 ! pure ] \
             export w{ordinal} def w{ordinal} [ {ordinal} ]",
        ));
    }
    let mut sources = vec![format!("module many@1 [{exports} ]").into_bytes()];
    for ordinal in (0..40).rev() {
        sources.push(format!("import many@1 as z{ordinal}").into_bytes());
    }
    sources.extend([
        b"module empty@1 [ ]".to_vec(),
        b"module one@1 [ export one def one [ 1 ] ]".to_vec(),
        b"one@1.one".to_vec(),
    ]);
    let frames = sources.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let (status, reports) = framed_declared_reports(
        &["--signed-self-recursion", "--retained-source-bytes", "262144"],
        &frames,
    )?;
    assert!(status.success());
    assert_eq!(reports.len(), frames.len());
    assert!(reports[..43].iter().all(|report| report["outcome"] == "linked"));
    assert_eq!(reports[43]["stage"], "wasm");
    assert_eq!(reports[43]["outcome"], "normal");
    assert_eq!(reports[43]["stack"][0]["value"], "1");
    assert_eq!(reports[43]["guest_requests"], 0);
    assert_eq!(reports[43]["protected_operations"], 0);
    Ok(())
}

#[test]
fn long_prefix_aliases_charge_compared_bytes_without_publishing_failed_module(
) -> Result<(), Box<dyn std::error::Error>> {
    let prefix = "n".repeat(5_000);
    let mut exports = String::new();
    for ordinal in 0..20 {
        exports.push_str(&format!(
            " signature w{ordinal} forall<S:stack> [ S -- S I64 ! pure ] \
             export w{ordinal} def w{ordinal} [ {ordinal} ]",
        ));
    }
    let mut sources = vec![format!("module {prefix}@1 [{exports} ]").into_bytes()];
    for ordinal in (0..40).rev() {
        sources.push(format!("import {prefix}@1 as {prefix}{ordinal:02}").into_bytes());
    }
    sources.extend([
        b"module empty@1 [ ]".to_vec(),
        b"module one@1 [ export one def one [ 1 ] ]".to_vec(),
        b"one@1.one".to_vec(),
        b"drop".to_vec(),
        format!(
            "module named@1 [ signature run forall<S:stack> [ S -- S I64 ! pure ] \
             export run def run [ {prefix}00.w0 ] ]",
        ).into_bytes(),
        b"named@1.run".to_vec(),
        b"drop".to_vec(),
    ]);
    let first_alias = format!("{prefix}39.w0");
    sources.push(format!(
        "module tooearly@1 [ signature run forall<S:stack> [ S -- S I64 ! pure ] \
         export run def run [ {} ] ]",
        vec![first_alias.as_str(); 9].join(" drop "),
    ).into_bytes());
    sources.push(b"module stilllive@1 [ export one def one [ 2 ] ]".to_vec());
    sources.push(b"stilllive@1.one".to_vec());
    let frames = sources.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let (status, reports) = framed_declared_reports(
        &["--signed-self-recursion", "--retained-source-bytes", "262144"],
        &frames,
    )?;
    assert_eq!(status.code(), Some(5));
    assert_eq!(reports.len(), frames.len());
    assert!(reports[..43].iter().all(|report| report["outcome"] == "linked"));
    assert_eq!(reports[43]["outcome"], "normal");
    assert_eq!(reports[43]["stack"][0]["value"], "1");
    assert_eq!(reports[45]["outcome"], "linked");
    assert_eq!(reports[46]["outcome"], "normal");
    assert_eq!(reports[46]["stack"][0]["value"], "0");
    assert_eq!(reports[48]["stage"], "resolve");
    assert_eq!(reports[48]["outcome"], "exhausted");
    assert_eq!(reports[48]["diagnostic"], "frontend work limit exceeded");
    assert_eq!(reports[49]["outcome"], "linked");
    assert_eq!(reports[50]["outcome"], "normal");
    assert_eq!(reports[50]["stack"][0]["value"], "2");
    assert_eq!(reports[50]["guest_requests"], 0);
    Ok(())
}

#[test]
fn original_signed_addition_executes_after_sealed_module_registration(
) -> Result<(), Box<dyn std::error::Error>> {
    let sources: &[&[u8]] = &[
        include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"),
            "/../../verification/calculator-exact-library/bitstep.noble")),
        include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"),
            "/../../verification/calculator-exact-library/bitpair.noble")),
        include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"),
            "/../../verification/calculator-exact-library/bitfull.noble")),
        include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"),
            "/../../verification/calculator-exact-library/bitcompare.noble")),
        include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"),
            "/../../verification/calculator-exact-library/bittrim.noble")),
        include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"),
            "/../../verification/calculator-exact-library/bitsigned.noble")),
        include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"),
            "/../../verification/calculator-exact-library/bitsum.noble")),
        include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"),
            "/../../verification/calculator-exact-library/bitaddcarry.noble")),
        include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"),
            "/../../verification/calculator-exact-library/bitorder.noble")),
        include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"),
            "/../../verification/calculator-exact-library/bitaddsigned.noble")),
        b"false true nil cons pair",
        b"false true nil cons pair",
        b"bitaddsigned@1.add",
    ];
    let (status, reports) = framed_declared_reports(
        &["--signed-self-recursion", "--retained-source-bytes", "262144"],
        sources,
    )?;
    assert!(status.success());
    assert_eq!(reports.len(), sources.len());
    assert!(reports[..10].iter().all(|report| report["outcome"] == "linked"));
    assert!(reports[10..].iter().all(|report|
        report["stage"] == "wasm" && report["outcome"] == "normal"));
    let result = &reports[12];
    assert_eq!(result["stack"][0]["value"][0]["value"], false);
    let bits = result["stack"][0]["value"][1]["value"]
        .as_array().ok_or("signed result did not return a bit list")?;
    assert_eq!(bits.iter().map(|bit| bit["value"].as_bool()).collect::<Vec<_>>(),
        vec![Some(false), Some(true)]);
    assert_eq!(result["guest_requests"], 0);
    assert_eq!(result["host_requests"], 0);
    assert_eq!(result["protected_operations"], 0);
    assert!(result["metrics"]["allocation_peak_bytes"].as_u64()
        .is_some_and(|bytes| bytes <= 65_536));
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
