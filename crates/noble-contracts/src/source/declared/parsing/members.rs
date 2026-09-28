pub(super) fn parse(
    input_bytes: &[u8],
    limits: crate::Limits,
    cursor: super::Cursor<'_>,
    name: alloc::string::String,
    version: u32,
) -> Result<super::ParsedUnit, crate::Diagnostic> {
    let span = cursor.span;
    let cursor = attempt!(open_body(cursor));
    let (cursor, members) = attempt!(collect_body(input_bytes, limits, cursor));
    if cursor.at != cursor.tokens.len() {
        return Err(super::parse_diagnostic(
            span,
            "module contains trailing expressions",
        ));
    }
    Ok(super::ParsedUnit::Module {
        name,
        version,
        members,
    })
}

fn open_body(mut cursor: super::Cursor<'_>) -> Result<super::Cursor<'_>, crate::Diagnostic> {
    if !super::open_at(cursor.tokens, cursor.at) {
        return Err(super::parse_diagnostic(
            cursor.span,
            "module body must be one bracketed declaration block",
        ));
    }
    cursor.at += 1;
    Ok(cursor)
}
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; bounded member collection stops at Close or the first parse_member Diagnostic and reports unclosed bodies without panicking; reassess fallible-check density."
)]
fn collect_body<'a>(
    input_bytes: &[u8],
    limits: crate::Limits,
    cursor: super::Cursor<'a>,
) -> Result<(super::Cursor<'a>, alloc::vec::Vec<super::Member>), crate::Diagnostic> {
    let span = cursor.span;
    let max_members = attempt!(usize::try_from(limits.nodes)
        .map_err(|_| super::parse_diagnostic(span, "member limit exceeds the host address space")));
    let member_count = max_members.min(cursor.tokens.len());
    let mut members = alloc::vec::Vec::with_capacity(member_count);
    let tokens = cursor.tokens;
    let mut cursor = cursor;
    let mut is_closed = false;
    let mut problem = None;
    while cursor.at < tokens.len() && !is_closed && problem.is_none() {
        let token = &tokens[cursor.at];
        if let crate::source::lexer::TokenKind::Close = &token.kind {
            cursor.at += 1;
            is_closed = true;
        } else {
            let next = super::Cursor {
                tokens,
                at: cursor.at,
                span: cursor.span,
            };
            match parse_member(input_bytes, limits, next, token) {
                Ok((next, member)) => {
                    cursor = next;
                    members.push(member);
                }
                Err(error) => problem = Some(error),
            }
        }
    }
    match problem {
        Some(problem) => Err(problem),
        None if is_closed => Ok((cursor, members)),
        None => Err(super::parse_diagnostic(span, "unclosed module body")),
    }
}

fn parse_member<'a>(
    input_bytes: &[u8],
    limits: crate::Limits,
    mut cursor: super::Cursor<'a>,
    token: &crate::source::lexer::Token,
) -> Result<(super::Cursor<'a>, super::Member), crate::Diagnostic> {
    cursor.at += 1;
    if let crate::source::lexer::TokenKind::Word(bytes) = &token.kind {
        return parse_word_member(cursor, bytes, token.span);
    }
    if let crate::source::lexer::TokenKind::Def = &token.kind {
        return super::body::parse(input_bytes, limits, cursor, token);
    }
    Err(super::parse_diagnostic(
        token.span,
        "unknown module declaration or initializer",
    ))
}

fn parse_word_member<'a>(
    cursor: super::Cursor<'a>,
    spelling: &[u8],
    span: crate::Span,
) -> Result<(super::Cursor<'a>, super::Member), crate::Diagnostic> {
    if spelling.eq(b"opaque") {
        return opaque(cursor);
    }
    if spelling.eq(b"variant") {
        return variant(cursor);
    }
    if spelling.eq(b"require") {
        return require(cursor);
    }
    if spelling.eq(b"export") {
        return export(cursor);
    }
    Err(super::parse_diagnostic(
        span,
        "unknown module declaration or initializer",
    ))
}

fn opaque(
    cursor: super::Cursor<'_>,
) -> Result<(super::Cursor<'_>, super::Member), crate::Diagnostic> {
    let (cursor, name) = attempt!(cursor.ident());
    let (cursor, base) = attempt!(cursor.next());
    let base = alloc::string::String::from(base);
    let (cursor, public) = attempt!(cursor.public());
    Ok((cursor, super::Member::Opaque { name, base, public }))
}

fn variant(
    cursor: super::Cursor<'_>,
) -> Result<(super::Cursor<'_>, super::Member), crate::Diagnostic> {
    let (cursor, name) = attempt!(cursor.ident());
    let (cursor, left, left_type, left_public) = attempt!(variant_arm(cursor));
    let (cursor, right, right_type, right_public) = attempt!(variant_arm(cursor));
    Ok((
        cursor,
        super::Member::Variant {
            name,
            left,
            left_type,
            left_public,
            right,
            right_type,
            right_public,
        },
    ))
}

fn variant_arm(
    cursor: super::Cursor<'_>,
) -> Result<
    (
        super::Cursor<'_>,
        alloc::string::String,
        alloc::string::String,
        bool,
    ),
    crate::Diagnostic,
> {
    let (cursor, left) = attempt!(cursor.ident());
    let (cursor, left_type) = attempt!(cursor.next());
    let left_type = alloc::string::String::from(left_type);
    let (cursor, left_public) = attempt!(cursor.public());
    Ok((cursor, left, left_type, left_public))
}

fn require(
    cursor: super::Cursor<'_>,
) -> Result<(super::Cursor<'_>, super::Member), crate::Diagnostic> {
    let (cursor, name) = attempt!(cursor.ident());
    let (cursor, input) = attempt!(cursor.next());
    let input = alloc::string::String::from(input);
    let cursor = attempt!(cursor.expected("--"));
    let cursor = attempt!(cursor.expected("!"));
    let (cursor, operation) = attempt!(cursor.next());
    Ok((
        cursor,
        super::Member::Require {
            name,
            input,
            operation: alloc::string::String::from(operation),
        },
    ))
}

fn export(
    cursor: super::Cursor<'_>,
) -> Result<(super::Cursor<'_>, super::Member), crate::Diagnostic> {
    let span = cursor.span;
    let (cursor, target) = attempt!(cursor.next());
    if !exported_name(target) {
        return Err(super::parse_diagnostic(
            span,
            "invalid exported schema, operation or definition",
        ));
    }
    Ok((
        cursor,
        super::Member::Export(alloc::string::String::from(target)),
    ))
}

fn exported_name(target: &str) -> bool {
    let bytes = target.as_bytes();
    let mut split = 0;
    while split < bytes.len() && bytes[split] != b'.' {
        split += 1;
    }
    let base = &target[..split];
    if !super::name(base) {
        return false;
    }
    split == bytes.len()
        || match split.checked_add(1) {
            Some(next) => super::name(&target[next..]),
            None => false,
        }
}
