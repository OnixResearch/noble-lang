# Issue #118: learned-procedure library decision

**Decision: accept the executed store/retrieval/composition mechanics and the narrowly
measured frozen-fixture result; do not close the full Cairn change or claim
incremental learning, general transfer, a teacher-attempt refusal, semantic
aliasing, or implementation/model correspondence.** The remaining acceptance
obligations are explicit below. The frozen criterion is
`criteria-2026-11-13.json` (SHA-256
`36b8efc1d7f99485f3e1764a596ce014ab24dcb62d6f029bf32456c196a4dc79`),
committed before either transfer attempt.

## Execution and decision evidence

The first run from clean source `e65f82c18d0763ffb0982527c348f359662c17cc`
exited 101. Its byte-identical raw command output, source-bound receipt and
erratum are in `receipts/2026-11-13/transfer-failed.raw.log`,
`transfer-failed.json` and `transfer-failed-erratum.json` (raw SHA-256
`533d9e494c32d598a4fbfb3a70b51693f66443d2dfd0395625e511f49c296f3f`).
Two defects invalidated the instrument: duplicate task separators made its JSON
unparseable, and querying a complete three-input→one-output target for each
individual two-input→one-output source excluded both source records before
composition. An in-memory separator repair was used only to diagnose the
instrument; neither transformed bytes nor the 0/8 learned result are a library
measurement. The original receipt remains unchanged.

The repaired harness has two non-held-out regressions: the two task rows parse
as JSON with one separator, and two admitted source candidates are found at
the actual three→two and two→one stack transitions. The repair was committed
before any repeat. Exactly one repeat from clean source
`dbbc03be112d7b6ea68c21e4bf9caf761fc28fd7` used the byte-identical
criterion and producer receipt (producer SHA-256
`4c05b46b3d439d7c5a95786d6a07e2c2017573d8a364fd6e8c52e2c70958387f`).
It passed its test (1 passed, 0 failed). Every arm/seed/sweep, identity manifest,
metrics record and fixture result is in `receipts/2026-11-13/transfer-repeat.json`;
the unmodified command output is `transfer-repeat.raw.log` (SHA-256
`f5c50e6bb5da33fd1ac76272ba709038fccff95c72fdbd53afd631e5539600db`).

| Arm, paired seeds 1/7/23/101 | Task cells | Examples | Final entries / canonical bytes (growth over 131-byte empty) | Generations | Retrieval queries / examined entries per seed | Retrieval-only monotonic ns by seed |
|---|---:|---:|---:|---:|---:|---|
| No library | 0/8 | 0/24 | 0 / 131 (0) | 1 | 4 / 0 | 9207, 1533, 1482, 1564 |
| Frozen hand library | 8/8 | 24/24 | 2 / 892 (+761) | 2 | 4 / 8 | 7785, 6603, 6360, 9007 |
| Accumulated producer library | **8/8** | **24/24** | 2 / 1815 (+1684) | 3 | 12 / 12 | 12033, 10048, 12532, 17475 |
| Size-matched non-transfer | 0/8 | 0/24 | 2 / 1944 (+1813) | 3 | 12 / 12 | 12341, 13086, 12312, 14227 |

The pre-outcome control pair is `arithProduct`, `arithScaled`: 1944 versus 1815
canonical bytes, a 129-byte (7.1%) difference under the frozen 10% limit.
Every reported restart manifest matched and every earlier-task regression
count is zero. The accumulated arm acquired two source tasks in successive
generations and retained three generations. All 16 arm/seed rows report
`proposer_calls = teacher_calls = 0`. The metric's break-even invocation field
is **null in every row**, with the explicit reason that per-invocation execution
cost differences were not measured. Retrieval-only nanoseconds are observations
from this fixture, not a speedup benchmark; the learned arm **ties** the
frozen-hand arm on task success. No runtime teacher-attempt control was
exercised (`teacher_attempts = 0` in all rows), so the attempted-call portion
of the frozen criterion is not established.

## Acceptance and remaining obligations

| Item | Status | Evidence / release condition |
|---|---|---|
| Content-addressed `procedure-record/1`, body/dependency identity, versioned promotion/cost, deterministic encoding and the shared #73 chunk view | DONE | `crates/noble-contracts/src/source/procedures.rs`, `procedures/persistence.rs`; `tests/editor/procedures/identity.rs`, `admission.rs`. Identities are experimental and scoped to one encoding/toolchain, not language-wide canonical hashes. |
| Reuse, two-record composition, ordinary source/kernel re-admission, invalid contract/effect/capability/policy refusals | DONE | `tests/editor/procedures/composition.rs`, `admission.rs`; `procedures/composition.rs`. Resulting authority is checked under the caller's budgets. |
| Exact-record deduplication and refusal of digest-only alias claims | DONE | `tests/editor/procedures/identity.rs` and `admission::an_alias_digest_cannot_replace_a_verified_exact_pair_certificate`; no claim that a digest proves semantic equivalence. |
| Positive semantic alias of distinct records with an exact #56 certificate (LPL-04) | **NOT DONE** | #56 currently supplies a verification lane, not a runtime exact-pair verifier of both actual bodies and both closures. Admission refuses alias requests. Reopen only with an exact-pair verifier callable without adding a contracts→ngraph dependency (or a higher-crate verifier trait), validating the pair and both closures before an alias edge or evidence is retained. |
| Two-process reload equality, deterministic dependency order, mutation/encoder-drift refusals, replacement and rollback | DONE | `tests/editor/procedures/identity.rs`, `reload-negative.rs`, `generations.rs`. Two child processes compare the exact identity manifest against the parent. |
| Frozen held-out composed-task evaluation and per-seed library growth/retrieval overhead | DONE for the **repaired fixture** | Criterion SHA-256 above; both attempts retained separately; one repaired repeat meets 8/8 and 24/24. Per-arm rows, bytes, query counts, restart, retention, regression and null break-even reasons are in `transfer-repeat.json`. This is not an incremental benefit over the frozen-hand library. |
| Held-out task generation by #116 with producer grammar/budget/trace/verifier status, and no target leakage | PARTIAL | The two retained acquisition sources bind to `verification/synthesis/receipts/2026-11-13/synthesis-v2.json`; the composed target references and input triples are fixture-authored, not a new #116 producer run. The harness checks no target id, composed target record or evaluator state enters the index. Reopen with a bound producer-generated target set and negative trace independent of this fixture. |
| No runtime teacher/proposer fallback, including an attempted-call refusal | PARTIAL | The fixture has no runtime proposer or teacher path and all 16 counts are zero; `tests/editor/procedures/metrics.rs` invalidates transfer metrics if either count is nonzero. There was **no attempted call** in the repeat, so the frozen negative-control clause is unproven. Reopen with a real attempt through an explicitly disabled adapter, an observed refusal and a claim-withdrawal check; do not fabricate calls or counters. |
| Proof obligations O1–O6 and proof-demanding promotion | PARTIAL | `proofs/foundation/NobleFoundation/ProcedureLibrary.lean` proves abstract typed-subject identity, reload, append/dedup and rollback; Rust tests exercise O5/O6. No checked refinement from Rust bytes/hash to Lean or #56 runtime exact-pair proof exists. Do not promote under a proof-demanding policy on the strength of the model alone. |
| Cairn validation and archive | PARTIAL | Pinned `cairn validate` passes; unchecked alias, producer-generation, teacher-attempt and refinement obligations remain in `.cairn/changes/learned-procedure-library/tasks.md`. Keep the change active; archive only after those rows are honestly discharged. |

Model proof, implementation tests, producer provenance, the observed repaired
fixture, and correspondence assumptions are separate evidence classes. The
chosen composition order and expected arithmetic references were supplied by
the frozen fixture. Do not infer lifelong learning, generalization across task
families, speedup, equivalence of same-signature bodies, or correctness of an
unverified semantic alias from these numbers.
