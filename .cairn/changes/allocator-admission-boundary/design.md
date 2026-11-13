## Context

This change states Noble's allocator boundary for the selected extraction route and implements the
measuring lane behind it. It is a consumer compatibility and model-admission audit (issue #121): it is
not a reproduced Aeneas soundness bug, not evidence that a current subject uses a custom allocator, and
not a refinement claim about any allocator.

Baseline re-verified at the commit that carries this change. Selected identities are those in
`policy/tool-selection.json`: Aeneas extractor `505b6ca35217e7be5c96c3e2f8045edfbdf47291`, Aeneas Lean
library `1238879f9ea550512096e9bf8bd6fe7407b3eaef`, Charon `b104e24fea7d721b71e6c39fd70f26ff20bc0980`,
extraction rustc `nightly-2026-08-18` (version string `1.100.0-nightly`), Lean 4.35.0.

## The measured erasure

Each statement below is a property of the selected pins, not of Rust in general.

| Layer | Evidence | Effect |
|---|---|---|
| Charon preset | `translated.options.hide_allocator: true` in every extracted cell's LLBC (`--preset=aeneas`) | the allocator is removed from signatures before Aeneas sees them |
| Aeneas `Vec` model | `Aeneas/Std/Vec.lean:29-32` (`structure Vec (α) where slice : Slice α`), `:81-82` (`Vec.new` has no allocator), `:88-89`, `:155-156`, `:607-609` (`keepParams := [true,false]`, `keepTraitClauses := [true,false]`) | `Vec<T, A>` is `Vec<T>`; construction, growth and clone carry no allocator argument, state or failure |
| Aeneas `Global`/`Box` model | `Aeneas/Std/Alloc.lean:14-19` (marker `inductive Global`, documented as filtered), `:21-25` (deref is the identity, allocator dropped), `:27-28` (`Clone<Global>::clone = .ok .mk`) | naming `Global` is indistinguishable from not naming it |
| Noble external models | `proofs/m3/NobleKernel/FunsExternal.lean:402-416`, `:467-470`, `:477-486`, `:488-...`; `proofs/m2/NobleKernel/FunsExternal.lean:194-232` | `reserve`, `try_reserve_exact`, `truncate`, `as_slice`, `swap_remove`, `remove`, `clear`, `pop`, `is_empty` and the `Box` impls take the allocator type and ignore it |
| Generated Lean (explicit `Global` root) | receipt cell `global_vec`: `axiom alloc.vec.Vec.new_in (T : Type) {A : Type} : A → Result (alloc.vec.Vec T)`, called as `new_in Std.U8 ()` | the explicit allocator is erased to unit and the constructor is unmodelled |
| Generated Lean (stateful allocator root) | receipt cell `stateful_vec`: `axiom stateful_alloc.CountingAllocator : Type` plus `CountingAllocator.new : Result CountingAllocator` | a fabricated allocator value with its counters absent |

Therefore a passing extraction, translation or Lean build is *not* allocator evidence, and the refusal
has to be Noble's.

## Decisions

### Decision: admit the implicit global allocator only

The admitted case is a subject that constructs `Vec`/`Box` through `Vec::new`, `Box::new`, `push`, `len`,
`reserve`, `clone` and drop without naming an allocator. Blocked cases: explicit `Global`, generic
`A: Allocator`, borrowed `&A`, `System`, and any concrete custom or stateful allocator. Full rationale
and the reopen conditions are in `verification/allocator-admission/decision.md`, which this specification
requirement now makes normative.

### Decision: the refusal is source-located and rule-based, not a heuristic

`verification/allocator-admission/admission.mjs` decides over the LLBC and the Aeneas `translation.json`
plus a binding of the identities the run was made with. It refuses on four independent observation
paths — the retained Rust source of the root, a reachable explicit-allocator constructor, a reachable
allocator contract, and an allocator axiom in the translation — and every refusal carries the file, line
and column it was derived from. A root that is absent, opaque, bound to another run, or carried by a
decision whose identities changed is refused rather than reused.

### Decision: keep explicit `Global` blocked even when a gated fixture extracts

The selected extraction nightly still has the allocator API unstable: compiling the fixture without a
crate-level gate fails with `E0658` ("use of unstable library feature `allocator_api`"). The extraction
variant therefore carries `#![feature(allocator_api)]` and is not a stable-1.100 subject. Its extraction
is recorded as pipeline evidence only.

## Matrix

The measured matrix, stage by stage, is the lane's receipt (`verification/allocator-admission/receipt-*.json`)
and its summary table in `verification/allocator-admission/README.md`. The stage columns are source
compilation (released 1.100.0 and the extraction nightly, gated and ungated), Charon extraction, Aeneas
translation, Lean checking and Noble admission; unsupported and unexecuted cells are stated as such.

## Evidence classes

| Claim | Class |
|---|---|
| Fixtures compile under released Rust 1.100.0 | executed source-language check |
| Charon/Aeneas produce the recorded LLBC and `translation.json` | executed extraction/translation, not acceptance |
| Generated Lean builds against the pinned library | executed Lean check of generated code, not a proof |
| The admission decision refuses the named allocator roots | executed rule evaluation over that evidence |
| Noble admits no allocator other than the implicit global one | **policy**, stated by VT-ALLOC-01 |
| Anything about allocator soundness, refinement or correspondence | **not claimed** |
