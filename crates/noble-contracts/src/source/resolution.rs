#![expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-maintainers; resolution rewrites only freshly parsed preparation-owned word nodes into call targets and advances bounded comparison worklists/meter; source bytes and the borrowed committed definitions/namespace remain unchanged."
)]

mod comparison;

pub(super) fn resolve(
    tree: &mut super::Tree,
    declaration: Option<&str>,
    session: &super::Session,
    meter: &mut crate::Meter,
) -> Result<(), crate::Diagnostic> {
    let mut at = 0usize;
    let mut failure = None;
    while at < tree.nodes.len() {
        let result = match tree.nodes.get_mut(at) {
            Some(node) => resolve_node(node, declaration, session, meter),
            None => Err(crate::internal(tree.span)),
        };
        if let Err(problem) = result {
            failure = Some(problem);
            break;
        }
        at += 1;
    }
    match failure {
        Some(problem) => Err(problem),
        None => Ok(()),
    }
}

fn resolve_node(
    node: &mut super::Node,
    declaration: Option<&str>,
    session: &super::Session,
    meter: &mut crate::Meter,
) -> Result<(), crate::Diagnostic> {
    attempt!(meter.charge(1, node.span));
    if let super::Kind::Word(word) = &node.kind {
        let target = attempt!(lookup(word, declaration, session, node.span, meter));
        node.kind = super::Kind::Call(target);
    }
    Ok(())
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; reverse lookup charges every inspected immutable definition and byte comparison, then rejects unavailable hosts, recursion, imports, or unbound words with typed diagnostics."
)]
fn lookup(
    word: &[u8],
    declaration: Option<&str>,
    session: &super::Session,
    span: crate::Span,
    meter: &mut crate::Meter,
) -> Result<super::Target, crate::Diagnostic> {
    let mut at = session.definitions.len();
    let mut found = None;
    let mut failure = None;
    while at > 0 {
        at -= 1;
        match named_at(word, session, at, span, meter) {
            Ok(Some(target)) => {
                found = Some(target);
                break;
            }
            Ok(None) => {}
            Err(problem) => {
                failure = Some(problem);
                break;
            }
        }
    }
    if let Some(problem) = failure {
        return Err(problem);
    }
    if let Some(target) = found {
        return Ok(target);
    }
    if let Some(definition) = crate::program::bootstrap_word(word) {
        if definition.0 < 22 || session.hosts {
            return Ok(super::Target::Builtin(definition.0));
        }
    }
    let is_recursive = match declaration {
        Some(name) => name.as_bytes() == word,
        None => false,
    };
    if is_recursive {
        return Err(crate::Diagnostic::new(
            crate::DiagnosticKind::Unsupported,
            span,
            "recursive definitions require a signature and are outside Core-Bootstrap",
        ));
    }
    if word == b"import" {
        return Err(crate::Diagnostic::new(
            crate::DiagnosticKind::Unsupported,
            span,
            "module imports are outside Core-Bootstrap",
        ));
    }
    Err(crate::invalid(
        span,
        "unbound word in immutable namespace snapshot",
    ))
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; namespace lookup charges both the entry and compared bytes, checks indexes, and constructs owned diagnostics on failure."
)]
fn named_at(
    word: &[u8],
    session: &super::Session,
    at: usize,
    span: crate::Span,
    meter: &mut crate::Meter,
) -> Result<Option<super::Target>, crate::Diagnostic> {
    attempt!(meter.charge(1, span));
    let definition = match session.definitions.get(at) {
        Some(definition) => definition,
        None => return Err(crate::internal(span)),
    };
    attempt!(meter.charge(
        attempt!(crate::index(
            word.len().saturating_add(definition.name.len()),
            span
        )),
        span,
    ));
    if definition.name.as_bytes() == word {
        Ok(Some(super::Target::Named(attempt!(crate::index(at, span)))))
    } else {
        Ok(None)
    }
}

/// Intern canonical resolved composition, not source spelling or formatting.
/// Identity is session-local: no portable hash/serialization claim is made.
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; canonical comparisons consume work in installation order and preserve the first matching identity; new identities use checked conversion and increment rather than unchecked arithmetic."
)]
pub(super) fn identity(
    tree: &super::Tree,
    session: &super::Session,
    meter: &mut crate::Meter,
) -> Result<u64, crate::Diagnostic> {
    let mut at = 0usize;
    let mut found = None;
    let mut failure = None;
    while at < session.definitions.len() {
        match same_definition(tree, at, session, meter) {
            Ok(Some(identity)) => {
                found = Some(identity);
                break;
            }
            Ok(None) => {}
            Err(problem) => {
                failure = Some(problem);
                break;
            }
        }
        at += 1;
    }
    if let Some(problem) = failure {
        return Err(problem);
    }
    if let Some(identity) = found {
        return Ok(identity);
    }
    match u64::try_from(session.definitions.len())
        .ok()
        .and_then(|id| id.checked_add(1))
    {
        Some(identity) => Ok(identity),
        None => Err(super::exhausted(
            tree.span,
            "definition identity limit exceeded",
        )),
    }
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; definition identity comparison performs non-const metered structural traversal and returns owned diagnostics on invalid namespace entries."
)]
fn same_definition(
    tree: &super::Tree,
    at: usize,
    session: &super::Session,
    meter: &mut crate::Meter,
) -> Result<Option<u64>, crate::Diagnostic> {
    attempt!(meter.charge(1, tree.span));
    let definition = match session.definitions.get(at) {
        Some(definition) => definition,
        None => return Err(crate::internal(tree.span)),
    };
    let is_same = attempt!(comparison::same_body(
        tree,
        &definition.tree,
        session,
        meter
    ));
    if is_same {
        Ok(Some(definition.identity))
    } else {
        Ok(None)
    }
}
