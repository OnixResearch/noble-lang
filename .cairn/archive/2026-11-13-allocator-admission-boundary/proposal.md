## Why

Issue #121 records that Rust 1.100.0 (released 2026-11-12) stabilises the custom-allocator API, so
`Vec<T, A>`/`Box<T, A>` code that reaches Noble's proof-backed subject closure is no longer exotic. The
verification-toolchain specification says which toolchain produces a subject and which external models
are admitted (VT-AENEAS-05), but it says nothing about an allocator.

The gap is not hypothetical. The selected Charon preset erases the allocator (`hide_allocator: true`),
the pinned Aeneas Lean library keeps only the element type in `alloc::vec::Vec` and drops the allocator
parameter at its `rust_fun` bindings, and Noble's external `Vec`/`Box` models accept an allocator type
and ignore it. An explicit `Vec::new_in(Global)` subject therefore translates into an unmodelled
allocator axiom and passes Lean, while a subject that names a stateful allocator gets an uninterpreted
allocator type with its state absent. Without a stated boundary, that pipeline success reads as
acceptance and a generic or custom allocator silently inherits a model that only ever applied to the
implicit global allocator.

## What Changes

- Add VT-ALLOC-01: the admitted allocator is the implicit global one; every allocator the subject names
  is refused at Noble's admission boundary with the Rust source position; a pipeline success is never
  allocator acceptance; adoption requires a selected toolchain with a stable allocator API **and** a
  reviewed allocator-aware model. r[VT-ALLOC-01]
- Record the measured boundary rather than assert it. The lane `verification/allocator-admission/` adds:
  - twelve fixture roots (implicit-Global baselines, explicit `Global`, generic `A: Allocator`,
    borrowed `&A`, `System`, and a concrete stateful allocator) in standalone crates that are not
    workspace members;
  - a runner that executes the released-1.100.0 compile, the extraction nightly's gated and ungated
    checks, Charon `--start-from` per root, Aeneas with `-emit-json`, a Lean build against the pinned
    library, and the admission decision, recording each stage separately;
  - a pure admission decision with rule pins and four executed negative controls;
  - a receipt that binds the extractor, Charon, rustc and Lean-library revisions, the translation
    options and the failing stage. r[VT-ALLOC-01]

## Impact

- **Files**: `.cairn/specs/verification-toolchain/spec.md` (canonical requirement),
  `.cairn/changes/allocator-admission-boundary/` (this delta),
  `verification/allocator-admission/` (fixtures, runner, decision, admission check, receipt), the
  generated view `specs/VERIFICATION-TOOLCHAIN.md`, `specs/requirements.json` (derived ledger) and a
  section in `verification/extraction.md`.
- **Code**: no workspace Rust. The fixtures are standalone crates with their own `[workspace]` table, so
  the compilation-unit cap, the source inventory and `unsafe_code = "forbid"` are untouched; the lane's
  one `unsafe impl Allocator` lives in the `stateful-alloc` dependency crate, outside the workspace.
- **Pins and receipts**: no pin, subject, claim, `VALIDATION.json` or retained receipt changes. The
  boundary is stated against the *current* selection and stays valid only for it (VT-PIN-01/02).
- **Testing**: the fixtures are compiled, extracted, translated, built and adjudicated by the lane's own
  receipt; nothing here is a refinement proof, a correspondence result or M1 acceptance.
