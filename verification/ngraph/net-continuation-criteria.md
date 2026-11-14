# Revision-3 I64 net continuation criteria (issue #37)

**Frozen before any net timing.** This file is the pre-registered decision rule for the first Rust
interaction-net backend. It must be committed before invoking the measurement mode. A subsequent
receipt may be reported against this rule but must not change it.

## Workload and comparison

Use the issue #117 `shared-prefix` candidate family, the fanout-heavy case: twelve candidate outputs
share the same source-produced prefix value, in one checked `ngraph-i64/0` graph. Evaluate the frozen
8,192-row input set from `verification/shared-candidates/criteria.json`. The poor-sharing
`distinct-prefix` family remains a diagnostic control, not a substitute for the selected workload.

Compare the **warm median wall time** of the `net-merged` arm with the packed `merged` arm from the
same interleaved run. Both process the identical graph and row file. `--net` rebuilds and validates
the value-bearing net for every input row; the measured command therefore includes graph-to-net
construction, independent net validation, reduction and read-back. There is no cross-input net cache
or excluded one-time compile step to amortize. A complete measurement run accounts for construction
from its first row: the accounting count is **one complete run**, and no repeated-run discount is
applied to construction. If the inclusive net run is slower, repetition cannot recover an unmeasured
one-time construction cost because no such cost is excluded from the timed arm.

The existing runner records one first (not page-cache-cold) round and nine warm rounds, interleaves
the packed and net arms, reports wall/CPU/RSS and net interaction/peak-agent counts, and gates all
rows against the independent oracle, packed evaluator and reference evaluator. Allocation counts are
not available because workspace `unsafe` is forbidden. The host cannot drop page caches without
root, so “first” means only “not previously executed in this measurement run.”

## Frozen GO / NO-GO

**GO** only if all of the following hold:

1. Every frozen row from every separate, merged and recomputing graph agrees across the independent
   BigInt oracle, normative reference evaluator, packed evaluator and net read-back (100% agreement).
2. The warm median ratio `packed merged / net merged` on `shared-prefix` is at least **1.15** (equivalently
   `net merged / packed merged` is at most `1 / 1.15`, approximately `0.8696`).
3. The run reports the complete inclusive construction accounting above: **one full run**, with no
   assumed net reuse or amortization beyond it.

Otherwise the decision is **NO-GO**. A correctness failure invalidates the timing result. There is no
threshold adjustment after measurement. GO permits only a further opt-in experiment: it never makes
`--net` the default, does not license an optimal-runtime claim, and does not establish a refinement
proof. The trusted Rust lowering/reducer and host timing limitations remain as stated in
`docs/i64-net-contract-v3.md`.
