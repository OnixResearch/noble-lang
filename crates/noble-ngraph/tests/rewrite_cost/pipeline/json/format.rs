//! Primitive JSON values used by the pipeline receipt.

use noble_ngraph::sem::Value;

use super::super::attempt::ScratchObservation;
use super::super::stats::Summary;

pub(super) fn quoted(value: &str) -> String {
    let mut result = String::with_capacity(value.len().saturating_add(2));
    result.push('"');
    for character in value.chars() {
        match character {
            '"' => result.push_str("\\\""),
            '\\' => result.push_str("\\\\"),
            '\n' => result.push_str("\\n"),
            '\r' => result.push_str("\\r"),
            '\t' => result.push_str("\\t"),
            character if character.is_control() => {
                result.push_str(&std::format!("\\u{:04x}", character as u32));
            }
            character => result.push(character),
        }
    }
    result.push('"');
    result
}

pub(super) fn optional_string(value: Option<&String>) -> String {
    value.map_or_else(|| "null".to_owned(), |value| quoted(value))
}

pub(super) fn optional_u64(value: Option<u64>) -> String {
    value.map_or_else(|| "null".to_owned(), |value| value.to_string())
}

pub(super) fn optional_usize(value: Option<usize>) -> String {
    value.map_or_else(|| "null".to_owned(), |value| value.to_string())
}

pub(super) fn summary(value: Option<Summary>) -> String {
    value.map_or_else(
        || "null".to_owned(),
        |value| {
            std::format!(
                "{{\"median_us\":{:.9},\"iqr_us\":{:.9},\"observations\":{}}}",
                value.median_us,
                value.iqr_us,
                value.observations
            )
        },
    )
}

pub(super) fn values(values: Option<&[Value]>) -> String {
    let Some(values) = values else {
        return "null".to_owned();
    };
    let rendered = values
        .iter()
        .map(|value| match value {
            Value::I64(value) => {
                std::format!("{{\"type\":\"i64\",\"value\":\"{}\"}}", value)
            }
            Value::Bool(value) => {
                std::format!("{{\"type\":\"bool\",\"value\":{}}}", value)
            }
            Value::F32(bits) => std::format!("{{\"type\":\"f32\",\"bits\":\"0x{:08x}\"}}", bits),
        })
        .collect::<Vec<_>>()
        .join(",");
    std::format!("[{rendered}]")
}

pub(super) fn scratch(value: Option<ScratchObservation>) -> String {
    value.map_or_else(
        || "null".to_owned(),
        |value| {
            std::format!(
                concat!(
                    "{{\"scope\":{},\"acquired\":{},\"released\":{},",
                    "\"live_bytes\":{},\"peak_bytes\":{},",
                    "\"memory_accounting\":\"library scratch leases; not allocator/RSS\"}}"
                ),
                quoted(value.scope),
                value.acquired,
                value.released,
                value.live_bytes,
                value.peak_bytes,
            )
        },
    )
}
