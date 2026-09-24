#![expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-maintainers; body admission mutates only fresh compiler-owned visitation bits; caller-owned nodes and text metadata remain immutable and all refusals return diagnostics."
)]

pub(super) fn nodes(
    candidate: &noble_kernel::untrusted::Candidate,
) -> Result<(), crate::Diagnostic> {
    let mut seen = alloc::vec![false; candidate.nodes.len()];
    let mut at = 0usize;
    let mut failure = None;
    let count = candidate.body.len();
    while at < count && failure.is_none() {
        let id = candidate.body[at];
        if let Err(error) = node(candidate, id, &mut seen) {
            failure = Some(error);
        }
        at = at.saturating_add(1);
    }
    match failure {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; node admission uses non-const TryFrom and checked runtime arena lookup before updating compiler-owned visitation bits."
)]
fn node(
    candidate: &noble_kernel::untrusted::Candidate,
    id: noble_kernel::untrusted::NodeId,
    seen: &mut [bool],
) -> Result<(), crate::Diagnostic> {
    let index = match usize::try_from(id.0) {
        Ok(index) => index,
        Err(_) => return Err(crate::Diagnostic::Invalid),
    };
    attempt!(super::visit(seen, index));
    match candidate.nodes.get(index) {
        Some(noble_kernel::untrusted::Node::Quotation { .. }) => {
            Err(crate::Diagnostic::Unsupported)
        }
        Some(
            noble_kernel::untrusted::Node::Literal { .. }
            | noble_kernel::untrusted::Node::Invocation { .. },
        ) => Ok(()),
        None => Err(crate::Diagnostic::Invalid),
    }
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; the bounded candidate arena indexes a fresh text-visited table; duplicate, missing, non-Text and invalid UTF-8 metadata are producer errors, and checked aggregate byte limits must fail through diagnostics rather than assertions."
)]
pub(super) fn validate_texts(
    body: &noble_kernel::execution::Body,
) -> Result<(), crate::Diagnostic> {
    let mut seen = alloc::vec![false; body.candidate.nodes.len()];
    let mut bytes = 0usize;
    let mut at = 0usize;
    let mut failure = None;
    let text_count = body.texts.len();
    while at < text_count && failure.is_none() {
        match text(body, &body.texts[at], &mut seen, bytes) {
            Ok(total) => bytes = total,
            Err(error) => failure = Some(error),
        }
        at = at.saturating_add(1);
    }
    if let Some(error) = failure {
        return Err(error);
    }
    at = 0;
    let node_count = body.candidate.nodes.len();
    while at < node_count && failure.is_none() {
        if is_text(&body.candidate.nodes[at]) && !seen[at] {
            failure = Some(crate::Diagnostic::Invalid);
        }
        at = at.saturating_add(1);
    }
    match failure {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

#[expect(
    tigerstyle::missing_const_fn,
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; non-const TryFrom, checked arena lookup and UTF-8 validation reject malformed literal metadata; duplicate or excessive bytes return diagnostics rather than assertions on producer inputs."
)]
fn text(
    body: &noble_kernel::execution::Body,
    text: &noble_kernel::execution::TextLiteral,
    seen: &mut [bool],
    bytes: usize,
) -> Result<usize, crate::Diagnostic> {
    let index = match usize::try_from(text.node.0) {
        Ok(index) => index,
        Err(_) => return Err(crate::Diagnostic::Invalid),
    };
    let is_visited = match seen.get(index).copied() {
        Some(is_visited) => is_visited,
        None => return Err(crate::Diagnostic::Invalid),
    };
    if is_visited {
        return Err(crate::Diagnostic::Invalid);
    }
    let node = match body.candidate.nodes.get(index) {
        Some(node) => node,
        None => return Err(crate::Diagnostic::Invalid),
    };
    if !is_text(node) || core::str::from_utf8(&text.bytes).is_err() {
        return Err(crate::Diagnostic::Invalid);
    }
    seen[index] = true;
    let bytes = bytes.saturating_add(text.bytes.len());
    if bytes > super::super::TEXT_BYTE_LIMIT {
        return Err(crate::Diagnostic::Exhausted);
    }
    Ok(bytes)
}

const fn is_text(node: &noble_kernel::untrusted::Node) -> bool {
    match node {
        noble_kernel::untrusted::Node::Literal { lit, .. } => {
            let lit = *lit;
            matches!(lit, noble_kernel::untrusted::Lit::Text)
        }
        noble_kernel::untrusted::Node::Invocation { .. }
        | noble_kernel::untrusted::Node::Quotation { .. } => false,
    }
}
