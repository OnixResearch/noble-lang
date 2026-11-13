## Phase 1: Audit, lane and boundary (this change)

- [x] [serial] Audit the selected pins' allocator treatment: Charon's `hide_allocator` option, the Aeneas `Std/Vec.lean` and `Std/Alloc.lean` bindings, Noble's `m3`/`m2` external `Vec`/`Box` models, and the generated Lean for an explicit-`Global` and a stateful-allocator root. r[VT-ALLOC-01]
- [x] [serial] Add the standalone fixture crates `verification/allocator-admission/fixtures/{probes,stateful-alloc}` with one `pub fn` root per matrix cell, the stable and extraction-nightly variants, `unsafe_code = "forbid"` in the probes crate and the lane's only `unsafe impl Allocator` in the stateful dependency. r[VT-ALLOC-01]
- [x] [serial] Implement the pure admission decision (`admission.mjs`) with source-located refusals for explicit allocator arguments, `Allocator` contracts, `*_in` constructors, allocator axioms, unreached roots, opaque bodies, changed identities and stale receipts; pin every rule and the four controls in `admission.test.mjs`. r[VT-ALLOC-01]
- [x] [serial] Implement the matrix runner (`run.mjs`): released-1.100.0 compile, extraction-nightly gated and ungated checks, Charon `--start-from` per root, Aeneas with `-emit-json`, Lean build against the pinned library, admission, and the executed negative controls; fail the run on any pre-registered expectation mismatch. r[VT-ALLOC-01]
- [x] [serial] Record the decision (`decision.md`), the model audit and matrix (`README.md`), and the adoption condition in this specification; regenerate the compatibility view and the derived requirement ledger. r[VT-ALLOC-01]
- [x] [serial] Produce the receipt from a clean commit and retain it beside the lane. r[VT-ALLOC-01]

## Adoption gates (not tasks of this change)

Both are prerequisites for widening the boundary; neither is undertaken here, and each needs its own
change with replayed evidence.

- Select an extraction toolchain in which the allocator API is stable (a VT-PIN-02 pin change), so the
  stable fixture variant extracts without a crate-level feature gate.
- Add a reviewed allocator-aware model under VT-AENEAS-05 that represents allocator state, allocation
  failure and deallocation, replacing the `keepParams` erasure and the `_A`-ignoring external models.
