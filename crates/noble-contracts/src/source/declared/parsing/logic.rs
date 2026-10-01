//! Parse one logical S-expression from a scanner-selected bracketed body.
//! This syntax is untrusted; independent proof/type checking happens later.

pub(super) fn parse(
    tokens: &[crate::source::lexer::LogicalToken],
    span: crate::Span,
) -> Result<crate::intrinsic::Form, crate::Diagnostic> {
    let (form, next) = attempt!(form(tokens, 0, span));
    if next != tokens.len() {
        return Err(crate::invalid(tokens[next].span, "logical body contains a trailing form"));
    }
    Ok(form)
}

fn form(
    tokens: &[crate::source::lexer::LogicalToken],
    at: usize,
    span: crate::Span,
) -> Result<(crate::intrinsic::Form, usize), crate::Diagnostic> {
    use crate::source::lexer::LogicalKind;
    let Some(token) = tokens.get(at) else {
        return Err(crate::invalid(span, "logical body must contain exactly one form"));
    };
    match &token.kind {
        LogicalKind::Atom(bytes) => {
            let text = attempt!(core::str::from_utf8(bytes)
                .map_err(|_| crate::invalid(token.span, "invalid logical UTF-8 atom")));
            Ok((
                crate::intrinsic::Form {
                    kind: crate::intrinsic::FormKind::Atom(alloc::string::String::from(text)),
                    span: token.span,
                },
                attempt!(at.checked_add(1).ok_or_else(|| {
                    crate::invalid(token.span, "logical token position exceeds address space")
                })),
            ))
        }
        LogicalKind::Colon | LogicalKind::Comma => {
            let spelling = if matches!(&token.kind, LogicalKind::Colon) { ":" } else { "," };
            Ok((
                crate::intrinsic::Form {
                    kind: crate::intrinsic::FormKind::Atom(alloc::string::String::from(spelling)),
                    span: token.span,
                },
                attempt!(at.checked_add(1).ok_or_else(|| {
                    crate::invalid(token.span, "logical token position exceeds address space")
                })),
            ))
        }
        LogicalKind::OpenParen => {
            let mut items = alloc::vec::Vec::new();
            let mut next = attempt!(at.checked_add(1).ok_or_else(|| {
                crate::invalid(token.span, "logical token position exceeds address space")
            }));
            loop {
                let Some(current) = tokens.get(next) else {
                    return Err(crate::invalid(token.span, "unclosed logical parenthesis"));
                };
                if matches!(&current.kind, LogicalKind::CloseParen) {
                    return Ok((
                        crate::intrinsic::Form {
                            kind: crate::intrinsic::FormKind::List(items),
                            span: crate::Span {
                                start: token.span.start,
                                end: current.span.end,
                            },
                        },
                        attempt!(next.checked_add(1).ok_or_else(|| {
                            crate::invalid(current.span, "logical token position exceeds address space")
                        })),
                    ));
                }
                let (item, after) = attempt!(form(tokens, next, span));
                // Every item consumes at least one token, so this bound never
                // precedes the ordinary unclosed/unmatched diagnostics.
                if items.len() >= tokens.len() {
                    return Err(crate::invalid(token.span, "logical list exceeds its token body"));
                }
                items.push(item);
                next = after;
            }
        }
        LogicalKind::CloseParen => Err(crate::invalid(token.span, "unmatched logical parenthesis")),
    }
}
