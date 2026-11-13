//! Allocator-admission fixture roots (#121).
//!
//! One `pub fn` per matrix cell. Each root constructs its collection itself, so the
//! declared extraction root (`--start-from probes::<root>`) is the only way the
//! collection operation becomes reachable: nothing is shared between roots.
//!
//! The crate has two variants selected by cargo feature:
//! * stable variant (no features): released Rust 1.100.0, where the custom-allocator
//!   API is stable, compiles this source with no crate-level feature attribute;
//! * extraction variant (`--features nightly-allocator-api`): the selected extraction
//!   nightly (1.100.0-nightly-2026-08-18) still has `allocator_api` unstable, so the
//!   crate root adds exactly `#![feature(allocator_api)]`.
//!
//! `unsafe_code = "forbid"` is this crate's manifest lint: the fixture roots must stay
//! inside the safe-source boundary Noble claims. The `unsafe impl Allocator` this
//! exercise needs lives in the separate `stateful-alloc` dependency.
#![cfg_attr(feature = "nightly-allocator-api", feature(allocator_api))]
#![forbid(unsafe_code)]

use std::alloc::{Allocator, Global, System};

// ---------------------------------------------------------------------------
// Implicit-Global controls: the ordinary subject surface Noble already admits.
// ---------------------------------------------------------------------------

/// Implicit-Global `Vec` baseline: construction, growth by `push`, length.
pub fn vec_baseline(input: u8) -> usize {
    let mut xs: Vec<u8> = Vec::new();
    xs.push(input);
    xs.len()
}

/// Implicit-Global `Box` baseline: construction, dereference, drop.
pub fn box_baseline(value: u32) -> u32 {
    let boxed: Box<u32> = Box::new(value);
    *boxed
}

/// Implicit-Global baseline that reaches growth, clone and (at scope exit) drop.
pub fn vec_growth_clone_baseline(input: u8, extra: usize) -> usize {
    let mut xs: Vec<u8> = Vec::new();
    xs.push(input);
    xs.reserve(extra);
    let copied = xs.clone();
    xs.len() + copied.len()
}

// ---------------------------------------------------------------------------
// Explicit-Global construction: stable in Rust 1.100.0, unstable on the
// selected extraction nightly. Admitted only if its own pipeline succeeds.
// ---------------------------------------------------------------------------

/// Explicit `Global` argument: `Vec::new_in(Global)`.
pub fn global_vec(input: u8) -> usize {
    let mut xs: Vec<u8, Global> = Vec::new_in(Global);
    xs.push(input);
    xs.len()
}

/// Explicit `Global` argument with a `Box` in the signature, as in the issue's probe.
pub fn global_box(value: u32) -> Box<u32, Global> {
    Box::new_in(value, Global)
}

// ---------------------------------------------------------------------------
// Generic and borrowed allocators.
// ---------------------------------------------------------------------------

/// Generic `A: Allocator`.
pub fn generic_vec<A: Allocator>(alloc: A, input: u8) -> usize {
    let mut xs: Vec<u8, A> = Vec::new_in(alloc);
    xs.push(input);
    xs.len()
}

/// Generic `A: Allocator` `Box`.
pub fn generic_box<A: Allocator>(alloc: A, value: u32) -> u32 {
    let boxed: Box<u32, A> = Box::new_in(value, alloc);
    *boxed
}

/// Generic allocator with growth and clone (both require `A: Clone`).
pub fn generic_vec_growth_clone<A: Allocator + Clone>(alloc: A, input: u8, extra: usize) -> usize {
    let mut xs: Vec<u8, A> = Vec::new_in(alloc);
    xs.push(input);
    xs.reserve(extra);
    let copied = xs.clone();
    xs.len() + copied.len()
}

/// Borrowed allocator: `Vec<u8, &A>` over `impl Allocator for &A`.
pub fn borrowed_vec<A: Allocator>(alloc: &A, input: u8) -> usize {
    let mut xs: Vec<u8, &A> = Vec::new_in(alloc);
    xs.push(input);
    xs.len()
}

// ---------------------------------------------------------------------------
// Concrete non-Global allocators.
// ---------------------------------------------------------------------------

/// `std::alloc::System`, the other standard-library allocator.
pub fn system_vec(input: u8) -> usize {
    let mut xs: Vec<u8, System> = Vec::new_in(System);
    xs.push(input);
    xs.len()
}

/// Concrete stateful allocator from the `stateful-alloc` dependency.
pub fn stateful_vec(input: u8) -> usize {
    let alloc = stateful_alloc::CountingAllocator::new();
    let mut xs: Vec<u8, stateful_alloc::CountingAllocator> = Vec::new_in(alloc);
    xs.push(input);
    xs.len()
}

/// Concrete stateful allocator with growth, clone and drop.
pub fn stateful_vec_growth_clone(input: u8, extra: usize) -> usize {
    let mut xs: Vec<u8, stateful_alloc::CountingAllocator> = Vec::new_in(stateful_alloc::CountingAllocator::new());
    xs.push(input);
    xs.reserve(extra);
    let copied = xs.clone();
    xs.len() + copied.len()
}
