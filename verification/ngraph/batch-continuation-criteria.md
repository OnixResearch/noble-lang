# CPU batch continuation criteria (issue #41), frozen before measurement

This file was written **before** the measurement lane was run. Nothing in it may be edited after
numbers exist; a later decision may only quote it.

## Lanes

Over one checked `I64` graph (two inputs, five arithmetic nodes, two outputs) and identical boundary
operands, all in a release build:

| Lane | What it measures |
|---|---|
| `reference` | the normative reference interpreter, once per row |
| `single` | the compiled evaluator, once per row, reusing one register workspace |
| `batch` | `batch::Prepared::run` over all rows, one workspace for the whole run |
| `native` | straightforward `i64` wrapping arithmetic — the host-native floor, informational |

Sizes: **1** row (first-result latency), **4096** rows (medium), **65536** rows (amortizing). Every
row's outputs must agree across `reference`, `single` and `batch`; that agreement is a gate, not a
finding.

## Decision rule

A **GO** for continued work on CPU batching requires **all** of:

1. every row's outputs agree across `reference`, `single` and `batch` at all three sizes;
2. the `batch` lane's warm throughput is at least **1.15×** the `single` lane's at 65536 rows
   (`batch_ns_per_row × 1.15 ≤ single_ns_per_row`);
3. the `batch` lane is **not slower** than the `single` lane at 4096 rows
   (`batch_ns_per_row ≤ single_ns_per_row`);
4. the `batch` lane's resident-set growth across one 65536-row run is at most **8 MiB**, and a
   second identical run grows it by at most **1 MiB** (no unbounded warm-loop growth).

Anything else is **NO-GO**: no further batching work (SIMD, F32, chunked staging) is started on this
issue, and the measured numbers are published as they are rather than explained away.

`native` is reported but is **not** a GO condition: the question is whether batching amortises over
the evaluator Noble already has, not whether Noble beats host-native arithmetic. If `batch` is
slower than `native` at every size, that is recorded as context for any later decision, and it does
not by itself fail the rule above.

## Instruments and their gaps

- **Timing**: `std::time::Instant`, release build, after one warm run of both compiled lanes.
- **Memory**: resident-set size read from `/proc/self/statm` before and after each lane, pages
  scaled by 4096 bytes on this host.
- **Allocation counts: unavailable.** The workspace forbids `unsafe_code`, so a counting global
  allocator cannot exist in any target. This lane therefore does **not** claim which allocations
  were removed; it reports resident growth and declares the gap.
- No assertion about elapsed time is made in any test; the numbers are reported observations of one
  host at one revision.
