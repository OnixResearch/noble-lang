#![no_std]
#![feature(register_tool)]
#![register_tool(charon, octet, tigerstyle)]
#![expect(
    tigerstyle::excessive_file_length,
    reason = "Owner: noble-wasm-maintainers; each selected source unit keeps one lowering stage, its byte layout and its refusal ordering in a single reviewable file"
)]
#![expect(
    tigerstyle::module_file_count,
    reason = "Owner: noble-wasm-maintainers; the selected source stage is already split by lowering phase at the policy's nine-file capacity"
)]
#![expect(
    tigerstyle::underscore_in_module_filename,
    reason = "Owner: noble-wasm-maintainers; source/emit/core_live.rs names the Core-live ownership stage beside the separate live stage; a nested core/live.rs path would imply a core module that does not exist."
)]
#![expect(
    tigerstyle::platform_dependent_cast,
    reason = "Owner: noble-wasm-maintainers; supported targets have usize >= 32 bits and every flagged cast converts a validated non-negative count or index"
)]
#![expect(
    tigerstyle::nested_conditionals,
    reason = "Owner: noble-wasm-maintainers; the ordered emitter dispatch and its early-return macro keep the outer constructor match adjacent to its shape arms, and flattening them would duplicate the constructor guards"
)]
#![expect(
    tigerstyle::raw_arithmetic_overflow,
    reason = "Owner: noble-wasm-maintainers; emission and preflight arithmetic operates on admitted counts and byte lengths bounded by the explicit table, signature, node and output limits; overflow is refused at admission"
)]
#![expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-wasm-maintainers; the emitter and preflight stages are ordered validators whose explicit bound checks and diagnostics are the review subject"
)]
#![expect(
    tigerstyle::fragile_exhaustive_enum_match,
    reason = "Owner: noble-wasm-maintainers; the kernel type, shape and action vocabularies are closed and a new variant must force every emission match to change"
)]
#![expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-wasm-maintainers; helper constness is not part of the compiler contract; the flagged helpers read runtime tables or allocate while preserving diagnostics"
)]
#![expect(
    tigerstyle::ambiguous_params,
    reason = "Owner: noble-wasm-maintainers; the flagged parameters are the published stack and interface coordinates, and every call site already fixes their order in the reviewed lowering step"
)]
//! Bounded, kernel-checked lowering to a fixed Wasm instruction vocabulary.
//!
//! The returned bytes are a complete WAT module. Assembly and execution belong
//! to the host; this pure boundary never trusts a producer's acceptance flag.

extern crate alloc;

macro_rules! attempt {
    ($step:expr) => {
        match $step {
            Ok(value) => value,
            Err(failure) => return Err(failure),
        }
    };
}

/// Sequence searches written as explicit index walks.
///
/// The pinned Aeneas cannot translate `Iterator::find` (its predicate has a
/// higher-ranked bound). Each helper visits in the same order, stops at the
/// same element and returns the same value as the method it replaces.
pub(crate) mod scan {
    /// The first element satisfying `predicate`, as `items.iter().find(..)`:
    /// elements are tested in order and none after the first match.
    pub(crate) fn find_first<T>(items: &[T], mut predicate: impl FnMut(&T) -> bool) -> Option<&T> {
        let mut index = 0;
        let mut found = None;
        while found.is_none() && index < items.len() {
            if predicate(&items[index]) {
                found = Some(index);
            }
            index += 1;
        }
        match found {
            Some(position) => Some(&items[position]),
            None => None,
        }
    }

    #[cfg(test)]
    mod tests {
        #[test]
        fn find_first_visits_in_order_and_stops_at_the_first_match() {
            let items = [3u32, 5, 4, 5, 6];
            let mut visited = alloc::vec::Vec::new();
            let found = super::find_first(&items, |item| {
                visited.push(*item);
                *item == 5
            });
            assert_eq!(found, items.iter().find(|item| **item == 5));
            assert!(core::ptr::eq(found.unwrap_or(&0), &items[1]));
            assert_eq!(visited, [3, 5]);
            assert_eq!(super::find_first(&items, |item| *item == 9), None);
        }
    }
}

mod admission;
pub mod assets;
pub mod component;
mod emit;
mod lowering;
mod operations;
mod output;
mod signatures;
pub mod source;

const NODE_LIMIT: usize = 256;
const BODY_LIMIT: usize = 128;
const BODY_OPERATION_LIMIT: usize = 128;
const EXPORT_LIMIT: usize = 32;
const FUNCTION_LIMIT: u32 = 1024;
const OUTPUT_BYTE_LIMIT: usize = 1_048_576;

/// Storage implementation, without changing the compiled program semantics.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Representation {
    WasmGc,
    ManagedLinearMemory,
}

/// A fail-closed compiler outcome. No failure returns partial module bytes.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Diagnostic {
    Invalid,
    Exhausted,
    Unsupported,
    Defective,
}

/// Recheck raw input, lower its monomorphic vocabulary, and emit complete WAT.
///
/// The root must be a closed, pure sequence of quotation literals producing
/// at most 32 Programs. Supported operations inside them remain native Wasm
/// instruction functions; semantic recipes never determine executable code.
pub fn compile(
    representation: Representation,
    request: &noble_kernel::untrusted::Request,
    candidate: &noble_kernel::untrusted::Candidate,
) -> Result<alloc::vec::Vec<u8>, Diagnostic> {
    let checked = attempt!(admission::check(request, candidate));
    let plan = attempt!(lowering::lower(candidate, &checked));
    emit::module(representation, candidate, &plan)
}
