//! Explicit compiler-service binding manifest. These rows describe host-supplied
//! adapters; neither a module requirement nor a parsed row grants authority.

mod adapter;

struct Entry {
    module: std::string::String,
    version: u32,
    operation: std::string::String,
    adapter: std::string::String,
    slot: u32,
    inputs: std::vec::Vec<noble_kernel::types::Ty>,
    outputs: std::vec::Vec<noble_kernel::types::Ty>,
    effects: std::vec::Vec<noble_kernel::types::EffId>,
    allowed: bool,
    script: std::vec::Vec<i64>,
}

pub(in crate::core) struct Manifest {
    entries: std::vec::Vec<Entry>,
}

fn invalid(reason: &str) -> super::super::output::Failure {
    super::super::output::Failure::new(
        super::super::output::ErrorContext {
            stage: "binding",
            outcome: "invalid-input",
        },
        reason,
    )
}

/// Read one explicit local manifest with a finite byte/row budget. The file
/// belongs to the application host, not to Noble guest source or import lookup.
pub(super) fn load(path: &std::path::Path) -> Result<Manifest, super::super::output::Failure> {
    let metadata =
        attempt!(std::fs::metadata(path).map_err(|_| invalid("binding manifest is unavailable")));
    if metadata.len() > 65_536 {
        return Err(invalid("binding manifest byte limit exceeded"));
    }
    let bytes =
        attempt!(std::fs::read(path).map_err(|_| invalid("binding manifest is unreadable")));
    if bytes.len() > 65_536 {
        return Err(invalid("binding manifest byte limit exceeded"));
    }
    let text =
        attempt!(std::str::from_utf8(&bytes).map_err(|_| invalid("binding manifest is not UTF-8")));
    Manifest::parse(text)
}

fn parse_line(line: &str, slot: u32) -> Result<Entry, super::super::output::Failure> {
    let words = std::vec::Vec::from_iter(line.split_ascii_whitespace());
    if words.len() < 8 || words.first() != Some(&"bind")
        || !matches!(words.get(2), Some(&"test.emit" | &"test.clock")) {
        return Err(invalid("binding line requires bind MODULE@VERSION test.emit|test.clock ADAPTER INPUT -- OUTPUT ! EFFECT allow|deny [script VALUES]"));
    }
    let (module, version) = attempt!(module_version(words[1]));
    let adapter = attempt!(adapter_identity(words[3]));
    let signature = attempt!(adapter::Signature::parse(&words, words[2]));
    if words[2] == "test.clock" {
        let plan = noble_kernel::contracts::ClockPlan {
            operation: words[2],
            adapter_identity: adapter,
            input: &signature.inputs,
            output: &signature.outputs,
            effects: std::slice::from_ref(&signature.effect),
            allowed: signature.allowed,
            script: &signature.script,
        };
        attempt!(plan.decide(words[2], 0)
            .map_err(|_| invalid("test.clock binding requires [] -- I64 ! test.clock and a finite script")));
    }
    Ok(Entry {
        module: module.into(),
        version,
        operation: words[2].into(),
        adapter: adapter.into(),
        slot,
        inputs: signature.inputs,
        outputs: signature.outputs,
        effects: std::vec![signature.effect],
        allowed: signature.allowed,
        script: signature.script,
    })
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; module names and canonical versions are untrusted manifest text. Invalid names and versions produce binding diagnostics, not invariant assertions."
)]
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; parsing a manifest version calls u32::from_str at runtime, which is not const on the selected compiler."
)]
fn module_version(word: &str) -> Result<(&str, u32), super::super::output::Failure> {
    let (module, version_text) = attempt!(word
        .rsplit_once('@')
        .ok_or_else(|| invalid("binding module requires explicit version")));
    if module.is_empty()
        || !module
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
    {
        return Err(invalid("binding module name is invalid"));
    }
    let version = attempt!(version_text
        .parse::<u32>()
        .map_err(|_| invalid("binding module version is invalid")));
    if version == 0 || version_text.as_bytes().first() == Some(&b'0') {
        return Err(invalid(
            "binding module version must be canonical and positive",
        ));
    }
    Ok((module, version))
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; adapter validation iterates runtime manifest bytes through Iterator::all and creates a binding diagnostic for invalid input. These operations are not const on the selected compiler."
)]
fn adapter_identity(word: &str) -> Result<&str, super::super::output::Failure> {
    if word.is_empty()
        || word.len() > 64
        || !word
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
    {
        return Err(invalid("binding adapter identity is invalid"));
    }
    Ok(word)
}

impl Manifest {
    fn parse(text: &str) -> Result<Self, super::super::output::Failure> {
        let mut manifest = Self {
            entries: std::vec::Vec::new(),
        };
        let mut remaining = text;
        while !remaining.is_empty() {
            let (raw, rest) = match remaining.split_once('\n') {
                Some(parts) => parts,
                None => (remaining, ""),
            };
            remaining = rest;
            let line = raw.trim();
            if !line.is_empty() {
                attempt!(manifest.push(line));
            }
        }
        Ok(manifest)
    }

    #[expect(
        tigerstyle::missing_const_fn,
        reason = "Owner: noble-maintainers; a parsed row allocates module and adapter identities and is pushed into the mutable Vec-backed manifest. This runtime accumulation is not const."
    )]
    fn push(&mut self, line: &str) -> Result<(), super::super::output::Failure> {
        if self.entries.len() >= 64 {
            return Err(invalid("binding manifest entry limit exceeded"));
        }
        let slot =
            attempt!(u32::try_from(self.entries.len())
                .map_err(|_| invalid("binding slot limit exceeded")));
        let entry = attempt!(parse_line(line, slot));
        if self
            .entries
            .iter()
            .any(|existing| existing.module == entry.module && existing.version == entry.version)
        {
            return Err(invalid("duplicate module operation binding"));
        }
        self.entries.push(entry);
        Ok(())
    }

    /// Typed rows for independent frontend/link validation. Policy is absent:
    /// source and kernel cannot turn the manifest's allow/deny into authority.
    pub fn operations(&self) -> std::vec::Vec<noble_contracts::source::BoundOperation> {
        self.entries
            .iter()
            .map(|entry| noble_contracts::source::BoundOperation {
                module_name: entry.module.clone(),
                module_version: entry.version,
                operation: entry.operation.clone(),
                adapter_identity: entry.adapter.clone(),
                adapter_slot: entry.slot,
                input: entry.inputs.clone(),
                output: entry.outputs.clone(),
                effects: entry.effects.clone(),
            })
            .collect()
    }

    /// Worker-owned policy and immutable slot identity, never guest source.
    pub fn worker(&self) -> Result<crate::workflow::encoding::Json, super::super::output::Failure> {
        let bindings = self.entries.iter().map(|entry| {
                    Ok(crate::workflow::encoding::object([
                        (
                            "slot",
                            crate::workflow::encoding::Json::Number(u64::from(entry.slot)),
                        ),
                        ("adapter", crate::workflow::encoding::string(&entry.adapter)),
                        ("operation", crate::workflow::encoding::string(&entry.operation)),
                        ("module", crate::workflow::encoding::string(&entry.module)),
                        (
                            "version",
                            crate::workflow::encoding::Json::Number(u64::from(entry.version)),
                        ),
                        (
                            "allowed",
                            crate::workflow::encoding::Json::Bool(entry.allowed),
                        ),
                        ("script", crate::workflow::encoding::Json::Array(
                            entry.script.iter().map(|value|
                                crate::workflow::encoding::Json::String(value.to_string())
                            ).collect(),
                        )),
                        ("dispatch", attempt!(dispatch(entry))),
                    ]))
                }).collect::<Result<std::vec::Vec<_>, _>>()?;
        Ok(crate::workflow::encoding::Json::Array(bindings))
    }
}

fn dispatch(entry: &Entry) -> Result<crate::workflow::encoding::Json, super::super::output::Failure> {
    if entry.operation != "test.clock" {
        return Ok(crate::workflow::encoding::Json::Array(std::vec::Vec::new()));
    }
    let plan = noble_kernel::contracts::ClockPlan {
        operation: &entry.operation,
        adapter_identity: &entry.adapter,
        input: &entry.inputs,
        output: &entry.outputs,
        effects: &entry.effects,
        allowed: entry.allowed,
        script: &entry.script,
    };
    let mut steps = std::vec::Vec::with_capacity(entry.script.len().saturating_add(1));
    for index in 0..=entry.script.len() {
        let decision = attempt!(plan.decide("test.clock", index)
            .map_err(|_| invalid("test.clock dispatch contract is invalid")));
        let (name, value) = match decision {
            noble_kernel::contracts::ClockDecision::Value(value) =>
                ("allow", Some(value.to_string())),
            noble_kernel::contracts::ClockDecision::Denied => ("deny", None),
            noble_kernel::contracts::ClockDecision::ScriptExhausted => ("script-exhausted", None),
            noble_kernel::contracts::ClockDecision::UnexpectedOperation =>
                return Err(invalid("test.clock dispatch unexpectedly changed operation")),
        };
        steps.push(crate::workflow::encoding::object([
            ("decision",crate::workflow::encoding::string(name)),
            ("value",crate::workflow::encoding::optional_string(value.as_deref())),
        ]));
    }
    Ok(crate::workflow::encoding::Json::Array(steps))
}
