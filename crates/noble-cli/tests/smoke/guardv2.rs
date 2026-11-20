//! The v2 `NonEmptyList` guard texts, executed on the real runtime.
//!
//! These controls run the exact rendered texts from
//! `.cairn/changes/bounded-applicability-guards` (and the frozen v1 texts) in a
//! live session. They do not need an admitted companion: the body and the
//! "program" cell are ordinary values, so the text-level semantics, the
//! rejection payload and the fact that the body never runs on `nil` are
//! observable directly. The companion-level invocation path (evidence
//! applicability, `op_invoke`, the v2 slot injection) additionally needs an
//! admitted list contract and the pinned Lean sandbox tools, so its case
//! execution belongs to the change's GUARD-CASE lane rather than this module.

use std::io::Write;

const SOURCE_WRAPPER: &str =
    "[ dup [ true ] [ drop drop false ] list.case [ drop 1 inl ] [ [ drop 99 ] run inr ] if ]";
const LIVE_WRAPPER: &str =
    "[ swap dup [ true ] [ drop drop false ] list.case [ drop drop 1 inl ] [ swap run inr ] if ]";
/// A subject that consumes one list and returns `99` either way. On `nil` the
/// guard must return the defined rejection instead, so a `99` result would
/// prove the body ran.
const BODY: &str = "[ [ 99 ] [ drop drop 99 ] list.case ]";

fn session(sources: &[&str]) -> Result<Vec<serde_json::Value>, Box<dyn std::error::Error>> {
    let mut input = Vec::new();
    for source in sources {
        writeln!(input, "{}", source.len())?;
        input.extend_from_slice(source.as_bytes());
    }
    let mut child = std::process::Command::new(env!("CARGO_BIN_EXE_noble"))
        .args(["session", "--framed"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(&input)?;
    }
    let output = child.wait_with_output()?;
    if !output.status.success() {
        return Err(format!(
            "session exited {:?}: {}",
            output.status.code(),
            String::from_utf8_lossy(&output.stdout)
        )
        .into());
    }
    let mut reports = Vec::new();
    for line in output.stdout.split(|byte| *byte == b'\n') {
        if line.is_empty() {
            continue;
        }
        reports.push(serde_json::from_slice(line)?);
    }
    Ok(reports)
}

/// The one result cell of a normal invocation report.
fn result_cell<'a>(
    report: &'a serde_json::Value,
    name: &'static str,
) -> Result<&'a serde_json::Value, Box<dyn std::error::Error>> {
    assert_eq!(report["outcome"], "normal", "{name}");
    let stack = report["stack"]
        .as_array()
        .ok_or_else(|| format!("{name}: stack is not an array"))?;
    assert_eq!(stack.len(), 1, "{name}: exactly one result cell");
    assert_eq!(stack[0]["type"], "Sum", "{name}");
    Ok(&stack[0])
}

/// Assert the defined rejection: `inl 1`, with no body result.
fn assert_rejection(
    report: &serde_json::Value,
    name: &'static str,
) -> Result<(), Box<dyn std::error::Error>> {
    let cell = result_cell(report, name)?;
    assert_eq!(cell["variant"], "left", "{name}");
    assert_eq!(cell["value"]["type"], "I64", "{name}");
    assert_eq!(cell["value"]["value"], "1", "{name}");
    Ok(())
}

/// Assert the accepted invocation: `inr payload` from the body.
fn assert_invocation(
    report: &serde_json::Value,
    payload: i64,
    name: &'static str,
) -> Result<(), Box<dyn std::error::Error>> {
    let cell = result_cell(report, name)?;
    assert_eq!(cell["variant"], "right", "{name}");
    assert_eq!(cell["value"]["type"], "I64", "{name}");
    assert_eq!(cell["value"]["value"], payload.to_string(), "{name}");
    Ok(())
}

#[test]
fn source_wrapper_text_rejects_empty_and_accepts_nonempty() -> Result<(), Box<dyn std::error::Error>>
{
    let empty = session(&[&format!("nil {SOURCE_WRAPPER} run")])?;
    let empty = empty.last().ok_or("no report for the empty list")?;
    assert_rejection(empty, "empty list")?;
    let nonempty = session(&[&format!("7 nil cons {SOURCE_WRAPPER} run")])?;
    let nonempty = nonempty.last().ok_or("no report for the nonempty list")?;
    assert_invocation(nonempty, 99, "nonempty list")?;
    Ok(())
}

#[test]
fn invocation_wrapper_text_preserves_the_argument_cell() -> Result<(), Box<dyn std::error::Error>> {
    // The live text runs on `[argument, Program]` and consumes both.
    let empty = session(&[&format!("nil {BODY} {LIVE_WRAPPER} run")])?;
    let empty = empty.last().ok_or("no report for the empty list")?;
    assert_rejection(empty, "empty list")?;
    let nonempty = session(&[&format!("unit nil cons {BODY} {LIVE_WRAPPER} run")])?;
    let nonempty = nonempty.last().ok_or("no report for the nonempty list")?;
    assert_invocation(nonempty, 99, "nonempty list")?;
    Ok(())
}

#[test]
fn invocation_prelude_removes_the_companion_cell_before_the_guard(
) -> Result<(), Box<dyn std::error::Error>> {
    // The companion-level path runs on `[list, companion cell, Program]` and
    // drops the companion operand cell, so the fixed live text sees exactly
    // `[list, Program]`. A Program stands in for the companion cell here: the
    // prelude is type-generic and the guard never reads that slot.
    let mut reports = session(&[
        "nil",
        BODY,
        BODY,
        "swap drop",
        &format!("{LIVE_WRAPPER} run"),
    ])?;
    let guarded = reports.pop().ok_or("no guard report")?;
    assert_rejection(&guarded, "empty list behind the prelude")?;
    let after_prelude = reports.pop().ok_or("no prelude report")?;
    let stack = after_prelude["stack"]
        .as_array()
        .ok_or("prelude stack is not an array")?;
    assert_eq!(
        stack.len(),
        2,
        "the prelude leaves the list and the Program"
    );
    assert_eq!(
        stack[0]["type"], "List",
        "the list cell stays at the bottom"
    );
    assert_eq!(stack[1]["type"], "Program", "the Program cell stays on top");
    // The nonempty case reaches the body and tags its result.
    let mut reports = session(&[
        "unit nil cons",
        BODY,
        BODY,
        "swap drop",
        &format!("{LIVE_WRAPPER} run"),
    ])?;
    let guarded = reports.pop().ok_or("no guard report")?;
    assert_invocation(&guarded, 99, "nonempty list behind the prelude")?;
    Ok(())
}

#[test]
fn v1_guard_texts_are_unchanged_at_runtime() -> Result<(), Box<dyn std::error::Error>> {
    let lt = "[ dup 9223372036854775807 = [ drop 1 inl ] [ [ drop 99 ] run inr ] if ]";
    let eq = "[ dup 5 = [ [ drop 99 ] run inr ] [ drop 1 inl ] if ]";
    for (source, accepted, name) in [
        (
            format!("9223372036854775807 {lt} run"),
            None,
            "lt-i64-max at MAX",
        ),
        (
            format!("9223372036854775806 {lt} run"),
            Some(99),
            "lt-i64-max at MAX-1",
        ),
        (format!("5 {eq} run"), Some(99), "eq-i64-literal at 5"),
        (format!("6 {eq} run"), None, "eq-i64-literal at 6"),
    ] {
        let reports = session(&[&source])?;
        let report = reports.last().ok_or("no report")?;
        match accepted {
            None => assert_rejection(report, name)?,
            Some(payload) => assert_invocation(report, payload, name)?,
        }
    }
    Ok(())
}
