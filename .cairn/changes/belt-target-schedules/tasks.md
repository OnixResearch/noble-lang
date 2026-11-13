## Phase 1: Design package

- [x] Specify fixed, versioned T1/T2 resource descriptors and their bounds.
- [x] Define issue lanes, unit occupancy and initiation intervals.
- [x] Define pre-cycle operand reads, no forwarding, latency-derived
  completion, deterministic same-cycle completion order, and belt insertion.
- [x] Define input/constant materialization, explicit save/load timing, and
  ordered final output reads.
- [x] Specify independent legality, semantic-preservation evidence, controls,
  mutations, and explicit non-claims.

## Phase 2: Not selected by this issue slice

- [ ] Implement a target schedule representation and independent checker.
- [ ] Add a Lean timed-machine model and semantic soundness theorem.
- [ ] Add T1/T2 differential fixtures, mutations, and cost measurements.
- [ ] Claim no implementation or measurement until a separately selected
  implementation issue completes those tasks.
