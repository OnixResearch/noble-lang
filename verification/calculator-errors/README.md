# Selected calculator numeric-error schema

`crates/noble-contracts/src/fixtures/calc-errors.noble` is a pure, versioned
Noble declared module. Link `result@1` from
`crates/noble-contracts/src/fixtures/result.noble` before `calc_errors@1`.
It exports nominal types, constructors and exhaustive matchers, not a
hard-coded error generator. A caller can construct `ResourceLimit/InputBytes`
with:

```text
unit calc_errors@1.LimitIO.InputBytes
calc_errors@1.LimitPrimary.IO calc_errors@1.LimitCategory.Primary
calc_errors@1.NumericError.ResourceLimit result@1.Result.Err
```

To constrain the otherwise polymorphic `Result.Err` success type to `I64`,
the observation's real caller retains that Result and consumes a duplicate
through `Result.match` with an `I64` success callback and an error callback.
The error callback actually runs. The current source signature parser rejects
non-generic local nominal types nested in generic signatures with
`unknown signature value type`; no fake constant branch or synthesized typed
`failed` production word is installed. A typed source-signature interface is a
separate upstream language gate.

The exported acyclic variants distinguish seven limit categories (`InputBytes`,
`NumericDigits`, `IntermediateIntegerBits`, `SyntaxDepth`, `CallDepth`,
`EvaluationWork`, `OutputSize`) and five numeric errors (`ZeroDenominator`,
`DivisionByZero`, `NonIntegral`, `OutOfRange`, `NonIntegerExponent`). Consumers
construct an error with `unit`, the corresponding exported leaf constructor,
each ancestor constructor, and `NumericError.ResourceLimit` or
`NumericError.Domain`; exported `.match` words permit exhaustive elimination.
No string-to-error decoder exists or is implied. The module does not establish
budget units, safe numeric arithmetic, BigInt, Rat, calculator execution,
CALC-08, or MA1 acceptance.

For a bounded observation without Cargo or Nix, set `NOBLE_BIN` to the exact
executable to exercise, `EXPECTED_SHA256` to its full SHA-256, and
`BUILD_SOURCE_ID` to its associated source snapshot (including HEAD and hashes
of modified runtime files). Set `OBSERVATION_DIR` to a fresh directory outside
the repository. The binary owner must identify the build's source snapshot;
the gate checks the binary hash but cannot establish source provenance. Run
the gate with `node` from the selected Node 24.13.0 toolchain.

```sh
node \
  verification/calculator-errors/gate.mjs \
  "${NOBLE_BIN:?}" "${EXPECTED_SHA256:?}" "${BUILD_SOURCE_ID:?}" \
  "${OBSERVATION_DIR:?}"
```

The observation retains
raw child stdin/stdout/stderr and source hashes but no persistent WAT/Wasm dumps.
It links both actual modules, executes the real compiled managed-Wasm guest
with optimization off and on, checks all seven category identities and five
numeric error identities, and requires static refusal of wrong nominal
category and unknown error name. The guest reports its compiled Wasm hash.
The JSON receipt records the exact prebuilt binary hash and owner-supplied
source identity; it does **not** authenticate the binary as built from this
calculator branch. An `observed` result is a finite tool observation, not
source-bound release evidence or promotion of calculator cases.
