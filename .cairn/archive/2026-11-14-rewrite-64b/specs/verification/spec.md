# Certified graph rewrite transition and pruning contract

## MODIFIED Requirements

### Requirement: NGRAPH-PROOF-01
r[NGRAPH-PROOF-01]

**NGRAPH-PROOF-01.**

Numeric graph assurance claims MUST separately state reference agreement, finite differential or test coverage, discharged theorem obligations and trusted compiler, runtime and toolchain boundaries. Extraction preservation, graph-checker soundness, compiled-evaluator refinement, FP32 reference-model correspondence, fact-transfer soundness and derivative-policy conformance MUST remain open obligations until separately proved or evidenced. Matching hashes, agreement between evaluators, finite differential tests and producer-supplied metadata MUST NOT be reported as discharging any of them.

For a chained graph-rewrite certificate, every transition MUST bind the exact logical identity of the graph immediately before and immediately after that transition. The independent checker MUST derive each intermediate graph by replaying the preceding accepted transition and MUST check the next transition's premise against that graph. A prune transition MUST derive its survivor mapping from the graph itself; the mapping MUST preserve creation order, operand order, ordered outputs and output multiplicity. Pruning MUST retain all output-, guard-, fact- and interface-reachable nodes, and a removed node MUST NOT reach any output or guard. The exact pruned candidate MUST pass the existing independent graph-admission checker before execution. A changed transition grammar MUST use a distinct certificate revision and MUST NOT change the meaning of a previously accepted revision. These finite model, test, and measurement results do not establish correspondence between Rust code and a Lean model.

#### Scenario: NGRAPH-CASE-16 chained rewrite admission and execution controls

- GIVEN the `Numeric-Graph-v1` profile and every field of `input` in [NGRAPH-CASE-16](../../../../../specs/conformance/numeric-graph-cases.json)
- WHEN the independent checker replays each certificate transition, derives its prune mapping, admits the resulting candidate, and executes only admitted candidates
- THEN every executed output vector MUST equal the source reference in order and multiplicity, every listed stale or malformed variant MUST be refused before candidate execution, and the unchanged legacy revisions MUST retain their prior semantics
