## Phase 1: Portable belt contract and implementation

- [x] [serial] Define the bounded I64/Bool graph fragment, artifact header and binding, explicit save/load semantics, and refusal behavior. r[NGRAPH-BELT-01] r[NGRAPH-BELT-02] r[NGRAPH-BELT-03]
- [x] [serial] Add the symbolic Lean belt machine, checker relation, and `validate_sound` theorem; register the module and corpus. r[NGRAPH-BELT-03] r[NGRAPH-BELT-04]
- [x] [serial] Add the opt-in Rust lowerer, artifact codec, independent checker, checker-issued byte-borrowing token, and checked interpreter. r[NGRAPH-BELT-01] r[NGRAPH-BELT-02] r[NGRAPH-BELT-03]
- [x] [serial] Exercise wraparound, asymmetric subtraction, equality, fanout, long-lived values, repeated/multiple outputs, capacity boundaries, malformed and mutated artifacts, and unsupported graph refusals in existing targets. r[NGRAPH-BELT-01] r[NGRAPH-BELT-02] r[NGRAPH-BELT-03]
- [x] [serial] Compare actual lowered artifacts with the graph reference, packed `Compiled`, and exact I64 expectations. r[NGRAPH-BELT-04]
- [x] [serial] Add the extracted-source B2 belt comparison against BigInt and the configured ordinary-Wasm path. r[NGRAPH-BELT-04]
- [x] [serial] Add generated Lean corpus freshness coverage and ignored cold/warm cost reporting with storage/preservation counts. r[NGRAPH-BELT-04]
- [x] [serial] Record target-neutral scope, evidence boundaries, and non-claims. r[NGRAPH-BELT-01] r[NGRAPH-BELT-04]

## Phase 2: Independent repository integration

- [x] [serial] Integrator regenerates and reviews shared source-bound evidence, then runs the clean-tree documentation, Cairn, Rust, Octet, and Lean gate set. r[NGRAPH-BELT-04] Evidence (campaign batch 3, 2026-11-13; recorded at closeout 2026-11-20): merge `f745b750` integrated this slice. The integrator renewed the source inventory and reviewed policy bindings (`d94aaeae`), regenerated the foundation claims audit (`ae8bc7f3`) and retained the clean-commit foundation receipt `verification/foundation/receipts/d94aaeae4/` (`ci.sh` overall=0, 1,836 inventoried theorems). On the merged head `fa080467`: `cargo test --workspace --all-targets --all-features` 1,197 passed, 0 failed, 28 ignored; doctests, Clippy `-D warnings` and fmt clean; Octet deny-all and architecture 0 findings; Lean m3 build and gate, foundation build and check all 0; all 16 flake gates `overall=0`. The belt measurement receipt is `verification/belt/receipt.json` (SHA-256 `3f4d3f6accd2edee722a5538210245622560b4cbf24066ceb9bbe295a9984f97`, `77ae0389`). The belt tests under `crates/noble-ngraph/tests/belt_dir/` run in the flake `rust-tests` check, which passed again on `8075d3c0` (campaign batch 18).
- [x] [serial] Integrator merges this slice and posts the issue result. r[NGRAPH-BELT-04] Evidence: merged as `f745b750` (on `main` since 2026-11-13). The campaign batch-3 result, with its acceptance table and the verification above, was posted to #113 on 2026-11-14, and #113 was closed the same day.

## Out of scope

- [x] [serial] No production-default backend, Mill ISA mapping, target scheduler, or hardware/performance claim is selected by #113. r[NGRAPH-BELT-01] r[NGRAPH-BELT-04]
