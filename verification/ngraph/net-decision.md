# Revision-3 I64 interaction-net decision (#37)

**Decision: NO-GO for continued runtime adoption.** Keep `noble ngraph eval --net` explicitly opt-in; it does not become the default. The backend passes the finite correctness gates, but it fails the frozen end-to-end speed threshold by a wide margin.

## Frozen rule and evidence

The rule is frozen in [net-continuation-criteria.md](net-continuation-criteria.md), before any net timing. It selects #117's fanout-heavy `shared-prefix` family, its 8,192 frozen rows, and requires every row on every separate, merged, and recomputing graph to agree across the independent BigInt oracle, normative reference, packed evaluator, and net read-back. It requires `packed merged / net merged >= 1.15`, with lowering and validation included in the measured net execution and construction accounting for one complete run. No threshold or criteria file was changed after measurement.

The source-bound final #117 run is [receipt-2026-11-14-d.json](../shared-candidates/receipt-2026-11-14-d.json), produced from a clean tree at source commit `68ea8a60575143429fe62aee8a2fd51079f56fb0` (`clean_tree: true`) with CLI SHA-256 `639932997493c112cf2108c5faa46ca8f602a282e1248274d20b3dd34cc3e3a7`. Its criteria SHA-256 is `054944e2e8c66461213b38e5779f2180792425ad510a0af94ae9f73ff521f551`; the committed criteria blob is `4b9559916be062c52d55797ff8c8df6391c48ed9`. The run used one warm-up plus nine interleaved warm rounds on a shared 32-CPU x86-64 host (load average 16.77); “first” is not page-cache-cold.

## Correctness

The receipt's `net-oracle` and `reference-all-net` gates hold for both families. For the selected 12-candidate shared-prefix family, the independent oracle, packed evaluator, and net agree on every row for all 12 separate candidate graphs and both merged forms; the supplementary `reference-all-net` gate also evaluates the normative reference over all 8,192 rows for each of those graphs and confirms agreement with the oracle, packed evaluator, and net. The same full-row gates hold on the distinct-prefix diagnostic family. This is finite differential evidence, not a refinement proof of the Rust implementation.

The frozen #117 criteria retain `reference_rows: 256`; that field and all original thresholds remain unchanged. The runner keeps that original prefix check and adds the separate `reference-all-net` gate for the #37 continuation requirement, covering every row and every separate/merged/recomputing graph. Thus the 256-row #117 reference sample was not silently substituted for the stronger #37 requirement, and no frozen criterion was weakened.

The compressed [current B2 extraction receipt](phase-b-extract-net-2026-11-14.json.gz) is source-bound to `9b07d413121f6109e2b2226f1641a56be08d1a84`; all B1, B2 and B3 records pass. For B2, all 15 named graphs (120,003 vectors) and 64 seeded Core programs (640,000 vectors) have zero L2–L5 mismatches. L1 agrees on all 685 vectors in both `--opt off` and `--opt on`, with zero ordinary-path quota outcomes; the 1,000-step I4 transition also agrees in both modes. The receipt records the pinned Node and debug CLI identities. The older 2026-11-13 receipt remains immutable historical evidence. At the final backend commit, the B2 target also passed with 22 tests and 2 intentionally ignored; the explicitly run ignored BigInt differential passed, and the corpus-freshness test confirmed that the retained Lean corpus matches fresh Rust lowerings. The freshness result is not itself a Lean kernel proof: that proof remains pending integration of the final `NetsLean` definitions and a successful repository foundation proof gate.

## Net performance against the frozen threshold

| Shared-prefix arm | Warm median wall | Warm median CPU | Warm median RSS | Interactions in one complete evaluation run | Peak live agents | Max retained-byte estimate |
|---|---:|---:|---:|---:|---:|---:|
| packed `merged` | 131.661 ms | 110 ms | 25,592 KiB | — | — | — |
| `net-merged` | 2,017.644 ms | 1,980 ms | 24,296 KiB | 819,200 | 84 | 5,684 B |
| packed `merged-duplicated` | 150.922 ms | 130 ms | 25,304 KiB | — | — | — |
| `net-merged-duplicated` | 7,373.835 ms | 7,290 ms | 24,516 KiB | 3,612,672 | 348 | 22,580 B |

The observed `packed merged / net merged` ratio is `0.06525`; the frozen minimum is `1.15`. Equivalently, net merged is about `15.33x` slower than packed merged. The net merged form does reduce work relative to its recomputing net control (`net-merged / net-merged-duplicated = 0.2736`), but that relative saving does not change the absolute comparison with the packed backend. The run accounts for construction once: every net input row includes graph-to-net lowering, independent validation, reduction, and read-back. No cross-input cache or repeated-run amortization is assumed.

The frozen #117 packed-arm rule is **GO** on final run d: d1 `0.1558`, d2 `0.8724`, d3 `0.0072`, and every d4 packed-arm growth is at most `684 KiB`, under the `1,024 KiB` bound. Run c's earlier d2 and d4 failures remain historical outcomes, not the final run's verdict. The separate #37 net speed rule remains **NO-GO**; neither outcome is a correctness failure.

## Scope and reopen conditions

The result is limited to the checked `ngraph-i64/0` subset and the frozen candidate family. It says nothing about multicore reduction, higher-order sharing, allocation counts, an optimal runtime, or general correspondence. Allocation counts remain unmeasured because workspace `unsafe` is forbidden. `--net` remains an explicit, non-default experiment; unsupported profiles and runtime exhaustion do not fall back silently to the packed evaluator.

Reopen only with a new pre-registered workload or implementation hypothesis that can plausibly beat packed execution including per-row construction and validation. Any new run must freeze its rule before timing; this result does not authorize relaxing the 1.15 threshold or reusing the current net construction cost as an unmeasured setup expense.
