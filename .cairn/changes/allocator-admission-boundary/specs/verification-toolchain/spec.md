# Allocator admission at the extraction boundary

## ADDED Requirements

### Requirement: VT-ALLOC-01
r[VT-ALLOC-01]

**VT-ALLOC-01.** A subject admitted through the selected Charon → Aeneas → Lean route MUST use the
implicit global allocator and MUST NOT name an allocator.

The selected pipeline cannot answer the question by itself, so its own success MUST NOT be read as an
answer. Charon reports `hide_allocator: true` under the selected preset; the pinned Aeneas Lean library
(`sources.aeneas_lean`) models `alloc::vec::Vec` as an element type and a slice, and drops the allocator
parameter and its trait clauses at its standard-library bindings; `alloc::alloc::Global` is a marker the
library documents as filtered out; and Noble's own external `Vec`/`Box` models take an allocator type
and ignore it. Consequently an explicit allocator argument is erased to a phantom type and an unmodelled
constructor, and both Aeneas and Lean accept the result. A successful Charon extraction, Aeneas
translation or Lean build MUST NOT be reported as allocator acceptance, and a successful stable-rustc
build MUST NOT be reported as extraction or refinement acceptance.

Noble's admission boundary MUST refuse, with the Rust source position, each of:

- a reachable explicit-allocator constructor (the `*_in` construction families) or an allocator axiom in
  the translation;
- a reachable `core::alloc::Allocator`, `AllocatorClone` or `GlobalAlloc` contract;
- a retained source that names an explicit `Global`, `System`, generic `A: Allocator`, borrowed `&A` or
  concrete custom or stateful allocator argument.

A decision recorded for one bound subject MUST NOT be reused for a different root, source, allocator
configuration, tool revision or translation option set; an unreached root, an opaque or omitted body, a
changed identity and a stale receipt MUST NOT produce admission.

Adoption requires both of the following, and neither alone:

1. a selected extraction toolchain in which the allocator API is stable — a VT-PIN-02 pin change with
   the normal pin review and replayed evidence;
2. an allocator-aware model reviewed under VT-AENEAS-05 that represents allocator state, allocation
   failure and deallocation.

A nightly-only, feature-gated fixture is not the stable language surface and MUST NOT certify explicit
`Global`. Until both conditions hold, explicit `Global`, generic, borrowed, `System` and concrete
stateful allocators remain explicitly blocked. Evidence for this boundary MUST preserve the Aeneas,
Charon, extraction-rustc and Lean-library revisions, the translation options, the fixture and source
digests, and the stage at which any run failed.
