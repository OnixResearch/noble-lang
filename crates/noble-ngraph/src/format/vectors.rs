//! Evaluation vector files `noble-ngraph-vectors/0` (design D5).
//!
//! ```text
//! noble-ngraph-vectors/0
//! types f32 f32
//! v 0x3fc00000 -0.5
//! v 0x7fc00000 0x80000000
//! ```
//!
//! `types` lists the graph's ordered input slot types (parameters never come
//! from vector files). The caller compares it with the graph (`E-INTERFACE`).

#[allow(unused_imports)]
use crate::prelude::*;

use super::value::{format_diag, row, row_text, type_names, types};
use crate::diag::Diagnostic;
use crate::ir::ScalarType;
use crate::sem::Value;

/// Schema line.
pub const SCHEMA: &str = "noble-ngraph-vectors/0";

/// A parsed vector file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Vectors {
    /// Declared input types.
    pub types: Vec<ScalarType>,
    /// Rows in file order.
    pub rows: Vec<Vec<Value>>,
}

/// Parses a vector file.
///
/// # Errors
/// `E-FORMAT` (schema, syntax, missing or duplicate `types`, row count),
/// `E-LITERAL-INEXACT` or `E-LITERAL-NONFINITE`.
pub fn parse(text: &str) -> Result<Vectors, Diagnostic> {
    let mut lines = super::lines(text);
    let header = lines
        .next()
        .ok_or_else(|| format_diag(1, "missing noble-ngraph-vectors/0 schema line"))?;
    if header.tokens != [SCHEMA] {
        return Err(format_diag(
            header.number,
            "missing noble-ngraph-vectors/0 schema line",
        ));
    }
    let mut declared: Option<Vec<ScalarType>> = None;
    let mut rows = Vec::new();
    for l in lines {
        match l.tokens[0] {
            "types" => {
                if declared.is_some() {
                    return Err(format_diag(l.number, "duplicate types line"));
                }
                declared = Some(types(&l.tokens[1..], l.number)?);
            }
            "v" => {
                let t = declared
                    .as_ref()
                    .ok_or_else(|| format_diag(l.number, "a v row before the types line"))?;
                rows.push(row(t, &l.tokens[1..], l.number)?);
            }
            o => return Err(format_diag(l.number, format!("unknown line kind {o:?}"))),
        }
    }
    let types = declared.ok_or_else(|| format_diag(header.number, "missing types line"))?;
    Ok(Vectors { types, rows })
}

/// Prints a vector file in canonical form.
#[must_use]
pub fn print(v: &Vectors) -> String {
    let mut s = format!("{SCHEMA}\ntypes {}\n", type_names(&v.types));
    for r in &v.rows {
        s.push_str("v ");
        s.push_str(&row_text(r));
        s.push('\n');
    }
    s
}
