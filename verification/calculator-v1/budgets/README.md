# CALC-08 budget brackets

`run.mjs` runs [`plan.json`](plan.json) against an exact prebuilt `noble`
binary. Each session is one `noble calculator` process with one stdin. A
category bracket changes exactly one of the seven guest budgets. The other six
keep their defaults (1,024 / 256 / 4,096 / 32 / 16 / 4,000,000 / 4,096), and
every setup, probe and follow-up line fits them. The units and charge sites
behind each predicted N are in
[design.md, Guest budget units](../../../.cairn/changes/calculator-v1/design.md).

The receipt convention follows [calculator-errors](../calculator-errors/README.md):

- the owner supplies the binary's SHA-256 and source identity, and the runner
  refuses a mismatch;
- output goes to a fresh directory outside the repository;
- the raw stdin, stdout and stderr, plus timestamped output events, are kept
  per session;
- the JSON receipt states explicit non-claims.

Each session runs under
`systemd-run --user --scope -p MemoryMax=2G -p MemorySwapMax=0` (some controls
override the cap). GNU `time` records the wall time and the peak RSS of the
`noble` process tree. A wall-clock timeout kills the whole process group.

```sh
node verification/calculator-v1/budgets/run.mjs \
  --binary "${NOBLE_BIN:?}" --binary-sha256 "${EXPECTED_SHA256:?}" \
  --source-id "${BUILD_SOURCE_ID:?}" --out "${OBSERVATION_DIR:?}" --concurrency 4
node verification/calculator-v1/budgets/summarize.mjs verification/calculator-v1/budgets/plan.json \
  verification/calculator-v1/budgets/summary.json "$OBSERVATION_DIR"/receipt.json [MORE_RECEIPTS...]
```

[`summary.json`](summary.json) is the compact committed form of one run. It
records the binary hash, argv, stdin lines, stdout and stderr lines, exit
status, wall time and peak RSS of every session. It also records whether each
session matched its plan expectation.

## Observations

Partial: a run on the release snapshot of `44329dc` (sha256
`e1fb70bac358e7ffe1a064295f985448b89899c2289d8a3d514cf0cdfc708127`) was in
progress when this was committed. `summary.json` is not committed yet. Observed
so far:

| Session | Budget | stdout | stderr | Wall |
|---|---|---|---|---|
| `output-size/N-1` | `--output-size 6` | `7`, `7` | `ResourceLimit OutputSize [14,14)` twice, then `UnknownName [0,1)` | 2,336 s |
| `call-depth/N-1` | `--call-depth 1` | `2` | `ResourceLimit CallDepth [8,9)` | 3,310 s |
| `input-bytes/N-2` | `--input-bytes 7` | — | `ResourceLimit InputBytes [0,8)`, `[0,9)`, then `UnknownName [0,1)` | 453 s |

- **OutputSize state control.** The session ran `let x=7`, `let x=1234567`,
  `x`, `let y=1234567` and `y`. Both refused definitions left the state
  unchanged: `x` kept its earlier value 7, and `y` stayed unbound.
- **Newline counting.** An unframed line counts its `\n`, both in InputBytes and
  in the 1-per-byte preparse Work charge. The original predictions omitted
  it, so `plan.json` now predicts N = 9 for `x+100000` and N = 38,053 for `77`.
  With the newline, the accounting reproduces the earlier pre-port
  measurement of 96,206 Work units for `2+3*4`.

## Non-claims

- No canonical CALC case state, evidence or implementation status changes.
- The observations hold for the recorded binary only. The runner checks the
  binary's hash but cannot prove which source built it.
- Each bracket shows one deterministic boundary per category. It does not show
  that every charge site, or every input, is bounded.
- The controls show distinct labels for a host timeout, a killed worker, a
  memory-capped scope, and host replay/transport preflight. They do not
  enumerate every native failure.
- Nothing here proves the accounting, exact arithmetic or the conversions.
