# GPU CPU-cost study (`gpu-offload-research` Phase 2)

`study.mjs` measures the CPU-side cost terms of the end-to-end cost model in
[`.cairn/changes/gpu-offload-research/design.md`](../../.cairn/changes/gpu-offload-research/design.md)
on the selected Node/V8 Wasm route (`crates/noble-cli/src/core/runtime/config.json`) under
Owner Decisions 2 and 3 (2026-11-20).

## What it measures

- **Dispatch cost `c_disp(w)`**: per-element `$push_i64`, `$enqueue_program`, `$dispatch`
  and `$pop_kind` over 13 `GPU-Map-Draft` eligible `[I64] -> [I64] ! {}` kernels spanning
  widths `w = 2..64` (including `+`, `-`, `*`, `=`, `dup`, `swap`, `drop`, and scalar `if`),
  compiled from `.noble` source by `noble run --emit` and measured for both `opt: off` and
  `opt: on` (`wasm-opt -O2` with the pinned `optimizer_flags`) under the exact pinned
  `node_flags` (`--no-liftoff --no-wasm-lazy-compilation --no-wasm-tier-up --stack-size=1024 --max-old-space-size=256`).
- **List/pack/unpack helpers**:
  - `c_alloc`: allocating one boxed `I64` cell + one `List.Cons` cell (96 bytes/element) via `$new`.
  - `c_traverse`: traversing a linked `List<I64>` (`$payload($a(node))`, `$b(node)`).
  - `c_pack`: linked `List<I64>` traversal + contiguous `i64.store` into a flat `[u64; n]` staging buffer (`c_pack_marginal = c_pack - c_traverse`).
  - `c_unpack_total`: flat `i64.load` + rebuilding `List<I64>` (2 cells/element), with marginal `c_unpack = c_unpack_total - c_alloc` and `c_load_only` isolated separately.
- **End-to-end sequential `c_cpu_map(w)`**: complete in-Wasm linked-list map (traverse -> dispatch -> box + cons) cross-checked against `c_traverse + c_disp(w) + c_alloc`.
- **Reachable `List<I64>` & step ceiling controls**: empirical at-bound pass and one-over quota refusal (`quota_reason = 1` allocation, `quota_reason = 6` execution steps) under default (`65,536` bytes) and maximum (`196,608` bytes / `4096` session cells, `100,000` steps) limits.

## Verification & independent oracle

Every boundary test (13 kernels × 15 K-NUM-01 overflow/carry/borrow/order/branch inputs × 2 opt levels = 390 evaluations) and every timed benchmark sample is checked against an independent arbitrary-precision `BigInt` implementation of the K-NUM-01 mathematical model
`((z + 2^63) mod 2^64) - 2^63` in [`kernels.mjs`](kernels.mjs), never against Wasm/V8 arithmetic.
Self-tests (`--self-test`) additionally include negative controls proving that wrong-kernel or bit-flipped checksums are rejected.

## Usage

From a clean checkout inside the pinned toolchain environment:

```sh
# Self-test oracle, negative tampered-output controls, and crossover math
node verification/gpu-cost/study.mjs --self-test

# Full clean-source run (builds `noble` via cargo if `--noble` is omitted)
node verification/gpu-cost/study.mjs --out <new-output-dir>
```

Pass `--noble <path-to-noble-binary>` to reuse a clean-source `noble` binary built from the
same commit (its SHA-256 is recorded before and after the run). Retained receipts require a
clean git tree (never `--allow-dirty`).

## Non-claims

This lane measures only the CPU-side Wasm terms and empirical `List<I64>` ceilings on the
selected Node/V8 route. It executes no GPU hardware, driver, SPIR-V, WGSL or PTX code,
implements no `batch.map` or packed array type, and promotes no conformance case or proof
obligation.

## Retained runs

(Populated upon clean-tree execution.)
