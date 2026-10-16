# Numeric graph profile (#14) Phase B receipts

Phase B of [numeric-graph-profile](../../.cairn/changes/numeric-graph-profile/design.md) (approved on #14 on 2026-10-15; decisions P1–P6 are in the design section "Phase B approval and decisions"). The experiment lives in the isolated crate [noble-ngraph-experiment](../../crates/noble-ngraph-experiment/Cargo.toml). It changes no noble-kernel, noble-contracts, noble-wasm or noble-cli source.

## Files

| File | Content |
|---|---|
| [phase-b-core.json](phase-b-core.json) | Core receipt (schema `noble-ngraph-phase-b-core-evidence/v1`): BE-CONFIG-01 run fields and one EV-STATE-01 record per item, each with `implementation`, `execution`, `proof`, `trust`, open obligations, trusted boundaries and its result. |
| [test-log.txt](test-log.txt) | `cargo test -p noble-ngraph-experiment` output (default suite, debug) at the receipt's source revision. |

The `I64` extraction leg (`I1`–`I4`, L1 ordinary Wasm/V8, the BigInt oracle and the source-level B3 controls) is recorded in its own receipt by that leg.

## Reproduce

From the repository root, inside the pinned devShell (`rust-toolchain.toml`, nightly-2026-03-21):

```text
cargo test -p noble-ngraph-experiment
cargo run --release -p noble-ngraph-experiment --bin ngraph-phase-b -- \
  --out verification/ngraph/phase-b-core.json \
  --source-rev "$(git rev-parse HEAD)" --toolchain "$(rustc -V)"
```

Defaults: 8 random graphs × 20,000 input vectors (48 operations each), 10^6 operand pairs per operation, 2,000 gradient samples, 500 training steps. The whole run took 0.66 s of wall time; the default test suite runs in about 1 s in debug.

## Run (from the receipt)

- Source revision `f5b9ce5180350796dc11474b3007b003b90c40de`, `rustc 1.96.0-nightly (ac7f9ec7d 2026-03-20)`, release profile (overflow checks on), target x86_64-linux.
- Host: AMD RYZEN AI MAX+ 395, 32 logical CPUs, **shared**: load average 4.72/5.12/5.72 at start and 4.66/5.10/5.71 at end. Timings in the receipt are informational only; Phase B makes no performance claim.
- Oracle `rustc_apfloat =0.2.3+llvm-462a31f5a5ab`; PRNG `splitmix64/0`; identities BLAKE3 (`blake3 =1.8.7`, pure).
- Wasm features (GC, function references, multi-value, tail call, component model): not applicable, because these legs execute no Wasm. Hardware counters: unsupported (not reported as zero).

## Results

| Item | Record | Execution | Result |
|---|---|---|---|
| B1 (core) | `B1-core F1-F3` | passed | `F1` (2→3→1 ReGLU, 22 params), `F2` (transition, 1,000 host steps) and `F3` (`index` over `[a, a, b, -0]` under all 9 rule pairs) pass the checker and agree on L2, B2-ORACLE, L3 and L4. |
| B2 (core) | `B2-core boundary vectors` | passed | All 14 vectors match the expected bits on every leg: reassociation `2^24`, FMA `+0`, lone `-0`, `relu(-0)=+0`, `reglu(-1,0)=-0`, `select`/`index` copy `-0`, `2^127·2=+∞`, `reglu(∞,0)=NaN`, NaN candidate, `2^-126·0.5=2^-127`, repeated reads `2a`. Index boundaries `-1, 0, n−1, n, ±(2^24−1)` agree under every rule. A simulated FTZ backend and a fused evaluation are both detected. |
| B2-ORACLE | `B2-ORACLE double rounding` | passed | 10^6 random pairs per operation: `rn32(x op64 y)` = APFloat = host binary32 for `+`, `−`, `×`, with 0 mismatches. This is finite evidence for the D3 inference, not a proof. |
| B2 (core) | `B2-core seeded random F32 graphs` | passed | 8 graphs (seeds 1–8, 55–57 nodes, 7–12 layers) × 20,000 vectors. 0 disagreements across 160,000 vectors: 41,784 `Normal`, 81,075 `GuardFailed` (first failing guard in canonical order matches) and 37,141 `E-FACT-INPUT`. Outputs included 86,172 NaN, 141,086 zero, 11,318 subnormal and 51,204 infinite values. |
| B3 (core) | `B3-core rejections on authored IR` | passed | 35 controls, each yielding its D6 code. Facts: caller-asserted range/grid/bit/one-hot, a missing fact, an underivable `derived` claim and an `evidence` fact (P5) give `E-FACT-UNCHECKED`; a violating input gives `E-FACT-INPUT`; `0.5` held in a bit gives `G-FACT-FALSE`. Indices: `2.5`, a missing integral fact, a range reaching `2^24`, out of range under `refuse`, NaN under a guard. Malformed IR: cycle, forward reference, type mix ×2, dangling node, unknown node, unknown op, inexact `0.1`, NaN constant ×2, infinite coefficient, duplicate slot, `param` in `i64`, unknown sub-profile. Budgets: live nodes, layers (lowered and the 16,384 default), 4,097 affine terms, 65,537 table entries, raising a budget (`E-PROFILE`), forward scratch. Static and preflight refusals evaluate 0 nodes; the guard failure stops before its dependent node. |
| B4 | `B4 gradients (F1 + MSE)` | passed | 2,000 points (seed `0xb40001`), 48,000 slot checks each. G1 against central differences: worst error 0.014 of the tolerance. G2 against G1: worst error 0.096 of the bound; 47,604 checks also meet the plain `|g1|`-relative bound. Corner, boolean (`E-DERIV-BOOLEAN`, straight-through, clamped-linear), index (candidate `a ← 2g`, reject, zero, backward-difference surrogate), `E-DERIV-I64` and `E-PARAM-UNDECLARED` are separate tests in [b4_gradients.rs](../../crates/noble-ngraph-experiment/tests/b4_gradients.rs). |
| B5 | `B5 training F1 with adam/0` | passed | Seed `0x14140005`, data seed `0x1414da7a`, 64 points (digest `7c40293c…0692`), 500 steps. MSE `0.30594` → `0.00052` (0.17% of initial). `GraphLogicalId` `6065e93f…d661` is unchanged, and so are the IR text and the data. `S₀` evaluates identically on all 64 points. Replay reproduces snapshot `075f58ca…7f9d`. Refusals: wrong graph `E-SNAPSHOT-STALE`, permuted layout `E-SNAPSHOT-LAYOUT`, stale revision `E-SNAPSHOT-STALE`. Old-subject evidence does not apply to the captured subject (`OtherSubject`). An infinite target gives `E-NONFINITE-UPDATE` with 0 published steps. |
| B6 | `B6 ownership and cleanup` | passed | 26 evaluation cases (cancellation at each of the 9 layers of `F1`, fuel exhaustion, guard failure, preflight refusal, normal return, each on L3 and L4) show scratch acquired = released and 0 live bytes; preflight refusals acquire none. Snapshot owners release in both orders. Training cancelled before step 4 publishes nothing new (last = the 3-step snapshot; leases 3/3/0). Retained digests are unchanged. |

## Investigations

- **G2 bound scale.** The first G2 run measured the error relative to the reverse pass over |forward values|, and 20 of 1,440 slot checks failed (worst ratio 3.6). The cause was cancellation in `d = y − t` near the target: the FP32 error of `d` scales with `|y|+|t|`, not with `|d|`. The bound `4·depth·2^-24` is now relative to the absolute-value magnitude of the whole computation (forward `|a|+|b|` and `|a|·|b|`, reverse over |local derivatives|). This is the standard first-order rounding scale, and the receipt also counts checks that meet the plain `|g1|`-relative bound. The tolerance itself was not changed.
- **Sensitivity.** Reversing the `affine` term order in L4 made the boundary-vector and random-graph tests fail. This was a manual mutation check during development; the mutation is not committed.

## Assumptions and boundaries

- `F32` graphs are authored IR (B0(b)). There is no ordinary-execution leg for `F32`, only independent references (Risks in the design).
- B2-ORACLE computes in binary64 and rounds once with Rust's `as f32`, which is RNE, the same rounding as `Math.fround`.
- Trusted boundaries: the rustc/LLVM codegen (no contraction), the host CPU's IEEE behaviour (no FTZ/DAZ), `rustc_apfloat` as the L2 oracle, BLAKE3 for identities (a hash identifies bytes), and this crate's checker, evaluators and trainer.
- B6 instrumentation uses scratch-lease counters, not a global allocator: a counting allocator needs `unsafe`, which the workspace forbids.

## Non-claims

- Finite tests and evaluator agreement do not prove the compiler, the graph checker (NG-PO-B), the compiled evaluator (NG-PO-C), the binary32 model (NG-PO-D), fact transfer (NG-PO-E), the derivative policy (NG-PO-F) or snapshot binding (NG-PO-G). NG-PO-A..H remain open and are not added to `obligations.json`.
- B5's threshold is a smoke test, not a convergence claim. Replay is shown on this platform only.
- No performance, GPU or backend-selection claim; that is Phase C.

# Phase C (C1–C3) measurements

The design's Phase C plan was approved on 2026-10-15 with the C3 thresholds kept.

## Files

| File | Content |
|---|---|
| [phase-c-core-run1.json](phase-c-core-run1.json) | Primary run (schema `noble-ngraph-phase-c-core-evidence/v1`). It records run pins, W3 (3 sizes), W4, the W1/W2 CPU legs, C2 claim separation and the C3 decision. |
| [phase-c-core-run2.json](phase-c-core-run2.json), [phase-c-core-run3.json](phase-c-core-run3.json) | Back-to-back repeats with the same command. They show run-to-run stability; the key ratios are given below. |

## Reproduce

```text
cargo build --release -p noble-ngraph-experiment --bin ngraph-phase-c
taskset -c 2 target/release/ngraph-phase-c --out verification/ngraph/phase-c-core-run1.json \
  --source-rev "$(git rev-parse HEAD)" --toolchain "$(rustc -V)" --pinned-cpu 2
```

The harness is a separate release binary and is not part of the default test suite. The workload shapes, and bitwise agreement between each native baseline and L3/L4, are default tests in [c1_workloads.rs](../../crates/noble-ngraph-experiment/tests/c1_workloads.rs). One run takes about 5 s.

## Run conditions

- Source `2358a74730d5fc14888c6a1c10085e6b0ed69575`; `rustc 1.96.0-nightly (ac7f9ec7d 2026-03-20)`; release profile (overflow checks on); x86-64 default target features, so no FMA instructions, and rustc does not contract.
- AMD RYZEN AI MAX+ 395 (32 logical CPUs), with the process pinned to CPU 2 using `taskset`. Governor `performance`, driver `amd-pstate-epp`.
- The host is **shared**. The load average was 6.3–8.0 on 32 CPUs across the three runs, which is the lowest observed during this session. Timings are wall-clock (`Instant`). Hardware counters, RSS, cache and energy are recorded as **unsupported**, not as zero. Byte figures are element-count estimates.
- Warm latency: N = 2,000 samples, each a calibrated batch of at least 20 µs divided by the batch size, so the values are per-evaluation medians and IQRs. Cold latency is the median of 21 repetitions of check + compile + first evaluation. Throughput is a 0.3 s loop. Training is measured as 15 points per step.
- Before any timing, the host-native baselines agree with L3 and L4 under `≡obs`: on 200 input vectors per W3 size, including random specials; on 200 W4 index vectors, including out-of-range refusals; on the W1 value; and on the W2 final state after 10^5 steps.

## C1 results (run 1)

Runs 2 and 3 match run 1 within ±10%. The exceptions are sub-0.05 ms cold figures (W3-S admission 0.036 ms in run 1 versus 0.025 ms in runs 2–3) and the W3-S native training step in run 2 (1.17 µs). Both vary by up to 30% on the shared host.

| Workload | Live nodes / depth | Admission (check) | Build (compile) | Cold total | Warm L4 median [IQR] | Warm L3 | Warm native | L4 ÷ native speed |
|---|---|---|---|---|---|---|---|---|
| W3-S (4→4→1, 45 params, 76 ops) | 125 / 12 | 0.036 ms | 0.005 ms | 0.049 ms | 332 ns [6] | 843 ns | 22.7 ns | 0.068× |
| W3-M (4→22→1, 243 params, 418 ops) | 665 / 30 | 0.121 ms | 0.014 ms | 0.160 ms | 1,468 ns [19] | 3,858 ns | 89.2 ns | 0.061× |
| W3-L (W3-M × 15 points, 243 params, 6,270 ops) | 6,573 / 30 | 1.097 ms | 0.221 ms | 1.358 ms | 9,711 ns [70] | 34,604 ns | 1,271 ns | **0.131×** |
| W4 (32 reads of a 10,000-entry table) | 10,065 / 3 | 9.80 ms | 1.54 ms | 11.35 ms | 706 ns [13] | 38,562 ns | 128 ns | 0.182× |
| W1 = `I1` (hand-built IR) | 8 / 4 | — | — | 4.2 µs | 65 ns | 76 ns | 0.70 ns | 0.011× |
| W2 = `I4`, 10^5 host steps (total) | 5 / 3 | — | — | — | 7.48 ms | 8.30 ms | 0.134 ms | 0.018× |

- **Bytes** (params / L3 activations / L4 register file / retained graph / retained compiled code):
  - W3-S: 180 / 2,000 / 504 / 12,318 / 2,864.
  - W3-M: 972 / 10,640 / 2,664 / 64,554 / 15,032.
  - W3-L: 972 / 105,168 / 26,296 / 631,890 / 179,560.
  - W4: 0 / 161,040 / 40,264 / 2,252,028 / 2,721,604. L4 copies the table operand list into each of the 32 index instructions; this is a measured inefficiency of the current L4.
- **Throughput** (evaluations/s, L4 / L3 / native):
  - W3-S: 2.99 M / 1.19 M / 41.8 M.
  - W3-M: 0.68 M / 0.25 M / 11.0 M.
  - W3-L: 0.10 M / 0.028 M / 0.75 M.
  - W4: 1.38 M / 0.025 M / 7.7 M.
- **Training step, 15 points** (ours = FP32 reference backward + `adam/0` + immutable snapshot publication; native = hand-written FP32 backward + binary64 Adam, updated in place):

  | Size | Ours | Native | Ours slower by |
  |---|---|---|---|
  | W3-S | 18.1 µs | 0.92 µs | 19.7× |
  | W3-M | 80.5 µs | 4.87 µs | 16.5× |
  | W3-L | 60.0 µs | 4.84 µs | 12.4× |

  Backward scratch estimates: 1,032 / 5,352 / 53,176 B.
- **Extraction time** does not apply to W3/W4: they are authored IR under B0(b), and graph authoring took 0.03–0.48 ms. For W1/W2 it comes from the extraction leg.
- **Ordinary Noble Wasm/V8 baseline**: none exists for W3/W4, because Noble's ordinary path has no `F32` under B0(b). For W1/W2 it comes from the extraction leg's L1 harness and is **pending**. W1/W2 here use hand-built IR equal to the `I1`/`I4` lowering.
- **Pinned handwired under pinned Bun** (optional, informational): not run.

## C2 claim separation

`phase-c-core-run1.json` → `c2` keeps the following as separate fields:

- reference agreement (the per-workload agreement checks and Phase B's L2/B2-ORACLE/L3/L4);
- finite coverage (finite only);
- theorem obligations (NG-PO-A..H, each `open`);
- trusted boundaries (codegen, host IEEE, oracle, BLAKE3, this crate and the native baselines, timer and shared host);
- unsupported instrumentation.

No hash match or agreement is reported as discharging an obligation.

## C3 recommendation: **stop**

- **Threshold test.** Warm W3-L: the compiled CPU graph (L4) is 0.131× the speed of the host-native baseline (0.131 / 0.131 / 0.131 across the three runs), about 7.6× **slower**. The kept threshold requires at least 2× faster, so the stop condition holds.
- **Admission.** Admission at W3 scale is 1.1 ms, within the 100 ms threshold. Training value was demonstrated (B5). Neither changes the outcome, because the speed condition alone triggers stop.
- **"Both baselines" for W3.** W3 is `ngraph-f32/0` authored IR (B0(b)), and Noble's ordinary Wasm/V8 path has no `F32`, so that baseline does not exist for W3. I applied the test to the one existing baseline, host-native Rust. A missing baseline cannot make the outcome more favourable: passing would have required beating host-native by 2× and also an ordinary-path comparison that cannot be made. W1/W2, where the ordinary path exists, are reported separately and do not stand in for W3.
- **GPU (#2).** Blocked. No transfer or launch cost was measured or modelled, and with a stop recommendation there is no compiled-CPU specialization to compare a GPU against.
- **Interpretation, not measured.** The compiled evaluator interprets one enum-dispatched instruction per node, about 1.5 ns per node on W3-L. A straightforward native loop is 7.6–16.5× faster on these shapes. Only a design change could plausibly reach the 2× threshold, such as batch-vectorized evaluation or native code generation. Doing so would be a new experiment and is not claimed here.
