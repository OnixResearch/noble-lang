## Phase 1: Design

- [x] [serial] Specify the finite add/multiply fragment, exact bit-pattern interpretation, separately defined real graph and explicit input relation. r[NGRAPH-ERR-01]
- [x] [serial] Specify the untrusted analyzer shape, the total Lean checker obligation and the proved machine-to-real bridge to #38. r[NGRAPH-ERR-02]
- [x] [serial] Specify the subnormal policy: a proved absolute-error rule or a mechanically established no-subnormal premise with unsupported domains refused. r[NGRAPH-ERR-03]
- [x] [serial] Specify certificate binding, consumer applicability and invalidation on snapshot edits. r[NGRAPH-ERR-04]
- [x] [serial] Specify the bounded certificate budget and the distinct unknown, unsupported and exhausted outcomes. r[NGRAPH-ERR-05]

## Phase 2: Prototype (future, not started)

- [ ] [serial] Implement a total Lean certificate checker for the fragment and prove that acceptance implies the per-output bound under the checked premises. r[NGRAPH-ERR-02]
- [ ] [serial] Prove the machine-to-real bridge to #38's binary32 reference for the covered fragment, including the selected subnormal policy. r[NGRAPH-ERR-02] r[NGRAPH-ERR-03]
  Progress (2026-10-29): `Binary32Error.lean` proves the exact rational value
  of finite normal encodings, universal ties-to-even quotient error, and
  conditional normal-range `roundPositive` absolute/relative bounds, with a
  non-vacuity witness at exact input `1/3`, plus a flush-to-zero negative
  control. Deriving the normal-bin scale premises from `floorLog2Ratio`, the
  subnormal policy, analyzer/checker bridge, and implementation correspondence
  remain open.
  Progress (2026-10-30): `Binary32Scale.lean` derives the lower scale premise
  for every positive input and the upper premise off the power boundary (ratios
  below one that are exact powers of two). There the upper premise is false and
  the carried result is exact, so the absolute and relative bounds now need only
  the carried-exponent range. The subnormal policy, analyzer/checker bridge, and
  implementation correspondence remain open.
- [ ] [serial] Demonstrate a nontrivial bound on an actual encoded graph, including a cancellation case and a near-overflow case. r[NGRAPH-ERR-01]
- [ ] [serial] Add the negative controls for underestimated error and ranges, missing input error, changed operation order, FMA contraction, wrong snapshot/domain/output, stale model, unproved underflow premise and oversized certificates. r[NGRAPH-ERR-03] r[NGRAPH-ERR-05]
- [ ] [serial] Add an independent expected-value lane that does not reuse the analyzer's arithmetic as the oracle. r[NGRAPH-ERR-02]
- [ ] [serial] Compare bound quality and checker cost with an untrusted analyzer on a pinned bounded corpus, recording numerical sampling as secondary evidence only. r[NGRAPH-ERR-05]
- [ ] [serial] Bind exact graph/parameter snapshot, domain, model and checker revisions in a retained receipt and publish a go/no-go before broadening operations. r[NGRAPH-ERR-04]
