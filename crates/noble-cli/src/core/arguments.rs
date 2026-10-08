pub(in crate::core) mod bindings;

pub(super) struct Options {
    pub source: Option<std::path::PathBuf>,
    pub compile_only: bool,
    pub inputs: std::vec::Vec<InputType>,
    pub modules: std::vec::Vec<std::path::PathBuf>,
    pub declared_modules: bool,
    pub retained_source_bytes: Option<u32>,
    pub signed_self_recursion: bool,
    pub text_byte_cursor: bool,
    pub bindings: Option<std::path::PathBuf>,
    pub manifest: Option<bindings::Manifest>,
    pub framed: bool,
    pub optimized: bool,
    pub emit: Option<std::path::PathBuf>,
    pub limits: noble_contracts::Limits,
}

#[octet::sealed_enum]
pub(super) enum InputType {
    Concrete(noble_kernel::types::Ty),
    Nominal(std::string::String),
}

impl Options {
    pub const fn declared_text(&self) -> bool {
        self.declared_modules && self.text_byte_cursor
    }

    pub fn retained_source_limit(&self) -> noble_contracts::source::RetainedSourceLimit {
        noble_contracts::source::RetainedSourceLimit::new(
            self.retained_source_bytes.unwrap_or(self.limits.bytes),
        )
        .expect("validated options")
    }
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
        modules: std::vec::Vec::new(),
        declared_modules: false,
        retained_source_bytes: None,
        signed_self_recursion: false,
        text_byte_cursor: false,
        bindings: None,
        manifest: None,
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
    let mut seen = [""; 13];
    let mut seen_count = 0_usize;
    while at < arguments.len() {
        let name = attempt!(arguments[at].to_str().ok_or_else(usage));
        if name != "--input-type" && name != "--module" && seen.contains(&name) {
            return Err(usage());
        }
        if name != "--input-type" && name != "--module" {
            *attempt!(seen.get_mut(seen_count).ok_or_else(usage)) = name;
            seen_count = attempt!(seen_count.checked_add(1).ok_or_else(usage));
        }
        at = attempt!(at.checked_add(1).ok_or_else(usage));
        if name == "--framed" && is_session {
            options.framed = true;
            continue;
        }
        if name == "--declared-modules" {
            options.declared_modules = true;
            continue;
        }
        if name == "--signed-self-recursion" {
            options.signed_self_recursion = true;
            continue;
        }
        if name == "--text-byte-cursor" {
            if !is_session && !options.compile_only {
                return Err(usage());
            }
            options.text_byte_cursor = true;
            continue;
        }
        let value = attempt!(arguments.get(at).ok_or_else(usage));
        at = attempt!(at.checked_add(1).ok_or_else(usage));
        attempt!(set(&mut options, name, value, maximum));
    }
    if options.declared_text() && options.bindings.is_some() {
        return Err(usage());
    }
    if !options.declared_text() && options.declared_modules != options.bindings.is_some() {
        return Err(usage());
    }
    if options.signed_self_recursion
        && (!options.declared_modules
            || is_session && !options.framed
            || !is_session && !options.compile_only)
    {
        return Err(usage());
    }
    if !options.declared_modules && !options.modules.is_empty() {
        return Err(usage());
    }
    if options.retained_source_bytes.is_some()
        && (!options.declared_modules || !is_session && !options.compile_only)
    {
        return Err(usage());
    }
    if !options.declared_modules
        && options
            .inputs
            .iter()
            .any(|input| matches!(input, InputType::Nominal(_)))
    {
        return Err(usage());
    }
    if let Some(path) = &options.bindings {
        options.manifest = Some(attempt!(bindings::load(path)));
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
                Some("I64") => InputType::Concrete(noble_kernel::types::Ty::I64),
                Some("Bool") => InputType::Concrete(noble_kernel::types::Ty::Bool),
                Some("Text") => InputType::Concrete(noble_kernel::types::Ty::Text),
                Some("Unit") => InputType::Concrete(noble_kernel::types::Ty::Unit),
                Some(name) if name.len() <= 128 && name.contains('@') && name.contains('.') => {
                    InputType::Nominal(name.into())
                }
                Some(_) | None => return Err(usage()),
            };
            if options.inputs.len() >= 128 {
                return Err(usage());
            }
            options.inputs.push(ty);
        }
        "--module" if options.compile_only && options.modules.len() < 64 => {
            options.modules.push(std::path::PathBuf::from(value));
        }
        "--opt" => match value.to_str() {
            Some("off") => options.optimized = false,
            Some("on") => options.optimized = true,
            Some(_) | None => return Err(usage()),
        },
        "--emit" => options.emit = Some(std::path::PathBuf::from(value)),
        "--bindings" => options.bindings = Some(std::path::PathBuf::from(value)),
        "--source-bytes" => options.limits.bytes = attempt!(limit(value, maximum.bytes)),
        "--retained-source-bytes" => {
            let bytes = attempt!(limit(
                value,
                noble_contracts::source::RetainedSourceLimit::MAX_BYTES,
            ));
            if bytes == 0 {
                return Err(usage());
            }
            options.retained_source_bytes = Some(bytes);
        }
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

#[cfg(test)]
mod tests {
    fn flags(arguments: &[&str]) -> Result<super::Options, std::string::String> {
        let arguments: std::vec::Vec<_> = arguments
            .iter()
            .map(|argument| std::ffi::OsString::from(*argument))
            .collect();
        super::parse(&arguments).map_err(|failure| failure.message)
    }

    #[test]
    fn declared_retention_defaults_to_selected_per_input_limit() -> Result<(), std::string::String>
    {
        let original = flags(&[
            "compile",
            "expression.noble",
            "--declared-modules",
            "--text-byte-cursor",
        ])?;
        assert_eq!(original.limits.bytes, 65_536);
        assert_eq!(original.retained_source_limit().bytes(), 65_536);

        let lowered = flags(&[
            "session",
            "--framed",
            "--declared-modules",
            "--text-byte-cursor",
            "--source-bytes",
            "128",
        ])?;
        assert_eq!(lowered.limits.bytes, 128);
        assert_eq!(lowered.retained_source_limit().bytes(), 128);
        let zero = flags(&[
            "session",
            "--declared-modules",
            "--text-byte-cursor",
            "--source-bytes",
            "0",
        ])?;
        assert_eq!(zero.retained_source_limit().bytes(), 0);
        Ok(())
    }

    #[test]
    fn declared_retention_opt_in_is_independent_of_per_input_limit(
    ) -> Result<(), std::string::String> {
        for arguments in [
            [
                "compile",
                "expression.noble",
                "--declared-modules",
                "--text-byte-cursor",
                "--source-bytes",
                "32",
                "--retained-source-bytes",
                "262144",
            ],
            [
                "compile",
                "expression.noble",
                "--retained-source-bytes",
                "262144",
                "--source-bytes",
                "32",
                "--text-byte-cursor",
                "--declared-modules",
            ],
        ] {
            let selected = flags(&arguments)?;
            assert_eq!(selected.limits.bytes, 32);
            assert_eq!(selected.retained_source_limit().bytes(), 262_144);
        }
        let smallest = flags(&[
            "session",
            "--declared-modules",
            "--text-byte-cursor",
            "--retained-source-bytes",
            "1",
            "--source-bytes",
            "0",
        ])?;
        assert_eq!(smallest.limits.bytes, 0);
        assert_eq!(smallest.retained_source_limit().bytes(), 1);
        Ok(())
    }

    #[test]
    fn retained_opt_in_refuses_non_declared_and_invalid_budgets() -> Result<(), std::string::String>
    {
        for arguments in [
            &[
                "compile",
                "expression.noble",
                "--retained-source-bytes",
                "262144",
            ][..],
            &["session", "--retained-source-bytes", "1"][..],
            &[
                "compile",
                "expression.noble",
                "--declared-modules",
                "--text-byte-cursor",
                "--retained-source-bytes",
                "0",
            ][..],
            &[
                "compile",
                "expression.noble",
                "--declared-modules",
                "--text-byte-cursor",
                "--retained-source-bytes",
                "262145",
            ][..],
            &[
                "compile",
                "expression.noble",
                "--declared-modules",
                "--text-byte-cursor",
                "--retained-source-bytes",
                "262144",
                "--source-bytes",
                "65537",
            ][..],
            &[
                "session",
                "--declared-modules",
                "--text-byte-cursor",
                "--retained-source-bytes",
                "1",
                "--retained-source-bytes",
                "2",
            ][..],
        ] {
            if flags(arguments).is_ok() {
                return Err(std::format!(
                    "accepted invalid source budget flags: {arguments:?}"
                ));
            }
        }
        let ordinary = flags(&["session", "--source-bytes", "0"])?;
        assert_eq!(ordinary.limits.bytes, 0);
        Ok(())
    }

    #[test]
    fn retained_opt_in_refuses_otherwise_valid_declared_run() -> Result<(), std::string::String> {
        let manifest = std::env::temp_dir().join(std::format!(
            "noble-retained-run-{}-{:?}.txt",
            std::process::id(),
            std::thread::current().id()
        ));
        std::fs::write(&manifest, "").map_err(|error| error.to_string())?;
        let path = manifest.to_string_lossy();
        let baseline = flags(&[
            "run",
            "expression.noble",
            "--declared-modules",
            "--bindings",
            &path,
        ]);
        let flagged = flags(&[
            "run",
            "expression.noble",
            "--declared-modules",
            "--bindings",
            &path,
            "--retained-source-bytes",
            "262144",
        ]);
        std::fs::remove_file(&manifest).map_err(|error| error.to_string())?;
        assert!(
            baseline.is_ok(),
            "run profile and empty manifest are parser-admissible"
        );
        assert!(
            flagged.is_err(),
            "retained opt-in must not extend declared run"
        );
        Ok(())
    }
}
