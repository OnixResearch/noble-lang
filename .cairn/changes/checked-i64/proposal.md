## Why

K-NUM-01 selects wrapping `I64` arithmetic for the bootstrap operators. The checked-arithmetic complement was deferred to D-03, so programs cannot request exact checked add, subtract, multiply, negation, absolute value or signed quotient/remainder without leaving the language contract unspecified. The foundation now contains a name-neutral mathematical model and universal theorems, but it does not establish a callable runtime implementation.

## What Changes

- Select D-03 as an opt-in `checked-i64` environment. Existing Core, declared-text, calculator and saved-Program identities remain unchanged unless this environment is selected.
- Expose exact checked arithmetic as `Sum<value, I64>` using the versioned `noble-checked-i64/v1` error-code table. Add signed less-than; derive `≤`, `min` and `max` in library code.
- Select truncation toward zero for the combined `i64.checked-div-rem` operation. Do not expose remainder-only or Euclidean division in this version.
- Add real source-to-Wasm execution evidence, boundary controls, mutation controls, resource-limit/rejection coverage and separately scoped proof/testing/refinement records.

## Impact

- **Files**: `.cairn/specs/language/spec.md`; `crates/noble-kernel`; `crates/noble-contracts`; `crates/noble-wasm`; `crates/noble-cli`; the existing `crates/noble-ngraph` source matches; `proofs/foundation`; `verification/checked-i64/`; the root `README.md`.
- **Testing**: focused kernel/source/Wasm/CLI tests; actual Wasm execution at optimization off and on against Lean-generated expectations; implementation mutants; workspace tests, clippy, format and Octet; Lean foundation audit; Cairn validation and document self-tests.
