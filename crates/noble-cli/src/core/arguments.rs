pub(super) struct Options {
    pub source: Option<std::path::PathBuf>,
    pub compile_only: bool,
    pub inputs: std::vec::Vec<noble_kernel::types::Ty>,
    pub framed: bool,
    pub optimized: bool,
    pub emit: Option<std::path::PathBuf>,
    pub limits: noble_contracts::Limits,
}

#[expect(
    tigerstyle::explicit_defaults,
    reason = "Owner: noble-maintainers; source flags are reductions of the frontend's declared default budgets. The same Limits snapshot initializes options and bounds every override in set; copying its fields here would create an independent ceiling contract."
)]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; parse rejects missing or duplicate flags, invalid OS argument text and exhausted option slots with usage failures. Untrusted command-line arguments must remain diagnostic inputs rather than assertion preconditions."
)]
pub(super) fn parse(arguments: &[std::ffi::OsString]) -> Result<Options, super::output::Failure> {
    let is_session = arguments
        .first()
        .is_some_and(|argument| argument == "session");
    let maximum = noble_contracts::Limits::default();
    let mut options = Options {
        source: None,
        compile_only: arguments
            .first()
            .is_some_and(|argument| argument == "compile"),
        inputs: std::vec::Vec::new(),
        framed: false,
        optimized: false,
        emit: None,
        limits: maximum,
    };
    let mut at = 1_usize;
    if !is_session {
        options.source = Some(std::path::PathBuf::from(attempt!(arguments
            .get(at)
            .ok_or_else(usage))));
        at = attempt!(at.checked_add(1).ok_or_else(usage));
    }
    let mut seen = [""; 7];
    let mut seen_count = 0_usize;
    while at < arguments.len() {
        let name = attempt!(arguments[at].to_str().ok_or_else(usage));
        if name != "--input-type" && seen.contains(&name) {
            return Err(usage());
        }
        if name != "--input-type" {
            *attempt!(seen.get_mut(seen_count).ok_or_else(usage)) = name;
            seen_count = attempt!(seen_count.checked_add(1).ok_or_else(usage));
        }
        at = attempt!(at.checked_add(1).ok_or_else(usage));
        if name == "--framed" && is_session {
            options.framed = true;
            continue;
        }
        let value = attempt!(arguments.get(at).ok_or_else(usage));
        at = attempt!(at.checked_add(1).ok_or_else(usage));
        attempt!(set(&mut options, name, value, maximum));
    }
    Ok(options)
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; set decodes OsStr values, appends input types to a Vec and allocates an emission PathBuf. These OS string, allocation and parsing APIs are not const on the pinned compiler."
)]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; set explicitly rejects unsupported input types, excessive inputs, invalid optimization modes and out-of-budget limits. Malformed flag values return usage failures rather than panicking."
)]
fn set(
    options: &mut Options,
    name: &str,
    value: &std::ffi::OsStr,
    maximum: noble_contracts::Limits,
) -> Result<(), super::output::Failure> {
    match name {
        "--input-type" if options.compile_only => {
            let ty = match value.to_str() {
                Some("I64") => noble_kernel::types::Ty::I64,
                Some("Bool") => noble_kernel::types::Ty::Bool,
                Some("Text") => noble_kernel::types::Ty::Text,
                Some("Unit") => noble_kernel::types::Ty::Unit,
                Some(_) | None => return Err(usage()),
            };
            if options.inputs.len() >= 128 {
                return Err(usage());
            }
            options.inputs.push(ty);
        }
        "--opt" => match value.to_str() {
            Some("off") => options.optimized = false,
            Some("on") => options.optimized = true,
            Some(_) | None => return Err(usage()),
        },
        "--emit" => options.emit = Some(std::path::PathBuf::from(value)),
        "--source-bytes" => options.limits.bytes = attempt!(limit(value, maximum.bytes)),
        "--source-nodes" => options.limits.nodes = attempt!(limit(value, maximum.nodes)),
        "--source-depth" => options.limits.depth = attempt!(limit(value, maximum.depth)),
        "--source-work" => options.limits.work = attempt!(limit(value, maximum.work)),
        _ => return Err(usage()),
    }
    Ok(())
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; limit decodes an OsStr and parses decimal text through FromStr before checking the configured ceiling. OS string decoding and FromStr are not const on the pinned compiler."
)]
fn limit(value: &std::ffi::OsStr, maximum: u32) -> Result<u32, super::output::Failure> {
    let number = attempt!(value
        .to_str()
        .and_then(|value| value.parse::<u32>().ok())
        .ok_or_else(usage));
    if number > maximum {
        return Err(usage());
    }
    Ok(number)
}

fn usage() -> super::output::Failure {
    super::output::Failure::new(
        super::output::ErrorContext {
            stage: "arguments",
            outcome: "invalid-input",
        },
        super::USAGE,
    )
}
