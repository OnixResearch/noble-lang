# Decision: Noble admits the implicit global allocator only (#121)

Status: **selected** (2026-11-13). Owner: verification lane `verification/allocator-admission`.
Applies to: every Rust subject admitted through the selected Charon → Aeneas → Lean pipeline
(`policy/tool-selection.json`).
Related: VT-AENEAS-05 (external models and admitted translation boundaries), VT-PIN-02 (tool
pinning), issue #97 (pin convergence), issue #5 (allocation strategy, a different question).

## Decision

1. **Admitted: the implicit global allocator.** A subject that constructs `Vec`/`Box` through
   `Vec::new`, `Box::new`, `push`, `len`, `reserve`, `clone` and drop, and never names an
   allocator, keeps its current treatment. The three `implicit-global` fixture roots are the
   positive controls; their translations reach no allocator constructor and carry no allocator
   axiom.
2. **Blocked: every allocator the subject names.** Explicit `Global` (`Vec::new_in(Global)`,
   `Box::new_in(x, Global)`), a generic `A: Allocator`, a borrowed `&A`, `System`, and any
   concrete custom or stateful allocator are **refused at Noble's admission boundary** with a
   source-located refusal (`ALLOCATOR-SOURCE`, `ALLOCATOR-CONSTRUCTOR`, `ALLOCATOR-AXIOM`,
   `ALLOCATOR-TRAIT`). This holds even though the selected pipeline *accepts* those subjects:
   see the reason below.
3. **The selection is a refusal, not an absence of evidence.** A rustc build, a Charon
   extraction, an Aeneas translation and a Lean build all succeed for the blocked roots. They
   are not acceptance. Only `admission.mjs` refuses, and its refusal carries the Rust position.

## Why the selected pipeline cannot decide this

The selected Charon preset (`--preset=aeneas`) sets `hide_allocator: true`, and the pinned
Aeneas Lean library (aeneas_lean `1238879f`) models no allocator:

- `Aeneas/Std/Vec.lean:29-32` defines `alloc::vec::Vec` as a `Slice` and an element type only;
  at `:88-89`, `:155-156` and `:607-609` the `rust_fun` bindings use
  `keepParams := [true,false]` (and `keepTraitClauses := [true,false]` for `Clone`), which drops
  the allocator type, its `Clone` clause and every operation on it.
- `Aeneas/Std/Alloc.lean:14-19` models `alloc::alloc::Global` as a marker `inductive Global`
  whose comment states it should not appear in generated code because it is systematically
  filtered; `:21-25` models `Box` deref and `deref_mut` as the identity with the allocator
  dropped; `:27-28` models `Clone<Global>::clone` as `.ok .mk`.
- Noble's own external models follow the same pattern: `proofs/m3/NobleKernel/FunsExternal.lean:402-416`
  (Box impls take `_A : Type` and ignore it) and `:467-470`, `:477-486`, `:488-...`
  (`reserve`, `try_reserve_exact`, `truncate`, `as_slice`, `swap_remove`, `remove`, `clear`,
  `pop`, `is_empty` take `(_A : Type)` or `(_ : Type)` and ignore it);
  `proofs/m2/NobleKernel/FunsExternal.lean:194-232` repeats it.

Measured consequence in this lane (recorded per cell in `receipt-<commit>.json`): for
`probes::global_vec` the generated Lean contains

```lean
axiom alloc.vec.Vec.new_in
  (T : Type) {A : Type} : A → Result (alloc.vec.Vec T)
```

called as `alloc.vec.Vec.new_in Std.U8 ()` — the explicit `Global` argument is erased to unit and
`new_in` is an unmodelled axiom. For `probes::stateful_vec` the concrete allocator becomes an
uninterpreted `axiom stateful_alloc.CountingAllocator : Type` with its counters absent. Lean
accepts both. Admitting them would be exactly the silent inheritance VT-AENEAS-05 forbids.

## Adoption condition (what would lift this)

Both conditions are required, and this lane is the place that records them:

- **(a) A selected toolchain where the allocator API is stable.** The pinned extraction nightly
  (`nightly-2026-08-18`, version string `1.100.0-nightly`) still has `allocator_api` unstable:
  compiling the fixture without the crate gate fails with eleven `E0658` errors
  (`help: add #![feature(allocator_api)]`). Released Rust 1.100.0 compiles the same source with
  no gate. Selecting a stable-1.100-or-newer *extraction* compiler is a `#97`/VT-PIN-02 pin
  change, with the normal pin review and replayed evidence; this lane does not make it.
- **(b) A reviewed allocator-aware model.** A model that keeps allocator state, proves the
  failure/`handle_alloc_error` behaviour, and represents deallocation must exist in the
  selected Lean library **and** in Noble's external models, with the review recorded under
  VT-AENEAS-05. Today's `keepParams` bindings and the `_A`-ignoring externals are the opposite.

Until both hold, no allocator other than the implicit global one is admissible, and a subject
that needs one must restructure its source or stay outside the proof-backed subject set.

**Explicit Global stays blocked even if a gated fixture extracts.** The `extraction-nightly`
variant of the fixtures carries `#![feature(allocator_api)]`; it is not a stable-1.100 subject,
so its extraction cannot certify the stable language surface that a real subject would use. The
receipt records that variant separately for exactly this reason.

## How the decision is enforced

- `admission.mjs` is the pure decision (LLBC + `translation.json` + a source/tool/configuration
  binding). `run.mjs` executes it per root and fails the run if any cell's verdict differs from
  the pre-registered expectation in its `cells` table or if a refusal arrives without the codes
  the family must produce.
- `admission.test.mjs` pins every rule, its source-located span, and the four negative controls
  on documents shaped like real charon-0.1.254/aeneas-0.1.0 output.
- The pre-registered table and the required refusal codes were fixed in the runner commit before
  the receipt run. The fixture *design* was informed by exploratory pipeline runs; the decision
  itself is the policy above, not a threshold measured from those runs.

## Reopen conditions

Reopen this decision when any of the following is true, and record which:

1. The selection policy pins an extraction compiler whose `release` line is `1.100.0` or later
   **and** the fixture's stable variant extracts without a crate-level feature gate.
2. The pinned Aeneas Lean library (or Noble's external models) gains allocator-state modelling
   reviewed under VT-AENEAS-05, and `translated.options.hide_allocator` is no longer the
   behaviour of the selected preset.
3. A subject in the admitted closure is found to name an allocator today (that would be a
   defect in the existing subject set, not a reason to widen admission).
