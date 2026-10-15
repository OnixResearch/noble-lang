# Calculator-v1 CLI acceptance runner

`acceptance.mjs` runs the canonical runtime calculator vectors (CALC-01, -02,
-03, -06, -07 and -09) against an exact prebuilt `noble` binary. It follows the
[calculator-errors](../calculator-errors/README.md) receipt convention: the
owner supplies the binary's SHA-256, build profile and build-source identity;
output goes to a fresh directory outside the repository; raw stdin, stdout and
stderr are retained per process; and a JSON receipt states explicit non-claims.
A receipt is an implementation observation only. It never changes a canonical
case's `state` or `evidence`.

## Vectors

`vectors.mjs` derives every vector from
`specs/conformance/calculator-cases.json` through the single reviewed table
[`cli-mapping.json`](cli-mapping.json). That table spells structured CALC-06
and CALC-07 operations with the README syntax `let NAME = EXPR` /
`fn NAME(P, ...)=EXPR`, maps CALC-09 modes to flags, and lists the CALC-07
variants `cyclic-imported-definition-graph` and `resource-capability-capture`
as `unsupported-on-cli`: the line grammar has no imports or resources. These
two appear in the receipt's `unsupported` list and are never counted as
passes. The three rejecting CALC-07 variants each add one follow-up call that
must report `UnknownName`, observing `rejected_definition_installed: false`.
Expected values and categories come from the cases file; where a case states
no span (CALC-03, CALC-06, CALC-07) the observed span is recorded but not
judged.
A successful `let` prints its bound value, and a successful `fn` prints
nothing. The cases file states no value for a definition. The runner
therefore requires the echoed value to equal the expression only when the
expression is a canonical integer literal (both CALC-06 `let`s are). Any
other echo only has to be an exact display.

Ids are `CALC-01/01`…, `CALC-06/session` and `CALC-07/<variant>`.
`--emit-vectors NEW_FILE` writes the generated list without running anything;
an edited copy can be passed back with `--vectors FILE`.

## Execution

Each vector runs as its own `noble calculator` process, and the CALC-06
session runs as one process. `supervise.mjs` owns that process group. It
kills the whole group with SIGKILL on timeout, and it samples peak RSS per
process (`noble` and its Node Wasm worker) from `/proc`. With `--memory-max`
the supervisor and the calculator run inside a
`systemd-run --user --scope -p MemoryMax=… -p MemorySwapMax=0
-p OOMPolicy=continue` scope; the receipt then also records the scope's exact
`memory.peak` and `memory.events`. The supervisor (about 50 MiB) counts
against that cap.

Each result is classified as exactly one of:

| Class | Meaning |
|---|---|
| `pass` | exit status, every stdout line and every diagnostic category (and stated span, with `provenance=unavailable`) match |
| `fail` | the calculator completed with a different guest result, including any unexpected `ResourceLimit` |
| `timeout` | the runner killed the process group at the wall-clock limit |
| `native-failure` | a signal, an exit status other than 0/1, a launch error, or a `NativeFailure:`/`InfrastructureFailure:`/`noble calculator:` line |
| `host-limit` | a memory-cap OOM kill, or a `HostLimit`/`TransportLimit` line |

A timeout or native failure is never a pass, and never a `ResourceLimit`.
Two negative controls run unless `--no-controls` is given. Both reuse the
exact argv and stdin of `--control-base` (default `CALC-02/02`, `1 2`). One
has a deliberately wrong expectation and must classify as `fail`; the other
has a forced `--control-timeout` (default 1 s) and must classify as
`timeout`. The runner exits non-zero if either control does not.

## Usage

Run the runner with the pinned Node 24.13.0. Defaults are: concurrency 2, a
7,200 s per-vector timeout, a 21,600 s CALC-06 session timeout, no memory cap
and default guest budgets. `--calculator-arg` appends an extra calculator flag
to every vector, and that flag is recorded.

```sh
node verification/calculator-v1/acceptance.mjs \
  --binary "${NOBLE_BIN:?}" --binary-sha256 "${EXPECTED_SHA256:?}" \
  --source-id "${BUILD_SOURCE_ID:?}" --build-profile release \
  --out "${OBSERVATION_DIR:?}" --memory-max 6G --concurrency 2 \
  [--case CALC-01,CALC-02] [--vector CALC-02/02] [--vectors FILE]
```

The output directory contains:

- `vectors/<id>/`: `stdin`, `stdout`, `stderr` and the supervisor spec and result;
- `progress.jsonl`: one line per finished process;
- `receipt.partial.json`: rewritten as the run proceeds;
- `receipt.json`: the final receipt;
- `ALL_DONE` or `FAILED`: written when the runner finishes.

The receipt records:

- the repository commit and any tracked changes;
- the SHA-256 of the cases file, the mapping table, the runner files, and every
  calculator fixture and worker source embedded through `include_bytes!` or
  `include_str!`;
- the binary hash, owner-supplied build profile and source identity;
- the runner Node, the worker Node path found in the binary and its version,
  and the `--internal-calculator-wat-version` assembler output;
- the run configuration;
- each process's exact argv, environment, stdin bytes, exit status, signal,
  timeout, wall time and memory.

The runner checks the binary hash. It cannot prove that the binary was built
from the recorded source.

## Receipt: 44329dc release snapshot

[`receipt-44329dc-release.json`](receipt-44329dc-release.json) is a compact
field subset of one full run. The complete runner receipt and the raw
per-vector stdin, stdout, stderr and supervisor files are in
[`receipt-44329dc-release.tar.gz`](receipt-44329dc-release.tar.gz) (its
`receipt.json` has SHA-256 `a6256582…b1d65d`, recorded in the compact file).
The run used:

- binary `noble` built from `44329dc` (owner-supplied identity), release
  profile, SHA-256
  `e1fb70bac358e7ffe1a064295f985448b89899c2289d8a3d514cf0cdfc708127`;
- runner commit `1e16953` with no tracked changes, on host aspen3 (32 CPUs),
  from 2026-10-15 17:20:22Z to 19:45:11Z;
- concurrency 6, a 7,200 s vector timeout, a 21,600 s session timeout,
  `MemoryMax=2G` and `MemorySwapMax=0` per vector scope, and default guest
  budgets.

| Case | Run | Pass | Wall time (s) | Peak scope memory (MiB) |
|---|---|---|---|---|
| CALC-01 | 15 | 15 | 392.9–2,051.1 | 270 |
| CALC-02 | 5 | 5 (category and span) | 51.0–169.3 | 257 |
| CALC-03 | 4 | 4 | 292.6–554.0 | 261 |
| CALC-06 | 1 session (9 lines) | 1 | 8,689.0 | 473 |
| CALC-07 | 5 (+2 unsupported on the CLI) | 5 | 266.6–851.5 | 414 |
| CALC-09 | 4 | 4 | 431.6–732.8 | 268 |

No vector timed out, failed natively, hit a host limit or was OOM-killed.
"Peak scope memory" is the scope's `memory.peak`, which includes the
supervisor. The largest single-process peak RSS was 243 MiB. Per-vector wall
times summed to 28,772 s.

CALC-06 printed `10`, `20`, `11`, `3`, `DivisionByZero [10,11)`, `20` and
`WrongArity [0,1)`. So the failed `let x = 1 / 0` left `x = 20`.
CALC-07 produced:

- `fn h()=q` → `UnknownName [7,8)`;
- `fn f(y, y)=y` → `DefinitionError [8,9)`;
- `fn f(y)=f(y)` → `DefinitionError [8,9)`.

Each of these three was followed by a call that reported `UnknownName`, so
the rejected function was not installed. `fn z()=1 / 0` was accepted
silently, and the call `z()` then gave `DivisionByZero [9,10)`. Both negative
controls classified as required (`fail` and `timeout`).

These are finite implementation observations of one prebuilt binary. They do
not change any case `state` or `evidence` in
[`calculator-cases.json`](../../specs/conformance/calculator-cases.json), and
they are not Calculator-v1 acceptance.

## Receipt: integrated eccbc02 release build

[`receipt-eccbc02-release.json`](receipt-eccbc02-release.json) is the
compact receipt for the same selection, run on the integrated tree. The full
receipt and the raw per-vector I/O are in
[`receipt-eccbc02-release.tar.gz`](receipt-eccbc02-release.tar.gz); its
`receipt.json` has SHA-256 `34002182…268fb2a`. The integrated tree adds
module reuse (#16), the committed session with restore (#17), width-ordered
`±` (#18) and the #23 stage split of `calculator-expression.noble`.

The run used:

- binary `noble` built from `eccbc024c837f21caf26ffe2d6be7e6c1d6bb1fb`
  (owner-supplied identity), release profile, SHA-256
  `f7f02dfbc9f6723a10879fd269abd4319a8736aa9d952ebb06f5d9bc2c5085ec`;
- runner commit `eccbc02` with no tracked changes, on aspen3, from
  2026-10-15 20:21:05Z to 20:34:33Z (13.5 min);
- the same settings as the 44329dc run: concurrency 6, a 7,200 s vector
  timeout, a 21,600 s session timeout, `MemoryMax=2G` and
  `MemorySwapMax=0` per scope, and default guest budgets.

| Case | Run | Pass | Wall time (s) | Peak scope memory (MiB) |
|---|---|---|---|---|
| CALC-01 | 15 | 15 | 98.0–176.6 | 493 |
| CALC-02 | 5 | 5 (category and span) | 15.2–52.8 | 282 |
| CALC-03 | 4 | 4 | 80.5–114.8 | 410 |
| CALC-06 | 1 session (9 lines) | 1 | 686.3 | 594 |
| CALC-07 | 5 (+2 unsupported on the CLI) | 5 | 65.6–166.4 | 479 |
| CALC-09 | 4 | 4 | 121.6–144.6 | 472 |

No vector timed out, failed natively, hit a host limit or was OOM-killed.
Both negative controls classified as required (`fail` and `timeout`).
The largest single-process peak RSS was 443 MiB; on 44329dc it was 243 MiB.
CALC-06 and CALC-07 printed exactly the same stdout and diagnostics,
including spans, as on 44329dc.

Per-vector wall time, 44329dc compared with eccbc02. Both runs used
concurrency 6 on the same host, but host load was not controlled, so a
ratio is an observation, not a benchmark.

| Vector | stdin | 44329dc (s) | eccbc02 (s) | Ratio |
|---|---|---|---|---|
| `CALC-01/01` | `1 / 3 + 1 / 6` | 1,121.7 | 147.5 | 7.6× |
| `CALC-01/02` | `0.1 + 0.2` | 671.5 | 148.2 | 4.5× |
| `CALC-01/03` | `0.00000000000000000001` | 663.3 | 98.0 | 6.8× |
| `CALC-01/04` | `9223372036854775807 + 1` | 1,156.9 | 133.9 | 8.6× |
| `CALC-01/05` | `9007199254740993 - 9007199254740992` | 2,051.1 | 131.4 | 15.6× |
| `CALC-01/06` | `2 + 3 * 4` | 871.8 | 138.9 | 6.3× |
| `CALC-01/07` | `(2 + 3) * 4` | 890.8 | 159.0 | 5.6× |
| `CALC-01/08` | `10 - 3` | 597.1 | 148.7 | 4.0× |
| `CALC-01/09` | `8 / 4 / 2` | 908.4 | 154.3 | 5.9× |
| `CALC-01/10` | `-2^2` | 647.0 | 175.7 | 3.7× |
| `CALC-01/11` | `(-2)^2` | 692.1 | 174.1 | 4.0× |
| `CALC-01/12` | `2^3^2` | 1,213.0 | 148.1 | 8.2× |
| `CALC-01/13` | `2^-3` | 632.3 | 153.7 | 4.1× |
| `CALC-01/14` | `4^(2/2)` | 773.0 | 176.6 | 4.4× |
| `CALC-01/15` | `0^0` | 392.9 | 118.4 | 3.3× |
| `CALC-02/01` | `1 + )` | 136.0 | 28.1 | 4.8× |
| `CALC-02/02` | `1 2` | 95.5 | 20.5 | 4.7× |
| `CALC-02/03` | `missing + 1` | 169.3 | 52.8 | 3.2× |
| `CALC-02/04` | `2(3)` | 108.9 | 24.6 | 4.4× |
| `CALC-02/05` | `0.1.2` | 51.0 | 15.2 | 3.4× |
| `CALC-03/01` | `1 / 0` | 292.6 | 80.5 | 3.6× |
| `CALC-03/02` | `0 / 0` | 296.1 | 80.5 | 3.7× |
| `CALC-03/03` | `0^-1` | 317.9 | 91.0 | 3.5× |
| `CALC-03/04` | `4^(1/2)` | 554.0 | 114.8 | 4.8× |
| `CALC-06/session` | `let x = 10 ⏎ fn f(y)=x + y ⏎ let x = 20 ⏎ f(1) ⏎ fn g(x)=x + 1 ⏎ g(2) ⏎ let x = 1 / 0 ⏎ x ⏎ f(1, 2)` | 8,689.0 | 686.3 | 12.7× |
| `CALC-07/unused-body-with-unknown-name` | `fn h()=q ⏎ h()` | 355.8 | 94.2 | 3.8× |
| `CALC-07/duplicate-parameter-name` | `fn f(y, y)=y ⏎ f(1, 2)` | 610.4 | 122.9 | 5.0× |
| `CALC-07/self-recursive-definition` | `fn f(y)=f(y) ⏎ f(1)` | 563.3 | 159.1 | 3.5× |
| `CALC-07/zero-argument-function-body-1-divided-by-0` | `fn z()=1 / 0` | 266.6 | 65.6 | 4.1× |
| `CALC-07/call-that-zero-argument-function` | `fn z()=1 / 0 ⏎ z()` | 851.5 | 166.4 | 5.1× |
| `CALC-09/01` | `2/1` | 431.6 | 121.6 | 3.5× |
| `CALC-09/02` | `3/10` | 512.9 | 123.4 | 4.2× |
| `CALC-09/03` | `3/10` (`--finite-decimal`) | 732.8 | 144.6 | 5.1× |
| `CALC-09/04` | `1/3` (`--finite-decimal`) | 454.3 | 121.8 | 3.7× |
| **sum** | | 28,772.3 | 4,520.3 | 6.4× |

These are finite implementation observations of one prebuilt binary. They do
not change any case `state` or `evidence` in
[`calculator-cases.json`](../../specs/conformance/calculator-cases.json), and
they are not Calculator-v1 acceptance.
