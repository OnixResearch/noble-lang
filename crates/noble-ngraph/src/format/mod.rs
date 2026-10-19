//! `Numeric-Graph-v1` file codecs (design D4–D6 of `numeric-graph-v1`).
//!
//! Every file starts with a schema line: `ngraph-ir/0 <sub-profile>` for
//! graphs ([`graph`]), `noble-ngraph-vectors/0` for evaluation vectors
//! ([`vectors`]), `noble-ngraph-data/0` for training data ([`data`]) and
//! `noble-ngraph-snapshot/0` for parameter snapshots ([`snapshot`]). `#`
//! starts a comment; blank lines, comments and runs of spaces carry no
//! meaning. Parsing only builds producer data: graphs are still admitted by
//! the independent checker, and snapshot identities are recomputed.

pub mod data;
pub mod graph;
pub mod snapshot;
pub mod value;
pub mod vectors;

/// One significant line: its 1-based number and its whitespace tokens.
pub(crate) struct Line<'a> {
    pub(crate) number: usize,
    pub(crate) tokens: alloc::vec::Vec<&'a str>,
}

/// Significant lines of `text`: comments removed, blank lines skipped.
pub(crate) fn lines(text: &str) -> impl Iterator<Item = Line<'_>> {
    text.lines().enumerate().filter_map(|(i, l)| {
        let body = l.split('#').next().unwrap_or("");
        let tokens: alloc::vec::Vec<&str> = body.split_whitespace().collect();
        (!tokens.is_empty()).then_some(Line {
            number: i + 1,
            tokens,
        })
    })
}

/// Decodes lowercase or uppercase hex bytes; `None` on odd length or a
/// non-hex digit.
#[must_use]
pub fn bytes_from_hex(s: &str) -> Option<alloc::vec::Vec<u8>> {
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
pub fn digest_from_hex(s: &str) -> Option<crate::ir::Digest> {
    let v = bytes_from_hex(s)?;
    <crate::ir::Digest>::try_from(v.as_slice()).ok()
}

/// Whether `t` is one or more ASCII decimal digits.
fn is_digits(t: &str) -> bool {
    !t.is_empty() && t.bytes().all(|b| b.is_ascii_digit())
}

/// An unsigned decimal integer: ASCII digits only, so a sign (including a
/// leading `+`) is refused, as everywhere in the `ngraph` file formats and
/// `noble ngraph` flags (design D5).
#[must_use]
pub fn decimal_u64(t: &str) -> Option<u64> {
    if is_digits(t) {
        t.parse().ok()
    } else {
        None
    }
}

/// [`decimal_u64`] within `u32`.
#[must_use]
pub fn decimal_u32(t: &str) -> Option<u32> {
    decimal_u64(t).and_then(|v| u32::try_from(v).ok())
}

/// A signed decimal integer: an optional `-`, then ASCII digits.
#[must_use]
pub fn decimal_i64(t: &str) -> Option<i64> {
    if is_digits(t.strip_prefix('-').unwrap_or(t)) {
        t.parse().ok()
    } else {
        None
    }
}

/// `0x` followed by one to `max_digits` hex digits (either case).
#[must_use]
pub fn hex_bits(t: &str, max_digits: usize) -> Option<u64> {
    let h = t.strip_prefix("0x")?;
    if h.is_empty() || h.len() > max_digits || !h.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    u64::from_str_radix(h, 16).ok()
}
