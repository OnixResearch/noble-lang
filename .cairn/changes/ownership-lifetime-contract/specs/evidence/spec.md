# Ownership lifetime design evidence

## ADDED Requirements

### Requirement: OWN-EV-01
r[OWN-EV-01]

Every OWN-CASE scenario design MUST remain `implementation: absent`, `execution: not-run`, `proof: open`, `trust: unassessed`, `evidence: []` until a separate implementation and actual execution or proof are independently recorded. Evidence for the ownership lifetime contract MUST state which layer it covers: abstract model, checker refinement, Rust lowering correspondence, Wasm runtime correspondence or host-ledger correspondence. Evidence for one layer MUST NOT be reported as evidence for another. A written conditional argument or a bounded test MUST NOT be reported as a mechanized proof or as complete runtime correctness. Evidence MUST state its assumptions about roots, reentrancy, counter widths, handle reuse and failure behavior. A no-tracing reclamation claim MUST be scoped to the named guest-cell algorithm and MUST NOT cover host version-retention bookkeeping or engine memory management.

#### Scenario: OWN-CASE-01 design state stays unexecuted

- GIVEN the OWN-CASE-01..12 designs recorded in the ownership lifetime design
- WHEN the change is validated without implementation or execution
- THEN every case remains absent/not-run/open/unassessed with no evidence and no layer is claimed
