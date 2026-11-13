# Revision-3 I64 interaction-net decision (#37)

**Decision: NO-GO for continued runtime adoption.** Keep `noble ngraph eval --net` explicitly opt-in; it does not become the default. The backend passes the finite correctness gates, but it fails the frozen end-to-end speed threshold by a wide margin.

## Frozen rule and evidence

The rule is frozen in [net-continuation-criteria.md](net-continuation-criteria.md), before any net timing. It selects #117's fanout-heavy `shared-prefix` family, its 8,192 frozen rows, and requires every row on every separate, merged, and recomputing graph to agree across the independent BigInt oracle, normative reference, packed evaluator, and net read-back. It requires `packed merged / net merged >= 1.15`, with lowering and validation included in the measured net execution and construction accounting for one complete run. No threshold or criteria file was changed after measurement.

The source-bound #117 run is [receipt-2026-11-13-c.json](../shared-candidates/receipt-2026-11-13-c.json), produced from clean source commit `ee34d988390a904080f137a9ec321cfe3108f66a` and CLI SHA-256 `f188f734a6d9b07ee502a8741fbfaceded96cb315e03dfd898bb77c9b846224b`. Its criteria SHA-256 is `054944e2e8c66461213b38e5779f2180792425ad510a0af94ae9f73ff521f551`; the committed criteria blob is `4b9559916be062c52d55797ff8c8df6391c48ed9`. The run used one warm-up plus nine interleaved warm rounds on a shared 32-CPU x86-64 host (load average 37.37); “first” is not page-cache-cold.

## Correctness

The receipt's `net-oracle` and `reference-all-net` gates hold for both families. For the selected 12-candidate shared-prefix family, the independent oracle, packed evaluator, and net agree on every row for all 12 separate candidate graphs and both merged forms; the supplementary `reference-all-net` gate also evaluates the normative reference over all 8,192 rows for each of those graphs and confirms agreement with the oracle, packed evaluator, and net. The same full-row gates hold on the distinct-prefix diagnostic family. This is finite differential evidence, not a refinement proof of the Rust implementation.

The frozen #117 criteria retain `reference_rows: 256`; that field and all original thresholds remain unchanged. The runner keeps that original prefix check and adds the separate `reference-all-net` gate for the #37 continuation requirement, covering every row and every separate/merged/recomputing graph. Thus the 256-row #117 reference sample was not silently substituted for the stronger #37 requirement, and no frozen criterion was weakened.

The compressed [B2 extraction receipt](phase-b-extract-net-2026-11-13.json.gz), also source-bound to `ee34d988390a904080f137a9ec321cfe3108f66a`, reports `result: passed`, `agree: true`, and no mismatches across the 15 named graphs (120,003 L2–L5 vectors) and 64 seeded random Core programs (640,000 vectors). L1 agrees on all 685 checked vectors in each `--opt off` and `--opt on` mode, with no ordinary-path quota outcomes. The B2 differential fixtures cover boundaries, nested pairs, fanout, captured recipes, rebinding, overflow, equality, Unit/empty, repeated outputs, and the I4 transition; all are retained in that receipt. The reducer's unit tests additionally check exact interaction counts and schedule invariance; this does not establish the Rust-to-Lean refinement theorem.

## Net performance against the frozen threshold

| Shared-prefix arm | Warm median wall | Warm median CPU | Warm median RSS | Interactions in one complete evaluation run | Peak live agents | Max retained-byte estimate |
|---|---:|---:|---:|---:|---:|---:|
| packed `merged` | 268.659 ms | 180 ms | 16,464 KiB | — | — | — |
| `net-merged` | 4,240.036 ms | 3,120 ms | 17,840 KiB | 819,200 | 84 | 5,684 B |
| packed `merged-duplicated` | 269.768 ms | 220 ms | 16,276 KiB | — | — | — |
| `net-merged-duplicated` | 12,163.044 ms | 10,690 ms | 18,324 KiB | 3,612,672 | 348 | 22,580 B |

The observed `packed merged / net merged` ratio is `0.06336`; the frozen minimum is `1.15`. Equivalently, net merged is about `15.78x` slower than packed merged. The net merged form does reduce work relative to its recomputing net control (`net-merged / net-merged-duplicated = 0.3486`), but that relative saving does not change the absolute comparison with the packed backend. The run accounts for construction once: every net input row includes graph-to-net lowering, independent validation, reduction, and read-back. No cross-input cache or repeated-run amortization is assumed.

The original #117 packed-arm rule separately remains **NO-GO** in run c: d2 is `0.9959` against `<= 0.90`, and d4 observes `+1,208 KiB` RSS growth for the shared-prefix merged arm against `<= 1,024 KiB`. The runner's summary is `NO-GO: an arm grew its peak resident set beyond the declared bound.` This #117 verdict is distinct from the #37 net speed NO-GO; neither is treated as a correctness failure.

## Scope and reopen conditions

The result is limited to the checked `ngraph-i64/0` subset and the frozen candidate family. It says nothing about multicore reduction, higher-order sharing, allocation counts, an optimal runtime, or general correspondence. Allocation counts remain unmeasured because workspace `unsafe` is forbidden. `--net` remains an explicit, non-default experiment; unsupported profiles and runtime exhaustion do not fall back silently to the packed evaluator.

Reopen only with a new pre-registered workload or implementation hypothesis that can plausibly beat packed execution including per-row construction and validation. Any new run must freeze its rule before timing; this result does not authorize relaxing the 1.15 threshold or reusing the current net construction cost as an unmeasured setup expense.
