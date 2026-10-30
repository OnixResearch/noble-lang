## Phase 1: Design

- [x] [serial] Specify the finite add/multiply fragment, exact bit-pattern interpretation, separately defined real graph and explicit input relation. r[NGRAPH-ERR-01]
- [x] [serial] Specify the untrusted analyzer shape, the total Lean checker obligation and the proved machine-to-real bridge to #38. r[NGRAPH-ERR-02]
- [x] [serial] Specify the subnormal policy: a proved absolute-error rule or a mechanically established no-subnormal premise with unsupported domains refused. r[NGRAPH-ERR-03]
- [x] [serial] Specify certificate binding, consumer applicability and invalidation on snapshot edits. r[NGRAPH-ERR-04]
- [x] [serial] Specify the bounded certificate budget and the distinct unknown, unsupported and exhausted outcomes. r[NGRAPH-ERR-05]

## Phase 2: Prototype (future, not started)

- [x] [serial] Implement a total Lean certificate checker for the fragment and prove that acceptance implies the per-output bound under the checked premises. r[NGRAPH-ERR-02]
  Ticked 2026-10-30: `Binary32Checker.lean` defines `check`, a total checker over a finite graph of
  inputs, constant bit patterns, `add` and `mul` in stored order, with one proposed real enclosure
  and absolute error bound per node and explicit input domains and input errors. `check_sound`
  proves that an accepted certificate gives every node a finite machine value, a real value inside
  its enclosure and an error within its bound, for every input meeting the declared domains
  (`Applies`, the consumer's obligation); `check_output_bound` states it per output. The outcomes
  keep `unknown` (a failed check, never a disproof), `unsupported` and `exhausted` apart. The
  machine enclosure is derived as `[lo - error, hi + error]` rather than proposed separately, and
  the overflow test `reach < 2^127` is sufficient, not tight. No analyzer, encoded graph or
  Rust/host correspondence is claimed.
- [x] [serial] Prove the machine-to-real bridge to #38's binary32 reference for the covered fragment, including the selected subnormal policy. r[NGRAPH-ERR-02] r[NGRAPH-ERR-03]
  Ticked 2026-10-30: `check_sound` is stated over #38's own `add` and `mul`, so the checker has no
  separate abstract rounding model to relate: the bridge is that theorem. The selected subnormal
  policy is the proved absolute rule. Every finite rounding is within `2^-24 · |x| + 2^-150`
  (`Binary32SequenceFinite.lean`: `add_mixed_error`, `mul_mixed_error`), and the checker's overflow
  test establishes its premise and a finite result (`noOverflow_of_lt`, `addFinite_of_small`,
  `mulFinite_of_small`, `add_finite`, `mul_finite`). `subnormal_needs_eta` shows the absolute term
  is load-bearing, and nothing is flushed. Rust, Wasm and host correspondence stay outside this
  task, as the design's non-claims state.
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
  Progress (2026-10-30): `Binary32Subnormal.lean` proves the subnormal branch:
  below the normal range the result is within `2^-150` of the exact quotient,
  which is attained and holds only absolutely, and every finite positive result is
  within half the spacing of its binade. The analyzer/checker bridge and
  implementation correspondence remain open.
- [ ] [serial] Demonstrate a nontrivial bound on an actual encoded graph, including a cancellation case and a near-overflow case. r[NGRAPH-ERR-01]
  Progress (2026-10-30): at model level only, `Binary32Checker.lean` certifies `x · x + (-1)` over
  `x ∈ [1, 2]`, which cancels towards zero near `x = 1` (`cancellation_certified`, with
  `cancellation_applies` showing its premises are satisfiable). It certifies squaring over
  `[1, 2^63]` and refuses it over `[1, 2^64]` (`near_overflow_square`). These are Lean model graphs:
  no encoded `noble-ngraph` graph is mapped to the fragment, so the task stays open.
- [ ] [serial] Add the negative controls for underestimated error and ranges, missing input error, changed operation order, FMA contraction, wrong snapshot/domain/output, stale model, unproved underflow premise and oversized certificates. r[NGRAPH-ERR-03] r[NGRAPH-ERR-05]
  Progress (2026-10-30): `Binary32Checker.lean` decides the checker-level controls.
  - Refused as `unknown`: an underestimated error, an underestimated range and a missing input error.
  - Classified `unsupported`: an operation outside the fragment, an operand that is not an earlier
    node, and an output naming no node.
  - Classified `exhausted`: an oversized rational or graph.
  - `subnormal_needs_eta` shows that a bound without the absolute term is refused and would be false.

  Changed operation order, FMA contraction, wrong snapshot or domain, and a stale model need the
  binding layer and stay open.
- [x] [serial] Add an independent expected-value lane that does not reuse the analyzer's arithmetic as the oracle. r[NGRAPH-ERR-02]
  Ticked 2026-10-30: `verification/numerical-certificate/lane.mjs` evaluates every node of every
  corpus graph on 2,000 sampled inputs per graph, both ways:
  - with the host's binary32 arithmetic;
  - with exact dyadic arithmetic over values decoded from their binary64 bits. That path shares
    neither the analyzer's float32 decoder nor its rational interval arithmetic.

  Across 404,000 node checks, no value leaves its certified enclosure or error bound. The two
  expected-value paths give identical worst errors on all 24 graphs. Scope: Lean model graphs and the
  host's binary32, not the Lean model's arithmetic; no adapter from encoded graphs exists, so none is
  exercised.
- [ ] [serial] Compare bound quality and checker cost with an untrusted analyzer on a pinned bounded corpus, recording numerical sampling as secondary evidence only. r[NGRAPH-ERR-05]
  Progress (2026-10-30): on the pinned corpus (seed `44`, `corpus.lean` sha256 `9ec794e5…`), the
  untrusted JS analyzer proposes the tightest certificates the checker's rules admit. The kernel
  accepts all 24 and refuses their weakened copies, at about 100 ms per kernel check.
  - Certified output bounds over the worst sampled output errors: min 1.20, quartiles
    2.57 / 4.01 / 7.40.
  - The large outliers come from mixed subnormal and large domains.
  - Sampling is recorded as secondary evidence only.

  This measures the checker's own rules against observation. No external untrusted analyzer
  (FPTaylor, Daisy) is compared, so the task stays open.
- [ ] [serial] Bind exact graph/parameter snapshot, domain, model and checker revisions in a retained receipt and publish a go/no-go before broadening operations. r[NGRAPH-ERR-04]
