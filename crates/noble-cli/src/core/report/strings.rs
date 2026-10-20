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
                    out.push(attempt!(self.string_escape()));
                }
                0..=0x1f => return Err("report string contains a raw control byte".into()),
                _ => {
                    out.push_str(attempt!(self.raw_string_run()));
                }
            }
        }
        Err("unterminated report string".into())
    }

    fn string_escape(&mut self) -> Result<char, std::string::String> {
        let escape = attempt!(self
            .peek()
            .ok_or_else(|| std::string::String::from("truncated string escape")));
        self.at += 1;
        match escape {
            b'"' => Ok('"'),
            b'\\' => Ok('\\'),
            b'/' => Ok('/'),
            b'b' => Ok('\u{8}'),
            b'f' => Ok('\u{c}'),
            b'n' => Ok('\n'),
            b'r' => Ok('\r'),
            b't' => Ok('\t'),
            b'u' => self.unicode_escape(),
            _ => Err("report document has an invalid string escape".into()),
        }
    }

    fn raw_string_run(&mut self) -> Result<&str, std::string::String> {
        let start = self.at;
        while let Some(byte) = self.peek() {
            if matches!(byte, b'"' | b'\\' | 0..=0x1f) {
                break;
            }
            self.at += 1;
        }
        std::str::from_utf8(&self.bytes[start..self.at])
            .map_err(|_| std::string::String::from("invalid UTF-8 in report string"))
    }

    fn unicode_escape(&mut self) -> Result<char, std::string::String> {
        let code = attempt!(self.unicode_code_unit());
        match code {
            0xd800..=0xdbff => self.surrogate_pair(code),
            0xdc00..=0xdfff => Err("unicode escape contains an isolated low surrogate".into()),
            _ => char::from_u32(u32::from(code))
                .ok_or_else(|| "unicode escape is not a scalar value".into()),
        }
    }

    fn surrogate_pair(&mut self, high: u16) -> Result<char, std::string::String> {
        attempt!(self.expect(b'\\'));
        attempt!(self.expect(b'u'));
        let low = attempt!(self.unicode_code_unit());
        if !(0xdc00..=0xdfff).contains(&low) {
            return Err("unicode escape has an invalid low surrogate".into());
        }
        let scalar = 0x1_0000 + ((u32::from(high) - 0xd800) << 10) + (u32::from(low) - 0xdc00);
        char::from_u32(scalar).ok_or_else(|| "unicode escape is not a scalar value".into())
    }

    fn unicode_code_unit(&mut self) -> Result<u16, std::string::String> {
        let end_byte = attempt!(self.at.checked_add(4).ok_or("truncated unicode escape"));
        let bytes = attempt!(self
            .bytes
            .get(self.at..end_byte)
            .ok_or("truncated unicode escape"));
        let text = attempt!(std::str::from_utf8(bytes)
            .map_err(|_| std::string::String::from("invalid unicode escape")));
        if !text.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err("invalid unicode escape digits".into());
        }
        let code =
            attempt!(u16::from_str_radix(text, 16).map_err(|_| "invalid unicode escape digits"));
        self.at = end_byte;
        Ok(code)
    }
}
