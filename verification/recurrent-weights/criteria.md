# Recurrent-weight continuation criteria (frozen before measurement)

Status: frozen for the host-loop baseline and bounded unrolled prototype. This file is immutable once any timed or resource measurement starts; later corrections belong in the outcome report and cannot relax these rules.

## Target and programs

The target is a Noble-owned deterministic threshold network represented only by `ngraph-f32/0` graphs and a `Param` snapshot. No training or fitting is involved. Each logical Boolean is represented by the exact binary32 bit value `+0` or `1`; all gates must preserve that exact domain. There are no I64-to-F32 casts. The two required programs are:

1. **Wrapping accumulator:** state is one unsigned 64-bit word encoded as 64 exact bit cells; input is one unsigned 64-bit word encoded the same way; transition is modulo-$2^{64}$ addition, output is the new state. The bit-level carry network is the only transition computation. Boundary inputs include zero, one, `2^63`, `2^64-1`, carry-chain patterns, and wraparound.
2. **Stack routing:** state is two 64-bit words and transition swaps their positions; output observes the post-swap state. Its inputs include distinct and equal words. This is a nontrivial routing program with no arithmetic cast.

The state codec is little-endian by bit index: cell `i` is bit `i`, `i=0..63`. Encoding accepts only exactly 64 finite binary32 `+0`/`1` cells per word; reject NaN, infinities, `-0`, fractional values, wrong lengths, wrong program/configuration and out-of-domain values. Decoding accepts the same exact values and reconstructs the corresponding unsigned word. The zero state and zero input initialize the host-loop lane. Weights are immutable Param snapshot bytes, separate from mutable state cells.

The bounded unrolled comparison is a statically composed graph for at most 4 transitions. It is not recurrent model state and MUST NOT be described as such. Every boundary between steps has an explicit encoded state. The host-loop lane invokes the same one-step graph repeatedly, passing outputs as subsequent inputs; host work is limited to state/input encoding, evaluator invocation, state handoff, output decoding, budget/cancellation checks and reporting. The host does not compute addition, carry, or swapping.

## Domains, bounds and host contract

Exhaustive semantic checks use accumulator state/input in `{0,1,2,3}` (16 pairs), plus required arithmetic boundary vectors as separately reported checks. Stack-routing exhausts both words over `{0,1,2,3}` (16 pairs). Trajectories are capped at 1,000 semantic steps per run, with separate 4-step bounded-unrolled comparisons. Teacher-forced runs supply the reference next state at each step and are diagnostic only; free-running runs feed only the target's own decoded next state forward, never reset between steps, and are the continuation gate. Each run has a hard graph-evaluation budget of 1,000 one-step evaluations or 4 unrolled chunks, whichever applies, and a cancellation check at every semantic transition boundary. Resume is permitted only from a complete state at an exact transition boundary with the same program, immutable weight identity, codec/configuration and remaining budget; partial states and mismatches refuse.

All host-visible operations and injected values are the ones listed above. No clock, RNG, file/network I/O, training, hidden state, attention/KV, GPU kernel, live publication, external runtime, or user-controlled operation is in scope. Runtime export/stock runtime coverage is **none** unless an actual runtime is added and independently exercised.

## Frozen GO/NO-GO rule

Continue implementation beyond the bounded prototype only if **all** clauses pass:

1. Emitter deterministically produces both programs; identical source/configuration yields byte-identical graph and snapshot; distinct inputs execute without rebuilding or mutating weights.
2. An emitter-independent checker (accepting only transition specification, codec and candidate weights/graph) exhaustively verifies both tiny domains and each required boundary vector against the selected reference semantics; it does not call or inspect emitter metadata.
3. All seeded incorrect transitions and corruption/configuration/state/budget/cancellation/resume controls refuse or produce an independently reported counterexample; there are no silent passes.
4. At least 1,000-step free-running trajectories complete without resets, and every decoded state/output equals the source transition at every step. Teacher-forced agreement is reported separately and cannot substitute for this clause.
5. For each program, the unrolled artifact agrees with 1-, 2- and 4-step host-loop execution and the independent source reference. All comparisons use identical inputs and weight bytes.
6. On the same host, build profile, source revision and inputs, warm median latency per semantic transition for the unrolled graph is no more than 1.25x the host-loop baseline at 4 transitions; the unrolled artifact's exact serialized weight bytes are no more than 4x the host-loop graph; peak RSS increase over baseline is at most 16 MiB. Five measured repetitions are required; report every sample and median. RSS is process-level and is not an allocation count.
7. Every measured path reports compile/build and load time, graph/weight bytes, mutable state bytes, peak RSS, latency per transition, complete end-to-end latency, reference graph cost, conventional native-reference cost, host operation counts, evaluator calls/microsteps, and skipped work. No skipped work is allowed in correctness-gated rows.

Any unmet clause is **NO-GO for further target expansion**. A failed or unavailable measurement is NOT a pass. No threshold may be reinterpreted after measurement. A slower prototype may still be retained as a semantic experiment, but does not change the no-go. Stock-runtime absence is a reported limitation, not evidence of portability.

## Measurement integrity

Before measurements, commit this file and the design note. Use a clean source commit and retain the exact commit, build profile, graph and snapshot digests, codec, host/runtime identity, inputs, repetition rows and raw output. Run producers with output outside the repository; commit the source first and the resulting receipt separately. Tests establish finite implementation behavior, not a Lean theorem or proof of arbitrary execution. Emitter correctness, checker correctness, decoder correctness and correspondence to any external inference runtime remain distinct obligations.
