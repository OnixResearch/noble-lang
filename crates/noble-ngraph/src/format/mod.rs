//! `Numeric-Graph-v1` file codecs (design D4–D6 of `numeric-graph-v1`).
//!
//! Every file starts with a schema line: `ngraph-ir/0 <sub-profile>` for
//! graphs ([`graph`]), `noble-ngraph-vectors/0` for evaluation vectors
//! ([`vectors`]), `noble-ngraph-data/0` for training data ([`data`]) and
//! `noble-ngraph-snapshot/0` for parameter snapshots ([`snapshot`]). `#`
//! starts a comment; blank lines, comments and runs of spaces carry no
//! meaning. Parsing only builds producer data: graphs are still admitted by
//! the independent checker, and snapshot identities are recomputed.

#[allow(unused_imports)]
use crate::prelude::*;

use crate::ir::Digest;

pub mod data;
pub mod graph;
pub mod snapshot;
pub mod value;
pub mod vectors;

/// One significant line: its 1-based number and its whitespace tokens.
pub(crate) struct Line<'a> {
    pub(crate) number: usize,
    pub(crate) tokens: Vec<&'a str>,
}

/// Significant lines of `text`: comments removed, blank lines skipped.
pub(crate) fn lines(text: &str) -> impl Iterator<Item = Line<'_>> {
    text.lines().enumerate().filter_map(|(i, l)| {
        let body = l.split('#').next().unwrap_or("");
        let tokens: Vec<&str> = body.split_whitespace().collect();
        (!tokens.is_empty()).then_some(Line {
            number: i + 1,
            tokens,
        })
    })
}

/// Decodes lowercase or uppercase hex bytes; `None` on odd length or a
/// non-hex digit.
#[must_use]
pub fn bytes_from_hex(s: &str) -> Option<Vec<u8>> {
    let b = s.as_bytes();
    if !b.len().is_multiple_of(2) {
        return None;
    }
    let digit = |c: u8| match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    };
    b.chunks_exact(2)
        .map(|p| Some(digit(p[0])? << 4 | digit(p[1])?))
        .collect()
}

/// Decodes a 64-digit hex digest.
#[must_use]
pub fn digest_from_hex(s: &str) -> Option<Digest> {
    let v = bytes_from_hex(s)?;
    <Digest>::try_from(v.as_slice()).ok()
}
