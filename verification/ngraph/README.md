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
