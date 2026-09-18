//! Minimal JSON parser for the executed documentation examples (DX-DOC-01).
//!
//! Just enough JSON for the example schema; nesting is bounded and anything
//! else is an error, never a panic. The value tree lives in `value`.

#[path = "value.rs"]
mod value;

pub use value::Json;

/// One parse in progress over the remaining text.
struct Scan<'a> {
    text: &'a [u8],
    at: usize,
    depth: u32,
}

/// Parse one complete JSON document.
pub fn parse(source: &str) -> Result<Json, String> {
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
    if scan.text.len() >= scan.at + wanted.len()
        && &scan.text[scan.at..scan.at + wanted.len()] == wanted
    {
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

fn scan_value(scan: &mut Scan) -> Result<Json, String> {
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
        b'"' => Ok(Json::Str(scan_string(scan)?)),
        b't' => {
            expect(scan, b"true")?;
            Ok(Json::Bool(true))
        }
        b'f' => {
            expect(scan, b"false")?;
            Ok(Json::Bool(false))
        }
        b'n' => {
            expect(scan, b"null")?;
            Ok(Json::Null)
        }
        b'-' | b'0'..=b'9' => scan_number(scan),
        _ => Err(format!("unexpected byte {lead} at byte {}", scan.at)),
    }
}

fn scan_object(scan: &mut Scan) -> Result<Json, String> {
    expect(scan, b"{")?;
    let mut entries: Vec<(String, Json)> = Vec::new();
    skip_space(scan);
    if scan.text.get(scan.at) == Some(&b'}') {
        scan.at += 1;
        return Ok(Json::Obj(entries));
    }
    loop {
        skip_space(scan);
        let key = scan_string(scan)?;
        skip_space(scan);
        expect(scan, b":")?;
        skip_space(scan);
        let value = scan_value(scan)?;
        if entries.iter().any(|(known, _)| *known == key) {
            return Err(format!("duplicate key `{key}`"));
        }
        entries.push((key, value));
        skip_space(scan);
        match scan.text.get(scan.at) {
            Some(b',') => scan.at += 1,
            Some(b'}') => {
                scan.at += 1;
                return Ok(Json::Obj(entries));
            }
            _ => return Err(format!("expected `,` or `}}` at byte {}", scan.at)),
        }
    }
}

fn scan_array(scan: &mut Scan) -> Result<Json, String> {
    expect(scan, b"[")?;
    let mut items: Vec<Json> = Vec::new();
    skip_space(scan);
    if scan.text.get(scan.at) == Some(&b']') {
        scan.at += 1;
        return Ok(Json::Arr(items));
    }
    loop {
        skip_space(scan);
        items.push(scan_value(scan)?);
        skip_space(scan);
        match scan.text.get(scan.at) {
            Some(b',') => scan.at += 1,
            Some(b']') => {
                scan.at += 1;
                return Ok(Json::Arr(items));
            }
            _ => return Err(format!("expected `,` or `]` at byte {}", scan.at)),
        }
    }
}

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
                    _ => return Err(format!("unsupported escape `{}`", escape as char)),
                }
            }
            0x20..=0x7f => out.push(byte as char),
            _ => {
                // Re-decode one multi-byte UTF-8 sequence from the source.
                let rest = &scan.text[scan.at - 1..];
                let length = utf8_length(rest)?;
                let chunk = &rest[..length];
                match std::str::from_utf8(chunk) {
                    Ok(text) => {
                        out.push_str(text);
                        scan.at += length - 1;
                    }
                    Err(_) => return Err("invalid UTF-8 in string".to_string()),
                }
            }
        }
    }
}

fn utf8_length(rest: &[u8]) -> Result<usize, String> {
    match rest.first() {
        Some(0xc0..=0xdf) if rest.len() >= 2 => Ok(2),
        Some(0xe0..=0xef) if rest.len() >= 3 => Ok(3),
        Some(0xf0..=0xf7) if rest.len() >= 4 => Ok(4),
        _ => Err("invalid UTF-8 lead byte".to_string()),
    }
}

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

fn scan_number(scan: &mut Scan) -> Result<Json, String> {
    let start = scan.at;
    if scan.text.get(scan.at) == Some(&b'-') {
        scan.at += 1;
    }
    let mut is_float = false;
    while let Some(byte) = scan.text.get(scan.at) {
        match byte {
            b'0'..=b'9' => scan.at += 1,
            b'.' | b'e' | b'E' | b'+' | b'-' => {
                is_float = true;
                scan.at += 1;
            }
            _ => break,
        }
    }
    let text = std::str::from_utf8(&scan.text[start..scan.at]).map_err(|_| "bad number")?;
    if is_float {
        text.parse::<f64>()
            .map(Json::Float)
            .map_err(|_| format!("invalid float `{text}`"))
    } else {
        text.parse::<i64>()
            .map(Json::Int)
            .map_err(|_| format!("invalid integer `{text}`"))
    }
}
