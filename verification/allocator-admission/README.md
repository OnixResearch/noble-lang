# Allocator admission at the selected extraction boundary

Issue: [#121](https://github.com/OnixResearch/noble-lang/issues/121).
Boundary requirement: [VT-ALLOC-01](../../.cairn/specs/verification-toolchain/spec.md).
Decision record: [`decision.md`](decision.md).
Measured evidence: `receipt-<commit>.json` in this directory (produced by `run.mjs` from a clean commit).

This lane answers one question: which allocator uses does Noble's **selected** Charon → Aeneas → Lean
pipeline support, which does it erase, and where must Noble refuse instead. Rust 1.100.0 (released
2026-11-12) stabilises the custom-allocator API, so the question stops being hypothetical.

## What this lane is not

- It is not an Aeneas or Charon soundness claim, and not a reproduced upstream bug.
- It is not evidence that a current Noble subject uses a custom allocator.
- A `compiled`, `extracted`, `translated` or `built` cell is **not** allocator acceptance. Only the
  `admission` column decides, and only for the bound run.
- Nothing here is a refinement theorem, a model-to-implementation correspondence, or M1 acceptance.

## Why a passing pipeline is not allocator evidence

The selected pins erase the allocator, so the pipeline cannot see it:

| Layer | Evidence | Effect |
|---|---|---|
| Charon preset | `translated.options.hide_allocator: true` in every extracted cell (`--preset=aeneas`) | the allocator leaves signatures before Aeneas sees them |
| Aeneas `Vec` model | `Aeneas/Std/Vec.lean:29-32` (`structure Vec (α) where slice : Slice α`), `:81-82` (`Vec.new` takes no allocator), `:88-89`, `:155-156`, `:607-609` (`keepParams := [true,false]`, `keepTraitClauses := [true,false]`) | `Vec<T, A>` is `Vec<T>`: no allocator argument, state, failure or deallocation |
| Aeneas `Global`/`Box` model | `Aeneas/Std/Alloc.lean:14-19` (marker `inductive Global`, documented as filtered), `:21-25` (deref is the identity, allocator dropped), `:27-28` (`Clone<Global>::clone = .ok .mk`) | naming `Global` is indistinguishable from not naming it |
| Noble external models | `proofs/m3/NobleKernel/FunsExternal.lean:402-416`, `:467-470`, `:477-486`, `:488-...`; `proofs/m2/NobleKernel/FunsExternal.lean:194-232` | `reserve`, `try_reserve_exact`, `truncate`, `as_slice`, `swap_remove`, `remove`, `clear`, `pop`, `is_empty`, and the `Box` impls take the allocator type and ignore it |

The Aeneas paths are inside the pinned library
`proofs/m5/.lake/packages/aeneas/backends/lean/` (library revision `sources.aeneas_lean`).

Two measured consequences, both recorded per cell in the receipt:

- `probes::global_vec` translates to `axiom alloc.vec.Vec.new_in (T : Type) {A : Type} : A → Result
  (alloc.vec.Vec T)`, called as `alloc.vec.Vec.new_in Std.U8 ()`. The explicit `Global` argument is
  erased to unit and the constructor is unmodelled.
- `probes::stateful_vec` translates the concrete allocator to `axiom stateful_alloc.CountingAllocator :
  Type` and `CountingAllocator.new : Result CountingAllocator`, with its counters absent.

Lean accepts both. This is exactly the silent inheritance VT-AENEAS-05 forbids, so the refusal lives at
Noble's admission boundary.

## Fixtures

`fixtures/probes` and `fixtures/stateful-alloc` are standalone crates with an empty `[workspace]` table.
They are not members of the repository workspace, so they add no compilation unit, no source-inventory
subject and no workspace lint change. `fixtures/probes` keeps `unsafe_code = "forbid"`; the lane's only
`unsafe impl Allocator` is in `fixtures/stateful-alloc`.

One `pub fn` root per matrix cell; each root constructs its own collection, so `--start-from
probes::<root>` is the only way an operation becomes reachable:

| Root | Family | Operations |
|---|---|---|
| `vec_baseline` | implicit-global | `Vec::new`, `push`, `len`, drop |
| `box_baseline` | implicit-global | `Box::new`, deref, drop |
| `vec_growth_clone_baseline` | implicit-global | `Vec::new`, `push`, `reserve`, `clone`, drop |
| `global_vec` | explicit-global | `Vec::new_in(Global)`, `push`, `len` |
| `global_box` | explicit-global | `Box::new_in(value, Global)`, returned |
| `generic_vec` | generic-allocator | `Vec::new_in(A)`, `push`, `len` |
| `generic_box` | generic-allocator | `Box::new_in(value, A)`, deref |
| `generic_vec_growth_clone` | generic-allocator | `Vec::new_in(A)`, `push`, `reserve`, `clone` |
| `borrowed_vec` | borrowed-allocator | `Vec<u8, &A>::new_in(&A)`, `push`, `len` |
| `system_vec` | system-allocator | `Vec::new_in(System)`, `push`, `len` |
| `stateful_vec` | stateful-allocator | `Vec::new_in(CountingAllocator)`, `push`, `len` |
| `stateful_vec_growth_clone` | stateful-allocator | `Vec::new_in(CountingAllocator)`, `push`, `reserve`, `clone` |

### Variants

The crate root carries `#![cfg_attr(feature = "nightly-allocator-api", feature(allocator_api))]`, so one
source tree serves both variants and they cannot drift:

- **stable variant** (no features): released Rust 1.100.0, where the API is stable, compiles it with no
  crate-level feature attribute;
- **extraction variant** (`--features nightly-allocator-api`, or `--all-features` as the selection policy
  does): adds exactly `#![feature(allocator_api)]` for the pinned extraction nightly, which still has the
  API unstable.

## Stages

`run.mjs` executes and records each stage separately; a stage result never stands in for another.

| Stage | What runs | What a pass means |
|---|---|---|
| `stable_1_100_ungated` | releases `cargo build` of the stable variant with released 1.100.0 | the Rust language surface accepts the fixture — nothing about extraction |
| `stable_1_100_gated` | released 1.100.0 with the nightly-only crate attribute | expected to fail: a nightly-gated subject is not a stable-1.100 subject |
| `extraction_nightly_ungated` | extracted nightly `cargo build` of the stable variant | expected to fail with `E0658`: the pinned extraction compiler does not have the stable API |
| `extraction_nightly_gated` | extracted nightly `cargo build` with the crate attribute | the extraction variant is representable — still not acceptance |
| `charon` | `charon cargo --preset=aeneas --error-on-warnings --start-from probes::<root>` with the policy's cargo arguments | the root's closure are in the LLBC |
| `aeneas` | `aeneas -backend lean -abort-on-error -warnings-as-errors -no-progress-bar -emit-json -dest …` | the closure translates; with the allocator erased |
| `lean` | `lake build <Generated>` against an isolated copy of the pinned Aeneas Lean packages | the generated file elaborates — not a proof of anything; any missing package oleans are built only in the run artifact |
| `admission` | `admission.mjs` over that LLBC, `translation.json` and binding | Noble admits or refuses, with the Rust position |

Source compilation is a crate-level verdict (one `cargo build` unit); the extraction, translation, Lean
and admission cells are genuinely per root because `--start-from` bounds each translation to that root's
closure.

Each cell records the LLBC and `translation.json` digests, the generated Lean file digests,
`translated.options` (including `hide_allocator`), whether the root appears in the translation, the
first failing stage with its diagnostic, and the admission decision with its spans.

## Admission rules

`admission.mjs` refuses — never silently admits — on each of these, with the Rust position:

| Code | Trigger |
|---|---|
| `ROOT-MISSING` | the declared root is absent from the translation (an unreached root cannot borrow another root's extraction) |
| `ROOT-OPAQUE` | the root is non-transparent or has no structured body |
| `ALLOCATOR-SOURCE` | the root's retained Rust source names `new_in(...)`, another `*_in` constructor, `Allocator`, or `System` |
| `ALLOCATOR-CONSTRUCTOR` | a reachable declaration is an explicit-allocator constructor |
| `ALLOCATOR-TRAIT` | a reachable declaration, trait declaration or trait implementation is the `Allocator`/`AllocatorClone`/`GlobalAlloc` contract |
| `ALLOCATOR-AXIOM` | the translation turns an allocator constructor or contract into an opaque external/axiom |
| `BINDING-IDENTITY` | the source digest, fixture path, selection, root or any tool revision differs from the frozen identity |
| `BINDING-OPTIONS` | the preset no longer erases the allocator, so this audit's premise does not hold |
| `BINDING-ROOT` | the LLBC's own `start_from` is not the root being decided |
| `STALE-RECEIPT` | a recorded decision is re-validated against identities that changed |

The rules are pinned by `admission.test.mjs` (`bun test`), including the exact source positions
(`global_vec` → `src/lib.rs:55:39`, `generic_vec` → `:70:22`, `borrowed_vec` → `:92:23`) and the
precision that a concrete `stateful_alloc::CountingAllocator` is refused as an allocator *use*, not
mistaken for the `Allocator` contract.

## Negative controls

`run.mjs` executes each control on real evidence and fails if one admits:

| Control | Method | Required refusal |
|---|---|---|
| unreached root | evaluate the real `generic_box` LLBC as if it were `global_vec` | `ROOT-MISSING`, `BINDING-ROOT` |
| omitted / opaque body | replace the real `vec_baseline` root body with `"Opaque"` | `ROOT-OPAQUE` |
| changed allocator identity | re-check the real baseline cell against a different Aeneas Lean library revision | `BINDING-IDENTITY` |
| changed source identity | re-check it against a different fixture source digest | `BINDING-IDENTITY` |
| stale receipt | re-validate the emitted baseline decision against a changed source digest | `STALE-RECEIPT` |
| unchanged receipt | re-validate the emitted baseline decision against its own binding | no refusal |

## The boundary and its adoption condition

The decision is [`decision.md`](decision.md) and VT-ALLOC-01: **admit the implicit global allocator
only**; refuse explicit `Global`, generic `A: Allocator`, borrowed `&A`, `System` and concrete custom or
stateful allocators with a source-located refusal. Adoption needs both a selected toolchain in which the
allocator API is stable (a VT-PIN-02 pin change) **and** an allocator-aware model reviewed under
VT-AENEAS-05. A nightly-gated fixture is not a stable-1.100 subject and never certifies explicit
`Global`.

## Reproducing

The runner is the selected Node and needs the released compiler column supplied explicitly; without
`--stable-prefix` that column is `not-run` with the reason, never a fabricated success.

```bash
# released Rust 1.100.0 (the official distribution, verified against its published digest)
curl -sSLO https://static.rust-lang.org/dist/rust-1.100.0-x86_64-unknown-linux-gnu.tar.xz
curl -sSLO https://static.rust-lang.org/dist/rust-1.100.0-x86_64-unknown-linux-gnu.tar.xz.sha256
sha256sum -c rust-1.100.0-x86_64-unknown-linux-gnu.tar.xz.sha256
tar -xf rust-1.100.0-x86_64-unknown-linux-gnu.tar.xz
./rust-1.100.0-x86_64-unknown-linux-gnu/install.sh --prefix "$PWD/rust-1.100.0" \
  --components=rustc,cargo,rust-std-x86_64-unknown-linux-gnu --disable-ldconfig

# the matrix, from a clean commit, with artifacts outside the repository
SELECTED_NODE verification/allocator-admission/run.mjs "$OUTSIDE/run" \
  --stable-prefix "$PWD/rust-1.100.0" \
  --stable-tarball rust-1.100.0-x86_64-unknown-linux-gnu.tar.xz \
  --stable-published-sha256 98ee6bec240e1cfe944f17e7092412dc5ef6e008484bb668c6936d35a7d15acd

# the rule pins
bun test verification/allocator-admission/admission.test.mjs
```

The run refuses a dirty tree, refuses to write inside the repository, refuses an unreviewed tool
override and refuses a stale selected configuration, then re-checks that the tree is still clean and
that the prebuilt Aeneas Lean library was not modified. `<OUTSIDE>/run/evidence.json` is the receipt.

## Results

Measured 2026-11-13 on the described pins. `receipt-<commit>.json` beside this file is the machine record
(`cells[].stages`, `controls.results`, `source_stage`, `tools`, `lean_library`, `selection`,
`source.files`).

### Source stage (crate-level)

| Stage | Expected | Observed | Diagnostic |
|---|---|---|---|
| `stable_1_100_ungated` — released 1.100.0, stable variant | `compiled` | **compiled** | — |
| `stable_1_100_gated` — released 1.100.0, extraction variant | `blocked` | **blocked** | `E0554: #![feature] may not be used on the stable release channel` |
| `extraction_nightly_ungated` — pinned extraction nightly, stable variant | `blocked` | **blocked** | 11 × `E0658: use of unstable library feature 'allocator_api'` |
| `extraction_nightly_gated` — pinned extraction nightly, extraction variant | `compiled` | **compiled** | — |

The stable column is the official `rust-1.100.0-x86_64-unknown-linux-gnu` distribution: `rustc 1.100.0
(0ef1bc386 2026-11-09)`, tarball SHA-256 `98ee6bec…5acd` matching the published digest, installed with
`install.sh --components=rustc,cargo,rust-std-x86_64-unknown-linux-gnu`.

### Per-root matrix

| Root | Family | Charon | Aeneas | Lean | Admission | Refusals |
|---|---|---|---|---|---|---|
| `vec_baseline` | implicit-global | extracted | translated | built | **admitted** | — |
| `box_baseline` | implicit-global | extracted | translated | built | **admitted** | — |
| `vec_growth_clone_baseline` | implicit-global | extracted | translated | built | **admitted** | — |
| `global_vec` | explicit-global | extracted | translated | built | refused | `ALLOCATOR-SOURCE`, `ALLOCATOR-CONSTRUCTOR`, `ALLOCATOR-AXIOM` |
| `global_box` | explicit-global | extracted | translated | built | refused | `ALLOCATOR-SOURCE`, `ALLOCATOR-CONSTRUCTOR`, `ALLOCATOR-AXIOM` |
| `generic_vec` | generic-allocator | extracted | translated | built | refused | `ALLOCATOR-SOURCE`, `ALLOCATOR-CONSTRUCTOR`, `ALLOCATOR-AXIOM` |
| `generic_box` | generic-allocator | extracted | translated | built | refused | `ALLOCATOR-SOURCE`, `ALLOCATOR-CONSTRUCTOR`, `ALLOCATOR-AXIOM` |
| `generic_vec_growth_clone` | generic-allocator | extracted | translated | built | refused | `ALLOCATOR-SOURCE`, `ALLOCATOR-CONSTRUCTOR`, `ALLOCATOR-AXIOM` |
| `borrowed_vec` | borrowed-allocator | extracted | translated | built | refused | `ALLOCATOR-SOURCE`, `ALLOCATOR-CONSTRUCTOR`, `ALLOCATOR-AXIOM` |
| `system_vec` | system-allocator | extracted | translated | built | refused | `ALLOCATOR-SOURCE`, `ALLOCATOR-CONSTRUCTOR`, `ALLOCATOR-AXIOM` |
| `stateful_vec` | stateful-allocator | extracted | translated | built | refused | `ALLOCATOR-SOURCE`, `ALLOCATOR-CONSTRUCTOR`, `ALLOCATOR-AXIOM` |
| `stateful_vec_growth_clone` | stateful-allocator | extracted | translated | built | refused | `ALLOCATOR-SOURCE`, `ALLOCATOR-CONSTRUCTOR`, `ALLOCATOR-AXIOM` |

Every root appears in its own translation (`root_in_translation: true` for all twelve) and every cell's
`translated.options` reports `hide_allocator: true` and `treat_box_as_builtin: true`.

**The pipeline discriminates nothing; the admission boundary is the only stage that refuses.** That is the
measured justification for VT-ALLOC-01: a `compiled`, `extracted`, `translated` or `built` cell is not
acceptance, and the positive controls are the three implicit-global roots that pass every stage and are
admitted with no refusal.

### Executed controls

| Control | Required | Observed |
|---|---|---|
| unreached root | `ROOT-MISSING`, `BINDING-ROOT` | refused with exactly those two: a real `box_baseline` extraction cannot stand in for `global_vec` |
| omitted / opaque body | `ROOT-OPAQUE` | refused with `ROOT-OPAQUE` |
| changed allocator identity (Aeneas Lean library revision) | `BINDING-IDENTITY` | refused with `BINDING-IDENTITY` |
| changed source identity | `BINDING-IDENTITY` | refused with `BINDING-IDENTITY` |
| stale receipt | `STALE-RECEIPT` | refused with `STALE-RECEIPT` |
| unchanged receipt revalidated | no refusal | no refusal |

### One diagnosed non-finding

A first fixture revision named the allocator parameter and the stateful local `alloc`. Aeneas emits the
erased allocator as a local and migrates the collection operations into field notation
(`alloc.vec.Vec.push xs input`), so Lean parsed that expression as a projection on the *local* rather than
the `alloc` namespace, and five roots failed elaboration with
`Invalid field notation: Field projection operates on types of the form 'C ...'`. That failure was a
fixture defect, not an allocator property; the parameters are now `custom` and `counter`, and the table
above is the post-rename run. It is recorded here because it would otherwise have been reported as an
allocator finding.
