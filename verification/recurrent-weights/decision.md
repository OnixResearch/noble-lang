# Recurrent-weight decision — 2026-11-13

## Decision: NO-GO for continuation beyond the bounded CPU prototype

The continuation criteria in `criteria.md` were committed at `0962b34` before any measurement. The prototype source was committed at `5adf7db`; the retained test receipt at `receipts/2026-11-13/test-run.json` binds that clean source commit. The only executed result is a focused correctness test: the compiled evaluator agrees with `eval::reference`, and both agree with wrapping `u64` addition for 16 tiny-domain pairs and six boundary pairs. This finite test is evidence for one test graph only; it is not a universal proof or evidence for a serving runtime.

The frozen GO rule is not met. There is only an accumulator graph; no stack-routing program, emitter-independent checker API, tiny exhaustive check across both programs, long free-running trajectory, bounded unrolled artifact, adversarial control matrix, latency/RSS measurements, or exported runtime. In particular, no performance measurements were taken. Per the precommitted rule, unmet clauses mean NO-GO; there is no post-measurement threshold adjustment. No stock runtime is present, so stock-runtime coverage is none.

## Unit disposition

| Brief unit | Status | Evidence / reason |
|---|---|---|
| U1 — frozen criteria and design note | DONE | `criteria.md`, `design.md`, commit `0962b34`; committed before the first test run. |
| U2 — deterministic host-loop baseline and checker lane | NOT DONE (partial prototype) | `crates/noble-ngraph/tests/models/recurrent.rs`, commit `23f0493` and follow-up `5adf7db`; receipt records one compiled/reference/source comparison. It has only the accumulator, no stack-routing graph, no reusable independent checker interface, no trajectory or teacher-forced/free-running separation, and the required fit of complete manifest identity is absent. |
| U3 — adversarial controls | NOT DONE | No retained controls for malformed input/state, corrupted weights, wrong decoder/configuration, insufficient budgets, cancellation/resume, or seeded incorrect transitions. |
| U4 — unrolled and runtime/cost comparison | NOT DONE | No bounded unrolled graph, no timing/RSS run, and no actual exported inference runtime. The existing test receipt makes no performance claim. |
| U5 — stop/continue decision | DONE (NO-GO) | This decision follows the frozen rule; source and receipt identities are above. |

## Reopen conditions

Reopen only with a scoped implementation that adds the second routing program and a genuinely emitter-independent checker, then independently completes every correctness and adversarial clause in `criteria.md`. Before any timing, commit that complete correctness-capable implementation and preserve the current criteria unchanged. If continuation is reconsidered, first add a separately identified bounded-unrolled artifact and run the frozen five-repetition cost protocol. Any exported runtime requires its own identity-bound execution evidence; CPU evaluator results do not transfer. Do not interpret this prototype as persistent model state, an external-runtime result, training evidence, a Lean theorem, or general compiler correctness.
