# Numeric graph profile (#14) Phase B receipts

Phase B of [numeric-graph-profile](../../.cairn/archive/2026-10-16-numeric-graph-profile/design.md) (approved on #14 on 2026-10-15; decisions P1–P6 are in the design section "Phase B approval and decisions"). The experiment lives in the isolated crate [noble-ngraph-experiment](../../crates/noble-ngraph-experiment/Cargo.toml). It changes no noble-kernel, noble-contracts, noble-wasm or noble-cli source.

Since the `numeric-graph-v1` change, the graph library is the `no_std` crate [noble-ngraph](../../crates/noble-ngraph/Cargo.toml) and `noble-ngraph-experiment` is its evidence harness (bins, L1, oracles, workloads, fixtures), with the BLAKE3 identity scheme injected. The receipts below were produced before that split, at the source revisions they record. After the split, `tests/identity_agreement.rs` reproduces every identity of the pre-split record `crates/noble-ngraph-experiment/tests/fixtures/identities-66eb7cd.txt` (including B5), and `ngraph-phase-b --out <tmp>` at `bc34ed0` reported `result: passed` (8/8 records) with the B5 identities below; that run is a check of the cutover, not a new receipt.

## Certified-rewrite quality corpus (issue #64)

[rewrite-quality-v1.txt](rewrite-quality-v1.txt) is the frozen optimization-quality corpus for the
certified graph rewrites of `crates/noble-ngraph/src/cert.rs`: its schema is
`noble-rewrite-quality/v1`, it binds the three rules it exercises and declares, per fixture, the
residual property its result must reach (`no-foldable-sites`). Fixtures are frozen before any tuning,
so a difficult one cannot be relabelled to improve coverage.

`cargo test -p noble-ngraph --test rewrite_quality` measures certificate validity and optimization
coverage **separately**, with four proposer controls over the same validator:

| Control | Measured |
|---|---|
| `Identity` | valid certificate over identical endpoints, residual equals the untouched site count — **correctness alone fails the quality target** |
| `FirstEligible` | valid certificate, one fold applied, sites left behind on every multi-site fixture |
| `SinglePass` | valid certificate; reaches the target on fixtures whose folds all consume original constants |
| `FixedPoint` | reaches the target on a chained fixture too, but that derivation is **refused** `Malformed`: the certificate format binds every premise to the original graph, so a step consuming an earlier step's result is unsupported rather than silently accepted |

That last row is the honest boundary of this corpus: chained opportunities are *possible* (the
residual reaches zero under `FixedPoint`) and *uncertified* (the checker refuses it). No optimization
claim is made about them, and `SinglePass` passing the frozen fixtures is not evidence that a
one-pass proposer is sufficient in general.

Not measured yet: performance (cold/warm execution, certificate bytes, break-even) and the end-to-end
stage-cost accounting of issue #64 item 5, which needs the integrated stages.

## Files

| File | Content |
|---|---|
| [phase-b-core.json](phase-b-core.json) | Core receipt (schema `noble-ngraph-phase-b-core-evidence/v1`): BE-CONFIG-01 run fields and one EV-STATE-01 record per item, each with `implementation`, `execution`, `proof`, `trust`, open obligations, trusted boundaries and its result. |
| [phase-b-extract.json](phase-b-extract.json) | `I64` extraction-leg receipt (schema `noble-ngraph-phase-b-extract-evidence/v1`): BE-CONFIG-01 run fields (including the pinned CLI and Node with their SHA-256) and the EV-STATE-01 records `B1-I64-lowering`, `B2-I64-differential` and `B3-I64-source-controls`. |
| [test-log.txt](test-log.txt) | `cargo test -p noble-ngraph-experiment` output (default suite, debug) at the receipt's source revision. Historical: it predates the `numeric-graph-v1` split. |

The `I64` extraction leg covers `I1`–`I4` extracted from Core source by D7, L1 (the ordinary Noble Wasm/V8 path through the real CLI), the L2 BigInt oracle and the source-level B3 controls. The core receipt covers the `F32` legs and the authored-IR controls.

## Reproduce

From the repository root, inside the pinned devShell (`rust-toolchain.toml`, nightly-2026-03-21):

```text
cargo test -p noble-ngraph -p noble-ngraph-experiment
cargo run --release -p noble-ngraph-experiment --bin ngraph-phase-b -- \
  --out verification/ngraph/phase-b-core.json \
  --source-rev "$(git rev-parse HEAD)" --toolchain "$(rustc -V)"
cargo run --release -p noble-ngraph-experiment --bin ngraph-phase-b-extract -- \
  --cli /abs/noble --node "$NOBLE_NODE/bin/node" \
  --out verification/ngraph/phase-b-extract.json \
  --source-rev "$(git rev-parse HEAD)" --toolchain "$(rustc -V)"
```

Core defaults: 8 random graphs × 20,000 input vectors (48 operations each), 10^6 operand pairs per operation, 2,000 gradient samples, 500 training steps. The whole run took 0.66 s of wall time; the default test suite runs in about 1 s in debug.

Extraction-leg defaults: seed `0x1404b201`, 64 random Core programs × 10,000 vectors, L1 on 16 of them × 24 vectors, `I4` for 1,000 host steps. `--cli` is a release `noble` built from `origin/main` (the receipt used `noble-integration-eccbc02-release`, whose SHA-256 it records). The run took 232 s. The L2 test `extract_b2.rs` that needs `NOBLE_NODE` is `#[ignore]` in the default suite.

## Run (from the receipt)

- Source revision `f5b9ce5180350796dc11474b3007b003b90c40de`, `rustc 1.96.0-nightly (ac7f9ec7d 2026-03-20)`, release profile (overflow checks on), target x86_64-linux.
- Host: AMD RYZEN AI MAX+ 395, 32 logical CPUs, **shared**: load average 4.72/5.12/5.72 at start and 4.66/5.10/5.71 at end. Timings in the receipt are informational only; Phase B makes no performance claim.
- Oracle `rustc_apfloat =0.2.3+llvm-462a31f5a5ab`; PRNG `splitmix64/0`; identities BLAKE3 (`blake3 =1.8.7`, pure).
- Wasm features (GC, function references, multi-value, tail call, component model): not applicable, because these legs execute no Wasm. Hardware counters: unsupported (not reported as zero).
- Extraction leg: source revision `f5806a0f2dec22c59ad53d5b446a8c4db800e32c`, same toolchain, profile and host (load average 4.89–7.57). L1 runs `noble session --framed --opt off|on` of the pinned CLI (SHA-256 `f7f02dfb…85ec`); its Wasm features are fixed by that runtime (managed-linear-memory backend; `--opt on` runs `wasm-opt --enable-gc --enable-reference-types --enable-bulk-memory -O2 --strip-debug`). L2 is a BigInt modulo-2^64 oracle under Node v24.13.0 (SHA-256 `ad2d5c03…8f3b`). Extraction budgets are the `/0` defaults (work units 4,194,304, expanded nodes 262,144, inline depth 64).

## Results

| Item | Record | Execution | Result |
|---|---|---|---|
| B1 (extraction) | `B1-I64-lowering` | passed | `I1`–`I4` are prepared by the ordinary Core frontend, re-accepted by the kernel and extracted to checked `ngraph-i64/0` graphs (8, 4, 5 and 5 nodes) that give the design results 24, 44, 25 and (13, 13) on L3 and L4. `I2` reads its one shared capture (`2`) twice. `I3` calls the same `sq` identity twice. `I4` runs 1,000 host steps on L3 and L4 and matches `s·k+i`. |
| B1 (core) | `B1-core F1-F3` | passed | `F1` (2→3→1 ReGLU, 22 params), `F2` (transition, 1,000 host steps) and `F3` (`index` over `[a, a, b, -0]` under all 9 rule pairs) pass the checker and agree on L2, B2-ORACLE, L3 and L4. |
| B2 (extraction) | `B2-I64-differential` | passed | L2 (BigInt), L3 and L4 agree on 10 named graphs (`I1`–`I4`; operand order `5 3 -`; overflow `MAX 1 +` and `MIN -1 *`, each as literals and over inputs) and 64 seeded random Core programs (up to 57 live nodes, inline depth 4, 349 pruned nodes in total), × 10,000 vectors each, with 0 mismatches. L1 ran 26 graphs (the 10 named plus 16 random) on 562 vectors per mode: with `--opt off` and with `--opt on`, 538 agree, 0 disagree, and 24 are ordinary-path quota outcomes. Those 24 are every vector of one random program (`random-1`): the CLI refuses its root at acceptance as `exhausted` ("independent source compilation refused"), while D7 extracts it. They are reported as a separate limit class (BE-LIMIT-01), not as agreement. `I4` runs 1,000 host steps through L1 in both modes, and every step agrees. |
| B2 (core) | `B2-core boundary vectors` | passed | All 14 vectors match the expected bits on every leg: reassociation `2^24`, FMA `+0`, lone `-0`, `relu(-0)=+0`, `reglu(-1,0)=-0`, `select`/`index` copy `-0`, `2^127·2=+∞`, `reglu(∞,0)=NaN`, NaN candidate, `2^-126·0.5=2^-127`, repeated reads `2a`. Index boundaries `-1, 0, n−1, n, ±(2^24−1)` agree under every rule. A simulated FTZ backend and a fused evaluation are both detected. |
| B2-ORACLE | `B2-ORACLE double rounding` | passed | 10^6 random pairs per operation: `rn32(x op64 y)` = APFloat = host binary32 for `+`, `−`, `×`, with 0 mismatches. This is finite evidence for the D3 inference, not a proof. |
| B2 (core) | `B2-core seeded random F32 graphs` | passed | 8 graphs (seeds 1–8, 55–57 nodes, 7–12 layers) × 20,000 vectors. 0 disagreements across 160,000 vectors: 41,784 `Normal`, 81,075 `GuardFailed` (first failing guard in canonical order matches) and 37,141 `E-FACT-INPUT`. Outputs included 86,172 NaN, 141,086 zero, 11,318 subnormal and 51,204 infinite values. |
| B3 (extraction) | `B3-I64-source-controls` | passed | 16 source-level controls, each yielding its D6 code with 0 graph nodes evaluated: `test.emit` and `test.emit` inside `run` (`E-EFFECT`); a `Resource` input and a `LiveRef` input (`E-RESOURCE`); live-slot dispatch (`E-LIVE-SLOT`); `run` of a Program input (`E-DYNAMIC-RUN`); self and mutual recursion (`E-RECURSION`, or `E-SUBJECT` when the subject's dependency record states the cycle); `reflect` (`E-REFLECT`); `if`, `case` and `list.case` (`E-CONTROL`); a Program output (`E-PROGRAM-OUTPUT`); and the `k = 20` chain `dₖ = [ dₖ₋₁ dₖ₋₁ ]` under the root `0 d20 drop 7` (`E-BUDGET-EXPANSION`, observed 262,145 > 262,144 although the result is dropped). Each control also records what the ordinary path does with the same source. |
| B3 (core) | `B3-core rejections on authored IR` | passed | 35 controls, each yielding its D6 code. Facts: caller-asserted range/grid/bit/one-hot, a missing fact, an underivable `derived` claim and an `evidence` fact (P5) give `E-FACT-UNCHECKED`; a violating input gives `E-FACT-INPUT`; `0.5` held in a bit gives `G-FACT-FALSE`. Indices: `2.5`, a missing integral fact, a range reaching `2^24`, out of range under `refuse`, NaN under a guard. Malformed IR: cycle, forward reference, type mix ×2, dangling node, unknown node, unknown op, inexact `0.1`, NaN constant ×2, infinite coefficient, duplicate slot, `param` in `i64`, unknown sub-profile. Budgets: live nodes, layers (lowered and the 16,384 default), 4,097 affine terms, 65,537 table entries, raising a budget …
| B4 | `B4 gradients (F1 + MSE)` | passed | 2,000 points (seed `0xb40001`), 48,000 slot checks each. G1 against central differences: worst error 0.014 of the tolerance. G2 against G1: worst error 0.096 of the bound; 47,604 checks also meet the plain `|g1|`-relative bound. Corner, boolean (`E-DERIV-BOOLEAN`, straight-through, clamped-linear), index (candidate `a ← 2g`, reject, zero, backward-difference surrogate), `E-DERIV-I64` and `E-PARAM-UNDECLARED` are separate tests in [b4_gradients.rs](../../crates/noble-ngraph-experiment/tests/b4_gradients.rs). |
| B5 | `B5 training F1 with adam/0` | passed | Seed `0x14140005`, data seed `0x1414da7a`, 64 points (digest `7c40293c…0692`), 500 steps. MSE `0.30594` → `0.00052` (0.17% of initial). `GraphLogicalId` `6065e93f…d661` is unchanged, and so are the IR text and the data. `S₀` evaluates identically on all 64 points. Replay reproduces snapshot `075f58ca…7f9d`. Refusals: wrong graph `E-SNAPSHOT-STALE`, permuted layout `E-SNAPSHOT-LAYOUT`, stale revision `E-SNAPSHOT-STALE`. Old-subject evidence does not apply to the captured subject (`OtherSubject`). An infinite target gives `E-NONFINITE-UPDATE` with 0 published steps. |
| B6 | `B6 ownership and cleanup` | passed | 26 evaluation cases (cancellation at each of the 9 layers of `F1`, fuel exhaustion, guard failure, preflight refusal, normal return, each on L3 and L4) show scratch acquired = released and 0 live bytes; preflight refusals acquire none. Snapshot owners release in both orders. Training cancelled before step 4 publishes nothing new (last = the 3-step snapshot; leases 3/3/0). Retained digests are unchanged. |

## Investigations

- **G2 bound scale.** The first G2 run measured the error relative to the reverse pass over |forward values|, and 20 of 1,440 slot checks failed (worst ratio 3.6). The cause was cancellation in `d = y − t` near the target: the FP32 error of `d` scales with `|y|+|t|`, not with `|d|`. The bound `4·depth·2^-24` is now relative to the absolute-value magnitude of the whole computation (forward `|a|+|b|` and `|a|·|b|`, reverse over |local derivatives|). This is the standard first-order rounding scale, and the receipt also counts checks that meet the plain `|g1|`-relative bound. The tolerance itself was not changed.
- **Sensitivity.** Reversing the `affine` term order in L4 made the boundary-vector and random-graph tests fail. This was a manual mutation check during development; the mutation is not committed.

## Extraction-leg deviations and interpretations

These are differences between the design text and what the extraction leg executed. None changes a D6 code's meaning or a B-item's expected result.

- **No `(block …)` syntax.** Current Core source has no `(block IN OUT [ body ])` form. Each root body is prepared by the ordinary frontend with the block's input types on its stack, which is what `noble session` submits after pushing those values. `sq = [ dup * ]` is written `def sq [ dup * ]`.
- **Stage of three refusals.** D7 places `E-DYNAMIC-RUN` (a `run` of an unknown Program) and `E-PROGRAM-OUTPUT` in symbolic execution (stage 2). The implementation raises both at admission stage 1(c), because a Program-typed value in the root's input or output interface already decides them. `E-LIVE-SLOT` is raised while the closure is recomputed (stage 1(b)), because a live-slot dispatch has no static closure.
- **Recursion is unreachable from Core source.** The Core frontend refuses recursive definitions ("recursive definitions require a signature and are outside Core-Bootstrap"), so the recursion controls are minimally forged, kernel-accepted submissions. `E-RECURSION` comes from admission stage 1(b), from a depth-first search over definition identities, rather than from the named-call expansion stack. When the forged subject's `Env::deps` also records the cycle, the kernel's own acceptance refuses the body first, and admission reports `E-SUBJECT`. Both variants are recorded.
- **The `k = 20` chain is hand-assembled.** The Core frontend stops the chain at `d10` with an `exhausted` "frontend node limit exceeded". The `k = 20` subject is therefore assembled from the frontend's `d0` submission, with one shared row per level, and accepted by the kernel. The ordinary path's own outcome on the same source is recorded next to it.
- **L1 runs in chunked sessions.** A Core session retains every prepared module, so long runs use fresh session processes of at most 240 submissions, re-committing the definitions in each. A transition carries its state across sessions on the host side. A quota outcome caused by accumulated session state restarts the vector in a fresh session and is counted (`session_restarts_after_ordinary_quota`); it is not an observation of the root.
- **Codes for unspecified cases.** Where the design names no code, the leg chose one and states the reason in the diagnostic detail. D7 1(c) requires `P` scalars in the root interface but names no code: a Program-typed root input gives `E-DYNAMIC-RUN` and a Program-typed root output gives `E-PROGRAM-OUTPUT`. D5 says raising a `/0` budget needs a new profile policy revision but names no code: the request is refused with `E-PROFILE`.
- **Subject identity is a producer digest.** The subject digest in `GraphLogicalId` is BLAKE3 over the kernel's `Debug` rendering of the subject. It identifies bytes; it is not independent validation.
- **Only L1 checks extraction against source.** L2 (BigInt), L3 and L4 all evaluate the *extracted* graph. L1 is the only leg that observes the source independently of extraction, and it covers a recorded sample (562 vectors per mode and the 1,000-step `I4` transition), not every vector.

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
| [phase-c-l1.json](phase-c-l1.json) | W1/W2 from Core source (schema `noble-ngraph-phase-c-l1-evidence/v1`). It records extraction timing per D7 stage, L3/L4 on the extracted graphs, host-native, and the ordinary Noble Wasm/V8 baseline (L1, `--opt off` and `on`), with run pins and the CLI SHA-256. |

## Reproduce

```text
cargo build --release -p noble-ngraph-experiment --bin ngraph-phase-c
taskset -c 2 target/release/ngraph-phase-c --out verification/ngraph/phase-c-core-run1.json \
  --source-rev "$(git rev-parse HEAD)" --toolchain "$(rustc -V)" --pinned-cpu 2
cargo build --release -p noble-ngraph-experiment --bin ngraph-phase-c-l1
target/release/ngraph-phase-c-l1 --cli /abs/noble --out verification/ngraph/phase-c-l1.json \
  --source-rev "$(git rev-parse HEAD)" --toolchain "$(rustc -V)"
```

The harnesses are separate release binaries and are not part of the default test suite. The workload shapes, and bitwise agreement between each native baseline and L3/L4, are default tests in [c1_workloads.rs](../../crates/noble-ngraph-experiment/tests/c1_workloads.rs). One `ngraph-phase-c` run takes about 5 s. One `ngraph-phase-c-l1` run took 2.7 h, almost all of it W2 through L1 with `--opt on`.

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
- **Extraction time** does not apply to W3/W4: they are authored IR under B0(b), and graph authoring took 0.03–0.48 ms. For W1/W2, see the next section.
- **Ordinary Noble Wasm/V8 baseline**: none exists for W3/W4, because Noble's ordinary path has no `F32` under B0(b). For W1/W2, see the next section. The W1/W2 rows above use hand-built IR, which has the same operations and outputs as the extracted `I1`/`I4` graphs (`phase-c-l1.json` checks this).
- **Pinned handwired under pinned Bun** (optional, informational): not run.

## C1 W1/W2 from source against both baselines (`phase-c-l1.json`)

Run conditions: source `e60d0d86706de8d11d3aa2deced631c89748766c`, same toolchain and profile. The CLI was `noble-integration-eccbc02-release` (`origin/main` code, SHA-256 in the receipt). The process was **not** pinned, because L1 spawns multi-threaded Node/V8 and wasm-opt processes. The host was more heavily loaded than for the core runs: load average 21.2 at start and 19.8–21.4 at end, on 32 CPUs. As a result, the CPU-leg figures here are 1.5–2.7× slower than the pinned core run above (W1 L4 137 ns versus 65 ns; W2 L4 20.6 ms versus 7.48 ms). The ratios below compare legs from the same run. W1 ran `--opt off` and then `--opt on`; the two W2 modes ran concurrently.

Agreement before timing: every L1 observation is compared with L4 on the extracted graph. W1: 1,000 vectors per mode (the design vector `(5, 4)` plus 999 `splitmix64/0` vectors, seed `0x1414c101`), and all 1,000 agree in both modes. W2: all 100,000 steps agree in both modes, and the final state `-6083400928511612821` equals L3, L4 and native. There were no quota outcomes or session restarts.

- **Extraction** (median of 21; W1 / W2): frontend prepare 140 / 148 µs; admission (kernel re-check) 53 / 60 µs; extraction including the graph checker 7.6 / 9.1 µs; L4 build 1.3 / 1.3 µs; source to first L4 result 206 / 232 µs. The extractor counters for W1 are 8 work units, 8 expanded nodes, 0 pruned and inline depth 0. Live nodes / depth: 8 / 4 and 5 / 3.
- **L1 cost model.** `noble session` prepares, compiles, instantiates and runs every submission, so one L1 evaluation is one root submission, and one W2 host step is an input submission plus a root submission. The receipt times both from report-line arrival. These are the costs of that host interface, not of a call into an already instantiated Wasm function, which the CLI does not expose. Guest-side metrics for the W1 design vector: 768 B allocated, 48 B operand peak, 18 execution steps, 1 MiB linear memory. Wasm module bytes, V8 heap and RSS are unsupported.

| Workload | L4 (compiled graph) | L3 | Host-native | L1 `--opt off` | L1 `--opt on` | L4 ÷ native speed | L4 ÷ L1 speed (off / on) |
|---|---|---|---|---|---|---|---|
| W1 warm, per evaluation (median [IQR]) | 137 ns [26] | 162 ns | 1.06 ns | 31.0 ms [5.0] | 42.3 ms [4.6] | 0.0077× | 2.3×10^5 / 3.1×10^5 |
| W1 cold (spawn or source to first result) | 206 µs | — | — | 388 ms | 263 ms | — | — |
| W2, 10^5 host steps (total) | 20.6 ms | 18.6 ms | 0.135 ms | 3,633 s | 9,710 s | 0.0066× | 1.8×10^5 / 4.7×10^5 |
| W2 per host step (L1 warm median [IQR]) | — | — | — | 33.0 ms (root 16.6 [2.6]) | 87.0 ms (root 43.4 [5.4]) | — | — |

On W1 and W2 the compiled graph is about five orders of magnitude faster than the ordinary path's per-submission host interface. It is still 130–150× slower than host-native, so it is not at least 2× faster than both baselines (`l4_at_least_2x_faster_than_both_baselines: false` for both workloads). Host loop throughput through L1 was 14.9 / 10.4 W1 evaluations/s and 27.5 / 10.3 W2 steps/s (off / on). There is no training step for these workloads (`E-DERIV-I64`).

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
- **W1/W2 against both baselines** (`phase-c-l1.json`). These do not change the outcome. L4 is far faster than the ordinary path's per-submission interface but 0.0066–0.0077× host-native, so it is not at least 2× faster than both baselines on any workload. W1/W2 are `I64` and do not stand in for the W3 test.
- **Interpretation, not measured.** The compiled evaluator interprets one enum-dispatched instruction per node, about 1.5 ns per node on W3-L. A straightforward native loop is 7.6–16.5× faster on these shapes. Only a design change could plausibly reach the 2× threshold, such as batch-vectorized evaluation or native code generation. Doing so would be a new experiment and is not claimed here.

# Supported-profile CPU follow-on (2026-10-16)

This is a separate, source-pinned measurement after the owner adopted the profile;
it does not replace the frozen Phase B/C receipts above or the lifecycle acceptance
and review packets in `verification/numeric-graph-v1/`. It makes no GPU, backend
selection, theorem, or claim of beating native Rust.

## Source, method and limits

The comparison uses the existing Phase C W1/W2/W3-S/M/L/W4 constructors and exact
input, parameter, dataset and optimizer bits, with the selected locked/offline
vendor and the same rustc/Cargo release profile in both lanes. The library baseline
is byte-identical to published anchor `cd76c608721f25ce7f7bc896dfc7cdeb54e9b004`.
Baseline harness commit `f03aa4d1a4d8612721fbed9a1577ae65befffa93` adds measurement
only; final implementation/harness commit is
`7e1de8f1eccdece595dea295a6e00a14c93b2e62`. The intermediate selected
`5ca3665` lane is retained, including its measured observer overhead, not silently
substituted for the final lane. Raw receipts, exact hashes, source manifests and
the reproduction configuration are in [`cpu-20261016/receipt.json`](cpu-20261016/receipt.json).

Each selected lane has three complete runs with the unchanged harness defaults
`--samples 2000 --cold-reps 21`
and one `--phase-profile true` function-level wall-time run; measurements use
CPU 2 via `taskset`, `nice -n 10`, and the campaign Cargo lock. `perf` was absent
from PATH and `perf_event_paranoid` was 2, so the profile is **real function-level
wall timing and structural retained/scratch-byte accounting**, not sampled CPU
stacks or an allocation-count profile. No unsafe global allocator was installed.
The host was shared, not idle or isolated: final-run load averages were lower
than baseline. The lock excluded campaign builds, not all host activity. Tiny
latency differences and overlapping ranges are not evidence of improvement.

The initial pre-edit/exploratory baseline remains contextual evidence only: its
then-used dependency/vendor context is not retrospectively called the selected
context. The final ratios below use only the paired selected receipts.

## Measured changes

Ranges below are the three runs' reported medians, not confidence intervals.
Admission/check, cold compile, warm evaluation and training are separate costs.
The one-step and derivative-admission rows come from the separate function-profile
runs and are not the steady 100-step measurement.

| Cost | Selected baseline | Final | Interpretation |
| --- | ---: | ---: | --- |
| W4 graph check | 5.979–6.209 ms | 6.156–6.434 ms | No checker speedup |
| W4 cold compile | 0.919–1.012 ms | 0.085–0.124 ms | 7.4–12.0× lower compile time across the ranges |
| W4 total cold check + compile + first evaluation | 6.893–7.267 ms | 6.259–6.573 ms | Checker still dominates cold admission |
| W4 retained compiled-code byte estimate | 2,721,604 B | 161,472 B | 16.85× smaller; structural estimate, not allocator/RSS measurement |
| W3-L derivative admission | 115.606 µs | 30.357 µs | 3.81× lower wall time in the function-profile pair |
| W3-L inclusive one-step training call | 196.467 µs | 103.003 µs | 1.91× lower wall time; includes admission/setup/finalization |
| W3-L steady training, per step, 15 points | 35.912–39.392 µs | 35.547–37.001 µs | Overlap; no steady-training throughput claim |
| W3-L warm compiled evaluation | 5.879–6.107 µs | 5.808–6.487 µs | Overlap; no warm-evaluation speedup claim |

The measured bottlenecks justify three boring implementation changes: compiled
table instructions borrow the single retained checked-graph table instead of
copying it into every instruction; derivative admission propagates parameter
influence once in canonical dependency order instead of repeating recursive
reachability walks; and observed/unobserved private instruction loops specialize
once per layer so the default path does not pay an optional observer branch per
instruction. There is no new backend, dependency, workspace target, numerical
profile, optimizer or scheduler/compiler revision. The separately approved
caller-owned safety observer hooks are exercised both with and without an observer.

On the final W3-L runs L4 remains only 0.142–0.155× host-native Rust speed, about
6.5–7.0× slower. These observations do not reverse the historical stop result or
establish that this profile is faster than either required baseline.

## Numerical and resource semantics

All 12 selected receipts (baseline, intermediate and final, including their
function-profile runs) retain exact W3 parameter/input/data bits, graph and packed
layout identities, final loss bits, snapshot identities and optimizer-state
hashes. The W3/W4 per-run L3/L4 agreements pass; I64 layouts and W2 final-state
agreements also match. Register/scratch estimates are unchanged; the intentional
retained-code reduction is reported separately. This is finite execution evidence,
not a discharged proof obligation.

Guard ordering and fuel/cancellation checkpoints are not moved. Compiled execution
still materializes input slots before its first layer fuel/cancellation check;
those writes are actual work and observer counters include them even when fuel
is zero. Historical `Report::evaluated_nodes` charges are not redefined as actual
events. Training still computes all pure model forward values before scanning
for the first canonical failure, as the anchor did; a failed point stops the
step and publishes no snapshot, but this is **not** a no-later-work cutoff claim.
Scratch leases and cancellation-before-step behavior remain independently
observed boundaries. See the resource proofs and production CLI smoke linked
from the receipt, rather than inferring work counts from output agreement.

The machine-readable index is [`cpu-20261016/receipt.json`](cpu-20261016/receipt.json).
It hashes every retained raw receipt, source manifest, harness/probe source and
recorded runner, and identifies benchmark/CLI/probe binaries and external frozen
source archives. The scripts are exact **host-specific recorded invocations**,
not automatic source-lane selectors: reproduce the specified source in separately
owned worktrees, retain the selected vendor/toolchain/configuration, and choose
new output paths rather than overwriting frozen evidence. Hold the campaign lock
around both the build and timing command. `run-selected-comparison.sh` records
the baseline/intermediate run; `run-selected-final.sh` records the final run.
The old environment capture supplies compiler version details; selected Cargo
home and vendor overrides are explicitly recorded separately in the receipt.

## Executed quality and production smoke

On the final selected source, `run-selected-quality.sh` completed with exit 0:
the complete `noble-ngraph` and `noble-ngraph-experiment` suites plus the existing
CLI `ngraph` suite passed (106 passed, two existing ignored tests), followed by
three-package all-target clippy with `-D warnings`, three-package format check,
and a release build of the production `noble` executable. A separate selected
baseline release ownership check passed all five tests; the combined log therefore
has 111 passes and two ignored tests. Commands and logs are retained with the receipt.

The actual final release CLI created a seed snapshot, trained ten steps from it,
saved the child, replay-verified it from the parent, and evaluated saved parameters
through both evaluators with exactly equal reports. Assertions check signed zero,
NaN, infinity and finite output encodings, missing-snapshot binding refusal,
canonical guard refusal, fuel-zero refusal, lineage and unchanged parent bytes.
The external native-oracle table probe also passed 612 explicit boundary,
NaN and refusal cases. The training refusal probe observed four forward/validation
events versus the reference evaluator's three-node cutoff, no publication,
unchanged snapshot bytes, and scratch `(acquired, released, live) = (1, 1, 0)`;
cancellation before a step observed zero events/publications/leases. These are
finite, specific exercised cases, not global no-work or resource-proof claims.
