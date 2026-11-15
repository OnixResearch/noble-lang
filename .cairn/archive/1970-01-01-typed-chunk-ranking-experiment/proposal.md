## Why

Issue #74 asks whether a small untrusted scorer can improve checked typed-chunk
completion from #73 by choosing which already-admitted candidates to try next.
The question is useful only if the corpus, candidate library, feature schema,
training recipe, budgets, controls and decision rule are frozen before held-out
evaluation. Without that boundary, fewer proposals could be mistaken for lower
end-to-end cost, or a signature match for correct behavior.

r[TCRANK-01] r[TCRANK-02] r[TCRANK-03] r[TCRANK-04] r[TCRANK-05]

## What Changes

- **ADDED** `TCRANK-01`: a frozen pure-I64/Bool corpus and provenance-aware
  split, leakage/dedup rules, and a bounded feature vector over the #73 request,
  partial state and candidate metadata.
- **ADDED** `TCRANK-02`: a tiny fixed F32 scorer graph using existing ngraph
  trainable slots, reverse-AD policy and immutable snapshots, with a fixed
  target, loss, optimizer policy and seed. It can only reorder the finite #73
  candidate queue.
- **ADDED** `TCRANK-03`: three matched baselines, a shuffled control, scorer-
  inclusive measurements, separate training cost, repetitions and fixed seeds.
- **ADDED** `TCRANK-04`: adversarial and integrity controls for invalid scores,
  stale identities, incorrect chunk metadata, exhausted budgets and equal-
  signature chunks with different behavior, plus deterministic fallback.
- **ADDED** `TCRANK-05`: preregistered numerical go/no-go thresholds and the
  compiled-select scope limit. A select gate does not skip branch computation.

This is design-only. It adds no runner, scorer, training, benchmark, model
weights, canonical specification or production behavior. Phase 2 remains
blocked on #73 Phase 2's exact chunk library, deterministic queue and ordinary
re-admission path.

## Impact

The design references the checked chunk contract in #73 and these existing
boundaries: `noble-ngraph/src/lib.rs`, `ir.rs`, `grad.rs`, `compiled.rs`, and
`noble-contracts/src/source/editor.rs`. Existing source preparation, kernel
acceptance, contracts, allowed effects, budgets and Programs remain authoritative
and unchanged. A scorer is never proof or acceptance authority.

## Non-goals

No implementation, training run, benchmark, learned chunk discovery, parser or
IR replacement, conditional-execution rewrite, GPU backend, topology-learning
system, proof-search framework, semantic relaxation or production adoption.
