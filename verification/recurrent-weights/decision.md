# Recurrent-weight evaluation — final disposition

## Frozen rule

The continuation criteria in `criteria.md` were committed before measurement in `0962b348` and have not been changed. The final receipt in `receipts/2026-11-13/evaluation-3e7b1d4.json` was produced from clean implementation commit `3e7b1d4daa4dc92e10ec7e5e96f69ba7fadd438a`.

## Unit disposition

| Brief unit | Status | Evidence |
|---|---|---|
| U1 — criteria and design | DONE | `criteria.md` and `design.md`, commit `0962b348`. The exact-bit accumulator and stack-routing target, codec, bounded host contract, and continuation thresholds were fixed before measurement. |
| U2 — host-loop baseline | DONE | `evaluation-3e7b1d4.json`: both deterministic programs, exhaustive tiny domains, arithmetic boundaries, and separate 1,000-step teacher-forced/free-running trajectories. The checker receives the source program specification, graph and weights; it does not call the emitter. |
| U3 — adversarial controls | DONE | `evaluation-3e7b1d4.json` binds the focused recurrent test output: malformed values/shapes, corrupted snapshot weights, a seeded incorrect transition, wrong program configuration and decoder, insufficient fuel, cancellation, and boundary resume all refuse or are detected. |
| U4 — bounded unrolling and costs | DONE | `evaluation-3e7b1d4.json` contains five repetitions per path and summaries for both programs. Each unrolled graph was checked against 1-, 2-, and 4-step host-loop and source transitions. Reference evaluator, conventional native reference, and compiled lanes are separate; dense, sparse, and stock inference-runtime coverage is explicitly none. |
| U5 — stop/continue decision | DONE — GO for continued bounded CPU prototype work only | Every clause of the frozen rule passed. The GO is not a portability or external-runtime claim. |

## Frozen criteria outcome

1. Both emitters repeat to byte-identical canonical graph bytes and identical snapshot identities/weight bytes; the same checked artifact handles distinct inputs.
2. The emitter-independent checker agrees with source transitions over both tiny domains and all accumulator boundary vectors.
3. All enumerated corruption/refusal controls are caught; the seeded incorrect transition is rejected.
4. Both programs complete 1,000-step free-running trajectories without resets, with source agreement at every transition; teacher-forced runs are reported separately.
5. Both unrolled artifacts match host-loop execution and source transitions at 1, 2, and 4 steps.
6. Five-repetition debug-profile median unrolled/host latency ratios: accumulator **0.70157×**, stack-swap **0.38467×** (≤1.25×). Weight bytes are 8/8 for the accumulator and 0/0 for routing (within 4×); process-RSS growth was **0 bytes** in measured lanes (≤16 MiB).
7. The 40 retained rows report end-to-end latency, compile/load, graph and weight bytes, mutable state bytes, RSS samples/peak, host operations, evaluator calls/microsteps, and skipped work (zero). Reference graph and native-reference paths are distinct and retained.

## Scope and reopen conditions

This evidence measures Noble's test-only compiled and reference CPU evaluators in the pinned Cargo test profile. Dense, sparse, exported, and stock inference-runtime coverage is **none**; no Lean theorem or external-runtime correspondence is claimed. The GO supports only continued work within the bounded pure CPU target. Any new target format, runtime, numeric representation, or host operation needs fresh independent correctness and clean-source measurements; any source or toolchain revision requires a new clean-commit receipt. Frozen thresholds may not be relaxed retroactively.

## Batch-3 remaining-work decision (2026-11-14)

The requested comparison against an exported inference runtime is **deferred, not substituted**. The admitted artifact is a Noble-owned `ngraph-f32/0` graph with a `Param` snapshot. Its one-step execution is stateless: the host supplies and receives the 64-bit word state. The retained receipt identifies only Noble's reference and compiled CPU evaluators and records exported/stock runtime coverage as none. The checked Cargo and flake manifests contain no pinned inference-runtime dependency or adapter. No exported runtime artifact, version/hash pair, offline runtime closure, or mapping from this graph and state codec to an external runtime is part of this experiment. Selecting an arbitrary system package, fetching a model/runtime, or treating the Noble evaluator as an exported runtime would not satisfy the issue's pinning and correspondence requirements, so none was attempted.

| Remaining item | Disposition | Evidence, exact blocker, and reopen condition |
|---|---|---|
| 1. Codec and one-step simulation against an actual exported inference runtime | DECIDED — NOT DONE | `design.md` specifies only the exact-bit `ngraph-f32/0` host-loop codec; `receipts/2026-11-13/evaluation-3e7b1d4.json` names the Noble CPU evaluators and no exported runtime. The manifests provide no pinned runtime adapter, and this lane has no exported artifact plus offline runtime closure to execute. Reopen only when an exported artifact and its runtime/adapter are available offline with immutable version and hash identities, and the supported graph/state ABI is declared before the comparison. |
| 2. Runtime exhaustive/free-running trajectories and corrupted-state/configuration refusals | DECIDED — PARTIAL | The retained host-loop tests and receipt cover the tiny domains, 1,000-step teacher-forced and free-running trajectories, and listed corruption/configuration controls for the Noble CPU evaluator only. They are not external-runtime evidence. Reopen with item 1's pinned offline runtime and a runtime-facing harness; repeat each control and trajectory there without replacing the existing CPU results. |
| 3. Legal snapshot indices and incomplete hybrid-state restores | DECIDED — NOT IMPLEMENTED FOR A HYBRID RUNTIME | For the existing host-loop contract, an index means the number of fully completed semantic transitions: legal positions are `0..=1000`, including the initial state at 0 and the request boundary at 1000; no position inside a one-step graph is legal. The current experiment has no serialized checkpoint API and no attention/KV or runtime scratch state, so it cannot exercise a hybrid restore. Any future hybrid restore MUST refuse unless its complete, runtime-required state is present, including recurrent state even when KV/cache or scratch data exists; it must also bind the exact boundary, program/configuration, weights and remaining budget. Reopen after a real runtime and its state ABI are pinned, then implement and test the completeness refusal against that runtime. |
| 4. Preserve the bounded host-loop GO; freeze criteria before expanding | DONE — GO UNCHANGED | The original GO remains limited to bounded CPU prototype work. `criteria.md` remains the pre-measurement rule; the absence of runtime evidence is not a pass and does not authorize runtime or architecture expansion. Reopen any expanded target only with new continuation criteria committed before its measurements. |

This decision closes no external-runtime acceptance claim. It preserves the prior bounded-CPU result while making the remaining runtime and hybrid-state obligations explicit; no mock runtime or synthetic hybrid checkpoint was added.

## Stock-runtime comparison disposition (2026-11-14, after batch 5)

The earlier batch-3/batch-5 rows above are historical decisions at their stated commits. This later unit **completes the bounded exported-runtime comparison** without changing `criteria.md` or the CPU host-loop GO. Its separate `stock-runtime-criteria.txt` was committed at `bc5c5638` before any ONNX model execution; the final clean-source receipt `receipts/2026-11-14/stock-runtime-701c64e.json` binds lint-clean implementation commit `701c64ed`. The earlier `stock-runtime-60c449b.json` remains an immutable preceding run; both runs bind the same model, graph and snapshot digests. The stock environment is the selected flake nixpkgs revision `9cf7092b`, ONNX Runtime **1.23.2** through the reviewed Python 3.13/ONNX 1.20.1 closure, CPUExecutionProvider, optimization disabled, one thread, offline at execution. The final receipt retains exact derivation/output paths, NAR hashes and ONNX/Noble/snapshot SHA-256 identities.

| Compared path | Actual coverage | Result |
|---|---|---|
| Noble transparent graph evaluator | Original test-only selected-semantics and compiled/reference comparisons from `evaluation-3e7b1d4.json`; new checked exporter re-evaluates and binds each stock vector through both Noble paths and source transition. | The CPU evidence remains distinct; not an inference engine. |
| Dense optimized inference | **None** | No dense GEMM/optimized implementation executed; no speed or long-run claim. |
| Sparse optimized inference | **None** | No sparse kernel implementation executed; no speed or long-run claim. |
| Exported stock ONNX inference | Two admitted one-step graphs; accumulator 16 tiny-domain pairs + 6 arithmetic boundaries + 1,000 teacher-forced + 1,000 free-running; stack-swap 16 pairs + 1,000 teacher-forced + 1,000 free-running. **4,038 stock transitions; 0 mismatches** with the independently computed source and Noble observations. | DONE for ONNX IR 10/opset 18 on pinned ORT CPU only. The 1,000 free-running steps feed each stock-decoded state back without resetting; teacher-forced is separately retained. |

The exporter generated 46,407-byte accumulator and 12,374-byte stack-swap ONNX models; both have deterministic exported bytes and model hashes in the receipt. The stock codec rejects invalid cell values/shapes and wrong program/codec/model identity. **50/50 controls refused** across the two programs, including corrupted model bytes, NaN/negative-zero/fractional state, wrong configuration, budget exhaustion, boundary cancellation, illegal snapshot indices, incomplete recurrent-state restore even with KV/scratch-like extras and unsupported hybrid material. Index 2 is restored and executed; 0 and 1000 are admitted as completed boundaries, while the request-end budget forbids another step. This is host-loop completeness testing, **not** a working hybrid KV/recurrent checkpoint implementation.

**Decision:** the stock-runtime comparison acceptance item is DONE within this exact bit-cell, one-step host-loop ABI. The existing bounded CPU continuation GO is unchanged. Emitter/decoder and runtime correspondence are finite tests, not Lean proofs, and ONNX guard omission relies on the declared exact-bit closure; actual hybrid-state serving, dense/sparse optimized runs and other provider/dtype/architecture portability remain unclaimed. Any such expansion needs a new precommitted criterion and a new clean-source receipt.
