# Revision-3 plan storage comparison criteria

This comparison is descriptive only. It does not authorize changing the opt-in status of `noble ngraph eval --net`, changing the frozen 1.15 threshold in `verification/ngraph/net-decision.md`, or claiming general interaction-net performance.

## Frozen comparison

- Compare the same checked revision-3 graph, lowered net, and ordered input row under `StoragePolicy::Reuse` and `StoragePolicy::Allocate`. The source graph is admitted and the net is lowered once per workload, outside the timed region; each timed call includes the reducer's independent net validation, plan compilation and checking, transaction reservations and commit, completion/read-back, and destruction of reducer-local state.
- Workload labels are `chain`, `fanout`, `erasure`, `asymmetric-subtraction`, and `wrapping-subtraction`. The graph/input fixtures are created in the existing `noble-ngraph` test target. Inputs and graph identity are held fixed between arms.
- Run one untimed warm-up per arm. Collect 31 paired observations per workload in one test process; alternate arm order by pair index. Use `Instant` around the complete `reduce` call. Report each raw duration and the median for each arm; report the median reuse/allocate ratio as descriptive arithmetic only. Do not impose a speed threshold or select a default from these measurements.
- Every pair must return exactly equal complete observations and abstract interaction counts. A mismatch fails the measurement test and no performance conclusion is retained.
- Record, for each arm, the cumulative per-reduction allocated/reused/freed slot counts, committed live nodes, transient arena slots, arena and queue high-water marks, maximum plan scratch bytes, total emitted plan bytes, interaction count, plan digest, and replay digest. The plan digest is over exact emitted bytes. The replay identity includes the net/input build key, revision-3 rule identity, plan revision, arena representation, independent checker, executor, and plan stream.
- The retained report binds the clean source commit, the measurement-test executable SHA-256, graph and input identities, all plan/replay digests, and the static rule/representation/checker/executor identities. Replaying with a changed plan under the same rule identity must change both plan and replay digests; this is an identity control, not a performance result.

## Interpretation

Counters describe the bounded Rust plan implementation, not an abstract proof. Equality in this finite fixture set is not a Rust-to-Lean correspondence proof. Timings include the full reducer call but exclude graph admission and net lowering by design, because the paired intervention is arena storage policy on the identical already-admitted net. The independent `--net` backend remains opt-in and the existing #37 decision remains unchanged.
