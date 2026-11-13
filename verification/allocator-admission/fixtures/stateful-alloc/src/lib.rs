//! A concrete stateful allocator dependency for the allocator-admission fixtures (#121).
//!
//! `Allocator` is an `unsafe trait`, so a real stateful implementation is this lane's
//! only `unsafe` code. Keeping it in a separate standalone crate means the probes crate
//! keeps `unsafe_code = "forbid"`, and neither crate is a member of the repository
//! workspace, whose `unsafe_code = "forbid"` applies to every admitted subject.
#![cfg_attr(feature = "nightly-allocator-api", feature(allocator_api))]

use std::alloc::{AllocError, Allocator, Global, Layout};
use std::cell::Cell;
use std::ptr::NonNull;

/// Counts allocations and deallocations, so the allocator carries state that a
/// value-only `Vec` model cannot represent.
#[derive(Clone, Debug, Default)]
pub struct CountingAllocator {
    allocations: Cell<usize>,
    deallocations: Cell<usize>,
}

impl CountingAllocator {
    /// A fresh allocator with zeroed counters.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of successful `allocate` calls.
    #[must_use]
    pub fn allocations(&self) -> usize {
        self.allocations.get()
    }

    /// Number of `deallocate` calls.
    #[must_use]
    pub fn deallocations(&self) -> usize {
        self.deallocations.get()
    }
}

// SAFETY: every block handed to a caller comes from `Global` with the caller's layout,
// and every `deallocate`/`grow`/`shrink` forwards to `Global` with the layout it was
// given. The counters are a side effect that cannot invalidate a block, so the
// `Allocator` safety contract (returned memory stays valid until it is invalidated
// through this allocator with the same layout) holds.
unsafe impl Allocator for CountingAllocator {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        self.allocations.set(self.allocations.get() + 1);
        Global.allocate(layout)
    }

    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        self.deallocations.set(self.deallocations.get() + 1);
        // SAFETY: the caller guarantees that `ptr` and `layout` denote a block that is
        // currently allocated through this allocator, which is always a `Global` block.
        unsafe { Global.deallocate(ptr, layout) }
    }
}
