//! Value tokens of vector and data files (design D5) and the report
//! rendering of observed values (design D7).

use alloc::{borrow::ToOwned, format, string::String, string::ToString, vec::Vec};

use crate::diag::{Code, Diagnostic, Site};
use crate::ir::ScalarType;
use crate::sem::Value;

fn format_error(line: usize, why: impl Into<String>) -> Diagnostic {
    Diagnostic::new(Code::Format, Site::Line(line), why)
}

/// Why an `f32` token was refused; each file format maps `Malformed` to its
/// own syntax code.
pub(crate) enum F32Refusal {
    /// Neither `0x` bits nor a decimal.
    Malformed,
    /// A `nan`/`inf` word (`E-LITERAL-NONFINITE`).
    Nonfinite,
    /// A decimal that no binary32 value equals (`E-LITERAL-INEXACT`).
    Inexact,
}

/// Reads an `f32` token of any `ngraph` file: `0x` plus one to eight hex
/// digits (any encoding, including NaN, infinities and subnormals), or an
/// exactly representable decimal ([`crate::sem::is_decimal`]). Linear in the
/// token's length.
pub(crate) fn f32_token(tok: &str) -> Result<u32, F32Refusal> {
    if tok.starts_with("0x") {
        return super::hex_bits(tok, 8)
            .and_then(|b| u32::try_from(b).ok())
            .ok_or(F32Refusal::Malformed);
    }
    let word = |w: &[u8]| w.eq_ignore_ascii_case(b"nan") || w.eq_ignore_ascii_case(b"inf");
    if tok.as_bytes().windows(3).any(word) {
        return Err(F32Refusal::Nonfinite);
    }
    if !crate::sem::is_decimal(tok) {
        return Err(F32Refusal::Malformed);
    }
    crate::sem::exact_f32_decimal(tok).ok_or(F32Refusal::Inexact)
}

/// Parses one value token of type `ty` on file line `line`.
///
/// - `f32`: `0x` plus one to eight hex digits (any encoding, including NaN,
///   infinities and subnormals) or an exactly representable finite decimal;
/// - `i64`: a signed decimal within range;
/// - `bool`: `true` or `false`.
///
/// # Errors
/// `E-LITERAL-INEXACT` for an inexact decimal, `E-LITERAL-NONFINITE` for a
/// `nan`/`inf` word, `E-FORMAT` otherwise.
pub fn parse(ty: ScalarType, tok: &str, line: usize) -> Result<Value, Diagnostic> {
    match ty {
        ScalarType::F32 => f32_token(tok).map(Value::F32).map_err(|r| match r {
            F32Refusal::Malformed => format_error(line, format!("bad f32 value {tok:?}")),
            F32Refusal::Nonfinite => Diagnostic::new(
                Code::LiteralNonfinite,
                Site::Line(line),
                format!("non-finite f32 value {tok}; give NaN and infinities as 0x bits"),
            ),
            F32Refusal::Inexact => Diagnostic::new(
                Code::LiteralInexact,
                Site::Line(line),
                format!("{tok} is not exactly representable in binary32"),
            ),
        }),
        ScalarType::I64 => super::decimal_i64(tok)
            .map(Value::I64)
            .ok_or_else(|| format_error(line, format!("bad i64 value {tok:?}"))),
        ScalarType::Bool => match tok {
            "true" => Ok(Value::Bool(true)),
            "false" => Ok(Value::Bool(false)),
            _ => Err(format_error(line, format!("bad bool value {tok:?}"))),
        },
    }
}

/// The canonical file token of a value: `f32` as its exact `0x%08x`
/// encoding, `i64` in decimal, `bool` as `true`/`false`.
#[must_use]
pub fn token(v: Value) -> String {
    match v {
        Value::F32(b) => format!("0x{b:08x}"),
        Value::I64(x) => x.to_string(),
        Value::Bool(x) => x.to_string(),
    }
}

/// The report rendering of an observed value: `f32:0x%08x`, with every NaN
/// encoding rendered as the single class `f32:nan` (D3 `≡obs`); `i64:N`;
/// `bool:true|false`.
#[must_use]
pub fn render(v: Value) -> String {
    match v {
        Value::F32(b) if f32::from_bits(b).is_nan() => "f32:nan".to_owned(),
        Value::F32(b) => format!("f32:0x{b:08x}"),
        Value::I64(x) => format!("i64:{x}"),
        Value::Bool(x) => format!("bool:{x}"),
    }
}

/// Parses the type tokens of a `types` line.
pub(crate) fn types(tokens: &[&str], line: usize) -> Result<Vec<ScalarType>, Diagnostic> {
    tokens
        .iter()
        .map(|t| {
            super::graph::scalar_type(t)
                .ok_or_else(|| format_error(line, format!("bad type {t:?}")))
        })
        .collect()
}

/// Parses one row of value tokens against `types`.
pub(crate) fn row(
    types: &[ScalarType],
    tokens: &[&str],
    line: usize,
) -> Result<Vec<Value>, Diagnostic> {
    if tokens.len() != types.len() {
        return Err(format_error(
            line,
            format!("{} values for {} types", tokens.len(), types.len()),
        ));
    }
    types
        .iter()
        .zip(tokens)
        .map(|(ty, tok)| parse(*ty, tok, line))
        .collect()
}

pub(crate) fn type_names(types: &[ScalarType]) -> String {
    types.iter().map(|t| t.name()).collect::<Vec<_>>().join(" ")
}

pub(crate) fn row_text(values: &[Value]) -> String {
    values
        .iter()
        .map(|v| token(*v))
        .collect::<Vec<_>>()
        .join(" ")
}

pub(crate) fn format_diag(line: usize, why: impl Into<String>) -> Diagnostic {
    format_error(line, why)
}
