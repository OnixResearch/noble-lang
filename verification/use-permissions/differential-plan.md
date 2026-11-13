# Differential and control plan for issue #112

This lane does not add a four-way production profile. It preserves three different questions:
kernel-checked model facts, finite expected decisions and source-profile behavior, and a future
implementation correspondence that is explicitly not claimed.

## Independent expected decisions

`cases.mjs` is a small Node reference evaluator independent of the Rust acceptance checker. Its
`expectedSingle` table is written as fixed decisions for each class and word; multi-position,
Program, join, and error-return cases also pass a literal expected result to `push`. `push` compares
that declared decision with the Node evaluator and throws instead of emitting a corpus on mismatch.
The resulting `cases.txt` stores both `decision` and `expected` on every row. The Lean `matrix`
separately stores concrete expected `Option` outputs and proves `matrix_holds` with `decide`; neither
the Node outcomes nor the Rust checker generate those Lean expectations.

This is an independent finite oracle, not a cross-language proof that the Node and Lean source
implementations agree. That gap is visible and bounded rather than hidden by generating all three
outputs from one implementation.

## Model decision coverage

The complete finite decision basis crosses all four classes with `dup`, `drop`, `swap`, `dip`,
`quote`, `compose`, and `run`. Positive and negative rows are kept for each meaningful bit:

* `Data (1,1)`: both `dup` and `drop` succeed; capture succeeds.
* `must-use (0,0)`: both operations and capture refuse.
* `affine (0,1)`: `dup` and capture refuse; `drop` succeeds.
* `must-handle (1,0)`: `dup` succeeds; `drop` and capture refuse.

Pair and Sum combine both payloads by bitwise conjunction; List and opaque wrappers inherit; a
variant wrapper combines both arms. The finite controls include nested must-use payloads, Program
positions, and the `must-handle` capture counterexample. A successful Program duplication after a
bad quotation is deliberately contrasted with the reference refusal at quotation, so no
duplicability of Program can erase an unpermitted captured payload.

The branch corpus includes identical arms, a missing result position, and a branch that duplicates
a must-handle tagged position in only one arm. Both arms may be locally admissible but the join
refuses because the ordered identity/obligation stacks differ. Equal counts with different owner
tags and same-permission positions with different tags are separate join controls in the Lean
model. These tags are symbolic positions/obligations, not runtime scalar values; this deliberately
small join model does not model differing branch-produced Data values or effect traces, and its
exact tag equality is not asserted as a production checker rule. The model's ordinary error return accepts an empty stack and discardable Data/affine
positions, and refuses must-use and must-handle positions. The #59 ownership `fail` control remains
separate: it refuses a nonempty Data operand stack.

The `source` versus `none` field labels each generated model row. Only one-word cases with plain
Data values are called source analogues; the four `.noble` forms exercise current `dup`/`drop` and
arithmetic. Permission-bearing values, raw Program positions, two-arm branches, and error returns
are not attributed to the CLI.

## Seeded incorrect rules

The Lean model names and refutes these deliberate rule mutations with retained concrete controls:

1. `drop` always succeeds, even without the drop bit.
2. `dup` always succeeds, even without the dup bit.
3. Capture checks only the dup bit.
4. Capture checks only the drop bit.
5. Capture ignores both bits.
6. An ordinary error return ignores outstanding obligations.
7. A branch join keeps the first arm without checking the other.
8. A Program inherits the environment permission instead of remaining unrestricted.

`forbidden_capture_then_program_duplication` composes the duplicate-only Capture mutation with
Program duplication and drops both copies. The mutant admits the chain, while the reference refuses
at `quote` for the must-handle payload. This is a model control only; it neither implements nor
tests a one-shot closure.

## Future production correspondence boundary

No implementation correspondence is currently run because there is no separate permission
checker to compare. If a future design passes the go criteria, add its comparison as a path module
under the existing `crates/noble-kernel/tests/acceptance/ownership.rs` target. For the current
profile, only the existing Data/resource semantics can be compared: a Resource or LiveRef cannot
be made duplicable/discardable by recursively wrapping it, and Program interface names do not
transfer ownership. Test expectations must come from this independent case specification, not from
the implementation under test. Do not extend #59's receipts or describe current `is_data` as four
separate outcomes.

The future correspondence test would need to bind cases to source/profile identity and distinguish
expected permission refusals from shape errors, join conflicts, and explicit bounded-check
exhaustion. That work is out of scope until the language change is independently approved.
