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
                at + 1,
            ))
        }
        LogicalKind::Colon | LogicalKind::Comma => {
            let spelling = if matches!(&token.kind, LogicalKind::Colon) { ":" } else { "," };
            Ok((
                crate::intrinsic::Form {
                    kind: crate::intrinsic::FormKind::Atom(alloc::string::String::from(spelling)),
                    span: token.span,
                },
                at + 1,
            ))
        }
        LogicalKind::OpenParen => {
            let mut items = alloc::vec::Vec::new();
            let mut next = at + 1;
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
                        next + 1,
                    ));
                }
                let (item, after) = attempt!(form(tokens, next, span));
                items.push(item);
                next = after;
            }
        }
        LogicalKind::CloseParen => Err(crate::invalid(token.span, "unmatched logical parenthesis")),
    }
}
