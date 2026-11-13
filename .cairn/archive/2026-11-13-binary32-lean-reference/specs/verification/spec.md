## ADDED Requirements

### Requirement: NGV-F32-01
r[NGV-F32-01]

A kernel-checked binary32 reference MUST select exactly one model with a documented trusted base, and MUST NOT depend on an opaque host floating-point primitive or an unaudited toolchain pin change.

The selected model MUST define raw 32-bit encodings with sign, 8-bit biased exponent and 23-bit fraction; classify infinity, NaN, both zero signs, subnormal and normal; interpret every finite encoding exactly as `mantissa * 2^exponent`; round to nearest with ties to even at one rounding per operation; retain gradual underflow at `2^-149`; form an infinity at or above `2^128` with the operand sign; keep signed zeros distinct; and treat every NaN encoding as one observation class while every other class keeps its exact bytes.

Addition, subtraction and multiplication MUST follow the IEEE 754 rules for NaN propagation, infinities, signed zeros, overflow and underflow. Reassociation and implicit contraction MUST be observable, so a fold order and a single rounding per operation are normative.

#### Scenario: NGV-F32-CASE-01 halfway tie and exact-sum divergence

- GIVEN the selected model and the exact sum of `1.0` and `2^-24`
- WHEN the sum is rounded once to binary32
- THEN the result is exactly `1.0`, while the exact sum is strictly larger, so round-to-real reasoning cannot discharge the binary32 requirement

### Requirement: NGV-F32-02
r[NGV-F32-02]

Reference expectations MUST be proposed by an independent external tool and MUST each be checked against the selected model by the Lean kernel. A proposal alone MUST NOT be evidence, and inability to check one MUST NOT be a pass.

The retained family MUST cover halfway ties in both directions, cancellation, both zero signs, the smallest and largest subnormals, the normal/subnormal boundary, gradual underflow, overflow in both signs, infinities and NaNs. Native or external tools MAY propose expected bits, but no `native_decide`, `decide +native` or native `#eval` MAY replace kernel checking.

#### Scenario: NGV-F32-CASE-02 retained expectations

- GIVEN the retained expectation list and the external proposal file
- WHEN the checker audits the theorem that binds every retained expectation
- THEN each expectation equals the selected model's answer, and the axiom audit reports only axioms inside the foundation allow-list

### Requirement: NGV-F32-03
r[NGV-F32-03]

A receipt MUST identify the model, the Lean and Mathlib pins, the proposer, the checker and the clean source revision, and MUST list every proved theorem with its printed statement and axioms. Stale evidence MUST be refused rather than reinterpreted.

Separate claims MUST be maintained for the proved model, tested implementations, trusted correspondence and open obligations. A model-level receipt MUST NOT be reported as implementation or correspondence evidence, and no universal target claim MAY come from one host.

#### Scenario: NGV-F32-CASE-03 receipt scope

- GIVEN the retained binary32 receipt
- WHEN a reviewer reads its scope and claim list
- THEN it names the nine model theorems and states that correspondence with the Rust evaluator, `wasm-opt`, V8 or any host FPU is not established
