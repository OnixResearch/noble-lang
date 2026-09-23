//! Minimal bounded JSON reader for engine-worker replies.
//!
//! Runtime reports arrive as JSON documents produced by the selected engine.
//! The contract driver must read individual observations (stack entries,
//! handles, inspection events) without trusting the document's shape, so every
//! read is bounded and every mismatch is a diagnostic rather than a panic.
//! This reader accepts ordinary JSON only: no comments, no trailing data.

mod strings;

/// Largest reply document this reader accepts; the worker protocol vector owns
/// the framing limit and this is the second, independent bound.
const DOCUMENT_LIMIT: usize = 4_194_304;
/// Deepest accepted nesting; deeper documents fail closed.
const DEPTH_LIMIT: usize = 32;
/// One bounded parse step's outcome.
pub(super) type Parsed = Result<Value, std::string::String>;

/// One parsed JSON value.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Value {
    Null,
    Bool(bool),
    Number(i64),
    Text(std::string::String),
    Array(std::vec::Vec<Value>),
    Object(std::collections::BTreeMap<std::string::String, Value>),
}

impl Value {
    pub(super) fn text(&self) -> Option<&str> {
        match self {
            Value::Text(value) => Some(value.as_str()),
            _ => None,
        }
    }

    pub(super) fn number(&self) -> Option<i64> {
        match self {
            Value::Number(value) => Some(*value),
            _ => None,
        }
    }

    /// An unsigned handle or index; negative numbers are refused.
    pub(super) fn index(&self) -> Option<u64> {
        match self.number() {
            Some(value) if value >= 0 => u64::try_from(value).ok(),
            Some(_) | None => None,
        }
    }

    pub(super) fn member(&self, name: &str) -> Option<&Value> {
        match self {
            Value::Object(entries) => entries.get(name),
            _ => None,
        }
    }

    pub(super) fn items(&self) -> Option<&[Value]> {
        match self {
            Value::Array(values) => Some(values.as_slice()),
            _ => None,
        }
    }
}

struct Cursor<'a> {
    bytes: &'a [u8],
    at: usize,
}

/// Parse one complete bounded JSON document.
pub(super) fn parse(document: &str) -> Parsed {
    if document.len() > DOCUMENT_LIMIT {
        return Err("report document exceeds the reader bound".into());
    }
    let mut cursor = Cursor {
        bytes: document.as_bytes(),
        at: 0,
    };
    cursor.skip_space();
    let value = attempt!(cursor.value(0));
    cursor.skip_space();
    if cursor.at != cursor.bytes.len() {
        return Err("trailing bytes after the report document".into());
    }
    Ok(value)
}

impl Cursor<'_> {
    fn skip_space(&mut self) {
        while self.at < self.bytes.len()
            && matches!(self.bytes[self.at], b' ' | b'\n' | b'\t' | b'\r')
        {
            self.at += 1;
        }
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.at).copied()
    }

    #[expect(
        tigerstyle::missing_const_fn,
        reason = "Owner: noble-maintainers; unexpected worker bytes produce an owned String diagnostic. String construction is not const on the pinned compiler."
    )]
    fn expect(&mut self, byte: u8) -> Result<(), std::string::String> {
        if self.peek() == Some(byte) {
            self.at += 1;
            Ok(())
        } else {
            Err("report document has an unexpected byte".into())
        }
    }

    #[expect(
        tigerstyle::missing_const_fn,
        reason = "Owner: noble-maintainers; literal validation uses runtime slice comparison and constructs owned String diagnostics, neither of which is const on the pinned compiler."
    )]
    fn literal(&mut self, word: &[u8]) -> Result<(), std::string::String> {
        let end_byte = attempt!(self
            .at
            .checked_add(word.len())
            .ok_or("report document has an invalid literal"));
        if self.bytes.get(self.at..end_byte) == Some(word) {
            self.at = end_byte;
            Ok(())
        } else {
            Err("report document has an invalid literal".into())
        }
    }

    #[expect(
        tigerstyle::missing_const_fn,
        reason = "Owner: noble-maintainers; value dispatches runtime parsing into growable String, Vec and BTreeMap values and returns owned diagnostics. Those APIs are not const on the pinned compiler."
    )]
    fn value(&mut self, depth: usize) -> Parsed {
        if depth >= DEPTH_LIMIT {
            return Err("report document nesting exceeds the reader bound".into());
        }
        match self.peek() {
            Some(b'n') => {
                attempt!(self.literal(b"null"));
                Ok(Value::Null)
            }
            Some(b't') => {
                attempt!(self.literal(b"true"));
                Ok(Value::Bool(true))
            }
            Some(b'f') => {
                attempt!(self.literal(b"false"));
                Ok(Value::Bool(false))
            }
            Some(b'"') => Ok(Value::Text(attempt!(self.string()))),
            Some(b'[') => self.array(depth),
            Some(b'{') => self.object(depth),
            Some(byte) if byte == b'-' || byte.is_ascii_digit() => self.number(),
            Some(_) | None => Err("report document has an invalid value".into()),
        }
    }

    fn number(&mut self) -> Parsed {
        let start = self.at;
        if self.peek() == Some(b'-') {
            self.at += 1;
        }
        let digits_start_byte = self.at;
        while self.peek().is_some_and(|byte| byte.is_ascii_digit()) {
            self.at += 1;
        }
        if self.at == digits_start_byte {
            return Err("report document has an invalid number".into());
        }
        let text = attempt!(std::str::from_utf8(&self.bytes[start..self.at])
            .map_err(|_| std::string::String::from("report document has invalid number bytes")));
        text.parse::<i64>()
            .map(Value::Number)
            .map_err(|_| "report number is outside signed 64-bit".into())
    }

    fn array(&mut self, depth: usize) -> Parsed {
        attempt!(self.expect(b'['));
        let mut items = std::vec::Vec::new();
        self.skip_space();
        if self.peek() == Some(b']') {
            self.at += 1;
            return Ok(Value::Array(items));
        }
        let child_depth = attempt!(depth
            .checked_add(1)
            .ok_or("report document nesting exceeds the reader bound"));
        // Each value consumes input bytes; grow only the values actually read.
        while self.at < self.bytes.len() && items.len() < self.bytes.len() {
            items.push(attempt!(self.value(child_depth)));
            self.skip_space();
            match self.peek() {
                Some(b',') => {
                    self.at += 1;
                    self.skip_space();
                }
                Some(b']') => {
                    self.at += 1;
                    return Ok(Value::Array(items));
                }
                Some(_) | None => return Err("report array has an unexpected byte".into()),
            }
        }
        Err("report array is unterminated".into())
    }

    fn object(&mut self, depth: usize) -> Parsed {
        attempt!(self.expect(b'{'));
        let mut entries = std::collections::BTreeMap::new();
        self.skip_space();
        if self.peek() == Some(b'}') {
            self.at += 1;
            return Ok(Value::Object(entries));
        }
        let child_depth = attempt!(depth
            .checked_add(1)
            .ok_or("report document nesting exceeds the reader bound"));
        // Keys consume input too; no whole-document reservation per object.
        while self.at < self.bytes.len() && entries.len() < self.bytes.len() {
            self.skip_space();
            let key = attempt!(self.string());
            self.skip_space();
            attempt!(self.expect(b':'));
            self.skip_space();
            let value = attempt!(self.value(child_depth));
            entries.insert(key, value);
            self.skip_space();
            match self.peek() {
                Some(b',') => {
                    self.at += 1;
                }
                Some(b'}') => {
                    self.at += 1;
                    return Ok(Value::Object(entries));
                }
                Some(_) | None => return Err("report object has an unexpected byte".into()),
            }
        }
        Err("report object is unterminated".into())
    }
}
