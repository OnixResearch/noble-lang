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
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_noble-cli"))
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
    let usage = "usage: noble-cli --internal-budget-smoke <u32>\n";
    let budget = "budget must be an unsigned 32-bit integer\n";
    let cases: &[(&[&str], &str)] = &[
        (&[], usage),
        (&["41 [ 1 + ] run"], usage),
        (&["--unknown", "42"], usage),
        (&["--internal-budget-smoke"], budget),
        (&["--internal-budget-smoke", ""], budget),
        (&["--internal-budget-smoke", "-1"], budget),
        (&["--internal-budget-smoke", "4294967296"], budget),
        (&["--internal-budget-smoke", "1.5"], budget),
        (&["--internal-budget-smoke", " 42"], budget),
        (&["--internal-budget-smoke", "42", "extra"], usage),
    ];
    for (args, expected) in cases {
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_noble-cli"))
            .args(*args)
            .output()?;
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert_eq!(output.stderr, expected.as_bytes());
    }
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
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_noble-cli"))
        .arg("--internal-budget-smoke")
        .arg(argument)
        .output()?;
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert_eq!(
        output.stderr,
        b"budget must be an unsigned 32-bit integer\n"
    );
    Ok(())
}
