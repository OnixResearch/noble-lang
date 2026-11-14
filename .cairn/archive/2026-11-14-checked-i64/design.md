## Context

K-NUM-01 currently defines wrapping `+`, `-` and `*`, signed `I64` equality and deeper-operand-first order. Its D-03 paragraph says the checked library names and contract are deferred. The default environment therefore retains its existing word table and arithmetic behavior. `proofs/foundation/NobleFoundation/CheckedI64.lean` already gives the selected mathematical reference: exact-range success/failure for checked add, subtract, multiply, negate and absolute value, plus truncating quotient/remainder laws, zero-divisor behavior and the `MIN / -1` overflow case. That module proves a mathematical model, not runtime correspondence.

The change chooses a separate `checked-i64` environment. Its builtins operate on signed I64 values and return ordinary data; they do not convert arithmetic-domain failure into a Wasm trap, host request or resource-exhaustion result. The selected environment is opt-in and disjoint from calculator and declared-text profiles.

## Decisions

### Decision: D-03 checked-I64-v1

**Choice:** Select `checked-i64` as an opt-in environment with these exact public words and signatures. `S` denotes an unchanged, arbitrary typed stack prefix; the deeper I64 operand is listed before the top operand. `inl` is the successful result and `inr` is the error code.

| Word | Signature | Successful payload |
|---|---|---|
| `i64.checked-add` | `S I64 I64 → S Sum<I64,I64>` | exact mathematical sum |
| `i64.checked-sub` | `S I64 I64 → S Sum<I64,I64>` | exact mathematical difference, deeper minus top |
| `i64.checked-mul` | `S I64 I64 → S Sum<I64,I64>` | exact mathematical product |
| `i64.checked-neg` | `S I64 → S Sum<I64,I64>` | exact mathematical negation |
| `i64.checked-abs` | `S I64 → S Sum<I64,I64>` | exact mathematical absolute value |
| `i64.lt` | `S I64 I64 → S Bool` | signed comparison, deeper operand `<` top operand |
| `i64.checked-div-rem` | `S I64 I64 → S Sum<Pair<I64,I64>,I64>` | `(quotient, remainder)`, quotient first |

The right-hand `I64` of each `Sum` is an error code in `noble-checked-i64/v1`:

| Code | Name | Meaning |
|---:|---|---|
| `1` | `Overflow` | exact result is outside `[-2^63, 2^63)`; includes `MIN / -1` |
| `2` | `DivisionByZero` | `i64.checked-div-rem` has a zero divisor |

Code `0` is reserved and is not an error value. The sum tag, not a numeric sentinel, distinguishes a successful I64 from an error code. No failure may expose a partial quotient, remainder or wrapped/saturated result.

| Operation | Overflow | Division by zero | Other arithmetic error |
|---|---|---|---|
| `i64.checked-add` | `inr(1)` iff exact sum is out of range | not applicable | none |
| `i64.checked-sub` | `inr(1)` iff exact difference is out of range | not applicable | none |
| `i64.checked-mul` | `inr(1)` iff exact product is out of range | not applicable | none |
| `i64.checked-neg` | `inr(1)` only for `MIN` | not applicable | none |
| `i64.checked-abs` | `inr(1)` only for `MIN` | not applicable | none |
| `i64.checked-div-rem` | `inr(1)` only for `MIN / -1` | `inr(2)` for divisor `0` | none |
| `i64.lt` | never | not applicable | none |
| derived `≤`, `min`, `max` | never; ordinary code invokes no checked arithmetic | not applicable | none |

`i64.checked-div-rem` is the only division operation in v1. Division truncates toward zero; for every nonzero divisor, `a = b*q + r`, `|r| < |b|`, and a nonzero remainder has the sign of `a`. A remainder-only operation and a Euclidean/floor-division variant are not exposed. The Wasm implementation checks zero and the exact `MIN / -1` pair before executing `i64.div_s` or `i64.rem_s`.

Signed `i64.lt` is the sole comparison primitive. `≤` is derived as `if a < b then true else a = b`; `min` and `max` select the corresponding operand using `<`. They are library definitions, not additional numeric authorities or kernel builtins.

**Rationale:** The model already characterizes the selected range and truncation laws. Keeping the API opt-in preserves Core word resolution, all previously accepted source and saved Program identities. A tagged `Sum` keeps success values and versioned error codes disjoint without an effectful exception path. Combined `div-rem` exposes no remainder-only policy ambiguity; its zero and `MIN / -1` cases are fully explicit. Signed comparison is sufficient to derive convenience operations.

**Evidence and criteria:** issue #39's accepted recommendation and `CheckedI64.lean` establish the semantic choice; unit 1 records it before implementation. Runtime agreement is not claimed by this decision. The implementation must pass every operation/error row, preserve the prefix and operand order, avoid traps on domain errors, and match Lean-generated cases at both supported optimization settings.

**Reopen if:** a required consumer cannot express the selected tagged result using ordinary data, an implementation must expose a partial result or host effect, the versioned code table cannot remain stable, or boundary execution disagrees with the Lean model. Any changed error policy or added remainder/Euclidean API requires a new version and decision rather than silently changing v1.

## Risks / Trade-offs

- The Lean model and generated Wasm are not connected by a proof in this slice. Keep model proof, implementation testing, refinement and residual trusted edges separately labelled.
- Adding new `Behavior` variants changes the kernel extraction subject and requires the integrator's source-inventory/extraction-policy renewal; no existing proof receipt is silently extended.
- The runtime must avoid WebAssembly's `i64.div_s` trap for zero and `MIN / -1` by classifying both inputs first.

## Evidence boundary

| Claim class | Status | Evidence and limits |
|---|---|---|
| Model semantics | Proved | `verification/checked-i64/receipts/foundation-summary.json` records the kernel-checked retained-expectations theorem within the 1,451-theorem foundation audit. It establishes model semantics, not runtime correspondence. |
| Implementation behavior | Tested | `verification/checked-i64/receipts/replay-2026-11-13.json` binds a clean source commit and records 57 retained vectors through real source-to-Wasm execution with optimization off and on, plus stack-prefix, refusal and resource-limit controls. The refusal cases expect zero host requests. `verification/checked-i64/receipts/mutants-2026-11-13.json` records all five implementation mutants caught by the retained-vector test. |
| Rust/Wasm refinement | Not proved | The finite replay and mutants do not establish a universal theorem that the implementation refines the Lean model. No such correspondence theorem is included in this slice. |
| Open integration work | Pending integrator | Rust source-inventory renewal and the kernel extraction-policy re-pin are required before integration. These repository-wide bindings are not changed by this slice. |
