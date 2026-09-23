## Why

MC1 exports pure contracts and checks exact Lean declarations; M4 executes ordinary Core-Bootstrap programs. Neither milestone provides first-class checked companions, runtime family instantiation, applicability-checked invocation or proof-required artifact release. MC2 connects those paths without moving proof search into ordinary execution.

## What Changes

- Add immutable Contract, Evidence and Certified values to the kernel, selected Wasm representation and live engine, including transport through arguments, results and aggregates.
- Add a bounded deterministic companion core for exact evidence admission, finite replay, subject binding, composition, runtime captures, installed guards and release decisions.
- Keep independent Lean execution in the CLI shell: an opaque admission request crosses to a complete checked observation, and the core checks exact statement/source/class/context binding before minting evidence.
- Add `noble companions` streaming sessions and `noble build --require-proof`, retaining the actual compiled subject and explicit trusted-build correspondence.
- Execute all fifteen canonical CONTRACT cases and every declared hostile variant, with frozen executable bytes, actual independent Lean controls and complete source-bound extraction/quality evidence.

## Impact

- **Files**: `crates/noble-kernel`, `crates/noble-contracts/src/companion`, `crates/noble-wasm`, `crates/noble-cli`, `proofs/mc2`, the renewed inherited extraction, `verification/mc2`, source/architecture inventories and supporting status/proof ledgers.
- **Testing**: complete workspace tests; actual CONTRACT-01 through CONTRACT-15 and eighteen independent-checker/core controls; current Rust-to-Charon-to-Aeneas-to-Lean extraction and compiled dependency/axiom audits; full unchanged 73-rule deny-all, all-target architecture/source coverage, boundary controls and specification validation.
- **Non-goals**: universal parser/compiler/backend refinement, proof of arbitrary host observation authenticity, termination guarantees, host authority, live-resource protocols, components, concurrency or calculator milestones. Broad proof obligations remain open beyond the explicitly accepted fragments.
