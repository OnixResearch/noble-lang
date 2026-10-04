use std::{io::Write, path::Path, process::Command};

fn invoke(temporary: &Path, action: &str, json: &str) -> Result<(i32, String), Box<dyn std::error::Error>> {
    let path = temporary.join(format!(
        "noble-editor-{}-{}-{:?}.json",
        std::process::id(),
        action,
        std::thread::current().id(),
    ));
    let mut file = std::fs::OpenOptions::new().write(true).create_new(true).open(&path)?;
    file.write_all(json.as_bytes())?;
    drop(file);
    let output = Command::new(env!("CARGO_BIN_EXE_noble"))
        .args(["editor", action])
        .arg(&path)
        .output()?;
    std::fs::remove_file(path)?;
    Ok((
        output.status.code().unwrap_or(-1),
        String::from_utf8(output.stdout)?,
    ))
}

#[test]
fn editor_four_variants_and_hostile_transport() -> Result<(), Box<dyn std::error::Error>> {
    let temporary = std::env::temp_dir();
    let incomplete = r#"{"format":1,"nodes":[{"kind":"integer","value":1},{"kind":"hole"},{"kind":"word","value":"+"}]}"#;
    let (code, analysis) = invoke(&temporary, "analyze", incomplete)?;
    assert_eq!(code, 0, "{analysis}");
    assert!(analysis.contains("\"input_stack\":\"?stack I64\""), "{analysis}");
    assert!(analysis.contains("\"required_output_stack\":\"?stack I64 I64\""), "{analysis}");
    assert!(analysis.contains("\"required_output_suffix\":[\"I64\"]"), "{analysis}");
    assert!(analysis.contains("\"unresolved\":true"), "{analysis}");
    assert!(analysis.contains("\"accepted_program_created\":false"), "{analysis}");
    assert!(analysis.contains("\"guest_requests\":0"), "{analysis}");
    let effectful = r#"{"format":1,"nodes":[{"kind":"integer","value":1},{"kind":"hole"},{"kind":"word","value":"test.emit"}]}"#;
    let (code, alternate) = invoke(&temporary, "analyze", effectful)?;
    assert_eq!(code, 0, "{alternate}");
    assert!(alternate.contains("\"required_output_suffix\":[\"Text\"]"), "{alternate}");
    assert!(alternate.contains("\"known\":[0]"), "{alternate}");
    assert!(alternate.contains("\"unresolved\":true"), "{alternate}");

    let nested = r#"{"format":1,"nodes":[{"kind":"quotation","nodes":[{"kind":"hole"}]}]}"#;
    let (code, nested_analysis) = invoke(&temporary, "analyze", nested)?;
    assert_eq!(code, 0, "{nested_analysis}");
    assert!(nested_analysis.contains("\"unresolved\":true"));

    // Direct, nested-quotation and serialized JSON ingress all refuse
    // *before* a submission, kernel candidate, guest request or host call.
    for json in [incomplete, nested, r#"{"format":1,"nodes":[{"kind":"hole"}]}"#] {
        let (code, report) = invoke(&temporary, "admit", json)?;
        assert_eq!(code, 2, "{report}");
        assert!(report.contains("\"stage\":\"admission\""), "{report}");
        assert!(report.contains("\"accepted_program_created\":false"), "{report}");
        assert!(report.contains("\"guest_requests\":0"), "{report}");
        assert!(report.contains("\"protected_operations\":0"), "{report}");
    }
    let complete = r#"{"format":1,"nodes":[{"kind":"integer","value":1},{"kind":"integer","value":2},{"kind":"word","value":"+"}]}"#;
    let (code, executed) = invoke(&temporary, "admit", complete)?;
    assert_eq!(code, 0, "{executed}");
    assert!(executed.contains("\"outcome\":\"normal\""), "{executed}");
    assert!(executed.contains("\"type\":\"I64\",\"value\":\"3\""), "{executed}");

    for hostile in [
        r#"{"format":2,"nodes":[]}"#,
        r#"{"format":1,"nodes":[{"kind":"integer","value":1,"effect":"pure"},{"kind":"hole"}]}"#,
        r#"{"format":1,"nodes":[{"kind":"word","value":"["},{"kind":"hole"}]}"#,
        r#"{"format":1,"nodes":[{"kind":"word","value":"1 2 +"}]}"#,
        r#"{"format":1,"nodes":[{"kind":"word","value":"\u005b"}]}"#,
        r#"{"format":1,"nodes":[{"kind":"quotation","nodes":[{"kind":"hole","effect":"pure"}]}]}"#,
        r#"{"format":1,"nodes":[{"ki\u006ed":"hole","kind":"integer","value":3}]}"#,
        r#"{"format":1,"nodes":[{"kind":"hole","kind":"integer","value":3}]}"#,
        r#"{"format":1,"nodes":[{"kind":"integer","value":3}],"nodes":[]}"#,
        r#"{"format":1,"nodes":"hole"}"#,
        "!",
        &" ".repeat(65_537),
    ] {
        let (code, report) = invoke(&temporary, "admit", hostile)?;
        assert_eq!(code, 2, "{report}");
        assert!(!report.contains("\"outcome\":\"normal\""), "{report}");
        assert!(report.contains("\"guest_requests\":0"), "{report}");
    }
    Ok(())
}
