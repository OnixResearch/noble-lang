#![expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-maintainers; resolution rewrites only freshly parsed preparation-owned word nodes into call targets and advances bounded comparison worklists/meter; source bytes and the borrowed committed definitions/namespace remain unchanged."
)]

pub(crate) mod comparison;

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
    if let Some(context) = &session.declared {
        return declared_lookup(word, declaration, context, span, meter);
    }
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
    if let Some(bindings) = &session.bindings {
        if let Some(target) = attempt!(imported(word, bindings, span, meter)) {
            return Ok(target);
        }
    }
    if word == b"text.byte" && session.text_cursor {
        return Ok(super::Target::Builtin(26));
    }
    if word == b"self.propose" {
        if session.live_selected.as_deref() == declaration {
            if declaration.is_some() {
                return Ok(super::Target::Builtin(24));
            }
        }
        return Err(crate::invalid(span, "self.propose requires the selected named definition"));
    }
    if word == b"self.generation" {
        return if declaration.is_some() && session.live_selected.as_deref() == declaration {
            Ok(super::Target::Builtin(25))
        } else {
            Err(crate::invalid(span, "self.generation requires the selected named definition"))
        };
    }
    if let Some(definition) = crate::program::bootstrap_word(word) {
        if definition.0 < 22 || session.hosts {
            return Ok(super::Target::Builtin(definition.0));
        }
    }
    if let Some(name) = declaration {
        if name.as_bytes() == word {
            return Err(crate::Diagnostic::new(
                crate::DiagnosticKind::Unsupported,
                span,
                "recursive definitions require a signature and are outside Core-Bootstrap",
            ));
        }
    }
    if word == b"import" {
        return Err(crate::Diagnostic::new(
            crate::DiagnosticKind::Unsupported,
            span,
            "module imports are outside Core-Bootstrap",
        ));
    }
    Err(crate::Diagnostic::new(
        crate::DiagnosticKind::Invalid,
        span,
        "unbound word in immutable namespace snapshot",
    ))
}
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; reverse namespace lookup charges Meter before each exact word comparison and preserves first budget error then builtin/unbound precedence; guest failures must not panic; reassess fallible-check density."
)]
fn declared_lookup(
    word: &[u8],
    declaration: Option<&str>,
    context: &super::declared::Context,
    span: crate::Span,
    meter: &mut crate::Meter,
) -> Result<super::Target, crate::Diagnostic> {
    let mut at = context.words.len();
    let mut found = None;
    let mut failure = None;
    while at > 0 {
        at -= 1;
        let entry = &context.words[at];
        if let Err(problem) = meter.charge(1, span) {
            failure = Some(problem);
            break;
        }
        if entry.0.as_bytes() == word {
            found = Some(entry.1);
            break;
        }
    }
    if let Some(problem) = failure {
        return Err(problem);
    }
    if let Some(target) = found {
        return Ok(target);
    }
    if let Some(definition) = crate::program::bootstrap_word(word) {
        if definition.0 < 22 {
            return Ok(super::Target::Builtin(definition.0));
        }
    }
    if declaration.is_some() {
        return Err(crate::invalid(span, "unbound or recursive module word"));
    }
    Err(crate::invalid(span, "unbound or unexported module word"))
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; the finite generated-binding scan charges each compared name and propagates index or meter failure before returning an optional exact match."
)]
fn imported(
    word: &[u8],
    bindings: &crate::component::Bindings,
    span: crate::Span,
    meter: &mut crate::Meter,
) -> Result<Option<super::Target>, crate::Diagnostic> {
    let mut at = 0usize;
    let mut found = None;
    let mut failure = None;
    while let Some((name, definition)) = bindings.words.get(at) {
        let count = match crate::index(name.len(), span) {
            Ok(count) => count,
            Err(problem) => {
                failure = Some(problem);
                break;
            }
        };
        if let Err(problem) = meter.charge(count, span) {
            failure = Some(problem);
            break;
        }
        if name.as_bytes() == word {
            found = Some(super::Target::Builtin(definition.0));
            break;
        }
        at = at.saturating_add(1);
    }
    match failure {
        Some(problem) => Err(problem),
        None => Ok(found),
    }
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
