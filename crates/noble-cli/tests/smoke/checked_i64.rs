use std::io::Write;

const PROPOSALS: &str = include_str!("../../../../verification/checked-i64/proposals.json");
const LEAN_EXPECTATIONS: &str =
    include_str!("../../../../proofs/foundation/NobleFoundation/CheckedI64Expectations.lean");

struct Case {
    name: String,
    operation: String,
    left: String,
    right: String,
    expected: String,
}

fn cases() -> Vec<Case> {
    let document: serde_json::Value =
        serde_json::from_str(PROPOSALS).expect("retained proposals are valid JSON");
    assert_eq!(document["schema"], "noble-checked-i64-proposals/v1");
    let rows = document["cases"]
        .as_array()
        .expect("proposal cases are an array");
    rows.iter()
        .map(|row| Case {
            name: row["name"]
                .as_str()
                .expect("case name is a string")
                .to_owned(),
            operation: row["operation"]
                .as_str()
                .expect("operation is a string")
                .to_owned(),
            left: row["left"]
                .as_str()
                .expect("left operand is an exact decimal string")
                .to_owned(),
            right: row["right"]
                .as_str()
                .expect("right operand is an exact decimal string")
                .to_owned(),
            expected: row["expected"]
                .as_str()
                .expect("expected result is a string")
                .to_owned(),
        })
        .collect()
}

fn lean_cases() -> Vec<(String, String, String, String)> {
    let table = LEAN_EXPECTATIONS
        .split_once("def retained")
        .and_then(|(_, tail)| tail.split_once('['))
        .and_then(|(_, tail)| tail.split_once(']'))
        .map(|(table, _)| table)
        .expect("Lean retained table is present");
    table
        .lines()
        .filter_map(|line| {
            let row = line.trim().trim_end_matches(',').trim();
            if row.is_empty() {
                return None;
            }
            let fields: Vec<_> = row
                .strip_prefix('(')
                .and_then(|row| row.strip_suffix(')'))
                .expect("Lean case row is a tuple")
                .split(',')
                .map(str::trim)
                .collect();
            assert_eq!(fields.len(), 4, "Lean case row has four fields");
            let string = |field: &str| {
                field
                    .strip_prefix('"')
                    .and_then(|value| value.strip_suffix('"'))
                    .expect("Lean operation and expected value are strings")
                    .to_owned()
            };
            Some((
                string(fields[0]),
                fields[1].to_owned(),
                fields[2].to_owned(),
                string(fields[3]),
            ))
        })
        .collect()
}

fn source(case: &Case, first: bool) -> String {
    let prefix = if first { "" } else { "drop " };
    let builtin = match case.operation.as_str() {
        "add" => "i64.checked-add",
        "sub" => "i64.checked-sub",
        "mul" => "i64.checked-mul",
        "neg" => "i64.checked-neg",
        "abs" => "i64.checked-abs",
        "lt" => "i64.lt",
        "divrem" => "i64.checked-div-rem",
        operation => panic!("unsupported checked-I64 operation {operation}"),
    };
    if matches!(case.operation.as_str(), "neg" | "abs") {
        format!("{prefix}{} {builtin}", case.left)
    } else {
        format!("{prefix}{} {} {builtin}", case.left, case.right)
    }
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
        .collect::<Vec<_>>();
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

fn assert_value(actual: &serde_json::Value, expected: &str, name: &str) {
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
            .expect("pair expectation has quotient and remainder");
        assert_eq!(actual["type"], "Sum", "{name}");
        assert_eq!(actual["variant"], "left", "{name}");
        assert_eq!(actual["value"]["type"], "Pair", "{name}");
        assert_eq!(actual["value"]["value"][0]["type"], "I64", "{name}");
        assert_eq!(actual["value"]["value"][0]["value"], quotient, "{name}");
        assert_eq!(actual["value"]["value"][1]["type"], "I64", "{name}");
        assert_eq!(actual["value"]["value"][1]["value"], remainder, "{name}");
    } else {
        panic!("{name}: unknown expectation encoding {expected}");
    }
}

#[test]
fn retained_checked_i64_expectations_match_lean_and_real_source_wasm() -> Result<(), Box<dyn std::error::Error>> {
    let cases = cases();
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
    assert_eq!(proposed, lean_cases(), "Wasm rows equal the Lean retained table");
    assert_eq!(cases.len(), 57);

    for optimization in ["off", "on"] {
        let reports = execute(optimization, &cases)?;
        assert_eq!(reports.len(), cases.len());
        for (report, case) in reports.iter().zip(&cases) {
            assert_eq!(report["profile"], "Checked-I64-v1", "{}", case.name);
            assert_eq!(report["stage"], "wasm", "{}", case.name);
            assert_eq!(report["outcome"], "normal", "{}", case.name);
            assert_eq!(report["guest_requests"], 0, "{}", case.name);
            assert_eq!(report["stack"].as_array().map(Vec::len), Some(1), "{}", case.name);
            assert_value(&report["stack"][0], &case.expected, &case.name);
        }
    }
    Ok(())
}

#[test]
fn checked_i64_preserves_stack_prefix_on_success_and_overflow() -> Result<(), Box<dyn std::error::Error>> {
    for optimization in ["off", "on"] {
        for (source, expected) in [
            ("97 1 -2 i64.checked-add", "ok:-1"),
            (
                "97 9223372036854775807 1 i64.checked-add",
                "error:1",
            ),
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
            assert_eq!(report["stack"].as_array().map(Vec::len), Some(2), "{source}");
            assert_eq!(report["stack"][0]["type"], "I64", "{source}");
            assert_eq!(report["stack"][0]["value"], "97", "{source}");
            assert_value(&report["stack"][1], expected, source);
        }
    }
    Ok(())
}

