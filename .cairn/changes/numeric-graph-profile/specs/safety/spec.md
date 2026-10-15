# Numeric graph scratch and retained ownership

## ADDED Requirements

### Requirement: NGRAPH-OWN-01
r[NGRAPH-OWN-01]

Per-evaluation activation, gradient and workspace scratch MUST be owned separately from retained graph code, parameter snapshots, optimizer state and captures. Evaluation results MUST be returned as fresh immutable values, never as views of reused scratch. On normal return, failure, guard failure, budget exhaustion and cancellation, scratch MUST be released or reset exactly once without publishing a partial snapshot or modifying a retained owner. Scratch reset MUST NOT invalidate a saved Program, a graph or a trained snapshot. Duplicate owners of an immutable snapshot or graph MUST be releasable in any order. Any device buffer or queue MUST be an explicitly managed host resource. It MUST NOT be graph data, a `Data` value or a capture.
