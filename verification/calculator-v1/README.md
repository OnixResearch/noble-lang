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
