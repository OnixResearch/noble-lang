# Runbook and workspace evidence

## Task 1.4: command contract

The [runbook](../../../verification/m1-runbook.md) names the available extraction, architecture, metadata, and scoped Miri procedures.
It reserves exact inventory, integrated extraction, native, control-harness, and acceptance interfaces for later tasks.
The [matrix](../../../verification/m1-controls.md) covers all eight control families from the design.
Each row separates observed, historical, and planned work. r[VT-M1-01] r[VT-M1-06] r[VT-M1-07]

Nix introspection confirmed that all five reserved apps and five additional check names are absent.
No successful placeholder or unavailable-command rejection entered the acceptance evidence.
Collection precedes extraction. Classification comparison follows extraction and uses independent expected inputs.
This command contract does not implement those comparators or freeze their future Rust DTO schemas.

The fresh-directory rehearsal restored all ten Git dependencies from the selected Lake manifest.
The dependency helper checked their revisions. All three copied input files remained byte-identical to the workspace configuration.
The cache phase restored 8,538 previously downloaded upstream artifacts into the new proof tree.
That cache reuse is an explicit assumption, not an independent proof audit or a fully cold download claim.

The actual workspace kernel passed fresh Charon/Aeneas extraction, a Lake build, and explicit Lean compilation into a new object.
The generated source copy matched the translator output. Backend Git sources matched after compilation.
The selected Miri driver passed both budget tests with strict provenance and a new sysroot cache.
The runbook retains tool-bootstrap, cache, extraction, compilation, and interpreted-test outcomes separately.

## Document-check scope

The document validator previously ignored links under `verification/` and `proofs/`.
Its loader also read ignored `.pi`, `.octet`, and `.lake` trees.
Three red regressions reproduced these gaps before the repair.

The pure link check and report input list now include the runbooks.
The loader excludes private evidence and compiler caches from document discovery.
All three regressions pass. The complete Bun regression suite now has 100 passing tests.
The conversion and document self-test counts remain 26 and 226.
This document scope is not the compiler-derived production source inventory.

## Task 2.1: existing kernel and CLI implementation

The completed phase-one prerequisites permit review of the existing workspace implementation. r[VT-M1-02]

- Cargo metadata contains exactly the kernel and CLI packages.
- The CLI depends on the kernel. The kernel declares no Cargo dependencies or features.
- The kernel uses `no_std` and inherits the workspace's unsafe-code prohibition.
- `consume_budget` uses `checked_sub(1)` and returns `Remaining` or `Exhausted` from explicit input.
- Tests cover zero, one, two, 42, and `u32::MAX`. The CLI retains its internal smoke route, not a Noble evaluator.

The manifests and Rust sources are unchanged from the previously checked selection tree.
The current Nix run passes all four workspace tests, formatting, strict Clippy, and the complete pinned deny-all/architecture gate.
This closes task 2.1, not task 2.2's rejection controls or the complete source/native inventory.
An active `no_std` attribute alone does not establish purity or transitive exclusion of `std`.

## Verification and retained failures

The primary repository retains the complete record under `.pi/m1-runbook/`.
The record includes commands, source archives, manifests, generated Lean, raw compiler observations, and BLAKE3 ledgers.
The fresh proof tree and its caches remain outside the disposable worktree.

All ten current Nix checks pass together.
The published pre-commit hook and direct full-scope Octet gate also pass, with valid artifact replay.
No target, policy declaration, required obligation, provider revision, tool-selection input, or Rust production body changed.

An initial metadata query failed inside an inherited compiler wrapper because the selected Rust directory was absent from `PATH`.
The retained retry supplies the selected directory and disables the inherited wrappers for that query.
The documented Nix-shell form also passes with private Cargo directories.
Cargo metadata establishes package edges, not compiler body coverage.

Task 1.4 closes command planning only. Task 2.1 closes the implemented workspace/budget slice only.
M1 acceptance, boundary rejection controls, source/native inventories, integrated control commands, CI, refinement, sync, and archive remain open.
