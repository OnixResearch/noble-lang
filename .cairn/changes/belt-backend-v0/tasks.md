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

- [ ] [serial] Integrator regenerates and reviews shared source-bound evidence, then runs the clean-tree documentation, Cairn, Rust, Octet, and Lean gate set. r[NGRAPH-BELT-04]
- [ ] [serial] Integrator merges this slice and posts the issue result. r[NGRAPH-BELT-04]

## Out of scope

- [x] [serial] No production-default backend, Mill ISA mapping, target scheduler, or hardware/performance claim is selected by #113. r[NGRAPH-BELT-01] r[NGRAPH-BELT-04]
