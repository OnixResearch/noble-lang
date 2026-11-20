//! Release-route argument decoding, independent of any check it performs.
//!
//! Grammar: `noble release --law FILE --law-digest SHA256 --source FILE
//! --contract FILE --proof FILE --artifact FILE [--timeout-ms N]`. Every
//! operand occurs exactly once; there is no positional operand and no
//! permissive route that admits without a frozen law, a submitted source,
//! contract, proof and already emitted artifact.

pub(super) struct Options {
    pub(super) law: std::path::PathBuf,
    /// The owner-supplied digest, normalized to lower-case hex.
    pub(super) law_digest: std::string::String,
    pub(super) source: std::path::PathBuf,
    pub(super) contract: std::path::PathBuf,
    pub(super) proof: std::path::PathBuf,
    pub(super) artifact: std::path::PathBuf,
    pub(super) timeout_ms: u64,
}

fn usage() -> crate::workflow::output::Failure {
    crate::workflow::output::Failure::error("usage", super::USAGE.into())
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
            "invalid-timeout",
            "timeout must be an integer from 1 through 600000 ms".into(),
        )),
    }
}

/// Decode one exact expected-law digest: 64 hex digits, case-insensitive.
fn digest(
    value: &std::ffi::OsStr,
) -> Result<std::string::String, crate::workflow::output::Failure> {
    let text = value.to_str().ok_or_else(usage)?;
    if text.len() != 64 {
        return Err(crate::workflow::output::Failure::error(
            "invalid-law-digest",
            "the expected owner-law digest must be exactly 64 hexadecimal digits".into(),
        ));
    }
    let mut normalized = std::string::String::with_capacity(64);
    for character in text.chars() {
        normalized.push(match character {
            '0'..='9' | 'a'..='f' | 'A'..='F' => character.to_ascii_lowercase(),
            _ => {
                return Err(crate::workflow::output::Failure::error(
                    "invalid-law-digest",
                    "the expected owner-law digest must be exactly 64 hexadecimal digits".into(),
                ))
            }
        });
    }
    Ok(normalized)
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; parse rejects missing values, duplicate options, positional operands and timeouts outside 1..=600000 ms with typed usage failures while preserving OS path bytes. User argument validity must not be asserted."
)]
pub(super) fn parse(
    arguments: &[std::ffi::OsString],
) -> Result<Options, crate::workflow::output::Failure> {
    let mut law: Option<std::path::PathBuf> = None;
    let mut law_digest: Option<std::string::String> = None;
    let mut source: Option<std::path::PathBuf> = None;
    let mut contract: Option<std::path::PathBuf> = None;
    let mut proof: Option<std::path::PathBuf> = None;
    let mut artifact: Option<std::path::PathBuf> = None;
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
            Some("--artifact") if artifact.is_none() => {
                artifact = Some(std::path::PathBuf::from(attempt!(option_value(
                    arguments, index
                ))));
                index += 2;
            }
            Some("--timeout-ms") if !has_timeout => {
                timeout_ms = attempt!(timeout(attempt!(option_value(arguments, index))));
                has_timeout = true;
                index += 2;
            }
            Some(_) | None => return Err(usage()),
        }
    }
    Ok(Options {
        law: attempt!(law.ok_or_else(usage)),
        law_digest: attempt!(law_digest.ok_or_else(usage)),
        source: attempt!(source.ok_or_else(usage)),
        contract: attempt!(contract.ok_or_else(usage)),
        proof: attempt!(proof.ok_or_else(usage)),
        artifact: attempt!(artifact.ok_or_else(usage)),
        timeout_ms,
    })
}
