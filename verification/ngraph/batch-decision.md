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
beats 157 ns/row at 65536 rows by 15 % against the measured 181 ns/row single lane
($181 / 1.15 \approx 157.39$, so the integer cutoff is 157). The earlier 177 ns/row figure was an
arithmetic error. Neither exists today, and neither is started here. Any
measurement that claims either must also state the instrumentation limitation recorded above: this
workspace cannot host a counting allocator under `unsafe_code = "forbid"`; out-of-process DHAT
allocation totals are now available, but they do not provide an in-process allocator seam or alter
the measured continuation decision.

## Cold, staging, output-copy and byte breakdown (item 6, 2026-11-07)

[`batch-breakdown.mjs`](batch-breakdown.mjs) runs `crates/noble-ngraph/tests/batch_rows/breakdown.rs`,
an ignored release harness over the same workload and rows, on an archived copy of the committed
source with the pinned toolchain. The retained receipt is
[`batch-breakdown-2026-11-07.json`](batch-breakdown-2026-11-07.json): clean source `d2a66ff3`, one
run, one host. Times are medians of five.

One-time costs per graph, as the first call in the process and the median of five fresh calls:

| Step | First call | Median |
|---|---|---|
| `check::check` | 18.7 µs | 2.7 µs |
| `Compiled::new` | 7.7 µs | 0.58 µs |
| `Prepared::new` | 0.83 µs | 0.14 µs |

The compiled graph retains 188 bytes, and one workspace's scratch is 64 bytes. Costs per batch
size:

| Rows | 1 | 4096 | 65536 |
|---|---|---|---|
| staging the input values | 70 ns | 5.0 µs | 77.9 µs |
| warm `Prepared::run` | 211 ns | 412 µs | 6.58 ms |
| copying every row's outputs out | 80 ns | 10.1 µs | 489 µs |
| staged input bytes | 32 | 131 072 | 2 097 152 |
| per-row report bytes | 128 | 524 288 | 8 388 608 |
| outputs owned by the reports | 32 | 131 072 | 2 097 152 |
| copied output bytes | 32 | 131 072 | 2 097 152 |
| resident growth across the first run | 0 | 724 992 | 11 141 120 |

Byte rows are declared from element counts and `size_of`, not measured. The per-row reports, 128
bytes each plus 32 bytes of outputs that each report owns, are five times the staged inputs. They
account for nearly all of the roughly 170 bytes per row of resident growth. That is the cost the
missing into-sink path above would remove. Staging inputs and copying outputs out cost about 1.2
and 7.5 ns per row at 65536 rows, small next to the run.

This is not a continuation measurement. Its warm run used a different build route from the
2026-11-01 lanes, and the two are not comparable, so the NO-GO above stands under its frozen
criteria. The authored workload has no extraction cost.

## Post-decision remeasurement and allocation counts (2026-11-13)

The ignored release timing lane was rerun after committing source `624c3de5`; the source tree was
clean. The full compact receipt is [`batch-measurements-2026-11-13.json`](batch-measurements-2026-11-13.json).
The same seven sizes were compared, with five timed repetitions per lane and size:

| Rows | 1 | 16 | 64 | 256 | 1024 | 4096 | 65536 |
|---|---:|---:|---:|---:|---:|---:|---:|
| `single` compiled (median ns/row) | 220 | 90.81 | 93.14 | 137.76 | 117.54 | 96.42 | 127.18 |
| `batch` median (ns/row) | 151 | 92 | 131.03 | 134.16 | 135.84 | 127.76 | 127.38 |

Batch is faster by the median at 1 and 256 rows only; the wins do not form a monotonic crossover.
At 4096 rows it is slower than the single-row lane, and at 65536 it is marginally slower
(127.38 vs 127.18 ns/row). The first and second identical 65536-row runs grew the resident set by
10,620,928 and 8,388,608 bytes, respectively. This is one-host post-decision reporting, not a
reapplication of the frozen continuation criteria; the original NO-GO remains unchanged.

Allocation counts were obtained out of process with the pinned Valgrind 3.27.1 DHAT tool, without
changing the workspace's `unsafe_code = "forbid"` policy. The full summary is
[`batch-allocation-2026-11-13.json`](batch-allocation-2026-11-13.json); at 65536 rows, the
reference, compiled single-row and batch lanes add approximately 3/192, 2/80 and 2/240
allocation blocks/bytes per row, respectively. The batch's counted bytes include 8 MiB for its
single report vector, 2 MiB of per-row preflight vectors, 2 MiB of output vectors, and harness
staging. DHAT totals sum allocations rather than measuring live memory and are not timing results.
Both dated receipts bind source commit `624c3de54a626c84f1ca5876f2381e6f940d42d7` and record the
pinned compiler/vendor provenance.

## Scope

This decision covers CPU independent-row batching only. It says nothing about native code
generation, GPUs, specialization/`#32`–`#33`, direct Wasm lowering (`#36`) or interaction nets
(`#37`). #14's historical performance STOP is unchanged, and no throughput, speed or memory claim
beyond the numbers in the retained receipts is made.
