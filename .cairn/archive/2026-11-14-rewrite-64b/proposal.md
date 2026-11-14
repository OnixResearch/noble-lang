## Why

The v3 receipt shows that the frozen rewrite corpus contains meaningful non-identity and chained candidates, but no optimized candidate executes: dead operand nodes block candidate admission, while the chainless certificate profile cannot bind a later premise to the graph produced by an earlier rewrite. The follow-up must make pruning and intermediate-state identity independently checkable without weakening admission or the frozen acceptance boundary.

## What Changes

- Add an `ngraph-cert/2` transition profile with per-transition before/after graph identities, chained rewrite premises, a final checker-derived unreachable-node prune, and exact decimal-token encoding/decoding. Preserve the existing chainless reader behavior.
- Keep graph admission unchanged. The pruned endpoint is submitted to the same independent admission checker before execution.
- Add Lean model lemmas for ordered output preservation under a survivor mapping and composition of accepted endpoint equivalence, plus a Rust-generated token corpus with an exact freshness test. The model, Rust tests and implementation correspondence remain separate claims.
- Pre-register `rewrite-cost-v4.txt` before the integrated run. Bind it to the unchanged v2 fixture corpus and reuse its 50 us/step ceiling and `no-foldable-sites` residual target. Keep v2/v3 plans and receipts immutable.

## Impact

- **Files**: `crates/noble-ngraph/src/cert.rs`; existing `noble-ngraph` integration-test targets and their modules; `proofs/foundation/NobleFoundation/`; `verification/ngraph/`; `.cairn/changes/rewrite-64b/`.
- **Testing**: focused certificate and cost tests, consumer tests, workspace clippy, formatting, Octet deny-all/architecture gate, pinned Lean proof audit, document checks, and Cairn validation. The v4 measurement is produced only after a clean commit.
