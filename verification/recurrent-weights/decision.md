# Recurrent-weight evaluation — final disposition

## Frozen rule

The continuation criteria in `criteria.md` were committed before measurement in `0962b348` and have not been changed. The final receipt in `receipts/2026-11-13/evaluation-bfcd0a6.json` was produced from clean implementation commit `bfcd0a60bb8efe1a22010df8719ffc9481a4bbd5`.

## Unit disposition

| Brief unit | Status | Evidence |
|---|---|---|
| U1 — criteria and design | DONE | `criteria.md` and `design.md`, commit `0962b348`. The exact-bit accumulator and stack-routing target, codec, bounded host contract, and continuation thresholds were fixed before measurement. |
| U2 — host-loop baseline | DONE | `evaluation-bfcd0a6.json`: both deterministic programs, exhaustive tiny domains, arithmetic boundaries, and separate 1,000-step teacher-forced/free-running trajectories. The checker receives the source program specification, graph and weights; it does not call the emitter. |
| U3 — adversarial controls | DONE | `evaluation-bfcd0a6.json` binds the focused recurrent test output: malformed values/shapes, corrupted snapshot weights, a seeded incorrect transition, wrong program configuration and decoder, insufficient fuel, cancellation, and boundary resume all refuse or are detected. |
| U4 — bounded unrolling and costs | DONE | `evaluation-bfcd0a6.json` contains five repetitions per path and summaries for both programs. Each unrolled graph was checked against 1-, 2-, and 4-step host-loop and source transitions. Reference evaluator, conventional native reference, and compiled lanes are separate; dense, sparse, and stock inference-runtime coverage is explicitly none. |
| U5 — stop/continue decision | DONE — GO for continued bounded CPU prototype work only | Every clause of the frozen rule passed. The GO is not a portability or external-runtime claim. |

## Frozen criteria outcome

1. Both emitters repeat to byte-identical canonical graph bytes and identical snapshot identities/weight bytes; the same checked artifact handles distinct inputs.
2. The emitter-independent checker agrees with source transitions over both tiny domains and all accumulator boundary vectors.
3. All enumerated corruption/refusal controls are caught; the seeded incorrect transition is rejected.
4. Both programs complete 1,000-step free-running trajectories without resets, with source agreement at every transition; teacher-forced runs are reported separately.
5. Both unrolled artifacts match host-loop execution and source transitions at 1, 2, and 4 steps.
6. Five-repetition debug-profile median unrolled/host latency ratios: accumulator **0.61661×**, stack-swap **0.33452×** (≤1.25×). Weight bytes are 8/8 for the accumulator and 0/0 for routing (within 4×); process-RSS growth was **0 bytes** in measured lanes (≤16 MiB).
7. The 40 retained rows report end-to-end latency, compile/load, graph and weight bytes, mutable state bytes, RSS samples/peak, host operations, evaluator calls/microsteps, and skipped work (zero). Reference graph and native-reference paths are distinct and retained.

## Scope and reopen conditions

This evidence measures Noble's test-only compiled and reference CPU evaluators in the pinned Cargo test profile. Dense, sparse, exported, and stock inference-runtime coverage is **none**; no Lean theorem or external-runtime correspondence is claimed. The GO supports only continued work within the bounded pure CPU target. Any new target format, runtime, numeric representation, or host operation needs fresh independent correctness and clean-source measurements; any source or toolchain revision requires a new clean-commit receipt. Frozen thresholds may not be relaxed retroactively.
