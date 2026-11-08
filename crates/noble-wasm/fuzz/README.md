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

## Feeding the semantic differential (#102)

The fuzz oracles check that the frontend and backend never contradict their own
diagnostics. The semantic differential in `verification/foundation/differential.mjs`
compares execution with Lean-checked expectations. The export step joins them:

```sh
"$(selected node)" verification/fuzz/export-differential.mjs \
  --corpus MINIMISED_SOURCE_CORPUS --output cases.json
"$(selected node)" verification/foundation/differential.mjs --seed 15 --count 80 \
  --foundation FOUNDATION_CHECK_JSON --output NEW_DIR --import cases.json
```

Minimise the `source` corpus first, with libFuzzer's `-merge=1` into an empty
directory. The exporter keeps a corpus input only if all of these hold:

- it is a single-unit Core subject (header profile 0 or 1), so it has no module,
  calculator or host words;
- its tokens, read by the Core lexer's rules, lie in the runner's fragment: I64
  and Bool literals, `+ - *`, `[ … ]` blocks, `quote`, `compose` and `run`;
- the reference run of its parsed model is normal.

Every other input is counted under the reason it was skipped:

- a short input or a module or calculator profile;
- several units, or text that is not UTF-8;
- an empty program or a program outside the fragment;
- a reference refusal (underflow or an out-of-range literal);
- a duplicate of an earlier program.

The runner appends the kept programs to its seeded cases. Each one runs its
**exact corpus text**, in both optimisation modes, against the Lean-checked
expectation of its parsed model. An importer modeling error therefore shows up as
a mismatch instead of being hidden by a re-rendered source. A mismatch is shrunk
like any other. The receipt's `imported` block states the export counts, the
imported cases and how many reached execution in both modes.

Run the merge with `-rss_limit_mb=8192`: under the smoke run's 1024 MB limit,
libFuzzer's own outer merge process stopped itself. It held 2.9 GB while parsing
its control file, which is not a property of any input.

**One run, on clean source `3843e914`.** The `source` target ran for 300
seconds: 832,090 executions, 6,426 coverage edges, a peak RSS of 49 MB and no
findings ([`receipt-source-3843e914.json`](../../../verification/fuzz/receipt-source-3843e914.json)).

The merge kept 1,770 units. The export
([`differential-cases-3843e914.json`](../../../verification/fuzz/differential-cases-3843e914.json))
imported 11 of them: 5 straight-line programs and 6 quotation programs. It skipped the
rest:

| Reason | Units |
|---|---|
| module or calculator profile | 1,332 |
| outside the fragment | 275 |
| several units | 125 |
| empty | 10 |
| reference refusal | 10 |
| not UTF-8 | 5 |
| short | 1 |
| duplicate | 1 |

[`differential-import-3843e914.json`](../../../verification/fuzz/differential-import-3843e914.json)
is the differential receipt.
- All 11 imported programs reached execution in both modes, 22 executions, and
  matched their Lean-checked expectations.
- So did the 80 seeded cases and the 3 rejection cases.
- The stacks agree with optimisation off and on.
- All seven mutation controls failed for their intended reasons.

Coverage is therefore stated rather than implied: the corpus contributes few
programs to this fragment, because most fuzzed subjects are not pure
arithmetic.

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
