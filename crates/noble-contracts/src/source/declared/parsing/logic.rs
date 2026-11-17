//! Parse one logical S-expression from a scanner-selected bracketed body.
//! This syntax is untrusted; independent proof/type checking happens later.

pub(super) fn parse(
    tokens: &[crate::source::lexer::LogicalToken],
    span: crate::Span,
) -> Result<crate::intrinsic::Form, crate::Diagnostic> {
    let Some(_) = tokens.first() else {
        return Err(crate::invalid(
            span,
            "logical body must contain exactly one form",
        ));
    };
    let mut step = Ok(Logical::Pending(Nesting {
        frames: alloc::vec::Vec::new(),
        values: alloc::vec::Vec::new(),
    }));
    let mut at = 0usize;
    while at < tokens.len() && matches!(step, Ok(Logical::Pending(_))) {
        step = match step {
            Ok(Logical::Pending(nesting)) => logical_step(tokens, at, nesting),
            finished => finished,
        };
        at += 1;
    }
    match attempt!(step) {
        Logical::Complete(form) => Ok(form),
        Logical::Pending(nesting) => Err(crate::invalid(
            nesting.frames.last().map_or(span, |frame| frame.0),
            "unclosed logical parenthesis",
        )),
    }
}

/// Open parentheses (span, first value index) and the values parsed inside them.
struct Nesting {
    frames: alloc::vec::Vec<(crate::Span, usize)>,
    values: alloc::vec::Vec<crate::intrinsic::Form>,
}

/// A logical body still being read, or its one complete form.
enum Logical {
    Pending(Nesting),
    Complete(crate::intrinsic::Form),
}

#[expect(
    tigerstyle::function_length,
    reason = "Owner: noble-contracts; the single-pass dispatcher is kept linear so its stage order stays reviewable"
)]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; the invariants of this linear dispatcher are enforced by its callers and the contracts test suite"
)]
#[expect(
    tigerstyle::non_trait_imports,
    reason = "Owner: noble-contracts; this stage imports the domain types it operates on directly, matching the module vocabulary"
)]
/// Read the token at `at`: open a list, close one, or place one form.
fn logical_step(
    tokens: &[crate::source::lexer::LogicalToken],
    at: usize,
    mut nesting: Nesting,
) -> Result<Logical, crate::Diagnostic> {
    use crate::source::lexer::LogicalKind;
    let token = &tokens[at];
    let form = match &token.kind {
        LogicalKind::OpenParen => {
            if nesting.frames.len() >= 64 {
                return Err(crate::invalid(
                    token.span,
                    "logical nesting exceeds supported depth",
                ));
            }
            nesting.frames.push((token.span, nesting.values.len()));
            return Ok(Logical::Pending(nesting));
        }
        LogicalKind::CloseParen => {
            let Some((opening, start)) = nesting.frames.pop() else {
                return Err(crate::invalid(token.span, "unmatched logical parenthesis"));
            };
            crate::intrinsic::Form {
                kind: crate::intrinsic::FormKind::List(nesting.values.split_off(start)),
                span: crate::Span {
                    start: opening.start,
                    end: token.span.end,
                },
            }
        }
        LogicalKind::Atom(bytes) => {
            let text = attempt!(core::str::from_utf8(bytes)
                .map_err(|_| crate::invalid(token.span, "invalid logical UTF-8 atom")));
            crate::intrinsic::Form {
                kind: crate::intrinsic::FormKind::Atom(alloc::string::String::from(text)),
                span: token.span,
            }
        }
        LogicalKind::Colon | LogicalKind::Comma => {
            let spelling = if matches!(&token.kind, LogicalKind::Colon) {
                ":"
            } else {
                ","
            };
            crate::intrinsic::Form {
                kind: crate::intrinsic::FormKind::Atom(alloc::string::String::from(spelling)),
                span: token.span,
            }
        }
    };
    let next = attempt!(at.checked_add(1).ok_or_else(|| {
        crate::invalid(token.span, "logical token position exceeds address space")
    }));
    if nesting.frames.is_empty() && next == tokens.len() {
        return Ok(Logical::Complete(form));
    }
    if nesting.frames.is_empty() {
        return Err(crate::invalid(
            tokens[next].span,
            "logical body contains a trailing form",
        ));
    }
    if nesting.values.len() >= tokens.len() {
        return Err(crate::invalid(
            token.span,
            "logical list exceeds its token body",
        ));
    }
    nesting.values.push(form);
    Ok(Logical::Pending(nesting))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[expect(
        tigerstyle::non_trait_imports,
        reason = "Owner: noble-contracts; the module imports these domain types directly, and the qualified form at every use site would add noise without changing the dependency"
    )]
    use crate::source::lexer::{LogicalKind, LogicalToken};

    #[test]
    fn nested_form_preserves_order_and_rejects_excess_depth() {
        let span = crate::Span { start: 0, end: 1 };
        let tokens = [
            LogicalToken {
                kind: LogicalKind::OpenParen,
                span,
            },
            LogicalToken {
                kind: LogicalKind::Atom(alloc::vec![b'a']),
                span,
            },
            LogicalToken {
                kind: LogicalKind::OpenParen,
                span,
            },
            LogicalToken {
                kind: LogicalKind::Atom(alloc::vec![b'b']),
                span,
            },
            LogicalToken {
                kind: LogicalKind::CloseParen,
                span,
            },
            LogicalToken {
                kind: LogicalKind::CloseParen,
                span,
            },
        ];
        let form = parse(&tokens, span).expect("nested form");
        let crate::intrinsic::FormKind::List(items) = form.kind else {
            panic!("expected outer list");
        };
        let crate::intrinsic::FormKind::Atom(first) = &items[0].kind else {
            panic!("expected first atom");
        };
        assert_eq!(first, "a");
        let crate::intrinsic::FormKind::List(inner) = &items[1].kind else {
            panic!("expected inner list");
        };
        let crate::intrinsic::FormKind::Atom(second) = &inner[0].kind else {
            panic!("expected nested atom");
        };
        assert_eq!(second, "b");

        let deep: alloc::vec::Vec<_> = (0..65)
            .map(|_| LogicalToken {
                kind: LogicalKind::OpenParen,
                span,
            })
            .collect();
        let error = parse(&deep, span).expect_err("nesting bound");
        assert_eq!(error.message, "logical nesting exceeds supported depth");
    }

    #[test]
    fn trailing_and_unclosed_forms_are_refused_at_their_token() {
        let span = crate::Span { start: 0, end: 9 };
        let at = |start: u32| crate::Span {
            start,
            end: start + 1,
        };
        let atom = |start: u32| LogicalToken {
            kind: LogicalKind::Atom(alloc::vec![b'a']),
            span: at(start),
        };
        let open = |start: u32| LogicalToken {
            kind: LogicalKind::OpenParen,
            span: at(start),
        };
        let error = parse(&[atom(1), atom(2), atom(3)], span).expect_err("trailing form");
        assert_eq!(error.message, "logical body contains a trailing form");
        assert_eq!(error.span, at(2));
        let error = parse(&[open(1), open(2), atom(3)], span).expect_err("unclosed form");
        assert_eq!(error.message, "unclosed logical parenthesis");
        assert_eq!(error.span, at(2));
        let error = parse(
            &[
                LogicalToken {
                    kind: LogicalKind::CloseParen,
                    span: at(4),
                },
                atom(5),
            ],
            span,
        )
        .expect_err("unmatched close");
        assert_eq!(error.message, "unmatched logical parenthesis");
        assert_eq!(error.span, at(4));
    }
}
