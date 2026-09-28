//! Parsed adapter signature and host authorization fields of a binding row.
//! These values describe a host adapter; they never grant guest authority.

pub(super) struct Signature {
    pub(super) inputs: std::vec::Vec<noble_kernel::types::Ty>,
    pub(super) outputs: std::vec::Vec<noble_kernel::types::Ty>,
    pub(super) effect: noble_kernel::types::EffId,
    pub(super) allowed: bool,
}

struct SignatureWords<'a> {
    inputs: &'a [&'a str],
    outputs: &'a [&'a str],
    effect: &'a str,
    policy: &'a str,
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
    let Some(["!", effect, policy]) = words.get(bang..) else {
        return Err(super::invalid("binding signature has invalid delimiters"));
    };
    Ok(SignatureWords {
        inputs,
        outputs,
        effect,
        policy,
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
    pub(super) fn parse(words: &[&str]) -> Result<Self, super::super::super::output::Failure> {
        let SignatureWords {
            inputs,
            outputs,
            effect,
            policy,
        } = attempt!(split_signature(words));
        let inputs = attempt!(types(inputs));
        let outputs = attempt!(types(outputs));
        let effect = match effect {
            "test.emit" => noble_kernel::types::EffId(0),
            "test.abort" => noble_kernel::types::EffId(1),
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
        Ok(Self {
            inputs,
            outputs,
            effect,
            allowed: is_allowed,
        })
    }
}
