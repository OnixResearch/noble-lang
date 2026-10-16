# Numeric graph snapshot files

## ADDED Requirements

### Requirement: NGRAPH-SNAP-02
r[NGRAPH-SNAP-02]

**Contents.** A snapshot file MUST begin with `noble-ngraph-snapshot/0`. It MUST record, as structured fields:

- the `ParamSnapshotId`, the `GraphLogicalId` and the ordered parameter layout;
- every parameter's exact binary32 encoding, in param-slot order;
- the optimizer-state digest;
- the complete NGRAPH-SNAP-01 training provenance, including the fact-binding digest of the model graph;
- a domain-separated digest of that provenance record.

**Loading.** Loading a snapshot file MUST recompute the layout, the snapshot identity and the provenance digest from the recorded fields, and MUST refuse any mismatch with `E-SNAPSHOT-INTEGRITY` before use.

**Binding.** Binding a snapshot to a graph MUST refuse:

- a different `GraphLogicalId`, or a different fact-binding digest, with `E-SNAPSHOT-STALE`;
- a different layout with `E-SNAPSHOT-LAYOUT`;
- a snapshot other than a caller-pinned snapshot identity with `E-SNAPSHOT-ID`.

Every binding refusal MUST occur before any graph node is evaluated.

**Publication.** Training MUST publish its result only as a new file at a path that did not exist. The write MUST never truncate, replace or modify an existing file, including the parent snapshot. A training call that stops MUST NOT leave a snapshot or partial file at the requested path.

**Replay.**

- Replaying a snapshot's provenance MUST first require that the supplied model, loss graph, training data and parent match the recorded identities; otherwise `E-SNAPSHOT-STALE`.
- On the same pinned platform, replay MUST then reproduce the parameter bytes, the optimizer-state digest and the snapshot identity exactly; otherwise `E-SNAPSHOT-REPLAY`.

**Non-claim.** A snapshot file identifies bytes. A matching identity or a successful replay MUST NOT be reported as evidence that the trained parameters, the optimizer or the evaluators are correct.

#### Scenario: NGRAPH-CASE-10 immutable training publication

- GIVEN the `Numeric-Graph-v1` profile and the `NGRAPH-CASE-10` model, loss, data, seed and follow-up runs in [the unexecuted case design](../../cases/numeric-graph-cases.json)
- WHEN `noble ngraph train` publishes snapshots, retrains to a second path, trains from the first snapshot and targets an existing path
- THEN the recorded identities MUST match, repeated training MUST be byte-identical, and the parent, the graph and existing paths MUST stay byte-unchanged; this active delta does not change that case's `state` or `evidence`

#### Scenario: NGRAPH-CASE-12 snapshot integrity and binding refusals

- GIVEN every `NGRAPH-CASE-12` tampered, foreign, permuted, changed-fact, pinned-identity and schema variant in [the unexecuted case design](../../cases/numeric-graph-cases.json)
- WHEN `noble ngraph eval` and `noble ngraph verify` load and bind each snapshot
- THEN each MUST be refused with its recorded code before any graph node is evaluated; this active delta does not change that case's `state` or `evidence`

#### Scenario: NGRAPH-CASE-13 provenance replay

- GIVEN the `NGRAPH-CASE-13` seed, parent, changed-input and forged replay variants in [the unexecuted case design](../../cases/numeric-graph-cases.json)
- WHEN `noble ngraph verify --replay` rebuilds and reruns each training contract
- THEN matching inputs MUST reproduce the snapshot exactly and every other variant MUST be refused with its recorded code; this active delta does not change that case's `state` or `evidence`

#### Scenario: NGRAPH-CASE-11 training contract refusals

- GIVEN every `NGRAPH-CASE-11` derivative, sub-profile, non-finite, budget, parent, guard, loss-interface and usage variant in [the unexecuted case design](../../cases/numeric-graph-cases.json)
- WHEN `noble ngraph train` runs each variant
- THEN each MUST report its recorded code and exit status and MUST NOT create an output or partial file, except the recorded opt-in success; this active delta does not change that case's `state` or `evidence`
