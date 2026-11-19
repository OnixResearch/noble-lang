//! Deterministic JSON encoding for the fixed report schema.

#[derive(Clone)]
pub(crate) enum Json {
    Null,
    Bool(bool),
    Number(u64),
    /// A signed integer, emitted with its sign. A signed value is never cast into `Number`: the
    /// cast would report a negative value as a different, positive one.
    Signed(i64),
    String(std::string::String),
    Array(std::vec::Vec<Json>),
    Object(std::vec::Vec<(&'static str, Json)>),
}

pub(crate) fn object<const N: usize>(entries: [(&'static str, Json); N]) -> Json {
    Json::Object(std::vec::Vec::from(entries))
}

pub(crate) fn string(value: impl AsRef<str>) -> Json {
    Json::String(value.as_ref().into())
}

pub(crate) fn optional_string(value: Option<&str>) -> Json {
    match value {
        Some(value) => string(value),
        None => Json::Null,
    }
}

pub(crate) fn strings(values: &[&str]) -> Json {
    Json::Array(values.iter().map(string).collect())
}

// Traversal states are closed: a new frame must define its JSON emission step.
#[octet::sealed_enum]
enum Frame<'a> {
    Value(&'a Json),
    Array(&'a [Json], bool),
    Object(&'a [(&'static str, Json)], bool),
}

impl Json {
    pub(crate) fn encode(&self) -> std::string::String {
        let mut output = std::string::String::with_capacity(4096);
        // The report schema has fixed nesting; collection lengths do not add
        // stack frames. The traversal retains only one tail per open container.
        let mut pending = std::vec::Vec::with_capacity(16);
        pending.push(Frame::Value(self));
        while let Some(frame) = pending.pop() {
            match frame {
                Frame::Value(value) => append_value(value, &mut output, &mut pending),
                Frame::Array(values, is_first) => match values.split_first() {
                    None => output.push(']'),
                    Some((value, rest)) => {
                        if !is_first {
                            output.push(',');
                        }
                        pending.push(Frame::Array(rest, false));
                        pending.push(Frame::Value(value));
                    }
                },
                Frame::Object(values, is_first) => match values.split_first() {
                    None => output.push('}'),
                    Some(((key, value), rest)) => {
                        if !is_first {
                            output.push(',');
                        }
                        quote_json(key, &mut output);
                        output.push(':');
                        pending.push(Frame::Object(rest, false));
                        pending.push(Frame::Value(value));
                    }
                },
            }
        }
        output
    }

    /// The exact byte length of [`Json::encode`], computed without building
    /// the text: the same bounded traversal, counting instead of appending.
    pub(crate) fn encoded_len(&self) -> usize {
        let mut length = 0usize;
        let mut pending = std::vec::Vec::with_capacity(16);
        pending.push(Frame::Value(self));
        while let Some(frame) = pending.pop() {
            let step = match frame {
                Frame::Value(value) => match value {
                    Self::Null => 4,
                    Self::Bool(value) => 5usize.saturating_sub(usize::from(*value)),
                    Self::Number(value) => decimal_len(*value),
                    Self::Signed(value) => {
                        decimal_len(value.unsigned_abs()).saturating_add(usize::from(*value < 0))
                    }
                    Self::String(value) => quoted_len(value),
                    Self::Array(values) => {
                        pending.push(Frame::Array(values, true));
                        1
                    }
                    Self::Object(values) => {
                        pending.push(Frame::Object(values, true));
                        1
                    }
                },
                Frame::Array(values, is_first) => match values.split_first() {
                    None => 1,
                    Some((value, rest)) => {
                        pending.push(Frame::Array(rest, false));
                        pending.push(Frame::Value(value));
                        usize::from(!is_first)
                    }
                },
                Frame::Object(values, is_first) => match values.split_first() {
                    None => 1,
                    Some(((key, value), rest)) => {
                        pending.push(Frame::Object(rest, false));
                        pending.push(Frame::Value(value));
                        usize::from(!is_first)
                            .saturating_add(quoted_len(key))
                            .saturating_add(1)
                    }
                },
            };
            length = length.saturating_add(step);
        }
        length
    }
}

#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-maintainers. Appending JSON bytes and traversal frames requires the caller's growable String and Vec, not fixed str and frame slices."
)]
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; append_value appends to growable String and Vec buffers and formats runtime numbers. These operations are not const on the pinned compiler; reassess when their const APIs are available."
)]
fn append_value<'a>(
    value: &'a Json,
    output: &mut std::string::String,
    pending: &mut std::vec::Vec<Frame<'a>>,
) {
    match value {
        Json::Null => output.push_str("null"),
        Json::Bool(value) => output.push_str(if *value { "true" } else { "false" }),
        Json::Number(value) => output.push_str(&value.to_string()),
        Json::Signed(value) => output.push_str(&value.to_string()),
        Json::String(value) => quote_json(value, output),
        Json::Array(values) => {
            output.push('[');
            pending.push(Frame::Array(values, true));
        }
        Json::Object(values) => {
            output.push('{');
            pending.push(Frame::Object(values, true));
        }
    }
}

#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-maintainers. Appending escaped JSON string syntax requires the caller's growable String, not a fixed str slice."
)]
pub(crate) fn quote_json(value: &str, output: &mut std::string::String) {
    output.reserve(value.len().saturating_add(2));
    output.push('"');
    value.chars().for_each(|character| match character {
        '"' => output.push_str("\\\""),
        '\\' => output.push_str("\\\\"),
        '\n' => output.push_str("\\n"),
        '\r' => output.push_str("\\r"),
        '\t' => output.push_str("\\t"),
        character if character < '\u{20}' => {
            output.push_str(&std::format!("\\u{:04x}", u32::from(character)));
        }
        character => output.push(character),
    });
    output.push('"');
}

/// The byte length of `quote_json(value)`.
pub(crate) fn quoted_len(value: &str) -> usize {
    value.chars().fold(2usize, |length, character| {
        length.saturating_add(match character {
            '"' | '\\' | '\n' | '\r' | '\t' => 2,
            character if character < '\u{20}' => 6,
            character => character.len_utf8(),
        })
    })
}

/// The number of decimal digits of `value`.
pub(crate) const fn decimal_len(value: u64) -> usize {
    let mut rest = value;
    let mut digits = 1usize;
    while rest >= 10 {
        rest /= 10;
        digits = digits.saturating_add(1);
    }
    digits
}

pub(crate) fn hex(bytes: &[u8]) -> std::string::String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut result = std::string::String::with_capacity(bytes.len().saturating_mul(2));
    bytes.iter().for_each(|byte| {
        result.push(char::from(DIGITS[usize::from(byte >> 4)]));
        result.push(char::from(DIGITS[usize::from(byte & 15)]));
    });
    result
}

#[cfg(test)]
mod tests {
    use super::{decimal_len, quoted_len, string, Json};

    #[test]
    fn encoded_len_is_the_length_of_the_encoding() {
        let values = [
            Json::Null,
            Json::Bool(true),
            Json::Bool(false),
            Json::Number(0),
            Json::Number(9),
            Json::Number(10),
            Json::Number(u64::MAX),
            Json::Signed(0),
            Json::Signed(-1),
            Json::Signed(i64::MIN),
            Json::Signed(i64::MAX),
            string(""),
            string("plain"),
            string("quote \" slash \\ newline \n tab \t return \r bell \u{7} é ∑"),
            Json::Array(std::vec::Vec::new()),
            Json::Object(std::vec::Vec::new()),
            Json::Array(std::vec![Json::Number(1), string("i64:-9"), Json::Null]),
            Json::Object(std::vec![
                (
                    "a",
                    Json::Array(std::vec![Json::Object(std::vec![("b", Json::Bool(false))])])
                ),
                ("escaped\tkey", Json::Signed(-42)),
                ("empty", Json::Array(std::vec::Vec::new())),
            ]),
        ];
        for value in &values {
            assert_eq!(
                value.encoded_len(),
                value.encode().len(),
                "{}",
                value.encode()
            );
        }
        assert_eq!(quoted_len("\u{1f}"), "\"\\u001f\"".len());
        assert_eq!(decimal_len(0), 1);
        assert_eq!(decimal_len(99), 2);
        assert_eq!(decimal_len(100), 3);
    }
}
