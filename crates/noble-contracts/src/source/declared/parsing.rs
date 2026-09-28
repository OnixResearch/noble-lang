//! Bounded lexical classification of declaration units.
mod body;
mod imports;
mod members;

// These are the complete syntactic forms of a Declared-Modules-v1 unit.
#[octet::sealed_enum]
pub(super) enum ParsedUnit {
    Module {
        name: alloc::string::String,
        version: u32,
        members: alloc::vec::Vec<Member>,
    },
    Import {
        name: alloc::string::String,
        version: u32,
        alias: alloc::string::String,
    },
    Definition,
    Expression,
}

// The v1 declaration grammar is closed; extending it requires updating registration.
#[octet::sealed_enum]
pub(super) enum Member {
    Opaque {
        name: alloc::string::String,
        base: alloc::string::String,
        public: bool,
    },
    Variant {
        name: alloc::string::String,
        left: alloc::string::String,
        left_type: alloc::string::String,
        left_public: bool,
        right: alloc::string::String,
        right_type: alloc::string::String,
        right_public: bool,
    },
    Require {
        name: alloc::string::String,
        input: alloc::string::String,
        operation: alloc::string::String,
    },
    Export(alloc::string::String),
    Definition(alloc::vec::Vec<u8>),
}

fn parse_diagnostic(span: crate::Span, reason: &str) -> crate::Diagnostic {
    crate::invalid(span, reason)
}

fn open_at(tokens: &[crate::source::lexer::Token], at: usize) -> bool {
    let Some(token) = tokens.get(at) else {
        return false;
    };
    let crate::source::lexer::TokenKind::Open = &token.kind else {
        return false;
    };
    true
}

fn word(
    token: Option<&crate::source::lexer::Token>,
    span: crate::Span,
) -> Result<&str, crate::Diagnostic> {
    let Some(crate::source::lexer::Token {
        kind: crate::source::lexer::TokenKind::Word(bytes),
        ..
    }) = token
    else {
        return Err(parse_diagnostic(span, "expected declaration word"));
    };
    core::str::from_utf8(bytes).map_err(|_| parse_diagnostic(span, "invalid UTF-8 word"))
}

pub(super) fn name(name: &str) -> bool {
    let mut bytes = name.bytes();
    let first = bytes.next();
    matches!(first, Some(b'a'..=b'z' | b'A'..=b'Z' | b'_'))
        && bytes.all(|c| c.is_ascii_alphanumeric() || c == b'_')
        && !reserved_name(name)
}

fn reserved_name(name: &str) -> bool {
    matches!(
        name,
        "def"
            | "module"
            | "import"
            | "as"
            | "opaque"
            | "variant"
            | "require"
            | "export"
            | "public"
            | "private"
            | "true"
            | "false"
            | "I64"
            | "Bool"
            | "Text"
            | "Unit"
            | "Pair"
            | "Sum"
            | "List"
            | "Resource"
    )
}
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; str.bytes and Iterator::all are non-const on pinned compilers; this validates canonical decimal versions; reassess const iteration."
)]
fn valid_version(version: &str) -> bool {
    !version.is_empty()
        && version.bytes().all(|c| c.is_ascii_digit())
        && version.as_bytes().first() != Some(&b'0')
}

fn versioned(
    text: &str,
    span: crate::Span,
) -> Result<(alloc::string::String, u32), crate::Diagnostic> {
    let (name_part, version) = attempt!(super::split_once_ascii(text, b'@')
        .ok_or_else(|| parse_diagnostic(span, "module requires name@version")));
    if !name(name_part) || !valid_version(version) {
        return Err(parse_diagnostic(span, "invalid module name or version"));
    }
    let version = attempt!(version
        .parse::<u32>()
        .map_err(|_| parse_diagnostic(span, "module version overflows")));
    Ok((alloc::string::String::from(name_part), version))
}

fn visibility(text: &str, span: crate::Span) -> Result<bool, crate::Diagnostic> {
    if text.eq("public") {
        return Ok(true);
    }
    if text.eq("private") {
        return Ok(false);
    }
    Err(parse_diagnostic(
        span,
        "expected public or private visibility",
    ))
}

struct Cursor<'a> {
    tokens: &'a [crate::source::lexer::Token],
    at: usize,
    span: crate::Span,
}

impl<'a> Cursor<'a> {
    fn next(mut self) -> Result<(Self, &'a str), crate::Diagnostic> {
        let value = attempt!(word(self.tokens.get(self.at), self.span));
        self.at += 1;
        Ok((self, value))
    }
    fn ident(self) -> Result<(Self, alloc::string::String), crate::Diagnostic> {
        let span = self.span;
        let (cursor, value) = attempt!(self.next());
        if !name(value) {
            return Err(parse_diagnostic(
                span,
                "invalid or reserved declaration name",
            ));
        }
        Ok((cursor, alloc::string::String::from(value)))
    }
    fn expected(self, expected: &str) -> Result<Self, crate::Diagnostic> {
        let span = self.span;
        let (cursor, actual) = attempt!(self.next());
        if actual.eq(expected) {
            Ok(cursor)
        } else {
            Err(parse_diagnostic(span, "invalid declaration signature"))
        }
    }
    fn public(self) -> Result<(Self, bool), crate::Diagnostic> {
        let span = self.span;
        let (cursor, value) = attempt!(self.next());
        Ok((cursor, attempt!(visibility(value, span))))
    }
}

pub(super) fn parse(
    input_bytes: &[u8],
    limits: crate::Limits,
) -> Result<ParsedUnit, crate::Diagnostic> {
    let (tokens, span) = attempt!(scan(input_bytes, limits));
    let first = tokens.first();
    let is_module = declaration_word(first, b"module");
    let is_import = declaration_word(first, b"import");
    if !is_module && !is_import {
        return Ok(ordinary_unit(first));
    }
    let cursor = Cursor {
        tokens: &tokens,
        at: 1,
        span,
    };
    parse_declaration(input_bytes, limits, cursor, is_import)
}

fn parse_declaration(
    input_bytes: &[u8],
    limits: crate::Limits,
    cursor: Cursor<'_>,
    is_import: bool,
) -> Result<ParsedUnit, crate::Diagnostic> {
    let span = cursor.span;
    let (cursor, version_word) = attempt!(cursor.next());
    let (name, version) = attempt!(versioned(version_word, span));
    if is_import {
        return imports::parse(cursor, name, version);
    }
    members::parse(input_bytes, limits, cursor, name, version)
}

fn ordinary_unit(first: Option<&crate::source::lexer::Token>) -> ParsedUnit {
    if matches!(
        first,
        Some(crate::source::lexer::Token {
            kind: crate::source::lexer::TokenKind::Def,
            ..
        })
    ) {
        ParsedUnit::Definition
    } else {
        ParsedUnit::Expression
    }
}

fn declaration_word(token: Option<&crate::source::lexer::Token>, spelling: &[u8]) -> bool {
    matches!(token, Some(crate::source::lexer::Token {
        kind: crate::source::lexer::TokenKind::Word(bytes), ..
    }) if bytes.as_slice().eq(spelling))
}
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; bounded Scanner.next and Meter preserve the first lexical Diagnostic before checked-span construction; guest errors must not panic; reassess fallible-check density."
)]
fn scan(
    input_bytes: &[u8],
    limits: crate::Limits,
) -> Result<(alloc::vec::Vec<crate::source::lexer::Token>, crate::Span), crate::Diagnostic> {
    let mut meter = crate::Meter::new(limits);
    let mut scanner = attempt!(crate::source::lexer::Scanner::new_declared(
        input_bytes,
        &mut meter
    ));
    let initial_slots = attempt!(token_slots(limits, input_bytes.len()));
    let mut tokens = alloc::vec::Vec::with_capacity(initial_slots);
    let mut next = scanner.next(&mut meter);
    let mut failure = None;
    while let Some(token) = match next {
        Ok(token) => token,
        Err(problem) => {
            failure = Some(problem);
            None
        }
    } {
        tokens.push(token);
        next = scanner.next(&mut meter);
    }
    if let Some(problem) = failure {
        return Err(problem);
    }
    let span = crate::Span {
        start: 0,
        end: attempt!(crate::index(
            input_bytes.len(),
            crate::Span { start: 0, end: 0 }
        )),
    };
    Ok((tokens, span))
}

fn token_slots(limits: crate::Limits, source_bytes: usize) -> Result<usize, crate::Diagnostic> {
    usize::try_from(limits.nodes)
        .map(|max_tokens| max_tokens.min(source_bytes))
        .map_err(|_| {
            parse_diagnostic(
                crate::Span { start: 0, end: 0 },
                "token limit exceeds the host address space",
            )
        })
}
