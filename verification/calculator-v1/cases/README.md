# Calculator-v1 Case Gate (`CALC-01` through `CALC-13`)

[`gate.mjs`](gate.mjs) executes source-bound verification lanes for
[`specs/conformance/calculator-cases.json`](../../../specs/conformance/calculator-cases.json)
against a clean commit and a debug `noble` binary built with
`--no-default-features`. Full raw logs and artifacts go to an external `--out`
directory; sanitized compact receipts may be retained under `runs/` via
`--write-compact`.

## Lanes and Completeness Rules

| Lane | Command / Runner | Covered Cases & Requirements |
|---|---|---|
| `cli` | [`../acceptance.mjs`](../acceptance.mjs) | All 40 CLI vectors (`CALC-01`, `CALC-02`, `CALC-03`, `CALC-06`, `CALC-07` 5/7 variants, `CALC-09` 4 display + 6 `later-features`) + 2 negative controls |
| `state-host` | Multi-line `noble calculator` + `NOBLE_CALCULATOR_TRACE` + `CoreEngine` (`Calculator-v1`) + `engine-calculator.test.mjs` | Proves `state_unchanged: true` across failed `CALC-02`/`CALC-03` lines and `calculator_core_host_requests: 0` (`hostFunctions = []`) |
| `oracle` | [`../oracle/compare.mjs`](../oracle/compare.mjs) `cases` and `receipts` | Independent `BigInt` oracle check against `calculator-cases.json` (38/38) and executed CLI receipts |
| `numeric-fast` | `cargo test --locked --offline -p noble-wasm --test calculator_numeric_paths` | Default-tier tests (`calc_04_zero_denominator`, `calc_05_core_wrap_is_core_semantics`, `calc_05_no_implicit_conversion`, `native_step_quota_is_not_a_guest_resource_limit`) |
| `numeric-slow` | `cargo test --locked --offline -p noble-wasm --test calculator_numeric_paths -- --ignored` | Slow `#[ignore]` vectors required for full `CALC-04` and `CALC-05` promotion (`--slow-timeout-s`) |
| `cli-unit` | `cargo test --locked --offline -p noble-cli --no-default-features --bin noble calculator::tests` + `--test calculator_engine` | Committed-session byte-exact restore (`failed_lines_of_every_class_restore_the_committed_session`) and runtime adapter tests |
| `budgets` | [`../budgets/run.mjs`](../budgets/run.mjs) + [`../budgets/summarize.mjs`](../budgets/summarize.mjs) | Full v2 `CALC-08` 20-session bracket & control matrix (requires `--slow-timeout-s` headroom in debug builds) |

## Promotion Eligibility (`deriveVerdicts`)

A case is marked `promotable` only when the working tree is clean, the binary
digest is unchanged across the run, and **every** required lane and `expected`
field for that case passed on the same clean `tested_commit`:

- `CALC-01`, `CALC-02`, `CALC-03`, `CALC-06`: require `cli`, `state-host`, `oracle` (plus `cli-unit` restore for `CALC-06`).
- `CALC-04`, `CALC-05`: require **both** `numeric-fast` and `numeric-slow`.
- `CALC-07`: `promotable-partial` (`implementation: "partial"`, `execution: "passed"`) when all 5 CLI variants + follow-up `UnknownName` probes + `state-host` pass; the 2 import/resource variants are `unsupported-on-cli` per `OWNER DECISION 2026-11-20` (Decision 17).
- `CALC-08`: requires `budgets` (20/20 matched, `resource_limit_lines: 0`), `numeric-fast` native step quota check, and [`../budgets/review-2026-11-23.md`](../budgets/review-2026-11-23.md).
- `CALC-09`: requires `cli` (all 10 vectors) and `oracle`, with the accepted `later_features` refusal reading and caveat recorded per `OWNER DECISION 2026-11-20` (Decision 26).
- `CALC-10`, `CALC-11`: `MA3` scope (Decision 17); stay `absent` / `not-run`.
- `CALC-12`, `CALC-13`: stay `absent` / `not-run` (release mutant/entry gates and open `acorr` / `MA-12/13` prerequisites).

## Usage

```sh
node verification/calculator-v1/cases/gate.mjs --self-test
cargo build --locked --offline -p noble-cli --bin noble --no-default-features
node verification/calculator-v1/cases/gate.mjs \
  --binary <NOBLE_BIN> \
  --out <EXTERNAL_OUT> \
  --lanes cli,state-host,oracle,numeric-fast,cli-unit \
  --concurrency 4 \
  --write-compact verification/calculator-v1/cases/runs/<YYYYMMDD>-<shortsha>/acceptance.json
```
