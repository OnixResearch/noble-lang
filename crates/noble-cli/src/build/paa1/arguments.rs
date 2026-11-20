//! PAA-1 argument decoding, independent of any check the consumer performs.
//!
//! Grammar:
//! `noble admit-proof-artifact CANDIDATE --law FILE --law-digest SHA256
//! --source FILE --contract FILE --proof FILE --pins FILE --call FILE
//! --argument I64 [--manifest FILE] [--opt off|on] [--policy-revision N]
//! [--timeout-ms N]`
//! and the release-owner preparation route
//! `noble admit-proof-artifact --emit-pins FILE [--cli-sha256 SHA256] [--opt off|on]`.
//!
//! Every admission operand occurs exactly once. There is no positional operand
//! besides the candidate artifact and no route that admits without a frozen
//! law, a consumer-selected source, contract, proof and pin document.

pub(crate) struct Options {
    pub(crate) artifact: std::path::PathBuf,
    pub(crate) law: std::path::PathBuf,
    /// The owner-supplied expected digest, normalized to lower-case hex.
    pub(crate) law_digest: std::string::String,
    pub(crate) source: std::path::PathBuf,
    pub(crate) contract: std::path::PathBuf,
    pub(crate) proof: std::path::PathBuf,
    pub(crate) pins: std::path::PathBuf,
    /// The candidate certified-call artifact the consumer executes.
    pub(crate) call: std::path::PathBuf,
    /// The host-selected argument the certified call runs the subject on.
    pub(crate) argument: i64,
    pub(crate) manifest: Option<std::path::PathBuf>,
    pub(crate) optimized: bool,
    pub(crate) policy_revision: u32,
    pub(crate) timeout_ms: u64,
}

/// The release-owner preparation route that writes the pin document the
/// admission route then verifies.
pub(crate) struct Pins {
    pub(crate) output: std::path::PathBuf,
    /// Record this digest instead of the running executable's, so the owner can
    /// pin a frozen CLI receipt.
    pub(crate) cli_sha256: Option<std::string::String>,
    pub(crate) optimized: bool,
}

pub(crate) enum Mode {
    Admit(Options),
    EmitPins(Pins),
}

fn usage() -> crate::workflow::output::Failure {
    crate::workflow::output::Failure::error("invalid-envelope", super::USAGE.into())
}

#[expect(
    tigerstyle::raw_arithmetic_overflow,
    reason = "Owner: noble-maintainers; the loop guard proves index < arguments.len(), so index + 1 <= arguments.len() <= usize::MAX. get reports a missing value when the successor equals the slice length."
)]
fn option_value(
    arguments: &[std::ffi::OsString],
    index: usize,
) -> Result<&std::ffi::OsStr, crate::workflow::output::Failure> {
    arguments
        .get(index + 1)
        .map(std::ffi::OsString::as_os_str)
        .ok_or_else(usage)
}

fn timeout(value: &std::ffi::OsStr) -> Result<u64, crate::workflow::output::Failure> {
    match value.to_str().and_then(|text| text.parse::<u64>().ok()) {
        Some(value) if value > 0 && value <= crate::workflow::MAX_TIMEOUT => Ok(value),
        Some(_) | None => Err(crate::workflow::output::Failure::error(
            "invalid-envelope",
            "timeout must be an integer from 1 through 600000 ms".into(),
        )),
    }
}

fn revision(value: &std::ffi::OsStr) -> Result<u32, crate::workflow::output::Failure> {
    match value.to_str().and_then(|text| text.parse::<u32>().ok()) {
        Some(value) if value > 0 => Ok(value),
        Some(_) | None => Err(crate::workflow::output::Failure::error(
            "invalid-envelope",
            "policy revision must be a positive 32-bit integer".into(),
        )),
    }
}

fn argument(value: &std::ffi::OsStr) -> Result<i64, crate::workflow::output::Failure> {
    value
        .to_str()
        .and_then(|text| text.parse::<i64>().ok())
        .ok_or_else(|| {
            crate::workflow::output::Failure::error(
                "invalid-envelope",
                "the certified-call argument must be a signed 64-bit integer".into(),
            )
        })
}

/// Decode one exact expected digest: 64 hex digits, case-insensitive.
fn digest(
    value: &std::ffi::OsStr,
) -> Result<std::string::String, crate::workflow::output::Failure> {
    let text = value.to_str().ok_or_else(usage)?;
    if text.len() != 64 {
        return Err(crate::workflow::output::Failure::error(
            "invalid-envelope",
            "an expected digest must be exactly 64 hexadecimal digits".into(),
        ));
    }
    let mut normalized = std::string::String::with_capacity(64);
    for character in text.chars() {
        normalized.push(match character {
            '0'..='9' | 'a'..='f' | 'A'..='F' => character.to_ascii_lowercase(),
            _ => {
                return Err(crate::workflow::output::Failure::error(
                    "invalid-envelope",
                    "an expected digest must be exactly 64 hexadecimal digits".into(),
                ))
            }
        });
    }
    Ok(normalized)
}

fn optimized(value: &std::ffi::OsStr) -> Result<bool, crate::workflow::output::Failure> {
    match value.to_str() {
        Some("on") => Ok(true),
        Some("off") => Ok(false),
        _ => Err(usage()),
    }
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; parse rejects missing values, duplicate options, positional operands and malformed digests, arguments and revisions with typed envelope failures while preserving OS path bytes. Invoker argument validity must not be asserted."
)]
pub(crate) fn parse(
    arguments: &[std::ffi::OsString],
) -> Result<Mode, crate::workflow::output::Failure> {
    let mut artifact: Option<std::path::PathBuf> = None;
    let mut law: Option<std::path::PathBuf> = None;
    let mut law_digest: Option<std::string::String> = None;
    let mut source: Option<std::path::PathBuf> = None;
    let mut contract: Option<std::path::PathBuf> = None;
    let mut proof: Option<std::path::PathBuf> = None;
    let mut pins: Option<std::path::PathBuf> = None;
    let mut call: Option<std::path::PathBuf> = None;
    let mut argument_value: Option<i64> = None;
    let mut manifest: Option<std::path::PathBuf> = None;
    let mut emit_pins: Option<std::path::PathBuf> = None;
    let mut cli_sha256: Option<std::string::String> = None;
    let mut opt = false;
    let mut has_opt = false;
    let mut policy_revision = crate::build::report::POLICY_REVISION;
    let mut has_policy_revision = false;
    let mut timeout_ms = crate::workflow::DEFAULT_TIMEOUT;
    let mut has_timeout = false;
    let mut index = 1;
    while index < arguments.len() {
        let name = arguments[index]
            .to_str()
            .filter(|name| name.starts_with("--"));
        match name {
            Some("--law") if law.is_none() => {
                law = Some(std::path::PathBuf::from(attempt!(option_value(
                    arguments, index
                ))));
                index += 2;
            }
            Some("--law-digest") if law_digest.is_none() => {
                law_digest = Some(attempt!(digest(attempt!(option_value(arguments, index)))));
                index += 2;
            }
            Some("--source") if source.is_none() => {
                source = Some(std::path::PathBuf::from(attempt!(option_value(
                    arguments, index
                ))));
                index += 2;
            }
            Some("--contract") if contract.is_none() => {
                contract = Some(std::path::PathBuf::from(attempt!(option_value(
                    arguments, index
                ))));
                index += 2;
            }
            Some("--proof") if proof.is_none() => {
                proof = Some(std::path::PathBuf::from(attempt!(option_value(
                    arguments, index
                ))));
                index += 2;
            }
            Some("--pins") if pins.is_none() => {
                pins = Some(std::path::PathBuf::from(attempt!(option_value(
                    arguments, index
                ))));
                index += 2;
            }
            Some("--call") if call.is_none() => {
                call = Some(std::path::PathBuf::from(attempt!(option_value(
                    arguments, index
                ))));
                index += 2;
            }
            Some("--argument") if argument_value.is_none() => {
                argument_value = Some(attempt!(argument(attempt!(option_value(arguments, index)))));
                index += 2;
            }
            Some("--manifest") if manifest.is_none() => {
                manifest = Some(std::path::PathBuf::from(attempt!(option_value(
                    arguments, index
                ))));
                index += 2;
            }
            Some("--emit-pins") if emit_pins.is_none() => {
                emit_pins = Some(std::path::PathBuf::from(attempt!(option_value(
                    arguments, index
                ))));
                index += 2;
            }
            Some("--cli-sha256") if cli_sha256.is_none() => {
                cli_sha256 = Some(attempt!(digest(attempt!(option_value(arguments, index)))));
                index += 2;
            }
            Some("--opt") if !has_opt => {
                opt = attempt!(optimized(attempt!(option_value(arguments, index))));
                has_opt = true;
                index += 2;
            }
            Some("--policy-revision") if !has_policy_revision => {
                policy_revision = attempt!(revision(attempt!(option_value(arguments, index))));
                has_policy_revision = true;
                index += 2;
            }
            Some("--timeout-ms") if !has_timeout => {
                timeout_ms = attempt!(timeout(attempt!(option_value(arguments, index))));
                has_timeout = true;
                index += 2;
            }
            Some(_) => return Err(usage()),
            None => match &artifact {
                Some(_) => return Err(usage()),
                None => {
                    artifact = Some(std::path::PathBuf::from(&arguments[index]));
                    index += 1;
                }
            },
        }
    }
    if let Some(output) = emit_pins {
        // Preparation observes and writes one document; a candidate, law,
        // policy revision or execution operand would be ignored authority.
        if artifact.is_some()
            || law.is_some()
            || law_digest.is_some()
            || source.is_some()
            || contract.is_some()
            || proof.is_some()
            || pins.is_some()
            || call.is_some()
            || argument_value.is_some()
            || manifest.is_some()
            || has_timeout
        {
            return Err(usage());
        }
        return Ok(Mode::EmitPins(Pins {
            output,
            cli_sha256,
            optimized: opt,
        }));
    }
    if cli_sha256.is_some() {
        return Err(usage());
    }
    Ok(Mode::Admit(Options {
        artifact: attempt!(artifact.ok_or_else(usage)),
        law: attempt!(law.ok_or_else(usage)),
        law_digest: attempt!(law_digest.ok_or_else(usage)),
        source: attempt!(source.ok_or_else(usage)),
        contract: attempt!(contract.ok_or_else(usage)),
        proof: attempt!(proof.ok_or_else(usage)),
        pins: attempt!(pins.ok_or_else(usage)),
        call: attempt!(call.ok_or_else(usage)),
        argument: attempt!(argument_value.ok_or_else(usage)),
        manifest,
        optimized: opt,
        policy_revision,
        timeout_ms,
    }))
}

#[cfg(test)]
mod tests {
    use super::parse;

    fn argv(values: &[&str]) -> std::vec::Vec<std::ffi::OsString> {
        let mut out = std::vec::Vec::from([std::ffi::OsString::from("admit-proof-artifact")]);
        out.extend(values.iter().map(std::ffi::OsString::from));
        out
    }

    const DIGEST: &str = "9a8eda12c74356ca0d312be840411d4445c6f377c16ee7b96a680581b2e91eac";

    fn admission() -> std::vec::Vec<&'static str> {
        std::vec![
            "candidate.wasm",
            "--law",
            "law.json",
            "--law-digest",
            DIGEST,
            "--source",
            "s.noble",
            "--contract",
            "c.contract",
            "--proof",
            "p.lean",
            "--pins",
            "pins.json",
            "--call",
            "call.wasm",
            "--argument",
            "41",
        ]
    }

    #[test]
    fn a_complete_admission_decodes_with_defaults() {
        let Ok(super::Mode::Admit(options)) = parse(&argv(&admission())) else {
            std::panic!("the complete admission must parse");
        };
        assert_eq!(options.argument, 41);
        assert!(!options.optimized);
        assert_eq!(options.policy_revision, 1);
        assert_eq!(options.timeout_ms, crate::workflow::DEFAULT_TIMEOUT);
        assert!(options.manifest.is_none());
    }

    #[test]
    fn every_admission_operand_is_required() {
        for missing in [
            "--law",
            "--law-digest",
            "--source",
            "--contract",
            "--proof",
            "--pins",
            "--call",
            "--argument",
        ] {
            let all = admission();
            let start = all.iter().position(|item| *item == missing);
            let mut values: std::vec::Vec<&str> = std::vec::Vec::new();
            for (index, value) in all.iter().copied().enumerate() {
                if start.is_some_and(|start| index == start || index == start + 1) {
                    continue;
                }
                values.push(value);
            }
            assert!(parse(&argv(&values)).is_err(), "{missing} must be required");
        }
    }
}
