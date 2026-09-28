//! Opt-in, bounded module declarations. Neither preparation nor publication invokes guest code.

mod links;
mod parsing;
mod state;
mod types;
pub use state::{ModuleKind, ModulePrepared, ModuleSession};

const SPAN: crate::Span = crate::Span { start: 0, end: 0 };
const MODULE_CAP: usize = 128;
const CONSTRUCTOR_CAP: usize = 256;

#[derive(Clone, Debug, PartialEq)]
pub struct BoundOperation {
    pub module_name: alloc::string::String,
    pub module_version: u32,
    pub operation: alloc::string::String,
    pub adapter_identity: alloc::string::String,
    pub adapter_slot: u32,
    pub input: alloc::vec::Vec<noble_kernel::types::Ty>,
    pub output: alloc::vec::Vec<noble_kernel::types::Ty>,
    pub effects: alloc::vec::Vec<noble_kernel::types::EffId>,
}

#[derive(Clone, Debug)]
pub(super) struct Context {
    pub environment: noble_kernel::contracts::Env,
    pub words: alloc::vec::Vec<(alloc::string::String, crate::source::Target)>,
    pub owner: Option<u64>,
}

#[derive(Clone, Debug, PartialEq)]
struct Export {
    name: alloc::string::String,
    ty: Option<noble_kernel::types::Ty>,
    words: alloc::vec::Vec<(alloc::string::String, crate::source::Target)>,
}

#[derive(Clone, Debug, PartialEq)]
struct Module {
    name: alloc::string::String,
    version: u32,
    identity: u64,
    exports: alloc::vec::Vec<Export>,
    adapter_slot: Option<u32>,
    source: alloc::vec::Vec<u8>,
}

#[derive(Clone, Debug, PartialEq)]
struct Alias {
    spelling: alloc::string::String,
    module: usize,
}

fn error(stage: crate::source::Stage, reason: &str) -> crate::source::Error {
    crate::source::Error::at(stage, crate::invalid(SPAN, reason))
}
fn diagnostic(stage: crate::source::Stage, problem: crate::Diagnostic) -> crate::source::Error {
    crate::source::Error::at(stage, problem)
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; slice starts_with/get and str indexing are non-const on both pinned compilers; reassess when those APIs become const."
)]
fn strip_qualified<'a>(text: &'a str, prefix: &[u8], separator: u8) -> Option<&'a str> {
    let bytes = text.as_bytes();
    let boundary = prefix.len();
    if bytes.starts_with(prefix) && bytes.get(boundary) == Some(&separator) {
        // The complete UTF-8 prefix and ASCII separator end at a character boundary.
        let next = boundary.saturating_add(1);
        Some(&text[next..])
    } else {
        None
    }
}

fn split_once_ascii(text: &str, separator: u8) -> Option<(&str, &str)> {
    let bytes = text.as_bytes();
    let mut at = 0usize;
    while at < bytes.len() && bytes[at] != separator {
        at += 1;
    }
    // Every caller supplies an ASCII separator, hence both cuts are boundaries.
    if at < bytes.len() {
        Some((&text[..at], &text[at.saturating_add(1)..]))
    } else {
        None
    }
}

const fn quotation_step(depth: u32, token: &crate::source::lexer::Token) -> Option<(u32, bool)> {
    if let crate::source::lexer::TokenKind::Open = &token.kind {
        let Some(next_depth) = depth.checked_add(1) else {
            return None;
        };
        return Some((next_depth, false));
    }
    if let crate::source::lexer::TokenKind::Close = &token.kind {
        let Some(next_depth) = depth.checked_sub(1) else {
            return None;
        };
        return Some((next_depth, next_depth == 0));
    }
    Some((depth, false))
}

fn append_decimal(mut out: alloc::string::String, value: u32) -> alloc::string::String {
    let mut divisor = 1u32;
    while value / divisor >= 10 {
        divisor *= 10;
    }
    let mut remaining = value;
    while divisor != 0 {
        let digit = remaining / divisor;
        remaining %= divisor;
        out.push(decimal_digit(digit));
        divisor /= 10;
    }
    out
}

const fn decimal_digit(digit: u32) -> char {
    match digit {
        0 => '0',
        1 => '1',
        2 => '2',
        3 => '3',
        4 => '4',
        5 => '5',
        6 => '6',
        7 => '7',
        8 => '8',
        _ => '9',
    }
}

struct Schema {
    name: alloc::string::String,
    kind: SchemaKind,
}
enum SchemaKind {
    Opaque {
        base: alloc::string::String,
        public: bool,
    },
    Variant {
        left: alloc::string::String,
        left_type: alloc::string::String,
        left_public: bool,
        right: alloc::string::String,
        right_type: alloc::string::String,
        right_public: bool,
    },
}

// A payload node is either accepted, rejected, or exposes checked children.
#[octet::sealed_enum]
enum Exposure<'a> {
    Public,
    Binary(&'a noble_kernel::types::Ty, &'a noble_kernel::types::Ty),
    Unary(&'a noble_kernel::types::Ty),
    Hidden,
}

/// Whether an external signature exposes only exported nominal interfaces.
/// An exported opaque descriptor hides its own checked representation; this
/// walk deliberately does not descend into that representation.
fn exposed_payload(
    root: &noble_kernel::types::Ty,
    environment: &noble_kernel::contracts::Env,
) -> bool {
    match exposed_node(root, environment) {
        Exposure::Public => true,
        Exposure::Hidden => false,
        Exposure::Binary(left, right) => {
            walk_exposure(alloc::vec::Vec::from([right, left]), environment)
        }
        Exposure::Unary(item) => walk_exposure(alloc::vec::Vec::from([item]), environment),
    }
}

struct ExposureState<'a> {
    pending: alloc::vec::Vec<&'a noble_kernel::types::Ty>,
    remaining: usize,
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; the bounded owned payload worklist rejects hidden or over-budget types without panicking on untrusted shapes; reassess fallible-check density."
)]
fn walk_exposure(
    pending: alloc::vec::Vec<&noble_kernel::types::Ty>,
    environment: &noble_kernel::contracts::Env,
) -> bool {
    let Some(remaining) = CONSTRUCTOR_CAP.checked_sub(1) else {
        return false;
    };
    let mut state = Some(ExposureState { pending, remaining });
    let mut has_work = true;
    while has_work {
        state = match state {
            Some(current) if current.pending.is_empty() => {
                has_work = false;
                Some(current)
            }
            Some(current) => exposure_step(current, environment),
            None => {
                has_work = false;
                None
            }
        };
    }
    state.is_some()
}

#[expect(
    tigerstyle::missing_const_fn,
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; Vec pop/push and owned state drop are non-const on pinned compilers; budget and pending-capacity failures reject without panic; reassess const APIs and fallible-check density."
)]
fn exposure_step<'a>(
    mut state: ExposureState<'a>,
    environment: &noble_kernel::contracts::Env,
) -> Option<ExposureState<'a>> {
    let Some(ty) = state.pending.pop() else {
        return Some(state);
    };
    if state.remaining == 0 {
        return None;
    }
    state.remaining -= 1;
    match exposed_node(ty, environment) {
        Exposure::Public => Some(state),
        Exposure::Hidden => None,
        Exposure::Binary(left, right) => {
            if CONSTRUCTOR_CAP < 2 || state.pending.len() > CONSTRUCTOR_CAP.saturating_sub(2) {
                return None;
            }
            state.pending.push(right);
            state.pending.push(left);
            Some(state)
        }
        Exposure::Unary(item) => {
            if state.pending.len() == CONSTRUCTOR_CAP {
                return None;
            }
            state.pending.push(item);
            Some(state)
        }
    }
}

fn exposed_node<'a>(
    ty: &'a noble_kernel::types::Ty,
    environment: &noble_kernel::contracts::Env,
) -> Exposure<'a> {
    if scalar_payload(ty) {
        return Exposure::Public;
    }
    if let noble_kernel::types::Ty::Pair(left, right) | noble_kernel::types::Ty::Sum(left, right) =
        ty
    {
        return Exposure::Binary(left, right);
    }
    if let noble_kernel::types::Ty::List(item) = ty {
        return Exposure::Unary(item);
    }
    if let noble_kernel::types::Ty::Nominal(id, _) = ty {
        return nominal_exposure(*id, environment);
    }
    Exposure::Hidden
}

fn nominal_exposure<'a>(
    id: noble_kernel::types::NominalTypeId,
    environment: &noble_kernel::contracts::Env,
) -> Exposure<'a> {
    if environment
        .nominal(id)
        .is_some_and(|declaration| declaration.exported)
    {
        Exposure::Public
    } else {
        Exposure::Hidden
    }
}

const fn scalar_payload(ty: &noble_kernel::types::Ty) -> bool {
    matches!(
        ty,
        noble_kernel::types::Ty::Unit
            | noble_kernel::types::Ty::Bool
            | noble_kernel::types::Ty::I64
            | noble_kernel::types::Ty::Text
    )
}
