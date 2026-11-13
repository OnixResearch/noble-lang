use std::io::Write;

struct Case {
    source: &'static str,
    variant: &'static str,
    result: &'static str,
    second: Option<&'static str>,
}

fn cases() -> [Case; 10] {
    [
        Case {
            source: "9223372036854775806 1 i64.checked-add",
            variant: "left",
            result: "9223372036854775807",
            second: None,
        },
        Case {
            source: "drop 7 3 i64.checked-sub",
            variant: "left",
            result: "4",
            second: None,
        },
        Case {
            source: "drop 6 7 i64.checked-mul",
            variant: "left",
            result: "42",
            second: None,
        },
        Case {
            source: "drop -2 i64.checked-neg",
            variant: "left",
            result: "2",
            second: None,
        },
        Case {
            source: "drop -2 i64.checked-abs",
            variant: "left",
            result: "2",
            second: None,
        },
        Case {
            source: "drop -1 0 i64.lt",
            variant: "bool",
            result: "true",
            second: None,
        },
        Case {
            source: "drop -7 2 i64.checked-div-rem",
            variant: "left",
            result: "-3",
            second: Some("-1"),
        },
        Case {
            source: "drop 9223372036854775807 1 i64.checked-add",
            variant: "right",
            result: "1",
            second: None,
        },
        Case {
            source: "drop 1 0 i64.checked-div-rem",
            variant: "right",
            result: "2",
            second: None,
        },
        Case {
            source: "drop -9223372036854775808 -1 i64.checked-div-rem",
            variant: "right",
            result: "1",
            second: None,
        },
    ]
}

fn execute(optimization: &str) -> Result<Vec<serde_json::Value>, Box<dyn std::error::Error>> {
    let mut input = Vec::new();
    for case in cases() {
        writeln!(input, "{}", case.source.len())?;
        input.extend_from_slice(case.source.as_bytes());
    }
    let mut child = std::process::Command::new(env!("CARGO_BIN_EXE_noble"))
        .args([
            "session",
            "--framed",
            "--checked-i64",
            "--opt",
            optimization,
        ])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;
    child
        .stdin
        .as_mut()
        .ok_or("missing checked-I64 session stdin")?
        .write_all(&input)?;
    drop(child.stdin.take());
    let output = child.wait_with_output()?;
    assert!(
        output.status.success(),
        "checked-I64 {optimization} session failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    std::str::from_utf8(&output.stdout)?
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<Vec<_>, _>>()
        .map_err(Into::into)
}

#[test]
fn checked_i64_builtins_execute_through_source_wasm_with_both_optimizations(
) -> Result<(), Box<dyn std::error::Error>> {
    for optimization in ["off", "on"] {
        let reports = execute(optimization)?;
        assert_eq!(reports.len(), 10);
        for (report, case) in reports.iter().zip(cases()) {
            assert_eq!(report["profile"], "Checked-I64-v1");
            assert_eq!(report["stage"], "wasm");
            assert_eq!(report["outcome"], "normal");
            assert_eq!(report["guest_requests"], 0);
            assert_eq!(report["stack"].as_array().map(Vec::len), Some(1));
            let value = &report["stack"][0];
            if case.variant == "bool" {
                assert_eq!(value["type"], "Bool");
                assert_eq!(value["value"], case.result == "true");
            } else {
                assert_eq!(value["type"], "Sum");
                assert_eq!(value["variant"], case.variant);
                if case.second.is_some() {
                    assert_eq!(value["value"]["type"], "Pair");
                    assert_eq!(value["value"]["value"][0]["value"], case.result);
                    assert_eq!(
                        value["value"]["value"][1]["value"],
                        case.second.unwrap_or("")
                    );
                } else {
                    assert_eq!(value["value"]["type"], "I64");
                    assert_eq!(value["value"]["value"], case.result);
                }
            }
        }
    }
    Ok(())
}
