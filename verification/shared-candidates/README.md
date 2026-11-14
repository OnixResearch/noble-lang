# Shared candidate execution over `ngraph-i64/0` (issue #117)

Evaluating a *family* of candidate programs — the shape a synthesis search produces — has four
comparison points in Noble today: evaluate each candidate separately, evaluate one merged
multi-output graph that shares the value the candidates have in common, evaluate the merged graph
with the shared value recomputed, and evaluate through the opt-in revision-3 interaction-net path.
The packed evaluator remains the default. The lane adds no compilation unit or Cargo feature gate.

The original #117 protocol, families, packed arms, metrics and thresholds are in
[criteria.md](criteria.md) and [criteria.json](criteria.json), committed as `1fa70cef` before runs a
and b; those files remain unchanged. The separate #37 net speed rule is frozen in
[`../ngraph/net-continuation-criteria.md`](../ngraph/net-continuation-criteria.md), committed before
run c. Four measurement runs are retained as receipts:

| Run | Commit | Receipt | Host load (1 min) |
| --- | --- | --- | --- |
| a | `4654cd3` | [receipt-2026-11-13-a.json](receipt-2026-11-13-a.json) | 81.66 on 32 CPUs |
| b | `4654cd3` | [receipt-2026-11-13-b.json](receipt-2026-11-13-b.json) | 55.33 on 32 CPUs |
| c (#37 net continuation) | `ee34d98` | [receipt-2026-11-13-c.json](receipt-2026-11-13-c.json) | 37.37 on 32 CPUs |
| d (clean final backend) | `68ea8a6` | [receipt-2026-11-14-d.json](receipt-2026-11-14-d.json) | 16.77 on 32 CPUs |

All four were produced by the producer's own command from a clean tree of the source commit, with
the raw graphs, vector files and per-invocation timings outside the repository. No receipt is edited
by hand.

## What ran

| Arm | Programme | Processes per round |
| --- | --- | --- |
| `separate` | twelve candidate graphs, each evaluated over all 8192 frozen rows | 12 |
| `merged` | one multi-output graph holding all twelve candidates, the shared value computed once | 1 |
| `merged-duplicated` | the same candidates in one graph, the shared value recomputed per candidate | 1 |
| `floor` | the merged graph over a single row — attribution only, but it does take part in d4 (see the note below) | 1 |
| `net-separate` | twelve candidate graphs, each evaluated through explicit `--net` over all 8192 rows | 12 |
| `net-merged` | the shared multi-output graph evaluated through explicit `--net` | 1 |
| `net-merged-duplicated` | the recomputing multi-output graph evaluated through explicit `--net` | 1 |
| `net-floor` | the merged graph over one row through explicit `--net` (descriptive; not in packed d4) | 1 |

Two twelve-candidate families over `ngraph-i64/0` with one `i64` input: `shared-prefix` (every
candidate is one common 18-node prefix value followed by its own tail, so a value is genuinely
shared) and `distinct-prefix` (each candidate is an independent program over the input, so nothing
but the input node is shared). Each candidate, the merged source and the recomputing source are Core
text generated from `criteria.json` and turned into graphs by the CLI's own extractor.

The full-row packed and net arms (`separate`, `merged`, and `merged-duplicated`) produce the same 12
expected outputs for every frozen row; the one-row `floor` arms are attribution-only. Agreement is
a gate, not a finding.

## What did not run

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

`verify` runs the correctness gates, including the full-row `reference-all-net` check, and thirteen
negative controls. `measure` repeats them, then times the packed and net arms, and refuses to run on
a dirty tree. The receipt records the source revision, the CLI's SHA-256, the criteria's SHA-256 and
committed blob, the frozen row set's digest and the host's load.

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

## Results — run c (`receipt-2026-11-13-c.json`)

Run c adds the net arms without changing `criteria.json` or `criteria.md`. It used one warm-up plus
nine interleaved warm rounds. “First” is the warm-up round, not a page-cache-cold measurement.

| family | arm | processes | first ms | warm median ms | IQR ms | CPU ms | peak RSS KiB |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| `shared-prefix` | `separate` | 12 | 3886.564 | 1388.136 | 1062.396 | 1050 | 13932 |
| `shared-prefix` | `merged` | 1 | 674.434 | 268.659 | 127.771 | 180 | 18212 |
| `shared-prefix` | `merged-duplicated` | 1 | 685.157 | 269.768 | 75.673 | 220 | 17984 |
| `shared-prefix` | `floor` | 1 | 53.793 | 16.898 | 17.522 | 0 | 9904 |
| `shared-prefix` | `net-separate` | 12 | 54967.374 | 23809.172 | 13861.291 | 16900 | 12372 |
| `shared-prefix` | `net-merged` | 1 | 13164.048 | 4240.036 | 2111.828 | 3120 | 18540 |
| `shared-prefix` | `net-merged-duplicated` | 1 | 29206.555 | 12163.044 | 10023.371 | 10690 | 18868 |
| `shared-prefix` | `net-floor` | 1 | 33.659 | 14.452 | 7.93 | 0 | 10368 |
| `distinct-prefix` | `separate` | 12 | 1191.979 | 1184.829 | 166.209 | 900 | 14144 |
| `distinct-prefix` | `merged` | 1 | 187.408 | 178.68 | 20.12 | 150 | 18820 |
| `distinct-prefix` | `merged-duplicated` | 1 | 186.553 | 193.482 | 21.766 | 160 | 18544 |
| `distinct-prefix` | `floor` | 1 | 14.215 | 12.219 | 2.473 | 0 | 10148 |
| `distinct-prefix` | `net-separate` | 12 | 11538.542 | 9098.232 | 546.743 | 8260 | 13556 |
| `distinct-prefix` | `net-merged` | 1 | 3112.095 | 2836.094 | 56.7 | 2700 | 19012 |
| `distinct-prefix` | `net-merged-duplicated` | 1 | 2866.033 | 2864.274 | 175.938 | 2750 | 18748 |
| `distinct-prefix` | `net-floor` | 1 | 12.42 | 12.454 | 4.903 | 0 | 10440 |

Net instrumentation for one complete 8192-row evaluation run:

| family | net arm | interactions | peak live agents | maximum retained-byte estimate |
| --- | --- | ---: | ---: | ---: |
| `shared-prefix` | `net-separate` | 3,522,560 | 33 | 2,321 B |
| `shared-prefix` | `net-merged` | 819,200 | 84 | 5,684 B |
| `shared-prefix` | `net-merged-duplicated` | 3,612,672 | 348 | 22,580 B |
| `distinct-prefix` | `net-separate` | 737,280 | 9 | 785 B |
| `distinct-prefix` | `net-merged` | 827,392 | 84 | 5,684 B |
| `distinct-prefix` | `net-merged-duplicated` | 827,392 | 84 | 5,684 B |

The net measurements include per-row graph-to-net lowering, independent validation, reduction and
read-back; construction is accounted for once across a complete run. The extra net arms do not alter
the frozen #117 d1–d4 gates, which remain defined over its original packed arms; the #37 net speed
threshold is evaluated separately below. The host was shared (32 CPUs, load average 37.37), with no
CPU pinning or page-cache drop.

## Results — run d (`receipt-2026-11-14-d.json`)

Run d remeasures the committed backend at `68ea8a6` with the unchanged frozen criteria. It used one
warm-up plus nine interleaved warm rounds; “first” is the warm-up, not a page-cache-cold measurement.

| family | arm | processes | first ms | warm median ms | IQR ms | CPU ms | warm max RSS KiB |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| `shared-prefix` | `separate` | 12 | 1077.366 | 844.837 | 118.547 | 600 | 20700 |
| `shared-prefix` | `merged` | 1 | 115.848 | 131.661 | 10.514 | 110 | 25868 |
| `shared-prefix` | `merged-duplicated` | 1 | 136.879 | 150.922 | 13.878 | 130 | 26336 |
| `shared-prefix` | `floor` | 1 | 8.501 | 10.078 | 2.911 | 0 | 16116 |
| `shared-prefix` | `net-separate` | 12 | 11248.195 | 11601.444 | 693.243 | 11240 | 21964 |
| `shared-prefix` | `net-merged` | 1 | 1884.519 | 2017.644 | 101.456 | 1980 | 26312 |
| `shared-prefix` | `net-merged-duplicated` | 1 | 7510.260 | 7373.835 | 585.933 | 7290 | 24896 |
| `shared-prefix` | `net-floor` | 1 | 7.698 | 10.074 | 2.067 | 0 | 16360 |
| `distinct-prefix` | `separate` | 12 | 709.105 | 716.920 | 32.014 | 480 | 20792 |
| `distinct-prefix` | `merged` | 1 | 109.555 | 111.833 | 3.355 | 100 | 26168 |
| `distinct-prefix` | `merged-duplicated` | 1 | 116.273 | 111.026 | 6.847 | 90 | 25992 |
| `distinct-prefix` | `floor` | 1 | 9.135 | 9.243 | 1.802 | 0 | 16184 |
| `distinct-prefix` | `net-separate` | 12 | 5990.725 | 5838.009 | 92.883 | 5620 | 22240 |
| `distinct-prefix` | `net-merged` | 1 | 1956.966 | 1946.629 | 61.125 | 1930 | 26308 |
| `distinct-prefix` | `net-merged-duplicated` | 1 | 1987.771 | 1950.996 | 111.502 | 1920 | 26020 |
| `distinct-prefix` | `net-floor` | 1 | 8.827 | 9.323 | 1.702 | 0 | 16352 |

Run d has complete full-row correctness and determinism gates. Its #117 ratios are d1 `0.1558`,
d2 `0.8724`, and d3 `0.0072`; d4 holds for every packed arm in both families (largest increase
`684 KiB`, below the `1,024 KiB` limit). The frozen #117 criteria therefore report **GO** for
merged candidate execution on this run. This remains a bounded result for the frozen workload, not
an optimal-runtime claim.

## The rule's verdict, per run

| Test | Threshold | Run a | Run b | Run c | Run d |
| --- | --- | --- | --- | --- | --- |
| d1 merged / separate, `shared-prefix` | ≤ 0.75 | 0.185 holds | 0.162 holds | 0.194 holds | 0.1558 holds |
| d2 merged / merged-duplicated, `shared-prefix` | ≤ 0.90 | 0.835 holds | 0.856 holds | **0.996 fails** | 0.8724 holds |
| d3 identical-program control, `distinct-prefix` | ≤ 0.10 | **0.156 fails** | 0.064 holds | 0.077 holds | 0.0072 holds |
| d4 peak-RSS growth, first → second warm round | ≤ 1024 KiB | **fails** (`separate` +1180) | **fails** (`merged` +1088) | **fails** (`merged` +1208) | holds (max +684) |

Run a's d3 failure makes run a's d2 unreadable by the frozen rule: its noise floor is larger than
the sharing difference it was meant to detect. Runs b and c pass d3, so their d2 values are
readable; run c's d2 also fails. Run d passes all four frozen gates, so **the rule yields GO for
run d**; this does not erase the earlier run-specific failures. The first three d4 failures moved
between arms and families (`separate` +1180 KiB in a,
`merged` +1088 KiB and `floor` +696 KiB in b, `merged` +1208 KiB in c), while every d4 increase
in d is at most 684 KiB. These results preserve the observed instrument variation rather than
adjusting the bound after measurement.

One wording conflict is worth quoting rather than smoothing over: `criteria.md` calls the one-row
`floor` arm "not gated", while `criteria.json`'s d4 says "every arm". The runner follows the
machine-readable half, so `floor` is included in d4. Both readings give the same d4 outcome on all
four runs: run a fails d4 on `separate` (+1180 KiB), run b on `merged` (+1088 KiB), run c on
`merged` (+1208 KiB), and run d passes every arm (largest +684 KiB). Neither frozen file was edited
after the numbers existed.

## #37 net continuation decision

The separate frozen #37 threshold compares the packed `merged` and `net-merged` warm medians from
the clean final run d: `packed / net = 131.661 / 2017.644 = 0.06525`, below the required `1.15`.
The complete end-to-end net arm is therefore about **15.33× slower** than packed execution on the
selected workload: **NO-GO**. Run d records `reference-all-net`, which verifies all 8,192 rows for
every separate, merged and recomputing graph against the BigInt oracle, packed evaluator, and net;
the gate holds. Correctness does not rescue the failed speed threshold.

## What the numbers do and do not say

Measured in runs a and b (the original packed-only measurements):

- Merging twelve candidates into one graph and one process costs about **1/6** of evaluating them in
  twelve processes over the same 8192 rows (run a: 296 ms against 1605 ms; run b: 299 ms against
  1842 ms). CPU time moves with it (200 ms against 1080-1130 ms), so the difference is work that
  disappeared, not a scheduling artifact.
- Reusing the shared value instead of recomputing it inside one process is worth about **14-16 %** of
  that single process (run b: 298.7 ms against 348.9 ms; run a: 296.2 ms against 354.6 ms), while
  the node count drops from 243 to 56.

Measured in run d (the net continuation): the selected `net-merged` median is 2,017.644 ms versus
131.661 ms for packed `merged`; despite reducing interactions from 3,612,672 in the recomputing net
to 819,200, the inclusive net path is **15.33× slower** than packed execution. The per-input
construction and validation cost is inside that timing; no reusable net cache was measured.

Not claimed:

- **No optimal-runtime claim.** The frozen #117 rule yields GO for run d, while the separate frozen
  #37 net speed threshold still fails by a wide margin. Run a's noise control failed, so its d2 is
  unreadable; run c still independently fails d2 and d4.
- No claim about allocation counts, GPU behaviour, or any purpose-built shared-candidate engine.
- No claim that the graph evaluator's cost is proportional to node count. It is not: 4.3× fewer live
  nodes bought 14 % less time in runs a and b. Runs c and d confirm that lower net interaction count
  alone does not make an evaluator faster: the measured net path remains much slower than packed
  execution.
  Any future net or labelled-SUP arm has to remove *per-row* work, not only node count, to beat the
  merged arm.

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

Every row below was exercised by the lane's controls in all three runs; the codes and exits are the
CLI's own, quoted from the reports.

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

**Fallback behaviour.** There is no backend fallback: `ngraph-i64/0` extraction either admits a
source into one graph or refuses it with the code above. Packed, `--reference`, and explicit `--net`
evaluation are separate selections; the CLI does not silently switch interpreters or backends. Runs
a and b compare the packed evaluator with the reference interpreter and independent oracle over the
first 256 rows of each family. Run c retains that frozen check and adds `reference-all-net`: all
8,192 rows for every separate, merged, and recomputing graph are compared across reference, packed,
net, and the independent oracle. A graph the packed evaluator refuses on its scratch budget reports
the same observation for every row (`crates/noble-cli/src/ngraph.rs`, the `scratch_refusal` arm of
`eval`); all lane rows evaluate normally, so that arm was not exercised here.

A wrong merged graph is *not* refused by the CLI — it is well-typed and well-formed. The
`permuted-outputs` control shows the CLI admitting a graph whose candidates are wired to the wrong
outputs, and the oracle comparison catching it; a `truncated-family` graph is admitted with the wrong
arity. Candidate identity therefore comes from the harness's bindings, not from admission.

## Instruments and their limits

- Wall time: `process.hrtime.bigint` around `spawnSync`; CPU and peak RSS from `GNU time 1.10`
  (`%U %S %M`). Runs a, b, c, and d were on a shared 32-CPU host at load averages 81.66, 55.33,
  37.37, and 16.77 respectively; no CPU pinning, no cache drop.
- "Cold" is the first round only. This host cannot drop page caches without root, so it means "not
  previously executed by this run", nothing more. All comparisons use warm medians.
- Peak RSS is a `wait4` high-water mark, and for the twelve-process arm it is the maximum over twelve
  independent processes, so the d4 growth test is weakest exactly there.
- Allocation counts are unavailable: the workspace forbids `unsafe`, so no counting allocator can
  exist. Nothing here claims which allocations a merge removed.

## Reopen conditions

1. The d2 margin was narrow in run c, while run d passed both d2 and the distinct-program noise
   control. The frozen rule is retained unchanged; any future run uses the same pre-registration or
   a new one committed before timing.
2. A memory bound derived from repeated identical runs, or RSS growth measured across a longer run
   rather than between two single rounds, would replace d4's 1-MiB snapshot comparison.
3. The #37 net arm and interaction counts are measured; it is a no-go under the frozen speed
   criterion. A future net design would need to reduce inclusive per-row work, not only interactions
   or node count. #105 would add the labelled DUP/SUP arm only under its own selected contract; it too
   must clear per-row work to beat the measured packed arm.

## Files

| File | Role |
| --- | --- |
| `criteria.md`, `criteria.json` | the frozen protocol, committed before measurement |
| `../ngraph/net-continuation-criteria.md` | the frozen #37 net speed and full-row correctness rule, committed before run c |
| `oracle.mjs` | the independent JS stack machine and the frozen program assembly |
| `run.mjs` | the runner: gates, negative controls, measurement, receipt |
| `oracle.test.mjs` | hand-derived unit tests for the oracle, the programs and the comparator |
| `receipt-2026-11-13-a.json`, `receipt-2026-11-13-b.json`, `receipt-2026-11-13-c.json`, `receipt-2026-11-14-d.json` | all four source-bound runs |
