//! Proof-required build decoding, independent of preparation or proof execution.
//!
//! Grammar: `noble build --require-proof SOURCE --contract FILE
//! [--proof FILE | --refutation FILE] [--out NEW_DIR] [--timeout-ms N]`.
//! SOURCE is the single positional operand; `--contract` names the contract
//! file. Options may follow `build` in any order. Each option occurs at most
//! once and `--proof`/`--refutation` share one mutually exclusive slot.

pub(super) struct Options {
    pub(super) require_proof: bool,
    pub(super) source: std::path::PathBuf,
    pub(super) contract: std::path::PathBuf,
    pub(super) evidence: Option<std::path::PathBuf>,
    pub(super) is_refutation: bool,
    pub(super) out: Option<std::path::PathBuf>,
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

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; timeout decodes OS argument text and parses decimal milliseconds through FromStr, returning an owned diagnostic on invalid input. OS string decoding, FromStr and diagnostic allocation are not const on the pinned compiler."
)]
fn timeout(value: &std::ffi::OsStr) -> Result<u64, crate::workflow::output::Failure> {
    match value.to_str().and_then(|text| text.parse::<u64>().ok()) {
        Some(value) if value > 0 && value <= crate::workflow::MAX_TIMEOUT => Ok(value),
        Some(_) | None => Err(crate::workflow::output::Failure::error(
            "invalid-timeout",
            "timeout must be an integer from 1 through 600000 ms".into(),
        )),
    }
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; parse rejects missing values, duplicate options, mutually exclusive evidence and timeouts outside 1..=600000 ms with typed usage failures while preserving OS path bytes. User argument validity must not be asserted."
)]
pub(super) fn parse(
    arguments: &[std::ffi::OsString],
) -> Result<Options, crate::workflow::output::Failure> {
    let mut has_require_proof = false;
    let mut source: Option<std::path::PathBuf> = None;
    let mut contract: Option<std::path::PathBuf> = None;
    let mut evidence: Option<std::path::PathBuf> = None;
    let mut is_refutation = false;
    let mut out: Option<std::path::PathBuf> = None;
    let mut timeout_ms = crate::workflow::DEFAULT_TIMEOUT;
    let mut has_timeout = false;
    let mut index = 1;
    while index < arguments.len() {
        let name = arguments[index]
            .to_str()
            .filter(|name| name.starts_with("--"));
        match name {
            Some("--require-proof") if !has_require_proof => {
                has_require_proof = true;
                index += 1;
            }
            Some("--contract") if contract.is_none() => {
                contract = Some(attempt!(option_value(arguments, index)).into());
                index += 2;
            }
            Some("--proof" | "--refutation") if evidence.is_none() => {
                is_refutation = arguments[index] == "--refutation";
                evidence = Some(attempt!(option_value(arguments, index)).into());
                index += 2;
            }
            Some("--out") if out.is_none() => {
                out = Some(attempt!(option_value(arguments, index)).into());
                index += 2;
            }
            Some("--timeout-ms") if !has_timeout => {
                let value = attempt!(option_value(arguments, index));
                timeout_ms = attempt!(timeout(value));
                has_timeout = true;
                index += 2;
            }
            Some(_) => return Err(usage()),
            None => match &source {
                Some(_) => return Err(usage()),
                None => {
                    source = Some(std::path::PathBuf::from(&arguments[index]));
                    index += 1;
                }
            },
        }
    }
    Ok(Options {
        require_proof: has_require_proof,
        source: attempt!(source.ok_or_else(usage)),
        contract: attempt!(contract.ok_or_else(usage)),
        evidence,
        is_refutation,
        out,
        timeout_ms,
    })
}
