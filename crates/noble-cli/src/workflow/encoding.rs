//! Deterministic JSON encoding for the fixed report schema.

#[derive(Clone)]
pub(super) enum Json {
    Null,
    Bool(bool),
    Number(u64),
    String(std::string::String),
    Array(std::vec::Vec<Json>),
    Object(std::vec::Vec<(&'static str, Json)>),
}

pub(super) fn object<const N: usize>(entries: [(&'static str, Json); N]) -> Json {
    Json::Object(std::vec::Vec::from(entries))
}

pub(super) fn string(value: impl AsRef<str>) -> Json {
    Json::String(value.as_ref().into())
}

pub(super) fn optional_string(value: Option<&str>) -> Json {
    match value {
        Some(value) => string(value),
        None => Json::Null,
    }
}

pub(super) fn strings(values: &[&str]) -> Json {
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
    pub(super) fn encode(&self) -> std::string::String {
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
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; quote_json reserves and appends runtime UTF-8 text, iterates chars and formats control-character escapes. Those APIs are not const on the pinned compiler; reassess if that support changes."
)]
fn quote_json(value: &str, output: &mut std::string::String) {
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

pub(super) fn hex(bytes: &[u8]) -> std::string::String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut result = std::string::String::with_capacity(bytes.len().saturating_mul(2));
    bytes.iter().for_each(|byte| {
        result.push(char::from(DIGITS[usize::from(byte >> 4)]));
        result.push(char::from(DIGITS[usize::from(byte & 15)]));
    });
    result
}
