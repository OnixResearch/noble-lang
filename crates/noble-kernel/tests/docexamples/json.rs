//! Minimal JSON parser for the executed documentation examples (DX-DOC-01).
//!
//! Just enough JSON for the example schema; nesting is bounded and anything
//! else is an error, never a panic. The value tree lives in `crate::value`.

#[path = "json/numbers.rs"]
mod numbers;

/// One parse in progress over the remaining text.
struct Scan<'a> {
    text: &'a [u8],
    at: usize,
    depth: u32,
}

/// Parse one complete JSON document.
pub(super) fn parse(source: &str) -> Result<crate::value::Json, String> {
    let mut scan = Scan {
        text: source.as_bytes(),
        at: 0,
        depth: 0,
    };
    skip_space(&mut scan);
    let value = scan_value(&mut scan)?;
    skip_space(&mut scan);
    if scan.at != scan.text.len() {
        return Err(format!("trailing content at byte {}", scan.at));
    }
    Ok(value)
}

fn skip_space(scan: &mut Scan) {
    while scan.at < scan.text.len() && matches!(scan.text[scan.at], b' ' | b'\t' | b'\n' | b'\r') {
        scan.at += 1;
    }
}

fn expect(scan: &mut Scan, wanted: &[u8]) -> Result<(), String> {
    if scan.text[scan.at..].starts_with(wanted) {
        scan.at += wanted.len();
        Ok(())
    } else {
        Err(format!(
            "expected `{}` at byte {}",
            String::from_utf8_lossy(wanted),
            scan.at
        ))
    }
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; scan_value reports excessive nesting, missing input and invalid tags through Result; hostile JSON must not be converted into assertion panics."
)]
fn scan_value(scan: &mut Scan) -> Result<crate::value::Json, String> {
    if scan.depth > 64 {
        return Err("nesting deeper than 64".to_string());
    }
    let lead = match scan.text.get(scan.at) {
        Some(byte) => *byte,
        None => return Err("unexpected end of input".to_string()),
    };
    match lead {
        b'{' => {
            scan.depth += 1;
            let object = scan_object(scan);
            scan.depth -= 1;
            object
        }
        b'[' => {
            scan.depth += 1;
            let array = scan_array(scan);
            scan.depth -= 1;
            array
        }
        b'"' => Ok(crate::value::Json::Str(scan_string(scan)?)),
        b't' => {
            expect(scan, b"true")?;
            Ok(crate::value::Json::Bool(true))
        }
        b'f' => {
            expect(scan, b"false")?;
            Ok(crate::value::Json::Bool(false))
        }
        b'n' => {
            expect(scan, b"null")?;
            Ok(crate::value::Json::Null)
        }
        b'-' | b'0'..=b'9' => numbers::parse(scan),
        _ => Err(format!("unexpected byte {lead} at byte {}", scan.at)),
    }
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; scan_object rejects malformed members, duplicate keys, delimiters and reservation failures through Result while keeping its source-derived iteration bound."
)]
fn scan_object(scan: &mut Scan) -> Result<crate::value::Json, String> {
    expect(scan, b"{")?;
    let mut entries: Vec<(String, crate::value::Json)> = Vec::new();
    skip_space(scan);
    if scan.text.get(scan.at) == Some(&b'}') {
        scan.at += 1;
        return Ok(crate::value::Json::Obj(entries));
    }
    // Every entry consumes source bytes; this bound cannot reject valid JSON.
    #[expect(
        tigerstyle::raw_arithmetic_overflow,
        reason = "Owner: noble-maintainers; expect and skip_space advance only across present source bytes, maintaining scan.at <= scan.text.len(); reassess if cursor advancement changes."
    )]
    let remaining = scan.text.len() - scan.at;
    for _ in 0..remaining {
        skip_space(scan);
        let key = scan_string(scan)?;
        skip_space(scan);
        expect(scan, b":")?;
        skip_space(scan);
        let value = scan_value(scan)?;
        if entries.iter().any(|(known, _)| *known == key) {
            return Err(format!("duplicate key `{key}`"));
        }
        #[expect(
            clippy::needless_late_init,
            reason = "Owner: noble-maintainers; pinned Octet recognizes fallible reservation assignments but skips let initializers and ? expansions; reassess when its finder supports both."
        )]
        let reservation;
        reservation = entries.try_reserve(1);
        reservation.map_err(|problem| {
            format!("cannot reserve object entry at byte {}: {problem}", scan.at)
        })?;
        entries.push((key, value));
        skip_space(scan);
        match scan.text.get(scan.at) {
            Some(b',') => scan.at += 1,
            Some(b'}') => {
                scan.at += 1;
                return Ok(crate::value::Json::Obj(entries));
            }
            Some(_) | None => return Err(format!("expected `,` or `}}` at byte {}", scan.at)),
        }
    }
    Err(format!("unterminated object at byte {}", scan.at))
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; scan_array returns malformed-item, delimiter and reservation errors rather than panicking, and bounds iterations by remaining source bytes."
)]
fn scan_array(scan: &mut Scan) -> Result<crate::value::Json, String> {
    expect(scan, b"[")?;
    let mut items: Vec<crate::value::Json> = Vec::new();
    skip_space(scan);
    if scan.text.get(scan.at) == Some(&b']') {
        scan.at += 1;
        return Ok(crate::value::Json::Arr(items));
    }
    #[expect(
        tigerstyle::raw_arithmetic_overflow,
        reason = "Owner: noble-maintainers; expect and skip_space consume only present source bytes, so the cursor never exceeds the source length; reassess if cursor advancement changes."
    )]
    let remaining = scan.text.len() - scan.at;
    for _ in 0..remaining {
        skip_space(scan);
        #[expect(
            clippy::needless_late_init,
            reason = "Owner: noble-maintainers; pinned Octet recognizes fallible reservation assignments but skips let initializers and ? expansions; reassess when its finder supports both."
        )]
        let reservation;
        reservation = items.try_reserve(1);
        reservation.map_err(|problem| {
            format!("cannot reserve array item at byte {}: {problem}", scan.at)
        })?;
        items.push(scan_value(scan)?);
        skip_space(scan);
        match scan.text.get(scan.at) {
            Some(b',') => scan.at += 1,
            Some(b']') => {
                scan.at += 1;
                return Ok(crate::value::Json::Arr(items));
            }
            Some(_) | None => return Err(format!("expected `,` or `]` at byte {}", scan.at)),
        }
    }
    Err(format!("unterminated array at byte {}", scan.at))
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; scan_string reports missing terminators, invalid escapes and malformed UTF-8 through Result; assertions on those input conditions would violate its parser contract."
)]
#[expect(
    tigerstyle::raw_arithmetic_overflow,
    reason = "Owner: noble-maintainers; the preceding-byte offset follows a successful lookup and increment, and utf8_length returns only 2..=4 present bytes; both subtractions remove one from a positive value."
)]
fn scan_string(scan: &mut Scan) -> Result<String, String> {
    expect(scan, b"\"")?;
    let mut out = String::new();
    loop {
        let byte = match scan.text.get(scan.at) {
            Some(byte) => *byte,
            None => return Err("unterminated string".to_string()),
        };
        scan.at += 1;
        match byte {
            b'"' => return Ok(out),
            b'\\' => {
                let escape = match scan.text.get(scan.at) {
                    Some(escape) => *escape,
                    None => return Err("unterminated escape".to_string()),
                };
                scan.at += 1;
                match escape {
                    b'"' => out.push('"'),
                    b'\\' => out.push('\\'),
                    b'/' => out.push('/'),
                    b'b' => out.push('\u{8}'),
                    b'f' => out.push('\u{c}'),
                    b'n' => out.push('\n'),
                    b'r' => out.push('\r'),
                    b't' => out.push('\t'),
                    b'u' => {
                        let code = scan_hex4(scan)?;
                        match char::from_u32(u32::from(code)) {
                            Some(character) => out.push(character),
                            None => return Err(format!("invalid unicode escape {code:04x}")),
                        }
                    }
                    _ => return Err(format!("unsupported escape `{}`", char::from(escape))),
                }
            }
            0x20..=0x7f => out.push(char::from(byte)),
            _ => {
                // Re-decode one multi-byte UTF-8 sequence from the source.
                let rest = &scan.text[scan.at - 1..];
                let length_bytes = utf8_length(rest)?;
                let chunk = &rest[..length_bytes];
                match std::str::from_utf8(chunk) {
                    Ok(text) => {
                        out.push_str(text);
                        scan.at += length_bytes - 1;
                    }
                    Err(_) => return Err("invalid UTF-8 in string".to_string()),
                }
            }
        }
    }
}

const fn utf8_length(rest: &[u8]) -> Result<usize, &'static str> {
    match rest.first() {
        Some(0xc0..=0xdf) if rest.len() >= 2 => Ok(2),
        Some(0xe0..=0xef) if rest.len() >= 3 => Ok(3),
        Some(0xf0..=0xf7) if rest.len() >= 4 => Ok(4),
        Some(_) | None => Err("invalid UTF-8 lead byte"),
    }
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; scan_hex4 returns truncated-input and invalid-digit errors; valid four-digit escape assembly requires no assertion padding."
)]
#[expect(
    tigerstyle::raw_arithmetic_overflow,
    reason = "Owner: noble-maintainers; digit range arms produce nibbles 0..=15, and exactly four steps accumulate at most 0xffff in u16; reassess if the radix or digit count changes."
)]
fn scan_hex4(scan: &mut Scan) -> Result<u16, String> {
    let mut code: u16 = 0;
    let mut step = 0;
    while step < 4 {
        let digit = match scan.text.get(scan.at) {
            Some(byte) => *byte,
            None => return Err("truncated unicode escape".to_string()),
        };
        let value = match digit {
            b'0'..=b'9' => u16::from(digit - b'0'),
            b'a'..=b'f' => u16::from(digit - b'a' + 10),
            b'A'..=b'F' => u16::from(digit - b'A' + 10),
            _ => return Err("invalid hex digit in unicode escape".to_string()),
        };
        code = code * 16 + value;
        scan.at += 1;
        step += 1;
    }
    Ok(code)
}
