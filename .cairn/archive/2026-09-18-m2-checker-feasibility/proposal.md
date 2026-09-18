# M2: Checker feasibility and the first acceptance refinement

## Why

M1 delivered the Rust workspace, the pinned Charon → Aeneas → Lean route, and a bounded extraction entry point, but the repository has no type checker and no proof.
[M2 in the roadmap](../../../specs/ROADMAP.md) is the required first checker/proof feasibility step: candidate schema, finite rules, actual extraction, one nontrivial refinement, and failing incomplete-coverage checks.
Every later proof obligation in `specs/verification/obligations.json` depends on this boundary existing: the acceptance checker is the subject of PO-09, PO-10, and PO-11.

## What Changes

- Define a versioned finite candidate schema and finite acceptance rules for a named Core-Bootstrap checking fragment (literal, resolved invocation, and quotation nodes with finite stack-tail constraints).
- Implement the acceptance representation and a nontrivial validation function in `crates/noble-kernel` with domain-specific types that separate untrusted candidates from accepted programs.
- Extract the representation and the validation function through the mandatory route and bind the generated module to the actual source and inventory identities, extending the M1 extraction record outside the smoke function.
- Add the Lean reference model for the fragment and the first refinement theorems in the required `check(candidate, expected_interface, environment) = Accepted(checked) ⟹ WellFormed(checked) ∧ TypingDerivation(checked, expected_interface)` shape (V-CHECK-03), separating reference soundness (PO-09), termination and positive acceptance coverage (PO-10), and the extracted-function refinement (PO-11).
- Extend the inventory, coverage, and proof-required gates so that a missing function, unsupported kernel body, unexplained external model, unfinished required refinement, or unexecuted required coverage case fails, with extracted, proved, modeled, excepted, and open counts reported separately.
- Provide the M2 developer-experience controls: stack and branch diagnostics for the fragment (B-DIAG-01, DX-DIAG-01), bounded property-harness controls (DX-PROPERTY-01/02), and static executable-documentation checks (DX-DOC-01/02).

The [spec delta](specs/verification-toolchain/spec.md) adds VT-M2-01 through VT-M2-04 to the existing verification-toolchain capability.
These requirements specialize the M2 exit criteria and trace to the requirements assigned to M2 in `specs/roadmap.json`.
The [design](design.md) fixes the named fragment, the schema, the reference model, and every acceptance control.
The [implementation tasks](tasks.md) record progress. Planning approval alone does not complete those tasks.

## Dependencies and Scope

M2 depends on M1, which is complete and integrated. M3 and MC1 depend on M2; this package implements neither.

### In scope

The named fragment: literal, invocation, and quotation nodes; rank-1 scheme instantiation witnesses; stack joins with finite stack-tail constraints; effect bounds and inclusion; recursive eligibility with resource-bearing negative fixtures; branch joins; finite limits; five-way outcomes; fragment diagnostics.
The actual acceptance representation and one nontrivial validation function extracted to Lean with bound identities.
The Lean reference model for the fragment and the first refinement theorems with fragment-scoped labels.
Gate extension: coverage statuses, proof-required rejection controls, property harness, static documentation checks.

### Out of scope

- The complete inference/unification algorithm and generalization (K-CHECK-02 follow-up work; candidates are supplied, not inferred).
- Recursive definitions, imports, preparation services, live resources, contracts, concurrency, and the Wasm backend.
- Evaluation semantics, runtime execution, recipe normalization proofs, and Wasm correspondence (PO-15 through PO-21, PO-17/18, SO-07 remain open).
- Any whole-kernel or stable-core claim: one proved validation function does not close the inventory (B-IMPL-02, V-GATE-01, V-GATE-05).

## Impact

**Planning files:** `.cairn/changes/m2-checker-feasibility/` records progress and the delta spec.

**Implementation files:** `crates/noble-kernel/` gains the checker representation and validation functions; `crates/noble-cli/` gains the acceptance harness; `nix/` gains the extended extraction subject and the M2 gate wiring; `proofs/m2/` holds the reference model and refinement targets; `policy/` and `verification/` record the updated classification, coverage, and evidence; `specs/` records the exercised scenario expectations once executed.

**Compatibility:** no Noble syntax, stack semantics, public encoding, or runtime behavior changes.
Ordinary execution is unaffected; the checker is additive and no existing gate is weakened.

**Testing:** package acceptance requires the proposal, design, and tasks gates, Cairn validation, and the existing document regression suite.
M2 implementation acceptance additionally requires the generated Lean module to compile, the reference and refinement targets to build under the pinned toolchain, and every positive and negative control in the design to pass.

## Completion Contract

M2 remains incomplete until every implementation task has retained evidence and every required configuration passes its gates.
Extraction success, test success, and document checks do not substitute for a required refinement proof (V-GATE-05).
The refinement theorems keep their exact fragment labels; no result may be presented as whole-language, whole-kernel, or Wasm correspondence.
A smaller named experiment can report its result without closing M2.

Spec sync and archive belong after implementation acceptance, not after approval of this planning package.
