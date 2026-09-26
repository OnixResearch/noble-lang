use super::*;

/// Length/UTF-8/container-depth rejection precedes any typed field creation.
const fn continues_quote(byte: u8, is_escaped: &mut bool) -> bool {
    if *is_escaped {
        *is_escaped = false;
        return true;
    }
    if byte == b'\\' {
        *is_escaped = true;
        return true;
    }
    byte != b'"'
}

const fn advance_depth(depth: &mut usize, byte: u8) -> Result<(), Error> {
    if matches!(byte, b'{' | b'[') {
        *depth += 1;
        if *depth > 3 {
            return Err(Error::DepthLimit);
        }
    } else if matches!(byte, b'}' | b']') {
        *depth = depth.saturating_sub(1);
    }
    Ok(())
}

struct DepthState {
    depth: usize,
    is_quoted: bool,
    is_escaped: bool,
}

impl DepthState {
    const fn feed(&mut self, byte: u8) -> Result<(), Error> {
        if self.is_quoted {
            self.is_quoted = continues_quote(byte, &mut self.is_escaped);
            return Ok(());
        }
        if byte == b'"' {
            self.is_quoted = true;
            return Ok(());
        }
        advance_depth(&mut self.depth, byte)
    }
}

pub(super) fn preflight(input: &[u8]) -> Result<&str, Error> {
    if input.len() > MAX_DESCRIPTOR_BYTES {
        return Err(Error::ByteLimit);
    }
    let source = attempt!(std::str::from_utf8(input).map_err(|_| Error::Utf8));
    let mut scan = DepthState {
        depth: 0,
        is_quoted: false,
        is_escaped: false,
    };
    let mut index = 0;
    while index < input.len() {
        attempt!(scan.feed(input[index]));
        index += 1;
    }
    Ok(source)
}

impl Decoder<'_> {
    pub(super) fn whitespace(&mut self) {
        while matches!(self.input.get(self.at), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.at += 1;
        }
    }

    pub(super) fn take(&mut self, byte: u8) -> bool {
        self.whitespace();
        if self.input.get(self.at) == Some(&byte) {
            self.at += 1;
            true
        } else {
            false
        }
    }

    pub(super) fn require(&mut self, byte: u8) -> Result<(), Error> {
        if self.take(byte) {
            Ok(())
        } else {
            Err(Error::Descriptor)
        }
    }

    fn put(&mut self, byte: u8, len: &mut usize) -> Result<(), Error> {
        let slot = attempt!(self
            .text
            .as_mut()
            .and_then(|text| text.get_mut(*len))
            .ok_or(Error::Descriptor));
        *slot = byte;
        *len += 1;
        Ok(())
    }

    fn hex_quad(&mut self) -> Result<u16, Error> {
        let mut value = 0u16;
        let mut index = 0;
        while index < 4 {
            let byte = *attempt!(self.input.get(self.at).ok_or(Error::Descriptor));
            self.at += 1;
            let digit = match byte {
                b'0'..=b'9' => u16::from(byte.saturating_sub(b'0')),
                b'a'..=b'f' => u16::from(byte.saturating_sub(b'a')).saturating_add(10),
                b'A'..=b'F' => u16::from(byte.saturating_sub(b'A')).saturating_add(10),
                _ => return Err(Error::Descriptor),
            };
            value = (value << 4) | digit;
            index += 1;
        }
        Ok(value)
    }

    fn unicode(&mut self, len: &mut usize) -> Result<(), Error> {
        let first = attempt!(self.hex_quad());
        let scalar = match first {
            0xD800..=0xDBFF => {
                let end = attempt!(self.at.checked_add(2).ok_or(Error::Descriptor));
                if self.input.get(self.at..end) != Some(&b"\\u"[..]) {
                    return Err(Error::Descriptor);
                }
                self.at = end;
                let second = attempt!(self.hex_quad());
                match second {
                    0xDC00..=0xDFFF => {
                        let high = attempt!(u32::from(first)
                            .checked_sub(0xD800)
                            .ok_or(Error::Descriptor));
                        let low = attempt!(u32::from(second)
                            .checked_sub(0xDC00)
                            .ok_or(Error::Descriptor));
                        let scalar_delta = attempt!(high
                            .checked_shl(10)
                            .and_then(|value| value.checked_add(low))
                            .ok_or(Error::Descriptor));
                        attempt!(0x10000_u32
                            .checked_add(scalar_delta)
                            .ok_or(Error::Descriptor))
                    }
                    _ => return Err(Error::Descriptor),
                }
            }
            0xDC00..=0xDFFF => return Err(Error::Descriptor),
            _ => u32::from(first),
        };
        let character = attempt!(char::from_u32(scalar).ok_or(Error::Descriptor));
        let mut utf8 = [0; 4];
        let encoded = character.encode_utf8(&mut utf8).as_bytes();
        let mut index = 0;
        while index < encoded.len() {
            attempt!(self.put(encoded[index], len));
            index += 1;
        }
        Ok(())
    }

    /// All standard JSON string escapes; raw control bytes and malformed UTF-16
    /// pairs are never normalized into an accepted field or key.
    pub(super) fn string(&mut self) -> Result<&str, Error> {
        attempt!(self.require(b'"'));
        let start = self.at;
        loop {
            let byte = *attempt!(self.input.get(self.at).ok_or(Error::Descriptor));
            match byte {
                b'"' => {
                    let end = self.at;
                    self.at += 1;
                    return self.source.get(start..end).ok_or(Error::Descriptor);
                }
                b'\\' => break,
                0..=31 => return Err(Error::Descriptor),
                _ => self.at += 1,
            }
        }
        let mut len = attempt!(self.at.checked_sub(start).ok_or(Error::Descriptor));
        if self.text.is_none() {
            self.text = Some([0; MAX_DESCRIPTOR_BYTES]);
        }
        let Some(text) = self.text.as_mut() else {
            return Err(Error::Descriptor);
        };
        text[..len].copy_from_slice(&self.input[start..self.at]);
        loop {
            let byte = *attempt!(self.input.get(self.at).ok_or(Error::Descriptor));
            self.at += 1;
            match byte {
                b'"' => {
                    let Some(text) = self.text.as_ref() else {
                        return Err(Error::Descriptor);
                    };
                    return std::str::from_utf8(&text[..len]).map_err(|_| Error::Descriptor);
                }
                b'\\' => {
                    let escaped = *attempt!(self.input.get(self.at).ok_or(Error::Descriptor));
                    self.at += 1;
                    match escaped {
                        b'"' | b'\\' | b'/' => attempt!(self.put(escaped, &mut len)),
                        b'b' => attempt!(self.put(8, &mut len)),
                        b'f' => attempt!(self.put(12, &mut len)),
                        b'n' => attempt!(self.put(b'\n', &mut len)),
                        b'r' => attempt!(self.put(b'\r', &mut len)),
                        b't' => attempt!(self.put(b'\t', &mut len)),
                        b'u' => attempt!(self.unicode(&mut len)),
                        _ => return Err(Error::Descriptor),
                    }
                }
                0..=31 => return Err(Error::Descriptor),
                _ => attempt!(self.put(byte, &mut len)),
            }
        }
    }

    pub(super) fn boolean(&mut self) -> Result<bool, Error> {
        self.whitespace();
        if self
            .input
            .get(self.at..)
            .is_some_and(|rest| rest.starts_with(b"true"))
        {
            self.at = self.at.saturating_add(4);
            Ok(true)
        } else if self
            .input
            .get(self.at..)
            .is_some_and(|rest| rest.starts_with(b"false"))
        {
            self.at = self.at.saturating_add(5);
            Ok(false)
        } else {
            Err(Error::Descriptor)
        }
    }
}
