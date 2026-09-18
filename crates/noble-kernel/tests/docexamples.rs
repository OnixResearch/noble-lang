//! Static documentation-example checks (DX-DOC-01/02).
//!
//! Every fenced code block tagged `noble-check` in the kernel crate's
//! sources or in [verification/m2-fragment.md] is executed against the
//! *actual* kernel checker, and its stated outcome must hold. Plain fenced
//! blocks — the fragment's illustrative rule tables and schema sketches —
//! are ignored: the illustrative control below proves one candidate-shaped
//! JSON block is skipped purely for lacking the tag. Failures retain the
//! file, line, stated expectation, and actual outcome.

#[path = "docexamples/decode.rs"]
mod decode;
#[path = "docexamples/json.rs"]
mod json;

use json::Json;
use noble_kernel::untrusted::Outcome;
use std::path::{Path, PathBuf};

/// One fenced block found by the scan.
struct Block {
    file: PathBuf,
    line: usize,
    info: String,
    body: String,
}

/// Strip one Rust doc-comment marker so fences inside doc comments scan the
/// same way as fences in Markdown.
fn strip_doc(line: &str) -> &str {
    let trimmed = line.trim_start();
    if let Some(rest) = trimmed.strip_prefix("///") {
        rest
    } else if let Some(rest) = trimmed.strip_prefix("//!") {
        rest
    } else {
        trimmed
    }
}

/// Scan one file's text for fenced blocks.
fn scan_text(file: PathBuf, text: &str, blocks: &mut Vec<Block>) {
    let mut open: Option<Block> = None;
    for (index, line) in text.lines().enumerate() {
        let stripped = strip_doc(line);
        if let Some(info) = stripped.strip_prefix("```") {
            match open.take() {
                Some(block) => blocks.push(block),
                None => {
                    open = Some(Block {
                        file: file.clone(),
                        line: index + 2,
                        info: info.trim().to_string(),
                        body: String::new(),
                    });
                }
            }
        } else if let Some(block) = open.as_mut() {
            block.body.push_str(strip_doc(line));
            block.body.push('\n');
        }
    }
}

/// Walk one directory recursively, collecting `.rs` files in sorted order.
fn collect_rust(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries = std::fs::read_dir(dir).map_err(|problem| format!("{dir:?}: {problem}"))?;
    let mut sorted: Vec<PathBuf> = entries
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .collect();
    sorted.sort();
    let mut index = 0;
    while index < sorted.len() {
        let path = sorted[index].clone();
        if path.is_dir() {
            collect_rust(&path, out)?;
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
        index += 1;
    }
    Ok(())
}

/// Scan every documented source: the kernel crate's sources and the
/// fragment definition.
fn scan_all() -> Result<Vec<Block>, String> {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files: Vec<PathBuf> = Vec::new();
    collect_rust(&manifest.join("src"), &mut files)?;
    files.push(manifest.join("../../verification/m2-fragment.md"));
    let mut blocks = Vec::new();
    for file in files {
        let text = std::fs::read_to_string(&file)
            .map_err(|problem| format!("cannot read {}: {problem}", file.display()))?;
        scan_text(file, &text, &mut blocks);
    }
    Ok(blocks)
}

/// The outcome an example states, from its `expect=` attribute.
fn stated(info: &str) -> Result<&str, String> {
    let mut found: Option<&str> = None;
    for part in info.split_whitespace() {
        if let Some(value) = part.strip_prefix("expect=") {
            found = Some(value);
        }
    }
    match found {
        Some(value) => Ok(value),
        None => Err(format!("noble-check fence misses `expect=`: info `{info}`")),
    }
}

fn outcome_name(outcome: &Outcome) -> &'static str {
    match outcome {
        Outcome::Accepted(_) => "accepted",
        Outcome::Invalid(_) => "invalid",
        Outcome::Unsupported(_) => "unsupported",
        Outcome::Exhausted(_) => "exhausted",
        Outcome::InternalFailure => "internal-failure",
    }
}

// r[verify DX-DOC-01]
// r[verify DX-DOC-02]
// r[verify VT-M2-01]
#[test]
fn documented_noble_check_examples_run_through_the_actual_checker() -> Result<(), String> {
    let blocks = scan_all()?;
    let mut executed = 0;
    let mut illustrative = 0;
    for block in &blocks {
        if block.info.split_whitespace().next() != Some("noble-check") {
            illustrative += 1;
            continue;
        }
        let expected = stated(&block.info)?;
        let parsed: Json = json::parse(&block.body).map_err(|problem| {
            format!(
                "{}:{}: JSON parse: {problem}",
                block.file.display(),
                block.line
            )
        })?;
        let example = decode::decode(&parsed).map_err(|problem| {
            format!("{}:{}: decode: {problem}", block.file.display(), block.line)
        })?;
        let env = noble_kernel::contracts::environment().map_err(|defect| format!("{defect:?}"))?;
        let outcome = noble_kernel::acceptance::check(&env, &example.request, &example.candidate);
        let actual = outcome_name(&outcome);
        if actual != expected {
            return Err(format!(
                "{}:{}: example states expect={expected}, checker returned {actual}; outcome {outcome:?}",
                block.file.display(),
                block.line
            ));
        }
        executed += 1;
    }
    if executed < 6 {
        return Err(format!(
            "expected at least six executed examples, found {executed}"
        ));
    }
    println!(
        "docexamples: {executed} noble-check examples executed against the actual checker, {illustrative} illustrative fenced blocks ignored"
    );
    Ok(())
}

/// The illustrative control: the fragment document's candidate-schema block
/// is valid JSON in the example shape (it names an environment definition
/// `add` that the bootstrap checker does not provide), so executing it would
/// fail — it is skipped only because it is not tagged `noble-check`.
// r[verify DX-DOC-02]
#[test]
fn illustrative_text_blocks_are_not_executed() -> Result<(), String> {
    let blocks = scan_all()?;
    let schema = blocks
        .iter()
        .find(|block| block.body.contains("\"def\": \"add\""))
        .ok_or("the illustrative candidate-schema block was not found")?;
    if schema.info.split_whitespace().next() == Some("noble-check") {
        return Err("the schema sketch must not be tagged noble-check".to_string());
    }
    let parsed = json::parse(&schema.body).map_err(|problem| {
        format!("control premise: schema block must parse as JSON: {problem}")
    })?;
    let decoded = decode::decode(&parsed);
    if decoded.is_ok() {
        return Err(
            "control premise: the schema block names `add`, which the bootstrap environment does not provide; decoding it should fail"
                .to_string(),
        );
    }
    println!(
        "docexamples/control: illustrative block {}:{} parses as JSON but is not executed; decoding it as evidence would fail: {:?}",
        schema.file.display(),
        schema.line,
        decoded.err()
    );
    let illustrative = blocks
        .iter()
        .filter(|block| block.info.split_whitespace().next() != Some("noble-check"))
        .count();
    if illustrative < 2 {
        return Err("expected the scan to find several illustrative blocks".to_string());
    }
    Ok(())
}
