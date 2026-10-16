//! Training-data files `noble-ngraph-data/0` (design D5).
//!
//! ```text
//! noble-ngraph-data/0
//! types f32 f32 ; f32
//! point 0x3f000000 0x3e800000 ; 0x3f400000
//! ```
//!
//! The types before `;` are the model's ordered input slot types, the types
//! after it the loss graph's target slot types. Row order is preserved; the
//! training-data digest binds it.

#[allow(unused_imports)]
use crate::prelude::*;

use super::value::{format_diag, row, row_text, type_names, types};
use crate::diag::{Code, Diagnostic, Site};
use crate::ir::ScalarType;
use crate::sem::Value;
use crate::train::Point;

/// Schema line.
pub const SCHEMA: &str = "noble-ngraph-data/0";

/// A parsed training-data file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Data {
    /// Declared model input types.
    pub inputs: Vec<ScalarType>,
    /// Declared loss target types.
    pub targets: Vec<ScalarType>,
    /// Points in file order: (inputs, targets).
    pub points: Vec<(Vec<Value>, Vec<Value>)>,
}

/// Splits tokens at the single `;` separator.
fn halves<'a>(tokens: &[&'a str], line: usize) -> Result<(Vec<&'a str>, Vec<&'a str>), Diagnostic> {
    let mut left = Vec::new();
    let mut right = Vec::new();
    let mut seen = 0;
    for tok in tokens {
        let mut parts = tok.split(';');
        let first = parts.next().unwrap_or("");
        if !first.is_empty() {
            if seen == 0 {
                left.push(first);
            } else {
                right.push(first);
            }
        }
        for p in parts {
            seen += 1;
            if !p.is_empty() {
                right.push(p);
            }
        }
    }
    if seen != 1 {
        return Err(format_diag(line, "expected exactly one `;` separator"));
    }
    Ok((left, right))
}

/// Parses a training-data file.
///
/// # Errors
/// `E-FORMAT` (schema, syntax, missing or duplicate `types`, separator, row
/// counts), `E-LITERAL-INEXACT` or `E-LITERAL-NONFINITE`.
pub fn parse(text: &str) -> Result<Data, Diagnostic> {
    let mut lines = super::lines(text);
    let header = lines
        .next()
        .ok_or_else(|| format_diag(1, "missing noble-ngraph-data/0 schema line"))?;
    if header.tokens != [SCHEMA] {
        return Err(format_diag(
            header.number,
            "missing noble-ngraph-data/0 schema line",
        ));
    }
    let mut declared: Option<(Vec<ScalarType>, Vec<ScalarType>)> = None;
    let mut points = Vec::new();
    for l in lines {
        match l.tokens[0] {
            "types" => {
                if declared.is_some() {
                    return Err(format_diag(l.number, "duplicate types line"));
                }
                let (a, b) = halves(&l.tokens[1..], l.number)?;
                declared = Some((types(&a, l.number)?, types(&b, l.number)?));
            }
            "point" => {
                let (ti, tt) = declared
                    .as_ref()
                    .ok_or_else(|| format_diag(l.number, "a point before the types line"))?;
                let (a, b) = halves(&l.tokens[1..], l.number)?;
                points.push((row(ti, &a, l.number)?, row(tt, &b, l.number)?));
            }
            o => return Err(format_diag(l.number, format!("unknown line kind {o:?}"))),
        }
    }
    let (inputs, targets) =
        declared.ok_or_else(|| format_diag(header.number, "missing types line"))?;
    Ok(Data {
        inputs,
        targets,
        points,
    })
}

/// Prints a training-data file in canonical form.
#[must_use]
pub fn print(d: &Data) -> String {
    let mut s = format!(
        "{SCHEMA}\ntypes {} ; {}\n",
        type_names(&d.inputs),
        type_names(&d.targets)
    );
    for (i, t) in &d.points {
        s.push_str("point ");
        s.push_str(&row_text(i));
        s.push_str(" ; ");
        s.push_str(&row_text(t));
        s.push('\n');
    }
    s
}

impl Data {
    /// The binary32 training points.
    ///
    /// # Errors
    /// `E-INTERFACE` if a declared type is not `f32`.
    pub fn to_points(&self) -> Result<Vec<Point>, Diagnostic> {
        if self
            .inputs
            .iter()
            .chain(&self.targets)
            .any(|t| *t != ScalarType::F32)
        {
            return Err(Diagnostic::new(
                Code::Interface,
                Site::Graph,
                "training data must be f32",
            ));
        }
        let f = |vs: &[Value]| {
            vs.iter()
                .map(|v| match v {
                    Value::F32(b) => f32::from_bits(*b),
                    Value::I64(_) | Value::Bool(_) => f32::NAN,
                })
                .collect::<Vec<f32>>()
        };
        Ok(self
            .points
            .iter()
            .map(|(i, t)| Point {
                inputs: f(i),
                targets: f(t),
            })
            .collect())
    }

    /// The file form of binary32 training points.
    #[must_use]
    pub fn of_points(points: &[Point]) -> Self {
        let f = |xs: &[f32]| xs.iter().map(|x| Value::f32(*x)).collect::<Vec<Value>>();
        let (ni, nt) = points
            .first()
            .map_or((0, 0), |p| (p.inputs.len(), p.targets.len()));
        Self {
            inputs: vec![ScalarType::F32; ni],
            targets: vec![ScalarType::F32; nt],
            points: points
                .iter()
                .map(|p| (f(&p.inputs), f(&p.targets)))
                .collect(),
        }
    }
}
