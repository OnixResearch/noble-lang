//! Argument parsing for `noble ngraph` (design D7, D9). Unknown, duplicated,
//! missing and inapplicable flags and malformed values are usage errors.

/// One D5 budget flag and the `Budgets` field it sets.
#[derive(Clone, Copy)]
pub(super) struct Kind {
    pub(super) flag: &'static str,
    set: fn(&mut noble_ngraph::Budgets, u64),
}

macro_rules! kind {
    ($flag:literal, $field:ident) => {
        Kind {
            flag: $flag,
            set: |b, v| b.$field = v,
        }
    };
}

/// Budgets charged by the extractor.
pub(super) const EXTRACTION: [Kind; 3] = [
    kind!("--budget-work", work_units),
    kind!("--budget-expansion", expanded_nodes),
    kind!("--budget-inline-depth", inline_depth),
];

/// Budgets charged by the graph checker.
pub(super) const CHECKER: [Kind; 9] = [
    kind!("--budget-live-nodes", live_nodes),
    kind!("--budget-layers", layers),
    kind!("--budget-inputs", inputs),
    kind!("--budget-outputs", outputs),
    kind!("--budget-affine-terms", affine_terms),
    kind!("--budget-table-length", table_length),
    kind!("--budget-table-entries", table_entries),
    kind!("--budget-facts", facts),
    kind!("--budget-graph-bytes", graph_bytes),
];

pub(super) const SNAPSHOT_BYTES: Kind = kind!("--budget-snapshot-bytes", snapshot_bytes);
pub(super) const FORWARD_SCRATCH: Kind = kind!("--budget-forward-scratch", forward_scratch);
pub(super) const BACKWARD_SCRATCH: Kind = kind!("--budget-backward-scratch", backward_scratch);
pub(super) const STEPS: Kind = kind!("--budget-steps", optimizer_steps);

/// What a subcommand accepts.
pub(super) struct Spec<'a> {
    /// Positional argument names (exact count).
    pub(super) positional: &'a [&'static str],
    /// Single-use value flags.
    pub(super) values: &'a [&'static str],
    /// Repeatable value flags.
    pub(super) repeated: &'a [&'static str],
    /// Boolean flags.
    pub(super) switches: &'a [&'static str],
    /// Budget flags.
    pub(super) budgets: &'a [Kind],
}

/// Parsed arguments.
pub(super) struct Parsed {
    positional: std::vec::Vec<std::ffi::OsString>,
    values: std::vec::Vec<(&'static str, std::ffi::OsString)>,
    repeated: std::vec::Vec<(&'static str, std::string::String)>,
    switches: std::vec::Vec<&'static str>,
    /// Budgets with the flags applied (not yet validated against the `/0`
    /// policy: a raised budget is a coded `E-PROFILE` refusal, not a usage
    /// error).
    pub(super) budgets: noble_ngraph::Budgets,
}

fn utf8(flag: &str, value: &std::ffi::OsStr) -> Result<std::string::String, std::string::String> {
    value
        .to_str()
        .map(str::to_owned)
        .ok_or_else(|| std::format!("{flag} takes UTF-8 text"))
}

/// Parses `arguments` (after `ngraph <subcommand>`) against `spec`.
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; argument parsing reports every malformed command line as a usage error rather than an assertion panic."
)]
pub(super) fn parse(
    spec: &Spec<'_>,
    arguments: &[std::ffi::OsString],
) -> Result<Parsed, std::string::String> {
    let mut parsed = Parsed {
        positional: std::vec::Vec::new(),
        values: std::vec::Vec::new(),
        repeated: std::vec::Vec::new(),
        switches: std::vec::Vec::new(),
        budgets: noble_ngraph::Budgets::default(),
    };
    let mut budget_seen: std::vec::Vec<&'static str> = std::vec::Vec::new();
    let mut at = 0;
    while at < arguments.len() {
        let argument = &arguments[at];
        at += 1;
        let Some(text) = argument.to_str().filter(|t| t.starts_with("--")) else {
            if parsed.positional.len() == spec.positional.len() {
                return Err("unexpected extra argument".into());
            }
            parsed.positional.push(argument.clone());
            continue;
        };
        let mut value = |flag: &str| -> Result<std::ffi::OsString, std::string::String> {
            match arguments.get(at).cloned() {
                Some(v) => {
                    at += 1;
                    Ok(v)
                }
                None => Err(std::format!("{flag} needs a value")),
            }
        };
        if let Some(flag) = spec.switches.iter().find(|f| **f == text) {
            if parsed.switches.contains(flag) {
                return Err(std::format!("{flag} is given twice"));
            }
            parsed.switches.push(*flag);
        } else if let Some(flag) = spec.values.iter().find(|f| **f == text) {
            if parsed.values.iter().any(|(f, _)| f == flag) {
                return Err(std::format!("{flag} is given twice"));
            }
            let v = attempt!(value(flag));
            parsed.values.push((*flag, v));
        } else if let Some(flag) = spec.repeated.iter().find(|f| **f == text) {
            let v = attempt!(value(flag));
            parsed.repeated.push((*flag, attempt!(utf8(flag, &v))));
        } else if let Some(kind) = spec.budgets.iter().find(|k| k.flag == text) {
            if budget_seen.contains(&kind.flag) {
                return Err(std::format!("{} is given twice", kind.flag));
            }
            budget_seen.push(kind.flag);
            let v = attempt!(value(kind.flag));
            let text = attempt!(utf8(kind.flag, &v));
            let n = attempt!(noble_ngraph::format::decimal_u64(&text)
                .ok_or_else(|| std::format!("{} takes an unsigned 64-bit integer", kind.flag)));
            (kind.set)(&mut parsed.budgets, n);
        } else {
            return Err(std::format!("{text} is not an option of this subcommand"));
        }
    }
    if parsed.positional.len() != spec.positional.len() {
        return Err(std::format!("expected {}", spec.positional.join(" ")));
    }
    Ok(parsed)
}

impl Parsed {
    /// The `k`-th positional argument as a path.
    pub(super) fn path(&self, k: usize) -> std::path::PathBuf {
        std::path::PathBuf::from(&self.positional[k])
    }

    /// Whether a boolean flag was given.
    pub(super) fn switch(&self, flag: &str) -> bool {
        self.switches.contains(&flag)
    }

    /// A value flag as a path.
    pub(super) fn path_value(&self, flag: &str) -> Option<std::path::PathBuf> {
        self.values
            .iter()
            .find(|(f, _)| *f == flag)
            .map(|(_, v)| std::path::PathBuf::from(v))
    }

    /// A value flag as UTF-8 text.
    pub(super) fn text(
        &self,
        flag: &str,
    ) -> Result<Option<std::string::String>, std::string::String> {
        match self.values.iter().find(|(f, _)| *f == flag) {
            Some((_, v)) => utf8(flag, v).map(Some),
            None => Ok(None),
        }
    }

    /// A required value flag as UTF-8 text.
    pub(super) fn required(&self, flag: &str) -> Result<std::string::String, std::string::String> {
        attempt!(self.text(flag)).ok_or_else(|| std::format!("{flag} is required"))
    }

    /// Whether a value flag was given.
    pub(super) fn has(&self, flag: &str) -> bool {
        self.values.iter().any(|(f, _)| *f == flag)
    }

    /// Every value of a repeatable flag, in order.
    pub(super) fn all(&self, flag: &str) -> std::vec::Vec<&str> {
        self.repeated
            .iter()
            .filter(|(f, _)| *f == flag)
            .map(|(_, v)| v.as_str())
            .collect()
    }
}

/// An unsigned integer: decimal digits, or `0x` plus one to 16 hex digits.
/// No sign is accepted, as in every `ngraph` file.
pub(super) fn unsigned(flag: &str, text: &str) -> Result<u64, std::string::String> {
    let parsed = if text.starts_with("0x") {
        noble_ngraph::format::hex_bits(text, 16)
    } else {
        noble_ngraph::format::decimal_u64(text)
    };
    parsed.ok_or_else(|| std::format!("{flag} takes an unsigned 64-bit integer"))
}

/// A binary64 value: `0x` plus exactly 16 hex digits of its encoding, or a
/// decimal ([`noble_ngraph::sem::is_decimal`]: no leading `+`, no `nan` or
/// `inf` word) rounded to nearest, ties to even. Whether the value is in an
/// optimizer's domain is the library's admission, not syntax.
pub(super) fn binary64(flag: &str, text: &str) -> Result<f64, std::string::String> {
    let parsed = if text.starts_with("0x") {
        noble_ngraph::format::hex_bits(text, 16)
            .filter(|_| text.len() == 18)
            .map(f64::from_bits)
    } else if noble_ngraph::sem::is_decimal(text) {
        text.parse::<f64>().ok()
    } else {
        None
    };
    parsed.ok_or_else(|| std::format!("{flag} takes 0x<16 hex digits> or a decimal"))
}

/// A 64-digit hex digest.
pub(super) fn digest(flag: &str, text: &str) -> Result<[u8; 32], std::string::String> {
    noble_ngraph::format::digest_from_hex(text)
        .filter(|_| text.len() == 64)
        .ok_or_else(|| std::format!("{flag} takes a 64-digit hex identity"))
}
