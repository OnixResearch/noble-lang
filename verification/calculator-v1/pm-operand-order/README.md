# Calculator ± operand order (issue #18) measurements

This record covers source commit `ca07c5c`, based on `44329dc`. That commit
changes `op_first_stage_second_cross` in `num@1` so that the d1·n2 product
takes the narrower operand as its multiplier, with d1 on a tie. It also
updates `op_work_probe` to charge the same roles:
S = M(a,f) + M(max(c,e), min(c,e)) + c + e + 2. The change is body-only: no
definition, export, signature, nominal, Budget layout, default or cap
changed. The formulas, per-product role table, visit-count derivation and
above-384-bit bound are in the
[design](../../../.cairn/changes/calculator-v1/design.md) under "Evaluation
Work accounting". Machine-readable values are in
[summary.json](summary.json).

## Binaries

All runs used release builds from the same toolchain.

| Binary | sha256 |
|---|---|
| base `44329dc` | `e1fb70bac358e7ffe1a064295f985448b89899c2289d8a3d514cf0cdfc708127` |
| after (this change) | `e5741ef0af9a854ef3c65d177027540db9858dfbb210b88e30336099bc509db1` |
| after + temporary metrics patch | `c7ef5d80cf2242ed87e13ad6771b1661862156a2d460a7ef0de7d0a16155f77d` |

The metrics patch (sha256
`5360c3389e7a1b7fd2dedc9faa4e1750a4f49fbdb2dadeeed2e59fd3e64f646d`) prints
`execution_steps` and `allocated_since_begin_bytes` per submission. It is not
committed.

## Decision

The product d1·n2 iterates over its multiplier, so the Q² term follows
whichever operand is the multiplier. C is the operator count, charged at
150·C. The submission counts in parentheses are for the d1·n2 product only.

| Operands | a,c,e,f / b | Former (n2) | Fixed swap (d1) | Width-ordered |
|---|---|---|---|---|
| `9007199254740993 - 9007199254740992` | 54,1,54,1 / 112 | 7,579 (546) | 1,272 (175) | 1,329 (175) |
| `9223372036854775807 + 1` | 63,1,1,1 / 68 | 788 (16) | 788 (16) | 792 (16) |
| `1 + 18014398509481983` | 1,1,54,1 / 59 | 6,996 (4,892) | 689 (175) | 746 (175) |
| `1/3 + 1/6` | 1,2,1,3 / 9 | 227 (19) | 242 (30) | 232 (19) |
| `0.1 + 0.2` | 1,4,1,3 / 11 | 273 (25) | 330 (53) | 280 (25) |
| `0.000001 + 1` | 1,20,1,1 / 25 | 391 (73) | 1,360 (470) | 414 (73) |
| `2^-60 + 3` | 1,61,2,1 / 67 | 1,272 (390) | 9,237 (619) | 1,337 (390) |
| `1.5 + 2.25` | 2,2,4,3 / 13 | 348 (56) | 306 (35) | 314 (35) |
| `1 / 18014398509481983` | 1,1,54,1 / 59 | 6,675 (4,892) | 368 (175) | 425 (175) |
| `0.000001 / 3` | 1,20,2,1 / 26 | 261 (147) | 1,215 (500) | 285 (147) |

A fixed swap moves the Q² term onto d1, and decimal and rational left
operands have wide denominators. The width-ordered choice is never worse
than the cheaper fixed order by more than c + e + 2 ≤ b units, and it adds no
guest submission.

## EvaluationWork brackets

Each N was predicted from the formulas before the runs. Command:
`noble calculator --evaluation-work N`, with the expression on stdin.

| Expression | Case | N−1 | N | Former model N |
|---|---|---|---|---|
| `10 - 3` | integer | 64,356 → `ResourceLimit EvaluationWork [7,7)` | 64,357 → `7` | 65,857 |
| `1 + 18014398509481983` | wide-right integer | 1,827,721 → `ResourceLimit EvaluationWork [22,22)` | 1,827,722 → `18014398509481984` | 2,765,222 |
| `0.1 + 0.2` | rational, d1 stays multiplier | 108,734 → `ResourceLimit EvaluationWork [10,10)` | 108,735 → `3/10` | 107,685 |
| `1.5 + 2.25` | rational, multiplier flips to d1 | 151,085 → `ResourceLimit EvaluationWork [11,11)` | 151,086 → `15/4` | 156,186 |
| `1 / 3` | division | 35,155 → `ResourceLimit EvaluationWork [6,6)` | 35,156 → `1/3` | 36,656 |

The N−1 runs exit with status 1 and the N runs with status 0. The first
prediction for `0.1 + 0.2` was 110,210, and 110,209 printed `3/10`. That
prediction had left out the denominator finish's use of the reduced
numerator width. The corrected value was then confirmed, and design.md
states the rule. At their predicted N, c01-05 (2,129,286) prints `1` and
c01-04 (2,346,724) prints `9223372036854775808`.

## Wall time

Each pair ran with default budgets and its two runs started together. The
host was shared and loaded, with a load average of about 50 on 32 cores, so
compare wall times only within a pair. These are single samples, not
benchmarks.

| Case | Base | After | Max RSS base / after |
|---|---|---|---|
| c01-05 `9007199254740993 - 9007199254740992` | 34:43.87 | 29:37.32 | 204 / 201 MB |
| c01-04 `9223372036854775807 + 1` | 21:37.73 | 21:44.89 | 204 / 208 MB |
| wide-right `1 + 18014398509481983` | 1:55:15 (user 6,687 s) | 20:17.09 | 205 / 204 MB |

Unpaired runs:

| Binary | Expression | Wall |
|---|---|---|
| base | `1 + 18014398509481983` | 2:18:12 |
| after | `1 + 18014398509481983` | 17:04.54 |
| base | c01-05 | 24:08.63 |
| base | c01-04 | 13:08.57 |

## Per-submission margins

The per-submission quotas are 100,000 steps and 1,376,208 allocated bytes
(the Calculator-v1 `limits_maximum` in `engine.mjs`). These measurements come
from the instrumented binary. Every submission ended `normal`: 1,488 for
c01-05 and 993 for the wide-right case.

| Expression | Submission | Steps | Allocated bytes |
|---|---|---|---|
| `1 + 18014398509481983` | `op_first_stage_sum_finish_add` (heaviest ±) | 9,319 | 80,544 |
| c01-05 | `op_first_stage_sum_finish_sub` | 8,982 | 76,848 |
| c01-05 | `op_first_stage_sum_begin` | 6,051 | 61,680 |
| c01-05 | `op_first_stage_second_cross` (changed) | 1,359 | 32,064 |
| c01-05 | `op_work_probe` (changed) | 1,469 | 36,240 |

## Tests

- `cargo test -p noble-wasm --test calculator_profile`: 17 passed.
- `cargo test -p noble-wasm --test calculator_collect`: 9 passed.
- `cargo test -p noble-contracts`: all suites passed.
- `cargo test --release -p noble-cli --test smoke -- calculator`: 1 passed.

## Pending

CALC-01 15/15 has not yet been run through the calculator acceptance runner
(#19) on the integrated build.

## Non-claims

- The Work bound is not proved. design.md carries a written derivation.
- The margins come from an uncommitted instrumentation patch, not a supported
  metrics output.
- The normalization, literal and display charges are unchanged and remain
  quadratic.
- This record does not claim CALC-01 acceptance.
