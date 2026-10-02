//! One-shot, host-selected Wasm artifact admission. Candidate metadata never
//! selects source, effect authority or an engine session.

use super::output::{ErrorContext, Failure, Report};
use super::report::Value;
use crate::workflow::encoding::{object, string, Json};

const USAGE: &str = "usage: noble admit-artifact WASM --effects CLAIMS_JSON [--source HOST_SOURCE] [--allow-effects test.emit,test.abort,test.clock] [--opt off|on]";
const ARTIFACT_LIMIT: u32 = 4_194_304;
const MANIFEST_LIMIT: u32 = 1024;
const EFFECTS: [&str; 3] = ["test.emit", "test.abort", "test.clock"];

struct Options {
    artifact: std::path::PathBuf,
    manifest: std::path::PathBuf,
    source: Option<std::path::PathBuf>,
    allowed: std::vec::Vec<&'static str>,
    optimized: bool,
}

fn refusal(outcome: &'static str, message: impl AsRef<str>) -> Report {
    Report {
        outcome: outcome.into(),
        json: object([
            ("schema", string("noble-artifact-admission/v1")),
            ("profile", string("Wasm-Draft")),
            ("stage", string("admission")),
            ("outcome", string(outcome)),
            ("diagnostic", string(message)),
            ("guest_requests", Json::Number(0)),
            ("protected_operations", Json::Number(0)),
        ]).encode(),
    }
}

fn usage() -> Failure {
    Failure::new(ErrorContext { stage: "admission", outcome: "invalid-input" }, USAGE)
}

fn parse_effects(text: &str) -> Result<std::vec::Vec<&'static str>, Failure> {
    if text.is_empty() {
        return Ok(std::vec::Vec::new());
    }
    let mut effects = std::vec::Vec::new();
    for name in text.split(',') {
        let effect = attempt!(EFFECTS.iter().copied().find(|effect| *effect == name).ok_or_else(usage));
        if effects.contains(&effect) {
            return Err(usage());
        }
        // Accepted effects are distinct members of EFFECTS, so this bound is
        // never reached; it is a separate exit ahead of each push.
        if effects.len() >= EFFECTS.len() {
            return Err(usage());
        }
        effects.push(effect);
    }
    Ok(effects)
}

fn parse(arguments: &[std::ffi::OsString]) -> Result<Options, Failure> {
    if arguments.len() < 4 || arguments.len() > 10 || !arguments.len().is_multiple_of(2) {
        return Err(usage());
    }
    let mut manifest = None;
    let mut source = None;
    let mut allowed = None;
    let mut optimized = None;
    for pair in arguments[2..].chunks_exact(2) {
        let key = attempt!(pair[0].to_str().ok_or_else(usage));
        match key {
            "--effects" if manifest.is_none() => manifest = Some(std::path::PathBuf::from(&pair[1])),
            "--source" if source.is_none() => source = Some(std::path::PathBuf::from(&pair[1])),
            "--allow-effects" if allowed.is_none() => {
                allowed = Some(attempt!(parse_effects(attempt!(pair[1].to_str().ok_or_else(usage)))));
            }
            "--opt" if optimized.is_none() => {
                optimized = Some(match pair[1].to_str() {
                    Some("on") => true,
                    Some("off") => false,
                    _ => return Err(usage()),
                });
            }
            _ => return Err(usage()),
        }
    }
    Ok(Options {
        artifact: std::path::PathBuf::from(&arguments[1]),
        manifest: attempt!(manifest.ok_or_else(usage)),
        source,
        allowed: allowed.unwrap_or_default(),
        optimized: optimized.unwrap_or(false),
    })
}

fn claims(bytes: &[u8]) -> Result<std::vec::Vec<&'static str>, std::string::String> {
    let text = std::str::from_utf8(bytes).map_err(|_| "manifest is not UTF-8")?;
    let manifest = super::report::parse(text)?;
    let Value::Object(fields) = manifest else {
        return Err("candidate manifest must be an object".into());
    };
    if fields.keys().any(|key| !matches!(key.as_str(),
        "claimed_effects" | "trusted_correspondence" | "allowed" | "source" | "digest")) {
        return Err("unknown candidate manifest field".into());
    }
    for field in ["trusted_correspondence", "allowed"] {
        if fields.get(field).is_some_and(|value| !matches!(value, Value::Bool(_))) {
            return Err("candidate manifest boolean has wrong type".into());
        }
    }
    for field in ["source", "digest"] {
        if fields.get(field).is_some_and(|value| !matches!(value, Value::Text(_))) {
            return Err("candidate manifest text has wrong type".into());
        }
    }
    // These four optional fields are deliberately discarded. A candidate's
    // true value, digest, or source string is never an authority decision.
    let values = fields.get("claimed_effects")
        .and_then(Value::items).ok_or("candidate claimed_effects must be an array")?;
    if values.len() > EFFECTS.len() {
        return Err("too many claimed effects".into());
    }
    let mut effects = std::vec::Vec::new();
    for value in values {
        let name = value.text().ok_or("effect identity must be text")?;
        let effect = EFFECTS.iter().copied().find(|effect| *effect == name)
            .ok_or("unknown effect identity")?;
        if effects.contains(&effect) {
            return Err("duplicate effect identity".into());
        }
        // Accepted effects are distinct members of EFFECTS, so this bound is
        // never reached; it is a separate exit ahead of each push.
        if effects.len() >= EFFECTS.len() {
            return Err("too many claimed effects".into());
        }
        effects.push(effect);
    }
    Ok(effects)
}

fn checked_source(source: &[u8]) -> Result<std::vec::Vec<u8>, Failure> {
    let frontend = noble_contracts::source::Session::new();
    let prepared = attempt!(frontend.prepare(source, &[], super::SOURCE_LIMITS).map_err(Failure::source));
    let submission = attempt!(prepared.submission().ok_or_else(|| Failure::new(
        ErrorContext { stage: "admission", outcome: "correspondence-reject" },
        "selected source is not an executable expression",
    )));
    let compiler = noble_wasm::source::Compiler::new();
    let module = attempt!(compiler.prepare(submission).map_err(Failure::backend));
    Ok(module.wat().to_vec())
}

fn decide(arguments: &[std::ffi::OsString]) -> Result<Report, Failure> {
    let options = attempt!(parse(arguments));
    let candidate = attempt!(super::framing::read_file(&options.artifact, ARTIFACT_LIMIT));
    let manifest = attempt!(super::framing::read_file(&options.manifest, MANIFEST_LIMIT));
    let claimed = match claims(&manifest) {
        Ok(claimed) => claimed,
        Err(message) => return Ok(refusal("invalid-manifest", message)),
    };
    let source = match &options.source {
        Some(path) => attempt!(super::framing::read_file(path, super::SOURCE_LIMITS.bytes)),
        None => std::vec::Vec::new(),
    };
    let wat = if options.source.is_some() {
        match checked_source(&source) {
            Ok(wat) => wat,
            Err(error) => return Ok(refusal("correspondence-reject", error.message)),
        }
    } else {
        std::vec::Vec::new()
    };
    let Ok(module_bound) = usize::try_from(ARTIFACT_LIMIT) else {
        return Ok(refusal(
            "correspondence-reject",
            "module bound exceeds the host address space",
        ));
    };
    if wat.len() > module_bound {
        return Ok(refusal("correspondence-reject", "selected source WAT exceeds module bound"));
    }
    let mut engine = attempt!(super::worker::Engine::start_admission(options.optimized));
    engine.admit(&candidate, &wat, &source, &claimed, &options.allowed)
}

pub(crate) fn run(arguments: &[std::ffi::OsString]) -> std::process::ExitCode {
    let report = match decide(arguments) {
        Ok(report) => report,
        Err(error) => refusal(error.context.outcome, error.message),
    };
    if let Err(error) = super::output::print(&report) {
        eprintln!("noble: {}", error.message);
        return std::process::ExitCode::from(error.exit());
    }
    std::process::ExitCode::from(report.exit())
}
