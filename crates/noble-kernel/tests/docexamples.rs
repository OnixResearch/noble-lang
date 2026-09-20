#![feature(register_tool)]
#![register_tool(tigerstyle)]
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
#[path = "docexamples/fences.rs"]
mod fences;
#[path = "docexamples/json.rs"]
mod json;
#[path = "docexamples/value.rs"]
mod value;

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

const fn outcome_name(outcome: &noble_kernel::untrusted::Outcome) -> &'static str {
    match outcome {
        noble_kernel::untrusted::Outcome::Accepted(_) => "accepted",
        noble_kernel::untrusted::Outcome::Invalid(_) => "invalid",
        noble_kernel::untrusted::Outcome::Unsupported(_) => "unsupported",
        noble_kernel::untrusted::Outcome::Exhausted(_) => "exhausted",
        noble_kernel::untrusted::Outcome::InternalFailure => "internal-failure",
    }
}

/// The bootstrap word table in `Definition` order (23 entries), for
/// coverage reporting.
const WORDS: [&str; 23] = [
    "dup",
    "drop",
    "swap",
    "dip",
    "+",
    "-",
    "*",
    "=",
    "quote",
    "compose",
    "run",
    "reflect",
    "unit",
    "pair",
    "unpair",
    "inl",
    "inr",
    "case",
    "if",
    "nil",
    "cons",
    "list.case",
    "test.emit",
];

/// Every word must keep at least one executed `noble-check` example: the
/// examples are the documented evidence per word, so a word whose
/// examples are deleted (or never written) must fail the suite, not
/// silently shrink the executed surface (task 3.2 coverage assertion).
fn require_word_coverage(covered: &[bool; WORDS.len()]) -> Result<(), String> {
    let missing: Vec<&str> = WORDS
        .iter()
        .zip(covered.iter())
        .filter(|(_, seen)| !**seen)
        .map(|(word, _)| *word)
        .collect();
    if missing.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "words without an executed noble-check example: {}",
            missing.join(", ")
        ))
    }
}

// r[verify DX-DOC-01]
// r[verify DX-DOC-02]
// r[verify VT-M2-01]
#[test]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; every stated outcome, minimum example count and per-word coverage is checked through descriptive Result failures; duplicating them as assertions would discard file/line diagnostics."
)]
fn documented_noble_check_examples_run_through_the_actual_checker() -> Result<(), String> {
    let blocks = fences::scan_all()?;
    let mut executed = 0;
    let mut illustrative = 0;
    let mut covered = [false; WORDS.len()];
    for block in &blocks {
        if block.info.split_whitespace().next() != Some("noble-check") {
            illustrative += 1;
            continue;
        }
        let expected = stated(&block.info)?;
        let parsed = json::parse(&block.body).map_err(|problem| {
            format!(
                "{}:{}: JSON parse: {problem}",
                block.file.display(),
                block.line
            )
        })?;
        let example = decode::example(&parsed).map_err(|problem| {
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
        for node in &example.candidate.nodes {
            if let noble_kernel::untrusted::Node::Invocation { def, .. } = node {
                let index = usize::try_from(def.0).map_err(|problem| {
                    format!("definition {} cannot index word coverage: {problem}", def.0)
                })?;
                if index < covered.len() {
                    covered[index] = true;
                }
            }
        }
    }
    if executed < 6 {
        return Err(format!(
            "expected at least six executed examples, found {executed}"
        ));
    }
    require_word_coverage(&covered)?;
    println!(
        "docexamples: {executed} noble-check examples executed against the actual checker, {illustrative} illustrative fenced blocks ignored, word coverage {}/{}",
        covered.iter().filter(|seen| **seen).count(),
        WORDS.len()
    );
    Ok(())
}

/// The illustrative control: the fragment document's candidate-schema block
/// is valid JSON in the example shape (it names an environment definition
/// `add` that the bootstrap checker does not provide), so executing it would
/// fail — it is skipped only because it is not tagged `noble-check`.
// r[verify DX-DOC-02]
#[test]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; this control checks the missing execution tag, successful JSON parse, failed schema decode and illustrative coverage through Result failures rather than duplicate assertions."
)]
fn illustrative_text_blocks_are_not_executed() -> Result<(), String> {
    let blocks = fences::scan_all()?;
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
    let decoded = decode::example(&parsed);
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
