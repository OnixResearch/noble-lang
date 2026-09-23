/// Require at least `minimum` arguments.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; argument refusal allocates a formatted runtime diagnostic through non-const String APIs."
)]
pub(in crate::core::companions) fn require(
    args: &[&str],
    minimum: usize,
) -> crate::core::companions::Attempt<()> {
    if args.len() < minimum {
        return Err(crate::core::companions::Refused::script(std::format!(
            "expected at least {minimum} argument(s), found {}",
            args.len()
        )));
    }
    Ok(())
}

/// The report's operation name for a script token.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; matching runtime str values is not const-compatible on the pinned compiler."
)]
pub(in crate::core::companions) fn operation_name(token: &str) -> &'static str {
    match token {
        "contract" => "contract",
        "submit" => "submit",
        "plain" => "plain",
        "stack" => "stack",
        "policy" => "policy",
        "project" => "project",
        "inspect" => "inspect",
        "certify" => "certify",
        "compose" => "compose",
        "instantiate" => "instantiate",
        "invoke" => "invoke",
        "derive" => "derive",
        _ => "unknown",
    }
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; dispatch performs mutable session operations and process/file I/O through non-const APIs."
)]
pub(in crate::core::companions) fn dispatch(
    driver: &mut crate::core::companions::Driver,
    operation: &str,
    args: &[&str],
) -> crate::core::companions::Attempt<crate::workflow::encoding::Json> {
    match operation {
        "contract" => {
            attempt!(crate::core::companions::entry::require(args, 1));
            crate::core::companions::operations::admission::op_contract(driver, args)
        }
        "submit" => {
            attempt!(crate::core::companions::entry::require(args, 1));
            driver.op_submit(args, false)
        }
        "plain" => {
            attempt!(crate::core::companions::entry::require(args, 1));
            driver.op_submit(args, true)
        }
        "stack" => driver.op_stack(),
        "policy" => {
            attempt!(crate::core::companions::entry::require(args, 1));
            driver.op_policy(args)
        }
        "project" => {
            attempt!(crate::core::companions::entry::require(args, 1));
            driver.op_project(args)
        }
        "inspect" => {
            attempt!(crate::core::companions::entry::require(args, 1));
            driver.op_inspect(args)
        }
        "certify" => {
            attempt!(crate::core::companions::entry::require(args, 1));
            crate::core::companions::operations::binding::op_certify(driver, args)
        }
        "compose" => {
            attempt!(crate::core::companions::entry::require(args, 3));
            crate::core::companions::operations::composition::op_compose(driver, args)
        }
        "instantiate" => {
            attempt!(crate::core::companions::entry::require(args, 2));
            crate::core::companions::operations::instantiation::op_instantiate(driver, args)
        }
        "invoke" => {
            attempt!(crate::core::companions::entry::require(args, 2));
            crate::core::companions::operations::invocation::op_invoke(driver, args)
        }
        "derive" => {
            attempt!(crate::core::companions::entry::require(args, 3));
            crate::core::companions::operations::derivations::op_derive(driver, args)
        }
        other => Err(crate::core::companions::Refused::script(std::format!(
            "unknown operation '{other}'"
        ))),
    }
}

/// A bounded streaming session: each operation replies before reading another.
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; malformed options and streaming I/O errors are reported with exit status 2 rather than panics on external input."
)]
pub(in crate::core::companions) fn run(arguments: &[std::ffi::OsString]) -> std::process::ExitCode {
    let mut timeout_ms = crate::core::companions::DEFAULT_TIMEOUT_MS;
    let mut is_optimized = false;
    let mut at = 1_usize;
    while at < arguments.len() {
        let token = arguments[at].to_string_lossy().into_owned();
        if token == "--timeout-ms" {
            at += 1;
            let Some(raw) = arguments.get(at) else {
                eprintln!("noble companions: --timeout-ms needs a value");
                return std::process::ExitCode::from(2);
            };
            match raw.to_string_lossy().parse::<u64>() {
                Ok(value) if value <= crate::core::companions::MAX_TIMEOUT_MS => timeout_ms = value,
                Ok(_) => {
                    eprintln!(
                        "noble companions: --timeout-ms is above {}",
                        crate::core::companions::MAX_TIMEOUT_MS
                    );
                    return std::process::ExitCode::from(2);
                }
                Err(_) => {
                    eprintln!("noble companions: --timeout-ms needs an integer");
                    return std::process::ExitCode::from(2);
                }
            }
        } else if token == "--opt" {
            at += 1;
            match arguments.get(at).and_then(|raw| raw.to_str()) {
                Some("on") => is_optimized = true,
                Some("off") => is_optimized = false,
                Some(_) | None => {
                    eprintln!("noble companions: --opt needs on or off");
                    return std::process::ExitCode::from(2);
                }
            }
        } else {
            eprintln!(
                "noble companions: unexpected argument '{token}'\n\n{}",
                crate::core::companions::USAGE
            );
            return std::process::ExitCode::from(2);
        }
        at += 1;
    }
    if let Err(message) = crate::core::companions::entry::run_stream(
        &mut std::io::stdin().lock(),
        timeout_ms,
        is_optimized,
    ) {
        eprintln!("noble companions: {message}");
        return std::process::ExitCode::from(2);
    }
    std::process::ExitCode::SUCCESS
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; each framed line consumes the checked whole-script byte budget, and decoding, dispatch and output failures remain typed results rather than input-triggered assertions."
)]
pub(in crate::core::companions) fn run_stream(
    reader: &mut impl std::io::BufRead,
    timeout_ms: u64,
    optimized: bool,
) -> Result<(), std::string::String> {
    let mut driver = crate::core::companions::Driver::new(timeout_ms, optimized);
    let mut remaining = crate::core::companions::SCRIPT_LIMIT;
    while let Some(bytes) = attempt!(
        crate::core::framing::read_line(reader, remaining).map_err(|failure| failure.message)
    ) {
        let count = attempt!(u32::try_from(bytes.len()).map_err(|error| error.to_string()));
        remaining = attempt!(remaining.checked_sub(count).ok_or_else(|| {
            std::format!(
                "script exceeds {} bytes",
                crate::core::companions::SCRIPT_LIMIT
            )
        }));
        let raw = attempt!(std::str::from_utf8(&bytes).map_err(|error| error.to_string()));
        let trimmed = raw.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let parts = crate::core::companions::entry::tokens(trimmed);
        let Some((operation, args)) = parts.split_first() else {
            continue;
        };
        let report = match crate::core::companions::entry::dispatch(&mut driver, operation, args) {
            Ok(report) => report,
            Err(refusal) => crate::core::companions::reporting::refusal_line(
                crate::core::companions::entry::operation_name(operation),
                &refusal,
            ),
        };
        attempt!(crate::core::companions::reporting::emit(&report));
    }
    Ok(())
}

/// Read one bounded regular file as bytes.
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; file type, size conversion and the actual framed-read byte limit are checked with errors. File races and I/O failures must not become assertion panics."
)]
pub(in crate::core::companions) fn read_bounded(
    path: &std::path::Path,
    limit_bytes: usize,
) -> Result<std::vec::Vec<u8>, std::string::String> {
    let metadata = attempt!(std::fs::symlink_metadata(path)
        .map_err(|error| std::format!("{}: {error}", path.display())));
    if !metadata.is_file() {
        return Err(std::format!("{}: not a regular file", path.display()));
    }
    let length_bytes =
        attempt!(usize::try_from(metadata.len())
            .map_err(|_| std::format!("{}: too large", path.display())));
    if length_bytes > limit_bytes {
        return Err(std::format!(
            "{}: {length_bytes} bytes exceeds the {limit_bytes}-byte budget",
            path.display()
        ));
    }
    let limit_bytes = attempt!(u32::try_from(limit_bytes)
        .map_err(|_| std::string::String::from("file budget exceeds the framing domain")));
    crate::core::framing::read_file(path, limit_bytes)
        .map_err(|failure| std::format!("{}: {}", path.display(), failure.message))
}

pub(in crate::core::companions) fn tokens(line: &str) -> std::vec::Vec<&str> {
    line.split_ascii_whitespace().collect()
}
