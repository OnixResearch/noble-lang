# M1: Rust/Nix workspace and assurance gates

## Why

Noble has specifications and document checks, but no Rust workspace or compatible verification toolchain.
[M1 in the roadmap](../../../specs/ROADMAP.md) requires this foundation before checker and Wasm feasibility work.
A build-only skeleton cannot satisfy M1 without actual extraction, source accounting, and enforced quality gates.

## What Changes

- Add a Rust workspace with a deterministic semantic kernel and a separate CLI shell.
- Select immutable Nix, Rust, Charon, Aeneas, Lean, Miri, and Octet inputs through compatibility experiments.
- Add complete source and native-boundary inventories with explicit open work and scoped exceptions.
- Add a real Rust-to-Lean extraction smoke test, negative controls, and source-bound evidence.
- Enforce the full Octet catalog, architecture policy, Rust checks, and extraction through local checks and CI.

The [spec delta](specs/verification-toolchain/spec.md) adds VT-M1-01 through VT-M1-07 to the existing verification-toolchain capability.
These requirements specialize the M1 exit criteria. They do not weaken accepted verification or evidence requirements.
The [design](design.md) maps each requirement to implementation work and acceptance controls.
The [implementation tasks](tasks.md) record current progress. Planning approval alone does not complete those tasks.

## Dependencies and Scope

M1 depends on the M0 canonical draft. No other active change is a prerequisite.
M2 and M3 depend on M1, but this package does not implement either milestone.

The first required host configuration is `x86_64-linux` with Rust target `x86_64-unknown-linux-gnu`.
Every compatible feature set and every workspace target belongs to the quality gate.
Additional host support requires an explicit matrix and executed evidence before a support claim.
The [selection evidence](tool-selection-evidence.md) records the immutable tools, scoped compatibility runs, and rejection controls.
The complete M1 assurance matrix remains open.

### In scope

The workspace establishes crate boundaries, a minimal production extraction subject, inventories, policy, build inputs, and required gates.
The extraction smoke test covers actual Noble-owned Rust. Its result does not establish a semantic refinement theorem.
The accepted Rust-to-Lean route remains mandatory for the entire semantic kernel and the target for all production Rust.

### Out of scope

- A language checker, parser, Wasm backend, runtime, or runnable Noble program.
- M2 refinement results, whole-kernel verification, or whole-project verification.
- WIT resources, native async, Syndicate, behavioral contracts, or the calculator.
- A byte-view dependency, a Verus exception, or new shared infrastructure without a concrete compatibility need.

## Impact

**Planning files:** `.cairn/changes/m1-workspace/` records progress. Accepted specs, compatibility views, ledgers, and milestone status stay unchanged.

**Implementation files:** the worktree now contains the Rust workspace, Nix-generated lock, policy, hook, and extraction probe.
The [component review](../../../verification/component-review.md) records the published Octet repairs and the passing incremental workspace gates.
Complete inventories, CI, rejection controls, and independent acceptance remain open.
Existing files do not establish completion of the implementation checklist.

**Compatibility:** no Noble syntax, stack semantics, public encoding, or runtime behavior changes in this package.
Existing Bun document checks and the pinned Cairn runner remain supported.

**Testing:** package acceptance requires proposal, design, and tasks gates, Cairn validation, and the existing document regression suite.
M1 implementation acceptance additionally requires real toolchain runs and all positive and negative controls in the design.

## Completion Contract

M1 remains incomplete until every implementation task has evidence and every required configuration passes its gates.
Unavailable tools, incompatible pins, unsupported required collection, and failed extraction remain blockers.
A smaller named experiment can report its result without closing M1.

Spec sync and archive belong after implementation, not after approval of this planning package.
Document-gate success does not execute Noble, establish compatible pins, or accept a proof.
