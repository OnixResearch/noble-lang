## Phase 1: Design package

- [x] [serial] Specify fixed, versioned T1/T2 resource descriptors and their bounds. r[NGRAPH-BELT-TARGET-01]
- [x] [serial] Define issue lanes, unit occupancy and initiation intervals. r[NGRAPH-BELT-TARGET-01] r[NGRAPH-BELT-TARGET-02]
- [x] [serial] Define pre-cycle operand reads, no forwarding, latency-derived completion, deterministic same-cycle completion order, and belt insertion. r[NGRAPH-BELT-TARGET-02]
- [x] [serial] Define input/constant materialization, explicit save/load timing, and ordered final output reads. r[NGRAPH-BELT-TARGET-02]
- [x] [serial] Specify independent legality, semantic-preservation evidence, controls, mutations, and explicit non-claims. r[NGRAPH-BELT-TARGET-03] r[NGRAPH-BELT-TARGET-04]

## Phase 2: Not selected by this issue slice

- [ ] [serial] Implement a target schedule representation and independent checker. r[NGRAPH-BELT-TARGET-01] r[NGRAPH-BELT-TARGET-02] r[NGRAPH-BELT-TARGET-03]
- [ ] [serial] Add a Lean timed-machine model and semantic soundness theorem. r[NGRAPH-BELT-TARGET-03]
- [ ] [serial] Add T1/T2 differential fixtures, mutations, and cost measurements. r[NGRAPH-BELT-TARGET-04]
- [ ] [serial] Claim no implementation or measurement until a separately selected implementation issue completes those tasks. r[NGRAPH-BELT-TARGET-03] r[NGRAPH-BELT-TARGET-04]
