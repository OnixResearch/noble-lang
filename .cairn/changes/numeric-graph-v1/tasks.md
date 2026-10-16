## Phase 0: design (this change)

- [x] [serial] Archive `numeric-graph-profile` after the owner's adoption decision (sync and archive, `CAIRN_ARCHIVE_DATE=2026-10-16`). Record the selected `Numeric-Graph-v1` boundary, crate split, formats, CLI, outcome mapping, test plan, integration impacts and open questions in `design.md`, with the MODIFIED NGRAPH-SCOPE-01 and ADDED NGRAPH-CLI-01, NGRAPH-FORMAT-01 and NGRAPH-SNAP-02 deltas and the draft cases `cases/numeric-graph-cases.json`. Evidence: Cairn proposal, design and tasks gates and `validate` only. These are document checks, not implementation, execution or proof evidence. r[NGRAPH-SCOPE-01] r[NGRAPH-CLI-01] r[NGRAPH-FORMAT-01] r[NGRAPH-SNAP-02]
- [ ] [serial] Owner review of design D1–D13 and answers to Q1–Q7, before any Rust change. Evidence: not run yet. r[NGRAPH-SCOPE-01]

## Phase 1: identity record and library crate

- [ ] [serial] At base `66eb7cd`, before any move, record the pre-cutover identities to `crates/noble-ngraph-experiment/tests/fixtures/identities-66eb7cd.txt` (design D3). Evidence: not run yet. r[NGRAPH-SNAP-02]
- [ ] [serial] Create `crates/noble-ngraph/`: `#![no_std]` plus `alloc`, role domain-core, depending only on `noble-kernel` and `noble-contracts`. Move the modules in the D2 map with `IdentityScheme` injection, `sem::soft` routines, `Cell` counters, `Rc` and `TOOL_REVISION`. Move `ir_check.rs` and `extract_b1.rs` and the `sem`/`prng` unit tests. Evidence: not run yet. r[NGRAPH-CLI-01] r[NGRAPH-SCOPE-01]
- [ ] [serial] Implement `format::graph` (D4: subject and shape records, hex evidence, required `derived` rule, canonical print) and `format::{value, vectors, data}` (D5). Evidence: not run yet. r[NGRAPH-FORMAT-01]
- [ ] [serial] Implement snapshot files (D6: structured provenance, provenance digest, integrity, fact-binding and pinned-id binding) and the four feature codes with the D8 outcome class in `diag`. Evidence: not run yet. r[NGRAPH-SNAP-02] r[NGRAPH-CLI-01]
- [ ] [serial] Close the D10 gaps: forward-pass fact and guard enforcement in training, loss-graph interface admission and parent-aware replay. Evidence: not run yet. r[NGRAPH-SNAP-02]

## Phase 2: experiment cutover

- [ ] [serial] Retarget `noble-ngraph-experiment` to `noble_ngraph::…` with a `Blake3Scheme`, with no re-exports. Move `extract/controls.rs` and the corner sampler into the harness, and set the bins' `change` field to the archive path. Add `identity_agreement.rs` and `format_roundtrip.rs`. Update the `verification/ngraph/README.md` reproduce commands. Evidence: not run yet. r[NGRAPH-FORMAT-01]

## Phase 3: CLI

- [ ] [serial] Add `crates/noble-cli/src/ngraph.rs` and `ngraph/{arguments,files,report}.rs`, the `ngraph` dispatch arm, and `--help` listing `ngraph` and `calculator`. Implement `check`, `extract`, `eval`, `train` and `verify` (D7), with `noble-ngraph-report/v1` reports, the D8 exit mapping, the D9 budget flags and host bounds, bounded reads and create-new plus hard-link publication. Evidence: not run yet. r[NGRAPH-CLI-01] r[NGRAPH-SNAP-02]

## Phase 4: tests

- [ ] [serial] Add the library tests `format_graph`, `format_values`, `snapshot_file`, `train_admission` and `soft_float`, and the experiment tests `identity_agreement` and `format_roundtrip` (NGRAPH-CASE-02, -06, -09, -14). Evidence: not run yet. r[NGRAPH-FORMAT-01] r[NGRAPH-SNAP-02]
- [ ] [serial] Add `crates/noble-cli/tests/ngraph.rs` covering NGRAPH-CASE-03..05, -07, -08 and -10..13 through `CARGO_BIN_EXE_noble`, with no Node. Evidence: not run yet. r[NGRAPH-CLI-01] r[NGRAPH-SNAP-02]
- [ ] [serial] Run `cargo test -p noble-ngraph -p noble-ngraph-experiment -p noble-cli` (the new CLI test target), clippy `-D warnings` and rustfmt once all of the above lands. Evidence: not run yet. r[NGRAPH-CLI-01]

## Phase 5: documentation

- [ ] [serial] Add the `README.md` section "Experimental numeric graphs (opt-in; no performance claim)" after the calculator section, plus a "What works today" row with the Phase C numbers and non-claims (NGRAPH-CASE-15). Evidence: not run yet. r[NGRAPH-SCOPE-01]

## Phase 6: integration (separate integrator)

- [ ] [serial] Workspace member, `Cargo.lock`, vendor re-pin and tool-selection hashes. Policy: the package, source and test roles for `noble-ngraph` with no rule or provider change, and the filesystem and observability rationales. Native-assurance and source-inventory renewal; Octet unit, shard and call capacity from a complete collection; `core_no_std` and `nix/octet-controls.nix` only if Q1 is approved. Evidence: not run yet. r[NGRAPH-CLI-01]

## Phase 7: lifecycle

- [ ] [serial] Before sync, move `cases/numeric-graph-cases.json` to `specs/conformance/numeric-graph-cases.json`, register it in `specs/spec-family.json` `scenario_files`, align the delta scenario blocks and links to the generated canonical form, and preview sync. Leave every case absent/not-run until it is actually executed. Evidence: not run yet. r[NGRAPH-CLI-01] r[NGRAPH-FORMAT-01] r[NGRAPH-SNAP-02]
- [ ] [serial] After the executed cases are recorded with real evidence, run the pinned Cairn gates, `validate`, the view and ledger refresh and the spec tests, then sync and archive with `CAIRN_ARCHIVE_DATE`. Evidence: not run yet. r[NGRAPH-SCOPE-01]
