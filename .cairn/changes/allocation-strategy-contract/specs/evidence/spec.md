# Allocation strategy design evidence

## ADDED Requirements

### Requirement: ALLOC-EV-01
r[ALLOC-EV-01]

Every ALLOC-CASE scenario design and every benchmark-plan workload MUST remain `implementation: absent`, `execution: not-run`, `proof: open`, `trust: unassessed`, `evidence: []` until it is actually implemented and executed, and the result is independently recorded.

A later performance claim MUST record:

- source and tool revisions, profile, strategy, capability set and accounting revision;
- hardware, warm/cold state, sample count and aggregation method;
- results that keep logical, committed, metadata, engine and process measurements separate.

A native allocator comparison MUST hold the backend and representation constant. No performance claim MAY omit a failed correctness case for the same workload. A recommendation, decision criterion or bounded test MUST NOT be reported as an allocator selection, a default change or a correspondence proof.

#### Scenario: ALLOC-CASE-01 design state stays unexecuted

- GIVEN the ALLOC-CASE-01..12 designs and W1–W8 benchmark plan recorded in the allocation strategy design
- WHEN the change is validated without implementation or measurement
- THEN every case and workload remains absent/not-run with no evidence, and no allocator is selected
