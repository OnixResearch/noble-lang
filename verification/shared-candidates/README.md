# Shared candidate execution over `ngraph-i64/0` (issue #117)

Evaluating a *family* of candidate programs — the shape a synthesis search produces — has three
comparison points in Noble today: evaluate each candidate separately, evaluate one merged
multi-output graph that shares the value the candidates have in common, and the gated interaction-net
path. This lane measures the two that exist. It changes no Rust, adds no compilation unit and adds no
feature gate.

The frozen protocol, its families, arms, metrics and thresholds are in
[criteria.md](criteria.md) and [criteria.json](criteria.json), committed as `1fa70cef` — before any
number below existed. Two measurement runs are retained as receipts:

| Run | Commit | Receipt | Host load (1 min) |
| --- | --- | --- | --- |
| a | `4654cd3` | [receipt-2026-11-13-a.json](receipt-2026-11-13-a.json) | 81.66 on 32 CPUs |
| b | `4654cd3` | [receipt-2026-11-13-b.json](receipt-2026-11-13-b.json) | 55.33 on 32 CPUs |

Both were produced by the producer's own command from a clean tree of that commit, with the raw
graphs, vector files and per-invocation timings outside the repository. Neither receipt is edited
by hand.

## What ran

| Arm | Programme | Processes per round |
| --- | --- | --- |
| `separate` | twelve candidate graphs, each evaluated over all 8192 frozen rows | 12 |
| `merged` | one multi-output graph holding all twelve candidates, the shared value computed once | 1 |
| `merged-duplicated` | the same candidates in one graph, the shared value recomputed per candidate | 1 |
| `floor` | the merged graph over a single row — attribution only, never gated | 1 |

Two twelve-candidate families over `ngraph-i64/0` with one `i64` input: `shared-prefix` (every
candidate is one common 18-node prefix value followed by its own tail, so a value is genuinely
shared) and `distinct-prefix` (each candidate is an independent program over the input, so nothing
but the input node is shared). Each candidate, the merged source and the recomputing source are Core
text generated from `criteria.json` and turned into graphs by the CLI's own extractor.

The twelve-process `separate` arm and the two single-process arms all produce the same 12 outputs for
every row; that agreement is a gate, not a finding.

## What did not run

- **The net arm** is not measured. `crates/` has no interaction-net entry point, `noble ngraph`
  dispatch has no net subcommand and `crates/noble-ngraph/Cargo.toml` declares no `[features]`, so
  there is no backend and no feature gate to preserve here. **Pending #37.** The lane asserts this
  absence: a future #37 landing trips the lane's `backend-gate` control instead of measuring
  something else under this name.
- **The labelled-SUP arm** is not measured: there is no labelled DUP/SUP contract. **Pending #105.**
- Allocation counts are not measured anywhere in the lane; see the instrument limits below.

## Reproduce

```sh
CLI=target/debug/noble    # cargo build -p noble-cli --bin noble --offline --locked
OUT=$HOME/.cache/noble-117   # any directory outside the repository
node verification/shared-candidates/run.mjs verify  --binary "$CLI" --work "$OUT/work"
node verification/shared-candidates/run.mjs measure --binary "$CLI" --work "$OUT/work" \
  --receipt "$OUT/receipt.json"
bun test verification/shared-candidates/
```

`verify` runs the correctness gates and the thirteen negative controls and takes about ten seconds;
`measure` repeats them, then times the arms, and refuses to run on a dirty tree. The receipt records
the source revision, the CLI's SHA-256, the criteria's SHA-256 and committed blob, the frozen row
set's digest and the host's load.

## Results — run a (`receipt-2026-11-13-a.json`)

| family | arm | processes | first ms | warm median ms | IQR ms | CPU ms | peak RSS KiB |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| `shared-prefix` | `separate` | 12 | 2629.736 | 1605.457 | 410.151 | 1080 | 13256 |
| `shared-prefix` | `merged` | 1 | 349.435 | 296.165 | 62.297 | 200 | 16844 |
| `shared-prefix` | `merged-duplicated` | 1 | 503.54 | 354.599 | 112.923 | 230 | 16212 |
| `shared-prefix` | `floor` | 1 | 23.638 | 16.614 | 6.109 | 0 | 9804 |
| `distinct-prefix` | `separate` | 12 | 5040.279 | 4227.995 | 714.885 | 1120 | 12384 |
| `distinct-prefix` | `merged` | 1 | 848.78 | 632.536 | 285.058 | 200 | 16244 |
| `distinct-prefix` | `merged-duplicated` | 1 | 850.104 | 749.753 | 357.719 | 200 | 15748 |
| `distinct-prefix` | `floor` | 1 | 22.768 | 33.426 | 7.742 | 0 | 9356 |

Extraction (producer work, reported, not gated): `shared-prefix` median 387.5 ms for all fourteen
sources, `distinct-prefix` 524.5 ms; peak RSS 10.7 MiB and 9.9 MiB.

## Results — run b (`receipt-2026-11-13-b.json`)

| family | arm | processes | first ms | warm median ms | IQR ms | CPU ms | peak RSS KiB |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| `shared-prefix` | `separate` | 12 | 1577.404 | 1841.83 | 212.279 | 1130 | 13152 |
| `shared-prefix` | `merged` | 1 | 311.824 | 298.719 | 44.243 | 200 | 17328 |
| `shared-prefix` | `merged-duplicated` | 1 | 376.29 | 348.895 | 72.263 | 240 | 16196 |
| `shared-prefix` | `floor` | 1 | 14.922 | 20.284 | 2.955 | 0 | 9812 |
| `distinct-prefix` | `separate` | 12 | 1861.271 | 1856.105 | 295.936 | 1060 | 13188 |
| `distinct-prefix` | `merged` | 1 | 215.994 | 299.627 | 38.257 | 190 | 16688 |
| `distinct-prefix` | `merged-duplicated` | 1 | 238.006 | 280.36 | 26.954 | 180 | 17036 |
| `distinct-prefix` | `floor` | 1 | 22.497 | 22.077 | 2.255 | 0 | 9420 |

Extraction: `shared-prefix` median 440.1 ms, `distinct-prefix` 411.3 ms; peak RSS 10.6 MiB each.

## The rule's verdict, per run

| Test | Threshold | Run a | Run b |
| --- | --- | --- | --- |
| d1 merged / separate, `shared-prefix` | ≤ 0.75 | 0.185 holds | 0.162 holds |
| d2 merged / merged-duplicated, `shared-prefix` | ≤ 0.90 | 0.835 holds | 0.856 holds |
| d3 identical-program control, `distinct-prefix` | ≤ 0.10 | **0.156 fails** | 0.064 holds |
| d4 peak-RSS growth, first → second warm round | ≤ 1024 KiB | **fails** (`separate` +1180) | **fails** (`merged` +1088) |

Run a's d3 failure makes run a's d2 unreadable by the frozen rule: its noise floor is larger than
the sharing difference it was meant to detect. Run b's noise control holds, so its d2 is readable —
but d4 fails in both runs, so **the frozen rule yields no GO**. d4's failures move between arms and
families from run to run (`separate` +1180 in a, `merged` +1088 and `floor` +696 in b, and −1036 and
−568 elsewhere), i.e. they sit inside the resolution of the instrument: a debug binary of about
16 MiB peak RSS varies by about a megabyte with dynamic-loader and page-cache state. The bound is
reported as it fell rather than explained away; it is an instrument defect of this lane, not a
sharing result, and it is not adjusted after seeing numbers.

## What the numbers do and do not say

Measured, both runs:

- Merging twelve candidates into one graph and one process costs about **1/6** of evaluating them in
  twelve processes over the same 8192 rows (run a: 296 ms against 1605 ms; run b: 299 ms against
  1842 ms). CPU time moves with it (200 ms against 1080-1130 ms), so the difference is work that
  disappeared, not a scheduling artifact.
- Reusing the shared value instead of recomputing it inside one process is worth about **14-16 %** of
  that single process (run b: 298.7 ms against 348.9 ms; run a: 296.2 ms against 354.6 ms), while
  the node count drops from 243 to 56.

Not claimed:

- **No optimal-runtime claim, and no GO.** The frozen rule's memory bound failed in both runs, and in
  run a the noise control failed as well. d1's effect is far larger than the noise floor in both
  runs; d2's effect is only one to two times it.
- No claim about allocation counts, interaction counts, GPU behaviour, or any purpose-built
  shared-candidate engine.
- No claim that the graph evaluator's cost is proportional to node count. It is not: 4.3× fewer live
  nodes bought 14 % less time, so per-row work dominates at these graph sizes. That is the reading
  that matters for #37/#105: a net or labelled-SUP arm has to remove *per-row* work, not only node
  count, to beat the merged arm.

## Structure: what sharing removes

The lane asserts the exact arithmetic, so the sharing effect is a countable structure, not only a
time:

| family | candidate live nodes | merged live nodes | recomputing live nodes |
| --- | --- | ---: | ---: |
| `shared-prefix` | 19-24 each, 254 summed | **56** | **243** |
| `distinct-prefix` | 4-7 each, 66 summed | 55 | 55 (same program) |

`56 = 18 (prefix, input included) + 38 (twelve tails)`; `243 = 18 + 11 × 17 further prefix copies +
38`. The merged graph is 4.3× smaller than the recomputing one and 4.5× smaller than the sum of the
candidate graphs. Extraction has no structural CSE pass over duplicated *source text* — the sharing
exists because the merged source computes the value once and duplicates it — which is exactly why the
`merged-duplicated` arm is the honest control for it.

## Unsupported constructs and backend fallback

Every row below was exercised by the lane's controls in both runs; the codes and exits are the CLI's
own, quoted from the reports.

| Construct | Admission | Observed | What happens to the output file |
| --- | --- | --- | --- |
| `if` (control flow) | extract | `E-CONTROL`, exit 4 | nothing is published |
| `test.emit` (host effect) | extract | `E-EFFECT`, exit 4 | nothing is published |
| Program-valued root (`drop [ 1 + ]`) | extract | `E-PROGRAM-OUTPUT`, exit 4 | nothing is published |
| Unknown word (`nosuchword`) | extract | `E-SUBJECT`, exit 2 | nothing is published |
| `--profile ngraph-f32/0` over an i64 source | extract | `E-PROFILE`, exit 2 | nothing is published |
| A raised budget (`--budget-work 4194305`) | extract | `E-PROFILE`, exit 2 | nothing is published |
| Unknown operation in a graph | check | `E-GRAPH-MALFORMED`, exit 2 | — |
| Dangling operand in a graph | check | `E-GRAPH-MALFORMED`, exit 2 | — |
| Vector types that do not match the graph | eval | `E-INTERFACE`, exit 2 | — |
| A vector row before its `types` line | eval | `E-FORMAT`, exit 2 | — |

**Fallback behaviour.** There is no backend fallback to document into: `ngraph-i64/0` extraction
either admits a source into one graph or refuses it with the code above, and evaluation is the
compiled evaluator or, with `--reference`, the normative reference interpreter. The two are checked
against each other and against this lane's independent oracle over the first 256 rows of every
family. A graph the compiled evaluator refuses on its scratch budget reports the same observation for
every row (`crates/noble-cli/src/ngraph.rs`, the `scratch_refusal` arm of `eval`); the CLI does not
silently switch interpreters, and this lane's rows all evaluate normally, so that arm was not
exercised here.

A wrong merged graph is *not* refused by the CLI — it is well-typed and well-formed. The
`permuted-outputs` control shows the CLI admitting a graph whose candidates are wired to the wrong
outputs, and the oracle comparison catching it; a `truncated-family` graph is admitted with the wrong
arity. Candidate identity therefore comes from the harness's bindings, not from admission.

## Instruments and their limits

- Wall time: `process.hrtime.bigint` around `spawnSync`; CPU and peak RSS from `GNU time 1.10`
  (`%U %S %M`). Both runs on a 32-core host shared with other builds at load 55-82; no CPU pinning,
  no cache drop.
- "Cold" is the first round only. This host cannot drop page caches without root, so it means "not
  previously executed by this run", nothing more. All comparisons use warm medians.
- Peak RSS is a `wait4` high-water mark, and for the twelve-process arm it is the maximum over twelve
  independent processes, so the d4 growth test is weakest exactly there.
- Allocation counts are unavailable: the workspace forbids `unsafe`, so no counting allocator can
  exist. Nothing here claims which allocations a merge removed.

## Reopen conditions

1. A quiet or CPU-pinned window would let the sharing comparison (d2) be read with a noise floor
   well below its effect; the current margin is one to two times the floor.
2. A memory bound derived from repeated identical runs, or RSS growth measured across a longer run
   rather than between two single rounds, would replace d4's 1-MiB snapshot comparison.
3. #37 would add the net arm and interaction counts; #105 would add the labelled DUP/SUP arm. Both
   must clear *per-row* work, not only node count, to beat the merged arm measured here.

## Files

| File | Role |
| --- | --- |
| `criteria.md`, `criteria.json` | the frozen protocol, committed before measurement |
| `oracle.mjs` | the independent JS stack machine and the frozen program assembly |
| `run.mjs` | the runner: gates, negative controls, measurement, receipt |
| `oracle.test.mjs` | hand-derived unit tests for the oracle, the programs and the comparator |
| `receipt-2026-11-13-a.json`, `receipt-2026-11-13-b.json` | the two runs |
