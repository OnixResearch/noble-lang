//! Opt-in size-change evidence rendering (issue #101).
//!
//! The checker itself is `noble_contracts::source::size_change_evidence`: it classifies each
//! self-call site of every recursive definition a module installed and decides the model's
//! shared-measure criterion. This module turns that evidence into the report field, attaches the
//! Lean certificate the front end generates, and — only when the caller names a Lean binary — has
//! that binary re-check the certificate.
//!
//! The field is present only under `--size-change-evidence`, so ordinary execution never carries
//! it and never runs Lean. A refusal is a report either way: the definition's verdict is
//! `unsupported` or `refused`, never silent.

/// One definition's report entry.
fn definition(entry: &noble_contracts::source::Evidence) -> crate::workflow::encoding::Json {
    use crate::workflow::encoding::{string, Json};
    let mut fields = std::vec::Vec::from([
        ("definition", string(&entry.definition)),
        (
            "measures",
            Json::Array(
                entry
                    .measures
                    .iter()
                    .map(|measure| string(measure.spelling()))
                    .collect(),
            ),
        ),
        (
            "sites",
            Json::Array(
                entry
                    .sites
                    .iter()
                    .map(|site| {
                        Json::Object(std::vec::Vec::from([
                            (
                                "span",
                                Json::Object(std::vec::Vec::from([
                                    ("start", Json::Number(u64::from(site.span.start))),
                                    ("end", Json::Number(u64::from(site.span.end))),
                                ])),
                            ),
                            (
                                "arguments",
                                Json::Array(
                                    site.arguments
                                        .iter()
                                        .map(|argument| string(argument.spelling()))
                                        .collect(),
                                ),
                            ),
                            (
                                "guarded",
                                Json::Array(
                                    site.guarded
                                        .iter()
                                        .map(|guarded| Json::Bool(*guarded))
                                        .collect(),
                                ),
                            ),
                        ]))
                    })
                    .collect(),
            ),
        ),
    ]);
    match &entry.verdict {
        noble_contracts::source::Verdict::Accepted { measure } => {
            fields.push(("verdict", string("accepted")));
            fields.push(("measure", Json::Number(u64::from(*measure))));
        }
        noble_contracts::source::Verdict::Refused { witnesses } => {
            fields.push(("verdict", string("refused")));
            fields.push((
                "witnesses",
                Json::Array(
                    witnesses
                        .iter()
                        .map(|witness| {
                            Json::Object(std::vec::Vec::from([
                                ("measure", Json::Number(u64::from(witness.measure))),
                                ("site", Json::Number(u64::from(witness.site))),
                                ("argument", string(witness.argument.spelling())),
                            ]))
                        })
                        .collect(),
                ),
            ));
        }
        noble_contracts::source::Verdict::Unsupported { reason, span } => {
            fields.push(("verdict", string("unsupported")));
            fields.push(("reason", string(reason.spelling())));
            fields.push((
                "span",
                Json::Object(std::vec::Vec::from([
                    ("start", Json::Number(u64::from(span.start))),
                    ("end", Json::Number(u64::from(span.end))),
                ])),
            ));
        }
    }
    Json::Object(fields)
}

/// A refusal names what failed; the definition's own evidence stays in the report.
fn refused(message: std::string::String) -> super::super::output::Failure {
    super::super::output::Failure::new(
        super::super::output::ErrorContext {
            stage: "check",
            outcome: "unsupported",
        },
        message,
    )
}

/// Read the binary bytes the report binds, refusing an oversized candidate first.
fn read_binary(path: &std::path::Path) -> Result<std::vec::Vec<u8>, super::super::output::Failure> {
    const MAX_BINARY_BYTES: u64 = 1 << 30;
    let metadata = std::fs::metadata(path)
        .map_err(|error| refused(std::format!("cannot stat {}: {error}", path.display())))?;
    if metadata.len() > MAX_BINARY_BYTES {
        return Err(refused(std::format!(
            "{} is {} bytes, over the {} byte binding budget",
            path.display(),
            metadata.len(),
            MAX_BINARY_BYTES
        )));
    }
    std::fs::read(path)
        .map_err(|error| refused(std::format!("cannot read {}: {error}", path.display())))
}

/// The last lines of a failed Lean run, bounded so the report stays readable.
fn message_tail(bytes: &[u8]) -> std::string::String {
    const MAX_TAIL: usize = 800;
    let text = String::from_utf8_lossy(bytes);
    let trimmed = text.trim();
    let mut start = trimmed.len().saturating_sub(MAX_TAIL);
    while !trimmed.is_char_boundary(start) {
        start += 1;
    }
    std::string::String::from(&trimmed[start..])
}

/// The selected Lean binary's reported version.
fn lean_version(
    binary: &std::path::Path,
) -> Result<std::string::String, super::super::output::Failure> {
    let output = std::process::Command::new(binary)
        .arg("--version")
        .output()
        .map_err(|error| {
            refused(std::format!(
                "cannot run {} --version: {error}",
                binary.display()
            ))
        })?;
    if !output.status.success() {
        return Err(refused(std::format!(
            "{} --version failed ({}): {}",
            binary.display(),
            output.status,
            message_tail(&output.stderr)
        )));
    }
    match std::str::from_utf8(&output.stdout) {
        Ok(text) => text
            .lines()
            .next()
            .map(std::string::String::from)
            .ok_or_else(|| refused(std::string::String::from("Lean printed no version line"))),
        Err(_) => Err(refused(std::string::String::from(
            "Lean printed a non-UTF-8 version line",
        ))),
    }
}

/// Re-check the certificate with the caller's Lean against the caller's library.
fn check_with_lean(
    certificate: &std::path::Path,
    binary: &std::path::Path,
    library: &std::path::Path,
) -> Result<crate::workflow::encoding::Json, super::super::output::Failure> {
    use crate::workflow::encoding::{string, Json};
    let bytes = read_binary(binary)?;
    let version = lean_version(binary)?;
    let output = std::process::Command::new(binary)
        .arg(certificate)
        .env("LEAN_PATH", library)
        .output()
        .map_err(|error| {
            refused(std::format!(
                "cannot run {} on {}: {error}",
                binary.display(),
                certificate.display()
            ))
        })?;
    if !output.status.success() {
        return Err(refused(std::format!(
            "Lean refused the size-change certificate ({}): {}",
            output.status,
            message_tail(&output.stderr)
        )));
    }
    Ok(Json::Object(std::vec::Vec::from([
        ("passed", Json::Bool(true)),
        ("binary", string(binary.display().to_string())),
        (
            "binary_sha256",
            string(crate::workflow::intrinsic::sha256(&bytes)),
        ),
        ("version", string(version)),
        ("library", string(library.display().to_string())),
        (
            "basis",
            string(
                "checked by this exact binary with LEAN_PATH set to this library path; the library's \
                 sources are bound by digest in the verification lane, not here",
            ),
        ),
    ])))
}

/// The report's `size_change` field: the verdict per definition, the attached certificate, and the
/// Lean re-check when one was requested.
pub(super) fn report(
    evidence: &[noble_contracts::source::Evidence],
    submission: u64,
    options: &super::super::arguments::Options,
) -> Result<crate::workflow::encoding::Json, super::super::output::Failure> {
    use crate::workflow::encoding::{string, Json};
    let directory = options.size_change_certificate.as_ref().ok_or_else(|| {
        refused(std::string::String::from(
            "--size-change-evidence requires --size-change-certificate NEW_DIRECTORY",
        ))
    })?;
    if let Err(error) = std::fs::create_dir(directory) {
        if error.kind() != std::io::ErrorKind::AlreadyExists || !directory.is_dir() {
            return Err(refused(std::format!(
                "cannot create the size-change certificate directory {}: {error}",
                directory.display()
            )));
        }
    }
    let path = directory.join(std::format!("submission-{submission}.lean"));
    let certificate = noble_contracts::source::certificate(evidence);
    let mut written = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|error| {
            refused(std::format!(
                "cannot create the size-change certificate {}: {error}",
                path.display()
            ))
        })?;
    std::io::Write::write_all(&mut written, certificate.as_bytes()).map_err(|error| {
        refused(std::format!(
            "cannot write the size-change certificate {}: {error}",
            path.display()
        ))
    })?;
    drop(written);
    let var = options
        .size_change_lean
        .as_ref()
        .zip(options.size_change_lean_library.as_ref());
    let checked = match var {
        Some((binary, library)) => check_with_lean(&path, binary, library)?,
        None => Json::Null,
    };
    Ok(Json::Object(std::vec::Vec::from([
        ("schema", string("noble-size-change/v1")),
        (
            "criterion",
            string("shared-measure: one declared measure decreases at every self-call site"),
        ),
        (
            "profile",
            string("list structure and guarded i64 counters under one stack prefix"),
        ),
        (
            "definitions",
            Json::Array(evidence.iter().map(definition).collect()),
        ),
        (
            "certificate",
            Json::Object(std::vec::Vec::from([
                ("path", string(path.display().to_string())),
                (
                    "sha256",
                    string(crate::workflow::intrinsic::sha256(certificate.as_bytes())),
                ),
                (
                    "bytes",
                    Json::Number(u64::try_from(certificate.len()).unwrap_or(0)),
                ),
            ])),
        ),
        ("lean_check", checked),
    ])))
}
