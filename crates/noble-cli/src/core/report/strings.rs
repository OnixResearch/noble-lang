//! String decoding shares the parent cursor's document-byte bound.

impl crate::core::report::Cursor<'_> {
    pub(super) fn string(&mut self) -> Result<std::string::String, std::string::String> {
        attempt!(self.expect(b'"'));
        let mut out = std::string::String::new();
        while let Some(byte) = self.peek() {
            match byte {
                b'"' => {
                    self.at += 1;
                    return Ok(out);
                }
                b'\\' => {
                    self.at += 1;
                    let escape = attempt!(self
                        .peek()
                        .ok_or_else(|| std::string::String::from("truncated string escape")));
                    self.at += 1;
                    match escape {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'b' => out.push('\u{8}'),
                        b'f' => out.push('\u{c}'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => out.push(attempt!(self.unicode_escape())),
                        _ => return Err("report document has an invalid string escape".into()),
                    }
                }
                _ => {
                    let rest = attempt!(std::str::from_utf8(&self.bytes[self.at..])
                        .map_err(|_| std::string::String::from("invalid UTF-8 in report string")));
                    let character = attempt!(rest
                        .chars()
                        .next()
                        .ok_or_else(|| std::string::String::from("truncated report string")));
                    out.push(character);
                    self.at += character.len_utf8();
                }
            }
        }
        Err("unterminated report string".into())
    }

    fn unicode_escape(&mut self) -> Result<char, std::string::String> {
        let end_byte = attempt!(self.at.checked_add(4).ok_or("truncated unicode escape"));
        let bytes = attempt!(self
            .bytes
            .get(self.at..end_byte)
            .ok_or("truncated unicode escape"));
        let text = attempt!(std::str::from_utf8(bytes)
            .map_err(|_| std::string::String::from("invalid unicode escape")));
        self.at = end_byte;
        let code =
            attempt!(u32::from_str_radix(text, 16).map_err(|_| "invalid unicode escape digits"));
        char::from_u32(code).ok_or_else(|| "unicode escape is not a scalar value".into())
    }
}
