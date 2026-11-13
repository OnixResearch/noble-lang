# Recurrent-weight decision — 2026-11-13

## Decision: NO-GO for continuation beyond the bounded CPU prototype

The continuation criteria in `criteria.md` were committed at `0962b34` before any measurement. The prototype source was committed at `5adf7db`; the final focused-test receipt at `receipts/2026-11-13/postfix-test-run.json` binds clean source commit `0dffd3fc47fa5ec5398a56553f0b00cb80a6d9cf`. The compiled evaluator agrees with `eval::reference`, and both agree with wrapping `u64` addition for 16 tiny-domain pairs and six boundary pairs. This finite test is evidence for one test graph only; it is not a universal proof or evidence for a serving runtime.

The frozen GO rule is not met. There is only an accumulator graph; no stack-routing program, emitter-independent checker API, tiny exhaustive check across both programs, long free-running trajectory, bounded unrolled artifact, adversarial control matrix, recurrent-target latency/RSS measurements, or exported runtime. No recurrent-target performance measurements were taken. Per the precommitted rule, unmet clauses mean NO-GO; there is no post-measurement threshold adjustment. No stock runtime is present, so stock-runtime coverage is none.

## Unit disposition

| Brief unit | Status | Evidence / reason |
|---|---|---|
| U1 — frozen criteria and design note | DONE | `criteria.md`, `design.md`, commit `0962b34`; committed before the first test run. |
| U2 — deterministic host-loop baseline and checker lane | NOT DONE (partial one-step prototype) | `crates/noble-ngraph/tests/models/recurrent.rs`, commits `23f0493`, `5adf7db`, `de93a74`, `0dffd3f`; receipt records one compiled/reference/source comparison. It has only the accumulator, no repeated host-loop trajectory, no stack-routing graph, no reusable independent checker interface, no teacher-forced/free-running separation, and no complete artifact manifest. |
| U3 — adversarial controls | NOT DONE | No retained controls for malformed input/state, corrupted weights, wrong decoder/configuration, insufficient budgets, cancellation/resume, or seeded incorrect transitions. |
| U4 — unrolled and runtime/cost comparison | NOT DONE | No bounded unrolled graph, no timing/RSS run, and no actual exported inference runtime. The existing test receipt makes no performance claim. |
| U5 — stop/continue decision | DONE (NO-GO) | This decision follows the frozen rule; source and receipt identities are above. |

## Verification record

- `cargo fmt --all -- --check`: passed.
- `cargo test -p noble-ngraph --all-targets --all-features --offline --locked`: passed; 157 passed, 0 failed, 9 ignored.
- Ignored batch measurement lane, `cargo test -p noble-ngraph --test batch_row_reference --offline --locked -- --ignored --nocapture`: passed; 2 passed. These are existing graph-batching observations, unrelated to the recurrent target and excluded from its decision.
- Workspace Clippy, `cargo clippy --workspace --all-targets --all-features --offline --locked -- -D warnings`: passed.
- `octet.sh`: final run passed, exit 0; summary reported `Status: clean`, `Findings: 0`, `Lint phase: clean (static_lint)`, and `Architecture phase: clean (policy-check)`.
- `bun tools/cairn-specs.mjs --self-test`: passed, 26 self-tests.
- `bun test tools/cairn-specs.test.mjs tools/check-specs.test.mjs tools/calculator-headroom.test.mjs tools/source-inventory-renew.test.mjs`: passed, 114 tests.
- `bun tools/check-specs.mjs --self-test`: passed, 230 self-tests.

## Reopen conditions

Reopen only with a scoped implementation that adds the second routing program and a genuinely emitter-independent checker, then independently completes every correctness and adversarial clause in `criteria.md`. Before any timing, commit that complete correctness-capable implementation and preserve the current criteria unchanged. If continuation is reconsidered, first add a separately identified bounded-unrolled artifact and run the frozen five-repetition cost protocol. Any exported runtime requires its own identity-bound execution evidence; CPU evaluator results do not transfer. Do not interpret this prototype as persistent model state, an external-runtime result, training evidence, a Lean theorem, or general compiler correctness.
