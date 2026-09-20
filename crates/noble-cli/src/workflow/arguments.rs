//! Command decoding, independent of source preparation or process execution.

pub(super) struct Options {
    pub(super) is_explanation: bool,
    pub(super) contract: std::path::PathBuf,
    pub(super) emit: Option<std::path::PathBuf>,
    pub(super) proof: Option<std::path::PathBuf>,
    pub(super) is_refutation: bool,
    pub(super) timeout_ms: u64,
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; parse rejects missing values, duplicate options and timeouts outside 1..=600000 ms with typed usage failures while preserving OS path bytes. User argument validity must not be asserted."
)]
pub(super) fn parse(arguments: &[std::ffi::OsString]) -> Result<Options, super::output::Failure> {
    let is_explanation = arguments
        .first()
        .is_some_and(|argument| argument == "explain-proof");
    let contract = attempt!(arguments
        .get(1)
        .ok_or_else(|| super::output::Failure::error("usage", super::USAGE.into())));
    let mut options = Options {
        is_explanation,
        contract: std::path::PathBuf::from(contract),
        emit: None,
        proof: None,
        is_refutation: false,
        timeout_ms: super::DEFAULT_TIMEOUT,
    };
    let mut has_timeout = false;
    let mut index = 2;
    while index < arguments.len() {
        if is_explanation {
            return Err(super::output::Failure::error("usage", super::USAGE.into()));
        }
        #[expect(
            tigerstyle::raw_arithmetic_overflow,
            reason = "Owner: noble-maintainers; the loop guard proves index < arguments.len(), so index + 1 <= arguments.len() <= usize::MAX. get reports a missing value when the successor equals the slice length."
        )]
        let value = attempt!(arguments
            .get(index + 1)
            .ok_or_else(|| super::output::Failure::error("usage", super::USAGE.into())));
        match arguments[index].to_str() {
            Some("--emit") if options.emit.is_none() => {
                options.emit = Some(std::path::PathBuf::from(value));
            }
            Some("--proof" | "--refutation") if options.proof.is_none() => {
                options.is_refutation = arguments[index] == "--refutation";
                options.proof = Some(std::path::PathBuf::from(value));
            }
            Some("--timeout-ms") if !has_timeout => {
                let parsed = value.to_str().and_then(|text| text.parse::<u64>().ok());
                match parsed {
                    Some(value) if value > 0 && value <= super::MAX_TIMEOUT => {
                        options.timeout_ms = value
                    }
                    Some(_) | None => {
                        return Err(super::output::Failure::error(
                            "invalid-timeout",
                            "timeout must be an integer from 1 through 600000 ms".into(),
                        ))
                    }
                }
                has_timeout = true;
            }
            Some(_) | None => {
                return Err(super::output::Failure::error("usage", super::USAGE.into()))
            }
        }
        index += 2;
    }
    Ok(options)
}
