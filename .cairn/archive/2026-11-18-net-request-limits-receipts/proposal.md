## Why

The opt-in revision-3 interaction-net path (`noble ngraph eval GRAPH VECTORS --net`, `docs/i64-net-contract-v3.md`) is the producer that OnixResearch/lattice#14 (pure inet worker profile) and lattice#22 (semantic interaction counts) want to qualify. Lattice's producer-readiness record (`lattice/.cairn/changes/qualify-pure-inet-worker-profile/evidence/producer-readiness.json`, 2026-11-14) lists the gaps that block it:

- `net::Limits` bounds only agent slots, wires and interaction fuel. `Report.retained_bytes` estimates the initial net and is not an enforced ceiling, and the CLI buffers its report in a growing `Vec` with no output-construction bound.
- `eval_net` reports interactions but names no contract, lowering or rule-table identity, no theorem behind the count, and no implementation-to-counter correspondence. Its count is therefore only observed.
- The report does not identify the producer build. A consumer cannot bind it to an executable.

Issues #37 and #124 closed with the model theorems `NobleFoundation.NetI64.i64_normal_form_unique`, `i64_equal_interaction_count` and `Certificate.accepted_exact_count`. They left the native boundary open. This change does not close that boundary. It adds the enforced ceilings, the executed per-run counter check and the report fields that a consumer needs to bind the exact claim.

## What Changes

- **Whole-request ceilings on `eval --net`.** `--max-input-bytes` (default 134,217,728) charges the graph and vector files before each read. `--max-retained-bytes` (default 268,435,456, the new `net::Limits::max_retained_bytes`) charges every lowering, validation, arena, output-boundary, worklist and read-back buffer before allocation or growth, with checked arithmetic. `--max-output-bytes` (default 67,108,864) charges each report line's exact encoded length before its input-proportional values are rendered. Each flag needs `--net` and may only lower its default. Exceeding a ceiling produces one `exhausted` line, `E-NET-BUDGET-input-bytes`, `-retained-bytes` or `-output-bytes` (exit 5), with `limit`, `cap` and `observed`, in place of the whole report.
- **Executed census check.** `net::reduce` reports `Normal` only when the reducer's interaction counter equals the validated net's census `2·#ops + #dups + #eras` (`net::census_check`). A mismatch is `InternalInvalid(InteractionCensus)`. Every report carries `expected_interactions`, and normal rows add `count_basis: "census-checked"`.
- **Receipt fields.** The `--net` header names the producer (package, version, build-declared source revision or `unavailable`, `executable_digest: null`). It also names the contract, lowering, rule-table, plan, checker and executor revisions, the schedule, storage policy and identity scheme, and the effective limits and input bytes. Finally it carries a `semantic_count` binding: unit, counter, boundary, census, count basis, the three fully qualified theorems over `NobleFoundation.NetI64.system`, the pinned proof sources and foundation receipt identities, and an explicit model-only boundary statement.
- **Build identity.** The flake's `noble` package declares `NOBLE_SOURCE_REVISION = self.rev or ""`. Cargo builds report the revision `unavailable`.
- **Tests and documentation.** Library and CLI tests cover each ceiling below, at and above its observed requirement, the output-refusal-instead-of-truncation rule, the census mismatch control, the receipt fields and the proof-identity drift check. `docs/i64-net-contract-v3.md` §9 documents the contract, and the README points to it.

## Non-claims

- **Not a refinement proof.** `census-checked` records one executed equality between two numbers of one run. It is not a proof that the Rust lowering, reducer or read-back implements the NetI64 system. The named theorems are model statements (`assurance: "model-only"`).
- **Not an RSS bound.** The byte ceilings are accounted ceilings over `size_of` × capacity of reducer-owned buffers. Allocator metadata, fragmentation, stack frames, decoded graph/vector rows and the hasher state fall outside them.
- **No executable digest.** The report names no digest of the running binary. A consumer binds an executable by its own measurement.
- No change to the NO-GO adoption decision, to ordinary evaluation, to the Lean proofs or to revision-3 semantics.

## Impact

- **Files:** this change package; `crates/noble-ngraph/src/net.rs`; `crates/noble-cli/src/ngraph.rs` and `ngraph/{net,files,report}.rs`; `crates/noble-cli/src/workflow/encoding.rs`; `crates/noble-cli/tests/ngraph.rs` and `tests/ngraph/{receipt,resource}.rs`; `flake.nix` with its re-pinned `policy/tool-selection{-files.ncl,.json}` binding; the regenerated header of `proofs/foundation/NobleFoundation/NetI64PlanCorpus.lean`; the renewed `policy/source-inventory.{ncl,json}`; `docs/i64-net-contract-v3.md`; `README.md`.
- **Requirements:** ADDED `NGRAPH-NET-LIMITS-01` and `NGRAPH-NET-RECEIPT-01` in `specs/developer-experience/spec.md`.
- **Testing:** `noble-ngraph` and `noble-cli` tests, workspace tests, Clippy and fmt, Octet deny-all, source-inventory renewal, source coverage, native assurance, the Cairn gates and `nix flake check`. Observed results are recorded in `tasks.md`.
- **Consumers:** lattice#14 and lattice#22 may consume the new fields and refusals. This change does not qualify a Lattice worker profile.
