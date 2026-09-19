# M3: Checker coverage of the full bootstrap rule set

## Why

M2 delivered the named finite fragment (`M2 fragment v0`), its extraction, and one proved validation function, but the proof is end-to-end over a single entry point: the coverage record classifies 474 generated functions with exactly **1 proved** (`acceptance.check`), 451 extracted, 17 modeled, and 5 excepted (`verification/m2-coverage.json`; `verification/m2-scope-review.md` item 7).
The acceptance-control audit in the [design](design.md) shows three core-bootstrap checker requirements only partially covered today: B-CHECK-02's explicit rejection of recursive-definition dependencies and user-declared recursive schemas, B-CHECK-05's rejection of cyclic type equations, and B-CHECK-06's eliminator depth (`case` and `list.case` are implemented and unit-controlled but absent from the property pool and the independent oracle; no `Sum<Resource<R>,I64>` payload-exposure control exists).
The bootstrap word table is also incomplete against the language specification: the kernel's 22-entry table omits the `=` word of the bootstrap numeric vocabulary (`crates/noble-kernel/src/contracts/bootstrap/data.rs` `table()`; `.cairn/specs/language/spec.md` section 4.4), and the property generator's 19-word pool omits `-`, `*`, `case`, and `list.case` (`crates/noble-kernel/tests/property/fit.rs` `WORDS`).
The verification-toolchain specification's implementation entry point requires exactly this next step once the M2 slice establishes the pattern: "extend extraction and refinement across the kernel and the remaining compiler/runtime logic" (`specs/VERIFICATION-TOOLCHAIN.md` section 8; `.cairn/specs/verification-toolchain/spec.md`).
This package is the checker/proof workstream's third change; it does not touch the roadmap's parallel Wasm milestone (M3 `wasm-feasibility` in `specs/roadmap.json`).

## What Changes

- Extend `M2 fragment v0` to the named **bootstrap fragment v1** by auditing every B-CHECK-01..07 requirement of `.cairn/specs/core-bootstrap/spec.md` against the M2 records and scoping the partial families in: explicit recursive-definition and recursive-schema rejection (B-CHECK-02), explicit cyclic-substitution/type-equation rejection with well-foundedness accounting (B-CHECK-05), and full branch-eliminator coverage (B-CHECK-06).
- Complete the word scheme table to every bootstrap word — the 22 words fixed by B-SCOPE-01 items 4-5 and the language specification (adding `=`), plus the supplied `test.emit` — with per-word contracts, positive and rejection controls, property generation, and independent oracle arms.
- Deepen proof coverage per rule and per word: per-word scheme refinements, per-rule soundness labels (each fragment rule's checker path proved against the reference judgment), per-word positive acceptance coverage, and termination re-established over fragment v1, growing the proved set beyond the single M2 entry point.
- Keep the M2 proof route unchanged (kernel Rust within the confirmed extraction subset, pinned Charon/Aeneas with `-split-files` externals, computable generated module, Lean reference model, refinement over generated constants) and **extend** the M2 gates — proof gate, coverage gate, refusal matrix, property harness, documentation examples, obligation ledger — rather than duplicating them.

The [spec delta](specs/verification-toolchain/spec.md) adds VT-M3-01 through VT-M3-04 to the existing verification-toolchain capability.
These requirements specialize this change's exit criteria.
The [design](design.md) fixes the fragment v1 definition, the word table plan, the theorem set, and every extended control.
The [implementation tasks](tasks.md) record progress. Planning approval alone does not complete those tasks.

## Dependencies and Scope

Depends on M2 (`m2-checker-feasibility`), which is complete and integrated at `2ffd19a`.
The roadmap's Wasm milestone, M4 and later, and the contract workstream do not depend on this package and are not implemented here.

### In scope

Fragment v1: the v0 node forms, schemes, limits, and outcome domain, plus the scoped-in B-CHECK-02/05/06 families and the complete 23-entry word table (22 language words including `=`, plus `test.emit`).
The extended kernel checker with per-word controls, the extended property harness and independent oracle over the full pool, per-rule and per-word Lean theorems in a new `proofs/m3` root, re-bound extraction, and the extended gates with their refusal controls.

### Out of scope

- Inference, unification, and generalization (K-CHECK-02 follow-up work; candidates still carry total instantiation witnesses).
- The roadmap's Wasm milestone (`BE-*` requirements), runtime execution, recipes, host boundaries, resource adapters, imports, preparation services, contracts, and concurrency.
- Any whole-kernel or stable-core claim: fragment v1 still does not close the inventory (B-IMPL-02, V-GATE-01, V-GATE-05).
- Conformance-ledger scenario execution: the scenario designs linked to B-CHECK-02/05/06 keep their design status; this package's controls are its own named fixtures.

## Impact

**Planning files:** `.cairn/changes/m3-checker-coverage/` records progress and the delta spec.

**Implementation files:** `crates/noble-kernel/` gains the v1 word table entry, the rejection paths, and the extended tests; `crates/noble-kernel/tests/property/` gains the full pool and the eliminator oracle arms; `proofs/m3/` holds the extended reference model and the per-rule/per-word theorem targets; `verification/` gains the v1 fragment definition, probe ladder, coverage record, gates, and evidence records; `nix/` and `policy/` record the extended extraction subject and classification.

**Compatibility:** no Noble syntax, stack semantics, public encoding, or runtime behavior changes.
The candidate schema keeps its `noble-candidate/v0` shape unless a v1 field is required by the scoped-in rejections, in which case the format revision changes and both revisions' controls are retained.
No existing gate is weakened.

**Testing:** package acceptance requires the proposal, design, and tasks gates plus Cairn validation.
Implementation acceptance additionally requires the regenerated Lean module to compile under the pinned backend, the extended reference and theorem targets to build with no proof holes, and every positive and negative control in the design to pass, including the new refusal mutations.

## Completion Contract

This change remains incomplete until every implementation task has retained evidence and every required configuration passes its gates.
The extended theorem set keeps fragment labels (`bootstrap fragment v1`); no result may be presented as whole-language, whole-kernel, Wasm correspondence, or as closing an obligation outside the fragment.
A smaller experiment can report its result without closing the change.
The M2 disclosures that v1 does not close (any remaining `charon::opaque` exceptions, any remaining provenance erasure) are re-disclosed with their evidence, never silently dropped.
