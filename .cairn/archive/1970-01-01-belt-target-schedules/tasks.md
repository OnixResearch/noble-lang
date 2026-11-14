## Phase 1: Design package

- [x] [serial] Specify fixed, versioned T1/T2 resource descriptors and their bounds. r[NGRAPH-BELT-TARGET-01]
- [x] [serial] Define issue lanes, unit occupancy and initiation intervals. r[NGRAPH-BELT-TARGET-01] r[NGRAPH-BELT-TARGET-02]
- [x] [serial] Define pre-cycle operand reads, no forwarding, latency-derived completion, deterministic same-cycle completion order, and belt insertion. r[NGRAPH-BELT-TARGET-02]
- [x] [serial] Define input/constant materialization, explicit save/load timing, and ordered final output reads. r[NGRAPH-BELT-TARGET-02]
- [x] [serial] Specify independent legality, semantic-preservation evidence, controls, mutations, and explicit non-claims. r[NGRAPH-BELT-TARGET-03] r[NGRAPH-BELT-TARGET-04]

## Phase 2: Selected by issue #114

- [x] [serial] Implement a target schedule representation and independent checker. Evidence: `crates/noble-ngraph/src/belt/target.rs` and `crates/noble-ngraph/tests/belt_target_dir/`; commits `1a0028c5`, `6b27d997`, `7476e6fc`. r[NGRAPH-BELT-TARGET-01] r[NGRAPH-BELT-TARGET-02] r[NGRAPH-BELT-TARGET-03]
- [x] [serial] Add a Lean timed-machine model and scoped completion/output theorem. Evidence: `proofs/foundation/NobleFoundation/BeltTarget.lean`, `BeltTargetCorpus.lean`, and the corpus freshness test; commits `2238adf9`, `298a5383`. The theorem gates ordered graph-reference values on latency-derived completion; operation execution, belt/save transitions, and Rust refinement remain open and are not claimed. r[NGRAPH-BELT-TARGET-03]
- [x] [serial] Add T1/T2 differential fixtures, mutations, boundary controls, and cost measurements. Evidence: target tests under `crates/noble-ngraph/tests/belt_target_dir/`; receipt `verification/belt-target/model-cost.json`, refreshed from clean source commit `ba7ac02e` in `711decc0` (the initial report from `05b0eeab` in `98faea1f` was superseded). Commits `0c63b138`, `5d78e293`, `7476e6fc`, `05b0eeab`, `98faea1f`, `711decc0`. r[NGRAPH-BELT-TARGET-04]
- [x] [serial] Keep implementation and measurement claims within the selected abstract-model scope. Evidence: `verification/belt-target/README.md` and the target row of `proofs/foundation/README.md` explicitly exclude production-backend, hardware-performance, independent arithmetic-machine, belt/save-model, and Rust-to-Lean-refinement claims. r[NGRAPH-BELT-TARGET-03] r[NGRAPH-BELT-TARGET-04]
