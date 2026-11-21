## Why

The archived change [numeric-graph-profile](../../archive/2026-10-16-numeric-graph-profile/design.md) (issue #14) published the `NumericGraph-Design` contract (NGRAPH-SCOPE-01, -F32-01, -EXTRACT-01, -FACT-01, -DERIV-01, -PROOF-01, -SNAP-01, -OWN-01, -BENCH-01). It also ran the Phase B feasibility experiment and the Phase C measurement in the isolated crate `crates/noble-ngraph-experiment/`. Phase C recommended **stop**: on warm W3-L (6,270 operations, 243 params) the compiled CPU graph (L4) takes 9,711 ns per evaluation against 1,271 ns for a straightforward host-native Rust implementation, which is 0.131× its speed. The kept threshold was at least 2× faster than both baselines. W3-S and W3-M reach 0.068× and 0.061×, W4 reaches 0.182×, and W1/W2 extracted from source reach 0.0077× and 0.0066× host-native. A #2 GPU experiment was blocked because no transfer or launch cost was measured or modelled. Admission at W3-L scale took 1.1 ms, within its 100 ms threshold.

On 2026-10-16 the owner overrode the stop recommendation: the profile becomes a supported opt-in feature. Today the profile exists only as experiment code with no stable file formats, no user entry point and no durable snapshot files. Its pure graph code sits in a measurement harness next to process, clock and file effects. r[NGRAPH-SCOPE-01]

## What Changes

- Select the opt-in implementation profile `Numeric-Graph-v1`. It is reachable only through `noble ngraph` and the new pure `noble-ngraph` library, and it leaves `run`, `session`, `compile`, `live`, `calculator`, K-NUM-01 and ordinary Program identity unchanged. B0(b) stays: `F32` graphs are authored in `ngraph-ir/0` and admitted only by the graph checker. Source extraction is `I64`-only, and there is no kernel `F32` type, word or frontend change. r[NGRAPH-SCOPE-01]
- Split the experiment. The IR, checker, diagnostics, facts, D7 `I64` extraction, reference and compiled evaluators, derivative policies, `adam/0` and snapshots move into `crates/noble-ngraph/`.
  - The new crate is a **domain-core**, `#![no_std]` + `alloc` library whose only dependencies are `noble-kernel` and `noble-contracts`. The only dependency rules are `domain-core → domain-core` and `composition-root → domain-core`, and every third-party provider is limited to composition-root callers.
  - Identity digests use an injected hashing scheme that `noble-cli` and the experiment implement with `blake3`, so every identity stays byte-identical.
  - Std-only float functions (`sqrt` in `adam/0`, `trunc`, `powi`) become correctly rounded software routines.
  - `noble-ngraph-experiment` stays the composition-root evidence harness: bins, L1 harness, workloads, the `rustc_apfloat` oracle, benches and the Phase B/C evidence tests. Receipt reproduce commands keep working.
  - No policy rule or provider caller role changes. r[NGRAPH-CLI-01]
- Add `noble ngraph check|extract|eval|train|verify`.
  - Reports are JSON lines with the schema `noble-ngraph-report/v1`, as for core reports.
  - Exit codes follow the core convention: 0 normal, 1 trap, 2 reject or usage, 4 unsupported, 5 exhausted.
  - Budgets are set with explicit flags that default to the D5 values.
  - Output is deterministic, and `F32` output is rendered as its exact binary32 encoding (all NaNs render as one class).
  - `noble --help` lists `ngraph` and the missing `calculator` usage. r[NGRAPH-CLI-01]
- Define versioned on-disk formats:
  - graph files (`ngraph-ir/0`). Today's text form omits the subject, closure and shapes that `GraphLogicalId` hashes, and it encodes `evidence` provenance asymmetrically. The file form records every identity-bearing field, and `parse(print(g))` preserves the identity;
  - vector files (`noble-ngraph-vectors/0`);
  - training-data files (`noble-ngraph-data/0`). r[NGRAPH-FORMAT-01]
- Define immutable snapshot files (`noble-ngraph-snapshot/0`) with recomputed identities, structured provenance and a provenance digest. They are published only as new files. Binding to the wrong graph, layout, fact bindings or pinned identity is refused, and replay from provenance is verified. r[NGRAPH-SNAP-02]
- Close three gaps found while designing the supported surface, all inside the library. The training forward pass enforces input facts, guards and refusing index reads exactly as the reference evaluator does. Training admission checks the loss graph's interface. Replay honours a parent snapshot.

## Non-claims

- **No performance, speedup, GPU or backend-selection claim.** The Phase C stop measurement stands as recorded; the owner's adoption does not reverse it. GPU stays with #2, and alternate CPU backends stay with #4.
- **No proof claim.** NG-PO-A..H stay open and are not added to `specs/verification/obligations.json`. Agreement between evaluators, matching hashes and replay are finite differential evidence only.
- **No kernel or language change.** K-NUM-01, V-NUM-01 and the accepted PO-09/10/11 fragment are untouched. No ordinary Program is reinterpreted, and the ordinary path is unchanged when the profile refuses a Program. r[NGRAPH-SCOPE-01]
- No cross-platform bit-reproducibility claim. Replay is defined on one pinned platform.

## Impact

- **Files (this change):**
  - `.cairn/changes/numeric-graph-v1/{proposal,design,tasks}.md`;
  - a MODIFIED `specs/language/spec.md` delta (NGRAPH-SCOPE-01);
  - ADDED deltas in `specs/developer-experience/spec.md` (NGRAPH-CLI-01, NGRAPH-FORMAT-01) and `specs/evidence/spec.md` (NGRAPH-SNAP-02), each with scenario blocks;
  - the draft unexecuted cases `cases/numeric-graph-cases.json` (NGRAPH-CASE-01..15, all absent/not-run/open/unassessed with empty evidence). They are registered in `specs/conformance/` at sync, because a registered case may cite only canonical requirement IDs.

  This design turn changes no Rust, canonical requirement, registered scenario JSON or ledger. The supporting edits made while archiving `numeric-graph-profile` are part of the preceding promotion, not of this delta: its sync, the `NGRAPH` evidence-route row in `tools/check-specs.mjs` (as for the `CB` prefix), the regenerated views and the receipts README link.
- **Implementation (later, after review):** new crate `crates/noble-ngraph/`, a cutover of `crates/noble-ngraph-experiment/`, the new module `crates/noble-cli/src/ngraph.rs` (with submodules), and tests, `README.md` and `verification/ngraph/README.md`. A separate integrator handles workspace, lock, vendor, policy and Octet renewal; see design "Integration impacts".
- **Testing:** today, Cairn document gates and validation only. The planned library and CLI tests are listed in design "Test plan". None has been run, and none is evidence until it is executed and recorded.
- **Coordination:** #2 (GPU), #4 (alternate CPU backends), #8 (applicability witnesses; `evidence` facts stay refused), #10 (artifact correspondence), #15 (binary32 model), #3/#5 (ownership and allocation). This change designs none of them.
