# Issue #118: learned-procedure library decision

**Decision: accept the executed store/retrieval/composition mechanics and the narrowly
measured frozen-fixture result; do not close the full Cairn change or claim
incremental learning, general transfer, a teacher-attempt refusal in the
original frozen run, semantic aliasing, or universal implementation/model correspondence.** The remaining acceptance
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
of the frozen criterion is not established. A later, **non-held-out** library
test requests both a proposer and teacher through
`ProcedureLibrary::request_runtime_producer`, supplying callbacks that count
actual invocations. Both requests return `Unsupported` before either callback
runs; the query budget records one attempt of each kind, zero retrievals and
the callbacks record zero calls. The library identity manifest is unchanged.
This independently checks the disabled boundary, not the earlier frozen
measurement or its one-shot teacher-attempt clause; no held-out task was rerun.

## Separate #116-produced target trial: defective criterion, not library failure

The first proposed producer criterion (`criteria-producer-targets-v1.json`) and
manifest were committed before any target generation and never run. Its
pre-run static-source erratum asserted that the two source-order/reference
pairs were wrong. The executed composition witness later **disproved that
assertion**: the first source consumes the top two inputs `(y,z)`, not the
bottom two `(x,y)`. For `[4,9,2]`, Commuted→Subtract yields `-7 = x-y-z`,
and Subtract→Commuted yields `25 = x+y-z+14`, exactly v1's references. The
correction is appended to `criteria-producer-targets-v1.erratum.json`; v1's
criteria and manifest remain byte-identical, and no v1 outcome is asserted.

The separately committed v2 criterion SHA-256
`884c2e51e3101107dbf6eca241339c9770970fb54a6349e8d9be7280cd834c81`
and manifest froze the **incorrectly swapped** reference pairing before
generation. A clean-source producer invocation failed after target generation,
before writing its receipt, because its pinned Bitwuzla store path had been
collected; `criteria-producer-targets-v2.run-erratum.json` records this
non-measurement. Exact pinned Bitwuzla, Z3 and GNU time store outputs were
restored and verified against the prior receipt's executable SHA-256 digests.
The authorized producer repeat from clean source `e323919` emitted
`receipts/producer-targets-v2.json`: both three-input candidates passed
producer-heldout examples and normal CLI re-admission, with solver answers
and round traces retained. Solver UNSAT does not independently certify them.

The first transfer invocation from clean source `b6b358c` failed before a
scored arm because the harness still restated a stale reference formula.
Its raw output and null-report receipt remain unchanged in
`receipts/producer-transfer-v2-failed.{raw.log,json}`, with an adjacent
erratum. The harness was then bound to references checked against both the
criterion and manifest, with a regression for every fixture target; the v2
criterion and manifest were not changed. The **final** authorized run from
clean source `b2c0fcf` is
`receipts/producer-transfer-v2-final.{raw.log,json}`. It completed all
16 rows, but its learned arm passed **0/8** task cells and **0/24**
examples. All scored arms recorded zero proposer/teacher attempts and
callbacks, matching restart identities, zero earlier-task regressions and a
size-matched control. The same one-shot execution attempted one proposer and
one teacher request on a separate fresh library for each of four seeds:
all eight attempts refused `Unsupported` before callback invocation, left
the library identity unchanged and retained zero records.

The v2 result is **NO-GO under its unchanged threshold, but is a
defective-criterion outcome, NOT evidence against transfer by the library**:
the accepted compositions agree with v1's reference pairing, not v2's.
`receipts/producer-transfer-v2-disposition.json` binds the observation. The
#116-generated held-out **transfer measurement remains NOT DONE**. Reopen
only after committing a new frozen criterion whose target references are
derived mechanically from the #116 producer's declared source semantics
and actual top-of-stack composition order, never manually restated, followed
by exactly one clean-source run. Do not rerun v2.

## Finite byte-encoder correspondence, not universal refinement

The kernel-checked `NobleFoundation/ProcedureLibraryWire.lean` models exactly
two restricted source records, including length-prefixed UTF-8, big-endian
integers, fixed chunk IDs and revisions, typed interfaces, body node tags,
provenance options and zero library dependencies. Its three inventoried
theorems check the two resulting byte lengths and an encoder prefix. The
separate clean-source Lean emitter produced
`receipts/encoder-corpus-v1.json` (SHA-256
`3936d12dc1ee017ce6b0d10ec7ec22cee60e29868ffb9814e5b15cb772e1dd4e`);
its verifier binds the model/emitter and Rust encoder/fixture digests.
`transfer::encoder_corpus::finite_subject_bytes_match_lean_generated`
reconstructs both Rust records, checks their chunk IDs and source revisions,
then compares the full immutable-subject bytes. Both cases passed. This is a
finite implementation/model **differential fixture**, not a universal
correspondence theorem for arbitrary chunks, dependencies, captures, digests,
semantic aliases or proof-demanding promotion. Those obligations remain open.

## Acceptance and remaining obligations

| Item | Status | Evidence / release condition |
|---|---|---|
| Content-addressed `procedure-record/1`, body/dependency identity, versioned promotion/cost, deterministic encoding and the shared #73 chunk view | DONE | `crates/noble-contracts/src/source/procedures.rs`, `procedures/persistence.rs`; `tests/editor/procedures/identity.rs`, `admission.rs`. Identities are experimental and scoped to one encoding/toolchain, not language-wide canonical hashes. |
| Reuse, two-record composition, ordinary source/kernel re-admission, invalid contract/effect/capability/policy refusals | DONE | `tests/editor/procedures/composition.rs`, `admission.rs`; `procedures/composition.rs`. Resulting authority is checked under the caller's budgets. |
| Exact-record deduplication and refusal of digest-only alias claims | DONE | `tests/editor/procedures/identity.rs` and `admission::an_alias_digest_cannot_replace_a_verified_exact_pair_certificate`; no claim that a digest proves semantic equivalence. |
| Positive semantic alias of distinct records with an exact #56 certificate (LPL-04) | **NOT DONE** | #56 currently supplies a verification lane, not a runtime exact-pair verifier of both actual bodies and both closures. Admission refuses alias requests. Reopen only with an exact-pair verifier callable without adding a contracts→ngraph dependency (or a higher-crate verifier trait), validating the pair and both closures before an alias edge or evidence is retained. |
| Two-process reload equality, deterministic dependency order, mutation/encoder-drift refusals, replacement and rollback | DONE | `tests/editor/procedures/identity.rs`, `reload-negative.rs`, `generations.rs`. Two child processes compare the exact identity manifest against the parent. |
| Frozen held-out composed-task evaluation and per-seed library growth/retrieval overhead | DONE for the **repaired fixture** | Criterion SHA-256 above; both attempts retained separately; one repaired repeat meets 8/8 and 24/24. Per-arm rows, bytes, query counts, restart, retention, regression and null break-even reasons are in `transfer-repeat.json`. This is not an incremental benefit over the frozen-hand library. |
| #116-generated held-out **transfer measurement**, with grammar/budget/trace/verifier status and no target leakage | **NOT DONE** | Acquisition sources remain bound to `verification/synthesis/receipts/2026-11-13/synthesis-v2.json`. V2 produced two three-input targets with retained trace, solver status and normal CLI re-admission; its final four-arm run covered every seed and isolated attempted-call refusal. But v2's source-order/reference pairing was defective and passed 0/8 for that reason; this is **not** a valid measurement of the library. V1's mapping was correct but never run. Reopen only under a new precommitted criterion deriving references mechanically from #116 source semantics and actual top-of-stack composition order, then exactly one clean-source run. |
| No runtime teacher/proposer fallback, including an attempted-call refusal | DONE for the v2 boundary; original frozen clause NOT DONE | The original frozen repeat records zero calls but no attempted call. The independent non-held-out refusal test remains. Separately, v2's final one-shot run records all 16 scored arms with zero attempts/calls and four fresh-library negative controls with one attempted call of each kind per seed, each refused before callbacks. No scored arm inherits those attempts. This establishes the attempted-call boundary in that execution, **not** successful transfer under v2's defective criterion or the original receipt's missed clause. |
| Proof obligations O1–O6 and proof-demanding promotion | PARTIAL | `proofs/foundation/NobleFoundation/ProcedureLibrary.lean` proves abstract typed-subject identity, reload, append/dedup and rollback; Rust tests exercise O5/O6. `NobleFoundation/ProcedureLibraryWire.lean` plus `receipts/encoder-corpus-v1.json` and the Rust differential test bind **two finite** subject encodings to kernel-checked model fixtures. No universal refinement from Rust bytes/hash to Lean or #56 runtime exact-pair proof exists. Do not promote under a proof-demanding policy on the strength of this finite corpus. |
| Cairn validation and archive | PARTIAL | Pinned `cairn validate` passes; unchecked positive exact-pair alias, its `LPL-T07` fixture, a **valid** #116-generated composed-target measurement and universal Rust/Lean correspondence remain in `.cairn/changes/learned-procedure-library/tasks.md`. The original frozen run's missed teacher-attempt clause is not retroactively discharged; v2 independently exercised that refusal boundary. Keep the change active; archive only after the open rows are honestly discharged. |

Model proof, implementation tests, producer provenance, the observed repaired
fixture, and correspondence assumptions are separate evidence classes. The
chosen composition order and expected arithmetic references were supplied by
the frozen fixture. Do not infer lifelong learning, generalization across task
families, speedup, equivalence of same-signature bodies, or correctness of an
unverified semantic alias from these numbers.
