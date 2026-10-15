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
