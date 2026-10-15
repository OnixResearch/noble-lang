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

These are runs of the release snapshot of `44329dc` (sha256
`e1fb70bac358e7ffe1a064295f985448b89899c2289d8a3d514cf0cdfc708127`). Each
session ran under a 2 GiB memory cap. The peak RSS of the `noble` process
tree was 195–225 MiB. Wall times are from a heavily loaded 32-core host.

| Category | Budget N−1 / N | Probe line | N−1 stderr (stdout) | N stdout | Wall N−1 / N |
|---|---|---|---|---|---|
| InputBytes | 8 / 9 | `x+100000` (9 bytes with `\n`) | `ResourceLimit InputBytes [0,9)` (`7`, `7`) | `7`, `100007`, `7` | 1,193 s / 1,901 s |
| NumericDigits | 2 / 3 | `x+123` | `ResourceLimit NumericDigits [2,5)` (`7`, `7`) | `7`, `130`, `7` | 1,157 s / 1,551 s |
| IntermediateIntegerBits | 25 / 26 | `1234567` (w = ⌊(3322·7+999)/1000⌋+2 = 26) | `ResourceLimit IntermediateIntegerBits [0,7)` (`7`, `7`) | `7`, `1234567`, `7` | 1,096 s / 1,214 s |
| SyntaxDepth | 2 / 3 | `(((x)))` | `ResourceLimit SyntaxDepth [2,3)` (`7`, `7`) | PENDING | 1,040 s / — |
| CallDepth | 1 / 2 | `f(1)`, where `f` calls `g` | `ResourceLimit CallDepth [8,9)` (`2` from the follow-up `g(1)`) | `3`, `2` | 3,310 s / 3,873 s |
| EvaluationWork | 38,052 / 38,053 | `77` (predicted 3 + 50·19² + (50+25+25)·10² + 100·10² = 38,053) | PENDING | PENDING | — |
| OutputSize | 6 / 7 | `let x=1234567` (7 bytes of display) | `ResourceLimit OutputSize [14,14)` twice, then `UnknownName [0,1)` (`7`, `7`) | `7` and `1234567` ×4 | 2,336 s / 4,206 s |

All the brackets except CallDepth run `let x=7`, then the probe, then `x`.
Every N−1 refusal names its category, and the follow-up `x` (or `g(1)`)
still sees the state from before the refusal.
`input-bytes 7` (N−2) refuses even the 8-byte setup line. An unframed line
counts its `\n`, both in InputBytes and in the one-unit-per-byte preparse
Work charge.

- **OutputSize state control.** `--output-size 6` ran `let x=7`,
  `let x=1234567`, `x`, `let y=1234567`, `y`. Both definitions were refused
  before commit: `x` kept 7, and `y` stayed unbound (`UnknownName`). With
  `--output-size 7`, the same lines committed.
- **Distinctness.** An earlier smoke run with the same binary checked three
  controls:
  - an oversized line gives `TransportLimit InputFrameBytes` and exit 2;
  - a forced 20 s timeout ends in SIGKILL with no output;
  - a 48 MiB memory-capped scope ends in SIGTERM with no output.

  None of them printed a `ResourceLimit` line. The `HostLimit
  HistoryBytes`/`ReplayWork` and killed-worker controls are in `plan.json`
  and not yet observed. The library harness
  (`native_step_quota_is_not_a_guest_resource_limit`) shows that a lowered
  runtime step quota is a runtime status with quota reason 6, and produces no
  guest value.

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
