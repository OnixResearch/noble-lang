use std::io::Write;

const PROPOSALS: &str = include_str!("../../../../../verification/checked-i64/proposals.json");

#[path = "expected.rs"]
mod expected;

struct Case {
    name: String,
    operation: String,
    left: String,
    right: String,
    expected: String,
}

fn invalid_data(message: impl Into<String>) -> Box<dyn std::error::Error> {
    Box::new(std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        message.into(),
    ))
}

fn case_field(row: &serde_json::Value, field: &str) -> Result<String, Box<dyn std::error::Error>> {
    row[field]
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| invalid_data(format!("proposal field {field} is not a string")))
}

fn cases() -> Result<Vec<Case>, Box<dyn std::error::Error>> {
    let document: serde_json::Value = serde_json::from_str(PROPOSALS)?;
    assert_eq!(document["schema"], "noble-checked-i64-proposals/v1");
    let rows = document["cases"]
        .as_array()
        .ok_or_else(|| invalid_data("proposal cases are not an array"))?;
    rows.iter()
        .map(|row| {
            Ok(Case {
                name: case_field(row, "name")?,
                operation: case_field(row, "operation")?,
                left: case_field(row, "left")?,
                right: case_field(row, "right")?,
                expected: case_field(row, "expected")?,
            })
        })
        .collect()
}

fn source(case: &Case, first: bool) -> Result<String, Box<dyn std::error::Error>> {
    let prefix = if first { "" } else { "drop " };
    let builtin = match case.operation.as_str() {
        "add" => "i64.checked-add",
        "sub" => "i64.checked-sub",
        "mul" => "i64.checked-mul",
        "neg" => "i64.checked-neg",
        "abs" => "i64.checked-abs",
        "lt" => "i64.lt",
        "divrem" => "i64.checked-div-rem",
        operation => {
            return Err(invalid_data(format!(
                "unsupported checked-I64 operation {operation}"
            )));
        }
    };
    Ok(if matches!(case.operation.as_str(), "neg" | "abs") {
        format!("{prefix}{} {builtin}", case.left)
    } else {
        format!("{prefix}{} {} {builtin}", case.left, case.right)
    })
}

pub(super) fn run_session(
    arguments: &[&str],
    sources: &[String],
) -> Result<std::process::Output, Box<dyn std::error::Error>> {
    let mut input = Vec::new();
    for source in sources {
        writeln!(input, "{}", source.len())?;
        input.extend_from_slice(source.as_bytes());
    }
    let mut child = std::process::Command::new(env!("CARGO_BIN_EXE_noble"))
        .args(arguments)
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
    Ok(child.wait_with_output()?)
}

fn execute(
    optimization: &str,
    cases: &[Case],
) -> Result<Vec<serde_json::Value>, Box<dyn std::error::Error>> {
    let sources = cases
        .iter()
        .enumerate()
        .map(|(index, case)| source(case, index == 0))
        .collect::<Result<Vec<_>, _>>()?;
    let output = run_session(
        &[
            "session",
            "--framed",
            "--checked-i64",
            "--opt",
            optimization,
        ],
        &sources,
    )?;
    assert!(
        output.status.success(),
        "checked-I64 {optimization} session failed: status={} stdout={} stderr={}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    std::str::from_utf8(&output.stdout)?
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<Vec<_>, _>>()
        .map_err(Into::into)
}

struct ValueExpectation<'a> {
    encoded: &'a str,
    case_name: &'a str,
}

fn assert_value(
    actual: &serde_json::Value,
    expectation: ValueExpectation<'_>,
) -> Result<(), Box<dyn std::error::Error>> {
    let expected = expectation.encoded;
    let name = expectation.case_name;
    if let Some(expected_bool) = expected.strip_prefix("bool:") {
        assert!(
            matches!(expected_bool, "true" | "false"),
            "{name}: malformed Bool expectation"
        );
        assert_eq!(actual["type"], "Bool", "{name}");
        assert_eq!(actual["value"], expected_bool == "true", "{name}");
    } else if let Some(payload) = expected.strip_prefix("ok:") {
        assert_eq!(actual["type"], "Sum", "{name}");
        assert_eq!(actual["variant"], "left", "{name}");
        assert_eq!(actual["value"]["type"], "I64", "{name}");
        assert_eq!(actual["value"]["value"], payload, "{name}");
    } else if let Some(code) = expected.strip_prefix("error:") {
        assert_eq!(actual["type"], "Sum", "{name}");
        assert_eq!(actual["variant"], "right", "{name}");
        assert_eq!(actual["value"]["type"], "I64", "{name}");
        assert_eq!(actual["value"]["value"], code, "{name}");
    } else if let Some(payload) = expected.strip_prefix("pair:") {
        let (quotient, remainder) = payload
            .split_once(':')
            .ok_or_else(|| invalid_data("pair expectation has no separator"))?;
        assert_eq!(actual["type"], "Sum", "{name}");
        assert_eq!(actual["variant"], "left", "{name}");
        assert_eq!(actual["value"]["type"], "Pair", "{name}");
        assert_eq!(actual["value"]["value"][0]["type"], "I64", "{name}");
        assert_eq!(actual["value"]["value"][0]["value"], quotient, "{name}");
        assert_eq!(actual["value"]["value"][1]["type"], "I64", "{name}");
        assert_eq!(actual["value"]["value"][1]["value"], remainder, "{name}");
    } else {
        return Err(invalid_data(format!(
            "{name}: unknown expectation encoding {expected}"
        )));
    }
    Ok(())
}

#[test]
fn retained_vectors_match_lean_and_real_source_wasm() -> Result<(), Box<dyn std::error::Error>> {
    let cases = cases()?;
    let proposed = cases
        .iter()
        .map(|case| {
            (
                case.operation.clone(),
                case.left.clone(),
                case.right.clone(),
                case.expected.clone(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        proposed,
        expected::lean_cases()?,
        "Wasm rows equal the Lean retained table"
    );
    assert_eq!(cases.len(), 57);

    for optimization in ["off", "on"] {
        let reports = execute(optimization, &cases)?;
        assert_eq!(reports.len(), cases.len());
        for (report, case) in reports.iter().zip(&cases) {
            assert_eq!(report["profile"], "Checked-I64-v1", "{}", case.name);
            assert_eq!(report["stage"], "wasm", "{}", case.name);
            assert_eq!(report["outcome"], "normal", "{}", case.name);
            assert_eq!(report["guest_requests"], 0, "{}", case.name);
            assert_eq!(
                report["stack"].as_array().map(Vec::len),
                Some(1),
                "{}",
                case.name
            );
            assert_value(
                &report["stack"][0],
                ValueExpectation {
                    encoded: &case.expected,
                    case_name: &case.name,
                },
            )?;
        }
    }
    Ok(())
}

#[test]
fn stack_prefix_is_preserved_on_success_and_overflow() -> Result<(), Box<dyn std::error::Error>> {
    for optimization in ["off", "on"] {
        for (source, expected) in [
            ("97 1 -2 i64.checked-add", "ok:-1"),
            ("97 9223372036854775807 1 i64.checked-add", "error:1"),
        ] {
            let output = run_session(
                &[
                    "session",
                    "--framed",
                    "--checked-i64",
                    "--opt",
                    optimization,
                ],
                &[source.to_owned()],
            )?;
            assert!(
                output.status.success(),
                "{source}, optimization {optimization}: {}",
                String::from_utf8_lossy(&output.stdout)
            );
            let report: serde_json::Value = serde_json::from_slice(&output.stdout)?;
            assert_eq!(report["outcome"], "normal", "{source}");
            assert_eq!(report["profile"], "Checked-I64-v1", "{source}");
            assert_eq!(
                report["stack"].as_array().map(Vec::len),
                Some(2),
                "{source}"
            );
            assert_eq!(report["stack"][0]["type"], "I64", "{source}");
            assert_eq!(report["stack"][0]["value"], "97", "{source}");
            assert_value(
                &report["stack"][1],
                ValueExpectation {
                    encoded: expected,
                    case_name: source,
                },
            )?;
        }
    }
    Ok(())
}
