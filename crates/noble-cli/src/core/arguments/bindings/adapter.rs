//! Parsed adapter signature and host authorization fields of a binding row.
//! These values describe a host adapter; they never grant guest authority.

pub(super) struct Signature {
    pub(super) inputs: std::vec::Vec<noble_kernel::types::Ty>,
    pub(super) outputs: std::vec::Vec<noble_kernel::types::Ty>,
    pub(super) effect: noble_kernel::types::EffId,
    pub(super) allowed: bool,
    pub(super) script: std::vec::Vec<i64>,
}

struct SignatureWords<'a> {
    inputs: &'a [&'a str],
    outputs: &'a [&'a str],
    effect: &'a str,
    policy: &'a str,
    script: Option<&'a str>,
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; delimiters and trailing fields are untrusted manifest grammar. Invalid ordering and spans must return binding diagnostics, never assertion failures."
)]
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; delimiter discovery uses runtime iterator position and checked slice parsing to report malformed manifest grammar. Iterator::position is not const on the selected compiler."
)]
fn split_signature<'a>(
    words: &'a [&'a str],
    operation: &str,
) -> Result<SignatureWords<'a>, super::super::super::output::Failure> {
    let dash = attempt!(words
        .iter()
        .position(|word| *word == "--")
        .ok_or_else(|| super::invalid("binding requires -- separator")));
    let bang = attempt!(words
        .iter()
        .position(|word| *word == "!")
        .ok_or_else(|| super::invalid("binding requires ! effect separator")));
    if dash < 4 || bang <= dash {
        return Err(super::invalid("binding signature has invalid delimiters"));
    }
    let Some(inputs) = words.get(4..dash) else {
        return Err(super::invalid("binding signature has invalid delimiters"));
    };
    let Some(outputs) = words
        .get(dash..bang)
        .and_then(|section| section.strip_prefix(&["--"]))
    else {
        return Err(super::invalid("binding signature has invalid delimiters"));
    };
    let trailer = words.get(bang..).ok_or_else(|| super::invalid("binding signature has invalid delimiters"))?;
    let (effect, policy, script) = match (operation, trailer) {
        ("test.emit", ["!", effect, policy]) => (*effect, *policy, None),
        ("test.clock", ["!", effect, policy, "script", script]) =>
            (*effect, *policy, Some(*script)),
        _ => return Err(super::invalid("binding signature requires exact effect, policy and operation script")),
    };

    Ok(SignatureWords {
        inputs,
        outputs,
        effect,
        policy,
        script,
    })
}

#[expect(
    clippy::while_let_on_iterator,
    reason = "Owner: noble-maintainers; the selected full Octet compiler architecture cannot resolve for-loop desugaring, so bounded manifest stack types use explicit iterator advancement."
)]
fn types(
    words: &[&str],
) -> Result<std::vec::Vec<noble_kernel::types::Ty>, super::super::super::output::Failure> {
    if words.len() > 4 {
        return Err(super::invalid("binding stack type limit exceeded"));
    }
    let mut parsed = std::vec::Vec::with_capacity(words.len());
    let mut remaining = words.iter();
    while let Some(word) = remaining.next() {
        let ty = match *word {
            "I64" => noble_kernel::types::Ty::I64,
            "Bool" => noble_kernel::types::Ty::Bool,
            "Text" => noble_kernel::types::Ty::Text,
            "Unit" => noble_kernel::types::Ty::Unit,
            _ => return Err(super::invalid("binding type is unsupported")),
        };
        parsed.push(ty);
    }
    Ok(parsed)
}

impl Signature {
    pub(super) fn parse(words: &[&str], operation: &str) -> Result<Self, super::super::super::output::Failure> {
        let SignatureWords {
            inputs,
            outputs,
            effect,
            policy,
            script,
        } = attempt!(split_signature(words, operation));
        let inputs = attempt!(types(inputs));
        let outputs = attempt!(types(outputs));
        let effect = match effect {
            "test.emit" => noble_kernel::types::EffId(0),
            "test.abort" => noble_kernel::types::EffId(1),
            "test.clock" => noble_kernel::contracts::TEST_CLOCK,
            _ => return Err(super::invalid("binding effect identity is unknown")),
        };
        let is_allowed = match policy {
            "allow" => true,
            "deny" => false,
            _ => {
                return Err(super::invalid(
                    "binding authorization policy must be allow or deny",
                ))
            }
        };
        let script = match script {
            Some(values) => {
                let mut script = std::vec::Vec::new();
                for value in values.split(',') {
                    let (digits, negative) = match value.strip_prefix('-') {
                        Some(digits) => (digits, true),
                        None => (value, false),
                    };
                    if script.len() >= 16 || digits.is_empty()
                        || !digits.bytes().all(|digit| digit.is_ascii_digit())
                        || digits.starts_with('0') && (digits.len() > 1 || negative) {
                        return Err(super::invalid("binding script is noncanonical or exceeds 16 values"));
                    }
                    script.push(attempt!(value.parse::<i64>()
                        .map_err(|_| super::invalid("binding script value is not I64"))));
                }
                script
            }
            None => std::vec::Vec::new(),
        };
        Ok(Self {
            inputs,
            outputs,
            effect,
            allowed: is_allowed,
            script,
        })
    }
}
