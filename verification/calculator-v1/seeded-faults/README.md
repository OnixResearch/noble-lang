# CALC-12 seeded incorrect implementations

CALC-EVIDENCE-01 requires that seeded incorrect implementations exercise the
calculator harness. Each fault named by
[CALC-12](../../../specs/conformance/calculator-cases.json) is a real source
mutant: a patch in [`patches/`](patches/), built into its own `noble` binary
and run through the #19 acceptance runner. The [oracle lane](../oracle/README.md)
then judges the run. A mutant counts as rejected for its intended reason only
when both of these hold:

1. The comparison verdict is `disagree` on the fault's probe vector.
2. `matches_fault_models` names exactly that fault: the harness model in
   [`../oracle/faults.mjs`](../oracle/faults.mjs) predicts exactly the
   observed output.

An unpatched control binary, built from the same base and target directory,
runs the same probes and must `agree`. This shows that the rejection comes from
the patch, not from the build.

**Status on 44329dc/ffcce53.** Four mutants (reversed-subtraction,
left-associated-power, non-normalized-rational, failure-mutates-session) are
rejected for their intended reason as real binaries. The integer-division,
I64-wrapping and binary-float-decimal-parser mutant binaries could not run:
the selected backend refused the mutated phase, so their runs are
`not-comparable` and those three faults are rejected only as harness-level
models. See [the results](../calc-12-results.md#three-mutants-the-real-binary-could-not-run).

| Fault | Patch | Mechanism | Probe (`fault-vectors.mjs`) |
|---|---|---|---|
| integer-division | guest `num@1.op_divisor_admit` | When both operands are integers, the dividend is replaced by its truncated I64 quotient `trunc(a/b)` (calculator `i64.div-rem` on the magnitudes, sign by XOR) and the divisor by `1`, so `a / b` yields `trunc(a/b)`. | `7 / 2` |
| binary-float-decimal-parser | guest `num@1.parse_decimal_finish` | A non-integer literal `num/den` becomes `round_half_up(num·2^55/den)/2^55`, computed with I64 words. For `0.1` this is exactly the IEEE-754 binary64 value `3602879701896397/36028797018963968`. | `0.1` |
| I64-wrapping | guest `num@1.op_begin_add` / `_sub` / `_mul` | When both operands are integers, they are converted to I64 and combined with Core's wrapping `+`, `-` or `*`; the wrapped result replaces the left operand, and the identity (`0`, or `1` for `*`) replaces the right. | `9223372036854775807 + 1` |
| reversed-subtraction | guest `num@1.op_begin_sub` | The operand pair is swapped (`[ swap pair ] dip`), so `a - b` computes `b - a`. | `10 - 3` |
| left-associated-power | guest `expr@1.should_pop` | At equal precedence the shunting-yard parser always pops, so `^` groups left. | `2^3^2` |
| non-normalized-rational | guest `num@1.gcd_unit_bits_p` | Every gcd test reports one, so the numerator and denominator are never divided by their gcd. | `2 / 4` |
| failure-mutates-session | host `calculator.rs` `Session::submit` | When a line fails, the shell clears the accepted history instead of keeping it, so a failed submission changes the session. | `let x = 20`, `let x = 1 / 0`, `x` |

## Why these placements

- **Six faults are guest mutants.** Arithmetic, parsing and normalization all
  live in the guest. The host only schedules phases (see the
  [CALC-13 review](../calc-13-review.md#host-shell-boundary)), so a fault there
  is a guest-source edit.
  - Every patch keeps the edited word's stack effect.
  - The six guest patches add no new definitions; they edit existing ones.
  - A mutant counts only if its binary passes the startup source check
    (`Session::new` type-checks all eleven modules); the results record this
    for each mutant.
- **failure-mutates-session is a host mutant.** The guest binds a `let` only
  after evaluation succeeds, so no guest-only edit can publish state on
  failure. Transactional publication belongs to the host's candidate/replay
  `submit`, so the fault is seeded there.
- **binary-float-decimal-parser rounds to a fixed binary fraction.** Noble has
  no binary floating-point type. Full binary64 emulation (exponent-aware
  rounding) would mean writing a soft-float in Noble, not seeding a fault.
  Rounding non-integer literals to a fixed binary fraction keeps the defect
  class: a decimal literal passes through a binary approximation. For the
  probe `0.1` the result equals binary64 exactly, so the harness IEEE model
  predicts the same output.
  - Integer literals stay exact in this mutant, unlike in a full binary64
    parser.
  - The rounding itself uses I64 words, so it is faithful only while
    `num·2^56` fits in I64, that is for a combined digit value below 128,
    such as `0.1` or `9.9`.
- **Three placements could not be made to run.** The selected backend bounds
  what one compiled phase may reach and how many definitions a module may
  hold. Attempt-1 patches put BigInt long division or rounding into large
  phases. The committed (attempt-2) patches for integer-division, I64-wrapping
  and binary-float act with the calculator profile's I64 words in
  `op_divisor_admit`, `op_begin_*` and `parse_decimal_finish`. The binary
  still refused those phases at run time (clone fanout or compiler Work
  exhausted). A third binary-float attempt with its own definition exceeded
  the module's definition count at startup
  ([all attempts](../calc-12-results.md#three-mutants-the-real-binary-could-not-run)).
  Integer-division and I64-wrapping therefore apply only when both operands
  are integers; rational operands pass through unchanged. Integer-division
  also assumes magnitudes below 2^63. Wrapping I64 arithmetic is exactly the
  semantics this I64-wrapping fault names.

## Reproduction

```sh
# FAULT is one of the names above, or `control` for the unpatched base
verification/calculator-v1/seeded-faults/build-mutant.sh FAULT BASE_REV SCRATCH_WORKTREE TARGET_DIR OUT_DIR
node verification/calculator-v1/seeded-faults/fault-vectors.mjs --fault FAULT --out VECTORS.json
node verification/calculator-v1/acceptance.mjs --binary OUT_DIR/noble-FAULT --binary-sha256 SHA \
  --source-id "BASE_REV + patches/FAULT.patch" --build-profile release --out RUN_DIR \
  --vectors VECTORS.json --no-controls
node verification/calculator-v1/oracle/compare.mjs receipts --run RUN_DIR --out REPORT.json
```

`build-mutant.sh` resets a dedicated scratch worktree to `BASE_REV`, applies
the patch, and builds the release CLI. It records the patch and binary
SHA-256 in `OUT_DIR/noble-FAULT.build`. Run it inside the repository's pinned
toolchain shell.

`fault-vectors.mjs` takes expectations from the exact oracle. It refuses to
emit a probe unless that fault's model and the oracle disagree on it.
`acceptance.mjs` is the #19 runner.

Results and receipts are recorded in [`../calc-12-results.md`](../calc-12-results.md).

These patches are test fixtures only. They are never applied to the
repository's guest sources or shell.
