# Core Specification Amendment — Safety

Document: AMEND-S001  
Revision: 0.1.0-draft.1  
Date: 2026-09-12  
Applies to: SPEC-0001 0.1.0-draft.1 + verification extension 0.1.0-draft.1  
Status: Proposed additive integration; Project sources have not been overwritten

## 1. Purpose

This amendment makes the project's general safety policy normative. It does not claim that the compiler/runtime already satisfies the requirements.

The amendment adds:

- no guest `unsafe` escape hatch;
- no Noble-language undefined-behavior outcome;
- no ordinary raw machine pointers/addresses or unchecked representation casts;
- explicit hostile-input validation at FFI/Wasm/wire/host boundaries;
- non-forgeable move-only authority/resource rules;
- a standard-concurrency safety requirement based on isolation rather than shared mutable guest memory;
- explicit safety theorem/implementation obligations and a new release gate;
- clarification that advanced type-system features remain desired/possible if they preserve the safety contract.

## 2. Surface-name reconciliation

The generated previews use the selected language names `run` and `reflect`. The supplied baseline/integration files still spell these operations `call` and `reify`. This package does not claim that the original Project sources were overwritten; the rename is represented only in the generated working copies and patch.

## 3. Integration files

- `SAFETY.md` — normative safety contract (SPEC-S001)
- `SAFETY-DECISIONS.md` — selected decisions and open work (DEC-S001)
- `SPEC-0001-safety-preview.md` — verification-integrated core working copy plus safety requirements and selected surface renames
- `VERIFICATION-safety-preview.md` — verification working copy with SO-01..SO-09
- `patches/SPEC-0001-safety.patch` — exact diff from the supplied verification integration preview
- `patches/VERIFICATION-safety.patch` — exact diff from the supplied VERIFICATION.md
- `conformance/safety-cases.json` — expected/not-run safety scenarios
- `verification/safety-obligations.json` — open safety proof obligations

## 4. Integration rule

For a newer branch, merge the safety requirements semantically rather than replacing newer syntax, concurrency, or host-profile work wholesale. In particular, preserve the current `#` comment direction and the Syndicate/Synit/Preserves concurrency decision.

## 5. Status discipline

The package defines requirements and expected scenarios only. It reports no executed Noble compiler/runtime tests and no completed Lean/Aeneas/Verus/backend safety proof.
