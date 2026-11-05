# Noble frontend and backend fuzz targets (#87)

Four in-process libFuzzer targets drive arbitrary bytes into the untrusted-input
boundary in front of the kernel. They live in one cargo-fuzz crate, excluded from
the workspace: libFuzzer is not in the reviewed offline vendor, and fuzzing is a
separate, opt-in dynamic-evidence run. The frontend targets sit beside the backend
one because all four share one oracle file.

| Target | Entry point | Property |
|---|---|---|
| `prepare` | `noble_contracts::prepare` (contract s-expressions) | never `Internal`; deterministic; monotone in its budget |
| `source` | `source::Session` (Core, host-free Core) and `source::ModuleSession` (modules, calculator), up to four NUL-separated units prepared and committed in order | never `Internal`; deterministic; monotone in its budget and retained-source cap |
| `prepare_to_wasm` | `prepare` then `noble_wasm::compile` for both representations | never `Defective` or `Invalid` for a prepared contract; deterministic; WAT parses and the module validates (`wasmparser`, all features) |
| `wit_world` | `component::World::parse` and `parse_checked_u64` | never `Internal`; deterministic; monotone in its budget |

The oracles are `crates/noble-wasm/tests/fuzz/oracles.rs`. The workspace test
target `forged_effects` compiles the same file and replays every committed
regression in `regressions/<target>/`, and every tracked fixture under four
budgets, without libFuzzer (`tests/fuzz/replay.rs`).

## Input layout

A five-byte header, then the subject bytes unchanged (no padding, so a lexer that
peeks ahead sees the real end). Header byte 0's low two bits select the budget:
`0b11` is `Limits::default()`, anything else a deliberately tiny one (bytes
1–512, nodes 1–64, depth 1–8, work 1–4096; a retained-source cap of 1–4081
bytes), so the limit and overflow branches are reached constantly. Header byte 4
selects the source profile.

## Monotonicity, and what it does not require

A refusal reached without exhausting the budget must be reproduced, with the same
stage, category and span, under the default budget, and an acceptance must stay
an acceptance. That is the property that catches a budget refusal reported as an
invalid input. Two stronger readings were tried and rejected as false positives:

- **Identical messages.** A join diagnostic's provenance text is rendered under
  whatever budget remains (`join_stacks(..).ok()`), so a smaller budget can drop
  that detail from an otherwise identical refusal.
- **No earlier invalid construct after exhaustion.** Each stage reads the whole
  subject before the next judges it, so an exhausted parse and a later stage's
  refusal of an earlier construct are both honest.

## Running

The pinned smoke run builds every target, seeds it from the tracked `.noble` and
`.wit` fixtures and records a receipt:

```sh
env -i HOME="$HOME" PATH=/run/current-system/sw/bin:/usr/bin:/bin \
  "$(selected node)" verification/fuzz/smoke.mjs --output /tmp/noble-fuzz-smoke
```

It needs network access once, to fetch `libfuzzer-sys` and the `wat`/`wasmparser`
versions pinned with checksums in `Cargo.lock`. `cargo fuzz run <target>` from
this directory also works with any nightly toolchain.

`verification/fuzz/receipt-3814dd83.json` is the retained receipt of one
60-second-per-target run on clean source `3814dd83`: all four targets exited 0
with no findings.

| Target | Executions | Coverage edges | Corpus units | Seeds |
|---|---|---|---|---|
| `prepare` | 239,913 | 2,577 | 504 | 328 |
| `source` | 28,289 | 5,908 | 880 | 1,337 |
| `prepare_to_wasm` | 177,988 | 7,702 | 427 | 328 |
| `wit_world` | 567,395 | 1,743 | 642 | 72 |

## Findings

| Regression | Defect | Fix |
|---|---|---|
| `regressions/source/module-byte-limit-is-exhaustion` | A module input over `Limits::bytes` was refused as `Invalid`, while the Core lexer refuses the identical condition as `Exhausted`, so a larger budget turned an "invalid" input into a link refusal | `ModuleSession::prepare` reports `Exhausted` over the whole input |
| `regressions/source/retained-cap-is-exhaustion` | A module that carried the session past its retained-source cap was refused as `Invalid` | the retained-cap refusals in `check_retained` are `Exhausted` |

## Non-claims

A clean run is dynamic evidence for the executed inputs only. The oracles check
that the frontend and backend never contradict their own diagnostics; they do not
show that any accepted program is correct, and running emitted modules in
Wasmtime is out of scope.
