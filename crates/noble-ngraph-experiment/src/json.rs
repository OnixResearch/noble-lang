//! Minimal deterministic JSON writer for evidence receipts (no dependency).

use std::fmt::Write as _;

/// A JSON value with ordered object keys.
#[derive(Clone, Debug, PartialEq)]
pub enum Json {
    /// `null`.
    Null,
    /// Boolean.
    Bool(bool),
    /// Unsigned integer.
    U(u64),
    /// Signed integer.
    I(i64),
    /// Finite binary64 (non-finite values are written as strings).
    F(f64),
    /// String.
    S(String),
    /// Array.
    A(Vec<Json>),
    /// Object (insertion order kept).
    O(Vec<(String, Json)>),
}

impl Json {
    /// An object from pairs.
    #[must_use]
    pub fn obj<const N: usize>(pairs: [(&str, Json); N]) -> Self {
        Self::O(pairs.into_iter().map(|(k, v)| (k.to_owned(), v)).collect())
    }

    /// A string.
    #[must_use]
    pub fn s(v: impl Into<String>) -> Self {
        Self::S(v.into())
    }

    /// Appends a key to an object.
    pub fn push(&mut self, k: &str, v: Self) {
        if let Self::O(o) = self {
            o.push((k.to_owned(), v));
        }
    }

    /// Pretty-printed text with a trailing newline.
    #[must_use]
    pub fn pretty(&self) -> String {
        let mut s = String::new();
        self.write(&mut s, 0);
        s.push('\n');
        s
    }

    fn write(&self, out: &mut String, depth: usize) {
        let pad = |out: &mut String, d: usize| {
            for _ in 0..d {
                out.push_str("  ");
            }
        };
        match self {
            Self::Null => out.push_str("null"),
            Self::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
            Self::U(v) => {
                let _ = write!(out, "{v}");
            }
            Self::I(v) => {
                let _ = write!(out, "{v}");
            }
            Self::F(v) => {
                if v.is_finite() {
                    let _ = write!(out, "{v:?}");
                } else {
                    let _ = write!(out, "\"{v}\"");
                }
            }
            Self::S(v) => escape(out, v),
            Self::A(items) => {
                if items.is_empty() {
                    out.push_str("[]");
                    return;
                }
                out.push_str("[\n");
                for (i, it) in items.iter().enumerate() {
                    pad(out, depth + 1);
                    it.write(out, depth + 1);
                    if i + 1 < items.len() {
                        out.push(',');
                    }
                    out.push('\n');
                }
                pad(out, depth);
                out.push(']');
            }
            Self::O(items) => {
                if items.is_empty() {
                    out.push_str("{}");
                    return;
                }
                out.push_str("{\n");
                for (i, (k, v)) in items.iter().enumerate() {
                    pad(out, depth + 1);
                    escape(out, k);
                    out.push_str(": ");
                    v.write(out, depth + 1);
                    if i + 1 < items.len() {
                        out.push(',');
                    }
                    out.push('\n');
                }
                pad(out, depth);
                out.push('}');
            }
        }
    }
}

fn escape(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
}
