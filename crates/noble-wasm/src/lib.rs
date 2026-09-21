#![no_std]
#![feature(register_tool)]
#![register_tool(charon, octet, tigerstyle)]
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

mod admission;
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
