## Phase 0: design (this change)

- [x] [serial] Archive `numeric-graph-profile` after the owner's adoption decision (sync and archive, `CAIRN_ARCHIVE_DATE=2026-10-16`). Record the selected `Numeric-Graph-v1` boundary, crate split, formats, CLI, outcome mapping, test plan, integration impacts and open questions in `design.md`, with the MODIFIED NGRAPH-SCOPE-01 and ADDED NGRAPH-CLI-01, NGRAPH-FORMAT-01 and NGRAPH-SNAP-02 deltas and the draft cases `cases/numeric-graph-cases.json`. Evidence: Cairn proposal, design and tasks gates and `validate` only. These are document checks, not implementation, execution or proof evidence. r[NGRAPH-SCOPE-01] r[NGRAPH-CLI-01] r[NGRAPH-FORMAT-01] r[NGRAPH-SNAP-02]
- [x] [serial] Review of design D1–D13 and answers to Q1–Q8, before any Rust change. Evidence: the director reviewed the design on 2026-10-16 under the owner’s 2026-10-16 adoption instruction (design “Review”; the owner may amend). Q1 yes; Q2–Q7 as recommended; Q8 accepted. r[NGRAPH-SCOPE-01]

## Phase 1: identity record and library crate

- [x] [serial] At base `66eb7cd`, before any move, record the pre-cutover identities to `crates/noble-ngraph-experiment/tests/fixtures/identities-66eb7cd.txt` (design D3). Evidence: commit `fc67e75`; a throwaway recorder test in a detached worktree at `66eb7cd` (removed afterwards), release profile, generation method in the file header. Its F1 id equals the receipt `6065e93f…d661`. r[NGRAPH-SNAP-02]
- [x] [serial] Create `crates/noble-ngraph/`: `#![no_std]` plus `alloc`, role domain-core, depending only on `noble-kernel` and `noble-contracts`. Move the modules in the D2 map with `IdentityScheme` injection, `sem::soft` routines, `Cell` counters, `Rc` and `TOOL_REVISION`. Move `ir_check.rs` and `extract_b1.rs` and the `sem`/`prng` unit tests. Evidence: commits `27dd22c` (git mv) and `bc34ed0`; `cargo tree -p noble-ngraph -e normal` shows only `noble-contracts` and `noble-kernel` (see the closing run below); `soft_float` (default tier and the ignored 10⁷ tier, release, passed in 0.44 s). r[NGRAPH-CLI-01] r[NGRAPH-SCOPE-01]
- [x] [serial] Implement `format::graph` (D4: subject and shape records, hex evidence, required `derived` rule, canonical print) and `format::{value, vectors, data}` (D5). Evidence: `bc34ed0`; tests `format_graph` and `format_values` (`17d197d`), experiment `format_roundtrip` over the whole identity corpus. r[NGRAPH-FORMAT-01]
- [x] [serial] Implement snapshot files (D6: structured provenance, provenance digest, integrity, fact-binding and pinned-id binding) and the four feature codes with the D8 outcome class in `diag`. Evidence: `bc34ed0`, `17d197d`; test `snapshot_file` re-implements the D6 provenance encoding and reproduces the published digest. r[NGRAPH-SNAP-02] r[NGRAPH-CLI-01]
- [x] [serial] Close the D10 gaps: forward-pass fact and guard enforcement in training, loss-graph interface admission and parent-aware replay. Evidence: `bc34ed0`; test `train_admission` (refused points stop training unpublished and agree with the reference evaluator; loss interface; budgets; replay from the parent, `E-SNAPSHOT-STALE`, `E-SNAPSHOT-REPLAY`). r[NGRAPH-SNAP-02]

## Phase 2: experiment cutover

- [x] [serial] Retarget `noble-ngraph-experiment` to `noble_ngraph::…` with a `Blake3Scheme`, with no re-exports. Move `extract/controls.rs` and the corner sampler into the harness, and set the bins' `change` field to the archive path. Add `identity_agreement.rs` and `format_roundtrip.rs`. Update the `verification/ngraph/README.md` reproduce commands. Evidence: `bc34ed0`, `17d197d`; `identity_agreement` reproduces all 31 recorded graph ids and fact bindings, the B2 observations, the B3 codes, 8 campaigns and the B5 identities; `ngraph-phase-b --out <tmp>` at `bc34ed0` reported `result: passed` (8/8 records, B5 final `075f58ca…7f9d`). r[NGRAPH-FORMAT-01]

## Phase 3: CLI

- [x] [serial] Add `crates/noble-cli/src/ngraph.rs` and `ngraph/{arguments,files,report}.rs`, the `ngraph` dispatch arm, and `--help` listing `ngraph` and `calculator`. Implement `check`, `extract`, `eval`, `train` and `verify` (D7), with `noble-ngraph-report/v1` reports, the D8 exit mapping, the D9 budget flags and host bounds, bounded reads and create-new plus hard-link publication. Evidence: `e281173`; the black-box suite below and a manual smoke of every subcommand with real files. r[NGRAPH-CLI-01] r[NGRAPH-SNAP-02]

## Phase 4: tests

- [x] [serial] Add the library tests `format_graph`, `format_values`, `snapshot_file`, `train_admission` and `soft_float`, and the experiment tests `identity_agreement` and `format_roundtrip` (NGRAPH-CASE-02, -06, -09, -14). Evidence: `17d197d`. r[NGRAPH-FORMAT-01] r[NGRAPH-SNAP-02]
- [x] [serial] Add `crates/noble-cli/tests/ngraph.rs` covering NGRAPH-CASE-03..05, -07, -08 and -10..13 through `CARGO_BIN_EXE_noble`, with no Node. Evidence: the independently written suite (`feat/ngraph-cli-tests` `429da01`, merged in `5f13837`) passed 14/14 unchanged; `5373615` adds the CASE-12/13 forgeries (independent D6 encoding) and the recorded B5 identities through the CLI: 16/16. r[NGRAPH-CLI-01] r[NGRAPH-SNAP-02]
- [x] [serial] Run `cargo test -p noble-ngraph -p noble-ngraph-experiment -p noble-cli` (the new CLI test target), clippy `-D warnings` and rustfmt once all of the above lands. Evidence (at `5373615` plus documentation only): `cargo test -p noble-ngraph -p noble-ngraph-experiment -p noble-cli` (debug) 180 passed, 0 failed, 3 ignored (the Node L2 `extract_b2` test, the 10⁷ `soft_float` tier and one existing `noble-cli` unit test), including `tests/ngraph.rs` 16/16; `cargo clippy -p noble-ngraph -p noble-ngraph-experiment -p noble-cli --all-targets -- -D warnings` clean; `cargo fmt -p noble-ngraph -p noble-ngraph-experiment -p noble-cli -- --check` clean; `cargo tree -p noble-ngraph -e normal,dev,build` lists only `noble-contracts` and `noble-kernel`. The Octet, tigerstyle, native-assurance and Nix gates were not run (Phase 6). r[NGRAPH-CLI-01]

## Phase 5: documentation

- [x] [serial] Add the `README.md` section "Experimental numeric graphs (opt-in; no performance claim)" after the calculator section, plus a "What works today" row with the Phase C numbers and non-claims (NGRAPH-CASE-15). Evidence: drafted on `feat/ngraph-cli-tests` (`8dc399d`, merged in `5f13837`), corrected against the implementation, with the example outputs pasted from real runs of the debug binary. r[NGRAPH-SCOPE-01]

## Phase 6: integration (separate integrator)

- [ ] [serial] Workspace member, `Cargo.lock`, vendor re-pin and tool-selection hashes. Policy: the package, source and test roles for `noble-ngraph` with no rule or provider change, and the filesystem and observability rationales. Native-assurance and source-inventory renewal; Octet unit, shard and call capacity from a complete collection; Q1 (approved): add `noble-ngraph` to `core_no_std` and to `nix/octet-controls.nix` `requiredCorePackages`, `requiredPackages` and `requiredScopes` as a tightening. Evidence: not run yet. r[NGRAPH-CLI-01]

## Phase 7: lifecycle

- [ ] [serial] Before sync, move `cases/numeric-graph-cases.json` to `specs/conformance/numeric-graph-cases.json`, register it in `specs/spec-family.json` `scenario_files`, align the delta scenario blocks and links to the generated canonical form, and preview sync. Leave every case absent/not-run until it is actually executed. Evidence: not run yet. r[NGRAPH-CLI-01] r[NGRAPH-FORMAT-01] r[NGRAPH-SNAP-02]
- [ ] [serial] After the executed cases are recorded with real evidence, run the pinned Cairn gates, `validate`, the view and ledger refresh and the spec tests, then sync and archive with `CAIRN_ARCHIVE_DATE`. Evidence: not run yet. r[NGRAPH-SCOPE-01]
