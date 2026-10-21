# Immutable numeric graph parameter snapshots

## ADDED Requirements

### Requirement: NGRAPH-SNAP-01
r[NGRAPH-SNAP-01]

An optimizer step MUST produce a new immutable parameter snapshot and MUST NOT modify a parent snapshot, graph, Program or capture. Snapshot identity MUST be domain-separated and MUST bind the graph logical identity, the ordered parameter layout and the exact little-endian parameter bytes. Training provenance MUST bind the parent snapshot, the source subject and closure identities, the profile semantic revision, the derivative policy and its per-node options, the fact bindings, the loss graph, the exact ordered training data digest, the seed and PRNG revision, the optimizer revision with exact hyperparameter bits, the step count and the tool revisions. Evaluating with a snapshot whose graph or layout identity does not match MUST be refused before evaluation. A hash identifies bytes; it MUST NOT be reported as a correctness claim. Capturing a snapshot into a new Program MUST create a new subject whose evidence is checked for applicability anew; proofs MUST NOT transfer automatically.
