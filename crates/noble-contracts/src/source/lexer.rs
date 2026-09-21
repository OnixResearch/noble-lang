mod text;
mod tokens;

pub(super) enum TokenKind {
    Open,
    Close,
    Def,
    Literal(noble_kernel::untrusted::Lit),
    Text(alloc::vec::Vec<u8>),
    Word(alloc::vec::Vec<u8>),
}

pub(super) struct Token {
    pub kind: TokenKind,
    pub span: crate::Span,
}

pub(super) struct Scanner<'a> {
    source: &'a [u8],
    at: usize,
    full: crate::Span,
}

impl<'a> Scanner<'a> {
    pub fn new(
        source_bytes: &'a [u8],
        meter: &mut crate::Meter,
    ) -> Result<Self, crate::Diagnostic> {
        let full = crate::Span {
            start: 0,
            end: attempt!(crate::index(
                source_bytes.len(),
                crate::Span { start: 0, end: 0 }
            )),
        };
        if full.end > meter.limits.bytes {
            return Err(super::exhausted(full, "source byte limit exceeded"));
        }
        attempt!(meter.charge(full.end, full));
        if core::str::from_utf8(source_bytes).is_err() {
            return Err(crate::invalid(full, "invalid UTF-8 source encoding"));
        }
        Ok(Self {
            source: source_bytes,
            at: 0,
            full,
        })
    }

    pub fn next(&mut self, meter: &mut crate::Meter) -> Result<Option<Token>, crate::Diagnostic> {
        attempt!(self.skip(meter));
        let start = self.at;
        let byte = match self.source.get(self.at).copied() {
            Some(byte) => byte,
            None => return Ok(None),
        };
        attempt!(meter.node(self.full));
        let kind = match byte {
            b'[' => {
                self.at += 1;
                TokenKind::Open
            }
            b']' => {
                self.at += 1;
                TokenKind::Close
            }
            b'"' => TokenKind::Text(attempt!(self.text(meter))),
            _ => attempt!(self.word(meter)),
        };
        Ok(Some(Token {
            kind,
            span: attempt!(self.span(start)),
        }))
    }

    fn skip(&mut self, meter: &mut crate::Meter) -> Result<(), crate::Diagnostic> {
        let mut failure = None;
        while let Some(byte) = self.source.get(self.at).copied() {
            match self.skip_step(byte, meter) {
                Ok(true) => {}
                Ok(false) => break,
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
        tigerstyle::missing_const_fn,
        reason = "Owner: noble-maintainers; trivia scanning charges source work and can construct an exhaustion diagnostic; comment scanning is a bounded runtime traversal."
    )]
    fn skip_step(&mut self, byte: u8, meter: &mut crate::Meter) -> Result<bool, crate::Diagnostic> {
        attempt!(meter.charge(1, self.full));
        if whitespace(byte) {
            self.at += 1;
            Ok(true)
        } else if byte == b'#' {
            attempt!(self.comment(meter));
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn comment(&mut self, meter: &mut crate::Meter) -> Result<(), crate::Diagnostic> {
        let mut failure = None;
        while let Some(byte) = self.source.get(self.at).copied() {
            if let Err(problem) = meter.charge(1, self.full) {
                failure = Some(problem);
                break;
            }
            if byte == b'\r' || byte == b'\n' {
                break;
            }
            self.at += 1;
        }
        match failure {
            Some(problem) => Err(problem),
            None => Ok(()),
        }
    }

    fn word(&mut self, meter: &mut crate::Meter) -> Result<TokenKind, crate::Diagnostic> {
        let start = self.at;
        let mut failure = None;
        while let Some(byte) = self.source.get(self.at).copied() {
            if let Err(problem) = meter.charge(1, self.full) {
                failure = Some(problem);
                break;
            }
            if whitespace(byte) || matches!(byte, b'#' | b'[' | b']' | b'"') {
                break;
            }
            self.at += 1;
        }
        if let Some(problem) = failure {
            return Err(problem);
        }
        let span = attempt!(self.span(start));
        let bytes = match self.source.get(start..self.at) {
            Some(bytes) => bytes,
            None => return Err(crate::internal(span)),
        };
        tokens::classify(bytes, span, meter)
    }

    fn span(&self, start: usize) -> Result<crate::Span, crate::Diagnostic> {
        Ok(crate::Span {
            start: attempt!(crate::index(start, self.full)),
            end: attempt!(crate::index(self.at, self.full)),
        })
    }

    #[expect(
        tigerstyle::missing_const_fn,
        reason = "Owner: noble-maintainers; consuming an escape byte charges work and returns an owned unterminated-literal diagnostic on missing input."
    )]
    fn byte(&mut self, meter: &mut crate::Meter) -> Result<u8, crate::Diagnostic> {
        attempt!(meter.charge(1, self.full));
        match self.source.get(self.at).copied() {
            Some(byte) => {
                self.at += 1;
                Ok(byte)
            }
            None => Err(crate::invalid(
                self.full,
                "unterminated text literal or escape",
            )),
        }
    }
}

const fn whitespace(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t' | b'\r' | b'\n')
}
