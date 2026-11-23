# Exact-Number `current28` Gate on `Signed-Declared-Text-v1`

Owned inside `.cairn/changes/ma-language-prerequisites` task 4 under Owner
Decisions 16 (2026-11-20) and 58 (2026-11-23). This gate does not create a
separate change and does not promote `CALC-01`–`CALC-13`.

## Architecture

The `current28` library implements arbitrary-precision signed integers
(`exact_num@1.BigInt`) and normalized exact rationals (`exact_num@1.Rat`) as
pure declared Noble modules on the unmodified 28-builtin
`Signed-Declared-Text-v1` profile (`noble session --framed --declared-modules
--text-byte-cursor --signed-self-recursion`):

- **Representation:** Canonical LSB-first `List<I64>` limbs in base `10^9`
  (`0 <= limb < 1_000_000_000`, `nil` for zero, non-zero MSB limb).
  `BigInt` is `opaque BigInt Pair<Bool, List<I64>>` with canonical zero
  `(false, nil)`. `Rat` is `opaque Rat Pair<BigInt, BigInt>` with `d > 0`,
  `gcd(|n|, d) = 1`, and canonical zero `0/1`.
- **Errors:** `exact_errors@1.ExactError` (`DivisionByZero`, `InvalidDecimal`)
  returned through the repository's `result@1.Result<A, E>`
  (`crates/noble-contracts/src/fixtures/result.noble`).
- **Budget discipline:** Every module contains 1–6 small stack-oriented
  definitions (zero `let`-binding blowup) so frontend inference stays well
  within the 16,384-node per-module budget and backend acceptance stays within
  the per-submission compiler work budget. Between completed guest operations,
  the driver issues `drop` and the signed framed host-control header
  `:release-observed-roots STORE_UUID OBSERVATION_ID` so every epoch retires
  cleanly (`heap_cells_after = 0`, `retained_core_entries = 4`).
- **Modules (`modules/`):**
  - `exact-errors.noble`: `ExactError` variant (`DivisionByZero`, `InvalidDecimal`).
  - `exact-i64.noble`: overflow-free `divmod_pos` and single-limb `i64_gcd`.
  - `exact-mag-core.noble`: `mag_rev_loop`, `mag_rev`, `mag_norm`, `mag_len_acc`.
  - `exact-mag-add.noble`: single-pass `mag_cmp_acc` and base-`10^9` `mag_add_carry`.
  - `exact-mag-sub.noble`: base-`10^9` `mag_sub_borrow` and `mag_sub`.
  - `exact-mag-ml.noble`: limb multiplication `mag_mul_limb`.
  - `exact-mag-div.noble`: `mag_div_limb_msb`, `mag_msb_top`, `mag_drop_k`.
  - `exact-knuth.noble`: Knuth Algorithm D normalization (`knuth_scale`) and
    2-limb quotient estimation (`knuth_est`).
  - `exact-parse-scan.noble`, `exact-parse-limbs.noble`, `exact-parse-dec.noble`,
    `exact-pow10.noble`: guest-native `text.byte` decimal scanners, 9-digit
    base-`10^9` limb packer, and exact `10^k` builder.
  - `exact-big-i64.noble`, `exact-big-cmp.noble`, `exact-big-route.noble`,
    `exact-num.noble`, `exact-rat-prep.noble`: `I64` conversion (including
    `I64_MIN`), signed comparison, add/sub routing, opaque `BigInt` and `Rat`
    constructors/unpackers/negation, and division-by-zero refusal.

## Running the Gate

Build the CLI binary from a clean tree and pass its path, SHA-256 digest, and a
fresh output directory:

```sh
cargo build -p noble-cli --bin noble
BINARY=target/debug/noble
DIGEST=$(sha256sum "$BINARY" | awk '{print $1}')
node verification/exact-current28/gate.mjs "$BINARY" "$DIGEST" <output-dir>
```

The gate runs both `--opt off` and `--opt on` framed sessions against the
independent JS `BigInt` oracle (`oracle.mjs`), verifies host-observation
control-frame refusals and retirements, checks emitted WAT/Wasm digests and
zero host function imports, and executes a guest-mutation authenticity control.

## Scope & Non-Claims

Passing this gate provides measured `current28` exact-number evidence on
`Signed-Declared-Text-v1`. It does **not** promote `CALC-01`–`CALC-13`, does
**not** close the Worker selected-vendor or combined signed-epoch/live sweep
requirements, and does **not** claim tail-call recursion beyond the 512-frame
continuation bound.
