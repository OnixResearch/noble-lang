### Requirement: K-NUM-01
r[K-NUM-01]

**K-NUM-01.** In this draft, `+`, `-`, and `*` operate on two `I64` values and wrap modulo `2^64`, interpreted back as a signed 64-bit value. Operand order is the deeper value followed by the top value. The result MUST NOT depend on debug versus optimized compilation.

For a mathematical result `z`, the signed result is:

```text
((z + 2^63) mod 2^64) - 2^63
```

`=` in the bootstrap numeric vocabulary compares two `I64` values and returns `Bool`. It is not an implicit generic equality operator for every type. Division, floating point, and the detailed checked-arithmetic library are not selected in this revision.

**Draft decision D-03.** Wrapping arithmetic selects a candidate from the RFC. Checked operations returning `Result` remain the intended library complement; their names and full contract are deferred.

For the `checked-i64` environment only, select version `noble-checked-i64/v1`. Its exact words and stack signatures (where `S` is an untouched stack prefix and binary operands are deeper then top) are:

| Word | Signature | Successful value |
|---|---|---|
| `i64.checked-add` | `S I64 I64 → S Sum<I64,I64>` | exact mathematical sum in `inl` |
| `i64.checked-sub` | `S I64 I64 → S Sum<I64,I64>` | exact deeper-minus-top difference in `inl` |
| `i64.checked-mul` | `S I64 I64 → S Sum<I64,I64>` | exact mathematical product in `inl` |
| `i64.checked-neg` | `S I64 → S Sum<I64,I64>` | exact mathematical negation in `inl` |
| `i64.checked-abs` | `S I64 → S Sum<I64,I64>` | exact mathematical absolute value in `inl` |
| `i64.lt` | `S I64 I64 → S Bool` | signed deeper-operand `<` top-operand comparison |
| `i64.checked-div-rem` | `S I64 I64 → S Sum<Pair<I64,I64>,I64>` | `(quotient, remainder)` in `inl`, quotient first |

The right `I64` alternative is an error code: `1 = Overflow`, `2 = DivisionByZero`; code `0` is reserved. The `Sum` tag distinguishes success from failure. Checked add/subtract/multiply return `inr(1)` exactly when the mathematical result is outside `[-2^63, 2^63)`. Checked negation and absolute value return `inr(1)` for `MIN`. `i64.checked-div-rem` returns `inr(2)` for a zero divisor, `inr(1)` for `MIN / -1`, and no partial quotient or remainder on either failure. It truncates toward zero and, for nonzero divisors, satisfies `a = b*q + r`, `|r| < |b|`, and `r` has the sign of `a` when nonzero. No remainder-only or Euclidean/floor-division word is selected.

`i64.lt` is signed. `≤`, `min`, and `max` are derived library definitions and do not add numeric builtins. The implementation checks divisor zero and `MIN / -1` before Wasm `i64.div_s`/`i64.rem_s`. Arithmetic-domain errors are ordinary result values; deterministic resource exhaustion remains a distinct runtime outcome.


<!-- cairn:scenario-links:start -->
#### Scenario: CORE-14 for K-NUM-01

- GIVEN the `Core-Bootstrap` profile and every field of `input` in [CORE-14](../../../../../specs/conformance/cases.json)
- WHEN the `runtime` procedure for case `CORE-14` runs against those inputs
- THEN the observations match every field of `expected` in case `CORE-14`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->
