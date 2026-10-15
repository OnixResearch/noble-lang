# CALC-12 invalid-evidence refusal checks

[`gate.mjs`](gate.mjs) judges an *evidence claim* about the exact calculator
against a subject manifest of the actual implementation. The manifest is the
pinned `noble` binary SHA-256 plus the SHA-256 of every guest fixture that
`crates/noble-cli/src/calculator.rs` embeds with `include_bytes!`.
[`controls.mjs`](controls.mjs) builds the four CALC-12 invalid-evidence
controls and three calibration controls, runs the gate on each, and writes a
receipt in a fresh directory outside the repository.

This lane is separate from the #19 acceptance runner and from the oracle
comparison, and it neither checks nor accepts any proof. Its strongest results
are:

- `not-refused-structurally` for a proof claim, which is **not** acceptance;
- `admissible-test-observation` for a finite test claim, which establishes no
  universal property.

| Control | Claim | Intended refusal |
|---|---|---|
| `proof-for-wrapping-I64` | The #15 Mathlib comparator for wrapping `i64.add/sub/mul` (`verification/mathlib-i64/compare.mjs`) certifies calculator `+ - *` | `wrong-numeric-model` (CALC-EVIDENCE-02) |
| `proof-for-handwritten-reference-only` | A proof about the hand-written CALC-12 reference rationals (`oracle/rational.mjs`) certifies normalization and addition | `subject-not-actual-implementation` |
| `proof-with-unmet-zero-divisor-precondition` | Division is exact for all inputs, assuming a nonzero divisor, with no `DivisionByZero` branch | `unmet-zero-divisor-precondition` |
| `unrelated-Wasm` | A real Wasm module that the pinned binary emits for Core-Bootstrap `1 2 +` (it yields `3`) evidences calculator `1+2` | `unrelated-wasm` (its report profile is `Core-Bootstrap`, not `Calculator-v1`) |
| `calibration/test-claimed-universal` | Finite vectors claimed to prove the parser correct for every input | `tests-claimed-universal` |
| `calibration/bound-proof-shape` | A bound exact-model division claim with an explicit `DivisionByZero` branch | `not-refused-structurally`: the gate does not refuse a claim merely for being a proof, and does not accept it |
| `calibration/bound-test-observation` (with `--comparison`) | Finite vectors agree with the independent oracle on the pinned binary | `admissible-test-observation` |

Each invalid-evidence claim is otherwise well formed. A control passes only
when the gate refuses it with exactly its intended refusal code and no other,
which shows the refusal happened for the intended reason.

```sh
node verification/calculator-v1/evidence-controls/controls.mjs \
  --binary NOBLE --binary-sha256 SHA --out NEW_DIR [--comparison ORACLE_REPORT.json]
```

The receipt (`NEW_DIR/receipt.json`) records:

- the manifest;
- the gate and controls file hashes;
- every generated claim with its SHA-256;
- each decision and its refusal codes;
- `all_ok`;
- explicit non-claims.

`ALL_DONE` or `FAILED` marks completion.
