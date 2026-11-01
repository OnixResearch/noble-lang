# CPU batch continuation decision (issue #41): NO-GO

The criteria in [`batch-continuation-criteria.md`](batch-continuation-criteria.md) were written
before the lane ran. The numbers below are the lane's output, retained in
[`batch-measurements-2026-11-01.json`](batch-measurements-2026-11-01.json): release build,
one host, one revision, warm lanes, the seven `i64` boundary operands over a five-node `I64`
graph (188 retained bytes, two outputs).

| Lane | 1 row (ns) | 4096 rows (ns/row) | 65536 rows (ns/row) |
|---|---|---|---|
| `reference` interpreter, per row | 4538 | 179 | 179 |
| `single` compiled evaluation, per row | 1102 | 182 | 181 |
| `batch` `Prepared::run` | 511 | 290 | 311 |
| `native` wrapping arithmetic | 70 | 0 | 0 |

## Rule by rule

| Criterion | Required | Measured | Result |
|---|---|---|---|
| every row's outputs agree across lanes | agreement | last row of each size agrees; 42 interleaved rows are compared per row in the existing `batch_row_reference` graph lane | met |
| `batch` ≥ 1.15× `single` at 65536 rows | 311 × 1.15 = 358 ≤ 181 | 311 ns/row vs 181 ns/row | **failed** |
| `batch` not slower than `single` at 4096 rows | ≤ 182 | 290 ns/row | **failed** |
| `batch` resident growth ≤ 8 MiB at 65536 rows | ≤ 8388608 | 11 276 288 bytes | **failed** |

**Decision: NO-GO.** No further CPU batching work is started on this issue: no SIMD, no `F32` slice,
no chunked/column-oriented staging, and no batching-related cache. The graph-backed
`batch::Prepared` entry point and its tests stay as they are, flagged as measured-not-continuing
rather than removed: it is the honest comparison point if anything changes.

## What the numbers say, stated narrowly

- Batching is **1.7× slower** per row than the compiled single-row loop (311 ns/row against 181),
  not faster. Reusing one workspace does not of itself amortise anything, which is exactly what the
  issue warned about.
- The 65536-row run grows the resident set by 11.3 MB, about 172 bytes per row: the run still
  collects one report (with fresh outputs) per row. An allocation-free into-sink path is missing.
- The native lane's 0 ns/row is **not** a floor: the optimiser eliminates the loop because only its
  final value escapes. Reading it as "native is infinitely fast" would be wrong; it is unusable as a
  comparison until that lane keeps every row's value alive.

## What a later GO would require

Both of: an into-sink run whose per-row reports are not owned allocations, and a row executor that
beats 177 ns/row at 65536 rows by 15 %. Neither exists today, and neither is started here. Any
measurement that claims either must also state the allocation-count gap recorded above: this
workspace cannot host a counting allocator under `unsafe_code = "forbid"`, so an allocation claim
needs an explicit instrumentation seam decision by an owner first.

## Scope

This decision covers CPU independent-row batching only. It says nothing about native code
generation, GPUs, specialization/`#32`–`#33`, direct Wasm lowering (`#36`) or interaction nets
(`#37`). #14's historical performance STOP is unchanged, and no throughput, speed or memory claim
beyond these six numbers is made.
