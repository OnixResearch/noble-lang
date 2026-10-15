# Allocation failure classes and cleanup

## ADDED Requirements

### Requirement: ALLOC-FAIL-01
r[ALLOC-FAIL-01]

Allocation outcomes MUST be classified distinctly as one of four classes:

- deterministic logical quota refusal;
- deterministic strategy exhaustion, such as no reusable slot or an epoch-retired slot;
- memory-growth failure below the declared maximum;
- engine or host out-of-memory and process failure.

These classes MUST NOT be collapsed into one diagnostic.

A failed allocation MUST publish no cell and inspect no child handle. A failed multi-cell construction MUST leave no partially built value reachable from any root. The already observed host-request prefix MUST be retained and never rolled back. Cleanup after a trap, cancellation or failed construction MUST follow the ownership and lifetime cleanup rules: host-side discard without resuming the guest, and exactly-once pin release. Storage cleanup MUST NOT perform, imply or substitute for a host-resource release.

#### Scenario: ALLOC-CASE-02 injected allocation failures stay distinct

- GIVEN the quota, exhaustion, memory-growth and worker out-of-memory injections of ALLOC-CASE-02 and the mid-construction trap of ALLOC-CASE-09
- WHEN each failure occurs
- THEN each reports its own class, no partially published cell is live, the host-request prefix is retained, and no host resource is released by storage cleanup
