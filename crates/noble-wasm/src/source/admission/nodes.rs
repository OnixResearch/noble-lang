//! Node and text admission for a checked candidate body.

pub(in crate::source) fn text(
    body: &noble_kernel::execution::Body,
    node: noble_kernel::untrusted::NodeId,
) -> Result<&[u8], crate::Diagnostic> {
    match crate::scan::find_first(&body.texts, |text| text.node == node) {
        Some(text) => Ok(text.bytes.as_slice()),
        None => Err(crate::Diagnostic::Invalid),
    }
}

pub(in crate::source) fn node(
    candidate: &noble_kernel::untrusted::Candidate,
    id: noble_kernel::untrusted::NodeId,
) -> Result<&noble_kernel::untrusted::Node, crate::Diagnostic> {
    match candidate.nodes.get(attempt!(crate::admission::index(id))) {
        Some(node) => Ok(node),
        None => Err(crate::Diagnostic::Invalid),
    }
}
