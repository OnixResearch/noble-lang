#![expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-maintainers; validation mutates only preparation-owned work accounting and fresh bounded scratch buffers; the borrowed submission and published compiler remain unchanged."
)]

pub(super) struct Totals {
    pub(super) nodes: usize,
    pub(super) bytes: usize,
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; checked aggregate counts and the shared meter bound every metadata scan; unknown formats, missing or duplicate Text payloads and invalid UTF-8 are producer errors returned as diagnostics."
)]
pub(super) fn check(
    body: &noble_kernel::execution::Body,
    totals: &mut Totals,
    work: &mut super::super::Work,
) -> Result<(), crate::Diagnostic> {
    if body.candidate.format != noble_kernel::untrusted::CANDIDATE_FORMAT
        || body.candidate.revision != noble_kernel::untrusted::SEMANTIC_REVISION
    {
        return Err(crate::Diagnostic::Unsupported);
    }
    totals.nodes = match totals.nodes.checked_add(body.candidate.nodes.len()) {
        Some(value) => value,
        None => return Err(crate::Diagnostic::Exhausted),
    };
    if totals.nodes > super::super::NODE_LIMIT
        || body.candidate.body.len() > super::super::NODE_LIMIT
        || body.texts.len() > body.candidate.nodes.len()
    {
        return Err(crate::Diagnostic::Exhausted);
    }
    let mut at = 0usize;
    let mut failure = None;
    while at < body.candidate.nodes.len() {
        match check_node(body, at, work) {
            Ok(()) => at += 1,
            Err(problem) => {
                failure = Some(problem);
                break;
            }
        }
    }
    if let Some(problem) = failure {
        return Err(problem);
    }
    at = 0;
    while at < body.texts.len() {
        match check_text(body, at, totals, work) {
            Ok(()) => at += 1,
            Err(problem) => {
                failure = Some(problem);
                break;
            }
        }
    }
    match failure {
        Some(problem) => Err(problem),
        None => Ok(()),
    }
}

#[expect(
    tigerstyle::assertion_density,
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; each node is charged before validating quotation bounds or performing a non-const metered metadata scan; missing and duplicate Text payloads return Invalid rather than asserting on supplied nodes."
)]
fn check_node(
    body: &noble_kernel::execution::Body,
    at: usize,
    work: &mut super::super::Work,
) -> Result<(), crate::Diagnostic> {
    attempt!(work.charge(1));
    match &body.candidate.nodes[at] {
        noble_kernel::untrusted::Node::Quotation { body, .. }
            if body.len() > super::super::NODE_LIMIT =>
        {
            Err(crate::Diagnostic::Exhausted)
        }
        noble_kernel::untrusted::Node::Literal {
            lit: noble_kernel::untrusted::Lit::Text,
            ..
        } => {
            if attempt!(text_count(body, at, work)) != 1 {
                Err(crate::Diagnostic::Invalid)
            } else {
                Ok(())
            }
        }
        _ => Ok(()),
    }
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; metadata count is bounded by the admitted arena and every comparison is charged before checked index conversion; the first conversion or meter failure stops the scan without panic."
)]
fn text_count(
    body: &noble_kernel::execution::Body,
    at: usize,
    work: &mut super::super::Work,
) -> Result<usize, crate::Diagnostic> {
    let mut found = 0usize;
    let mut text = 0usize;
    let mut failure = None;
    while text < body.texts.len() {
        match text_matches(body.texts[text].node, at, work) {
            Ok(matches) => {
                if matches {
                    found += 1;
                }
                text += 1;
            }
            Err(problem) => {
                failure = Some(problem);
                break;
            }
        }
    }
    match failure {
        Some(problem) => Err(problem),
        None => Ok(found),
    }
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; metered node comparison uses the non-const checked NodeId conversion and preserves its diagnostic before comparing the index."
)]
fn text_matches(
    node: noble_kernel::untrusted::NodeId,
    at: usize,
    work: &mut super::super::Work,
) -> Result<bool, crate::Diagnostic> {
    attempt!(work.charge(1));
    Ok(attempt!(crate::admission::index(node)) == at)
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; checked text-node lookup and aggregate byte bounds precede non-const byte metering and UTF-8 traversal; malformed metadata and oversized payloads return diagnostics instead of assertions."
)]
fn check_text(
    body: &noble_kernel::execution::Body,
    at: usize,
    totals: &mut Totals,
    work: &mut super::super::Work,
) -> Result<(), crate::Diagnostic> {
    let text = &body.texts[at];
    match attempt!(super::node(&body.candidate, text.node)) {
        noble_kernel::untrusted::Node::Literal {
            lit: noble_kernel::untrusted::Lit::Text,
            ..
        } => {}
        _ => return Err(crate::Diagnostic::Invalid),
    }
    totals.bytes = match totals.bytes.checked_add(text.bytes.len()) {
        Some(value) => value,
        None => return Err(crate::Diagnostic::Exhausted),
    };
    if totals.bytes > 786_432 {
        return Err(crate::Diagnostic::Exhausted);
    }
    attempt!(work.entries(text.bytes.len()));
    utf8(&text.bytes)
}

fn utf8(bytes: &[u8]) -> Result<(), crate::Diagnostic> {
    let mut index = 0usize;
    let mut failure = None;
    while index < bytes.len() {
        match sequence(bytes, index) {
            Ok(width) => index += width,
            Err(problem) => {
                failure = Some(problem);
                break;
            }
        }
    }
    match failure {
        Some(problem) => Err(problem),
        None => Ok(()),
    }
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; each scalar width is checked against the remaining bytes before continuation access; malformed, overlong, surrogate and out-of-range sequences return Invalid rather than asserting on text input."
)]
fn sequence(bytes: &[u8], index: usize) -> Result<usize, crate::Diagnostic> {
    let first = bytes[index];
    let width = match first {
        0x00..=0x7f => 1usize,
        0xc2..=0xdf => 2,
        0xe0..=0xef => 3,
        0xf0..=0xf4 => 4,
        _ => return Err(crate::Diagnostic::Invalid),
    };
    let remaining = &bytes[index..];
    if remaining.len() < width {
        return Err(crate::Diagnostic::Invalid);
    }
    let mut offset_bytes = 1usize;
    let mut is_valid = true;
    while offset_bytes < width {
        let byte = remaining[offset_bytes];
        if byte & 0xc0 != 0x80 {
            is_valid = false;
            break;
        }
        if offset_bytes == 1 && !valid_leading_pair([first, byte]) {
            is_valid = false;
            break;
        }
        offset_bytes += 1;
    }
    if is_valid {
        Ok(width)
    } else {
        Err(crate::Diagnostic::Invalid)
    }
}

const fn valid_leading_pair(bytes: [u8; 2]) -> bool {
    match bytes[0] {
        0xe0 => bytes[1] >= 0xa0,
        0xed => bytes[1] <= 0x9f,
        0xf0 => bytes[1] >= 0x90,
        0xf4 => bytes[1] <= 0x8f,
        _ => true,
    }
}
