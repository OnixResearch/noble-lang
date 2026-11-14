## Phase 1: Certificate implementation, independent checks and model

- [x] [serial] Record D1–D3, preserve the `ngraph-cert/0` contract, implement checked dense dead-node pruning with exact ordered-output preservation, and add adversarial Rust tests for live/output/guard/numbering/order boundaries. r[NGRAPH-PROOF-01]
- [x] [serial] Implement the `ngraph-cert/2` transition codec and checker; prove stale/intermediate graph, chain, malformed token, resource bound and endpoint refusals, and test chained, identity, reordered/repeated-output candidates. r[NGRAPH-PROOF-01]
- [x] [serial] Extend the Lean rewrite model with prune/output-map and accepted-chain endpoint lemmas; retain a Rust-generated exact-token Lean corpus and freshness test. State model, implementation test and correspondence status separately. r[NGRAPH-PROOF-01]

## Phase 2: Pre-registration and integrated measurement

- [x] [serial] Commit `verification/ngraph/rewrite-cost-v4.txt` before timing; bind the frozen v2 corpus by digest and reuse its cost threshold and residual target unchanged. Preserve v2/v3 bytes. r[NGRAPH-PROOF-01]
- [x] [serial] Produce the v4 receipt from a clean implementation commit. Reconcile every source, candidate admission and execution row; report non-identity execution count, residual-target attainment, break-even, cached/uncached cost, and GO/NO-GO without filtering refusals. r[NGRAPH-PROOF-01]

## Phase 3: Documentation, verification and closeout

- [x] [serial] Update the rewrite README and receipt index with measured v4 results; validate the active Cairn change and complete the unit evidence ledger. r[NGRAPH-PROOF-01]
- [x] [serial] Pass noble-ngraph and consumer tests, workspace clippy, fmt, `octet.sh`, the Lean audit and document checks. Commit the completed source and evidence, leaving v2/v3 plans and receipts byte-identical. r[NGRAPH-PROOF-01]
