# Issue #118: learned-procedure library decision

**Decision: accept the executed store/retrieval/composition mechanics, the
bounded exact-pair alias fragment and the one-shot GO for #116-generated
held-out tasks under mechanically derived references. Do not close the full
Cairn change or claim incremental learning, general transfer, a
teacher-attempt refusal in the original frozen run, a speedup or universal
implementation/model correspondence.** The remaining proof obligation is
explicit below. The original frozen criterion is
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
`receipts/producer-transfer-v2-disposition.json` binds the observation.
Neither old criterion nor outcome was edited or rerun.

## Valid #116-produced target trial: separately frozen v3 GO

Before generating either target, `derive-producer-targets.mjs` symbolically
executed the **retained #116 source programs** in actual top-of-stack order
to produce `criteria-producer-targets-v3.json` (SHA-256
`15f3e324943566c6877001f2d1a4be0dc0ff9880edc4ffad046adcc2abfd68f1`)
and `producer-targets-v3.txt` (SHA-256
`d0d4a604796d1fbdce0c5e0597a0296c735d9ef3f252be6cf33cf08086b8f8e3`).
An independent non-held-out Rust stack interpreter checks both references
against the source programs; the committed derivation lane's `--check`
reproduced both files byte for byte. V1's witness-correct mapping is retained
unchanged, while v2's swapped mappings remain a recorded defective criterion.

Exactly one clean-source #116 producer invocation from `6905332` generated
both v3 target candidates; `receipts/producer-targets-v3.json` retains their
grammar, budget, search and counterexample trace, two external solver answers
per target, producer-heldout agreement and normal CLI re-admission. The
programs were `+ -` and `- 7 7 + + +`. External UNSAT answers were marked
`inconclusive` without independently accepted proofs; the receipt's `proved`
task status is **not** an independent proof. Neither generated target program
was admitted to a scored library: the source procedures came only from the
previously frozen #116 acquisition receipt. No target answer, candidate body
or evaluator state entered the retrieval index.

An initial transfer invocation found a **dirty** alias test file and refused
at the runner's clean-source precondition, before opening a raw log, writing a
receipt or scoring an arm. After that formatting change was committed as
`c0a1072`, exactly **one** clean-source v3 transfer run completed:
`receipts/producer-transfer-v3.json` and its byte-identical
`producer-transfer-v3.raw.log`, raw SHA-256
`fdda4082a1fb1940c9c4a1d66028a3f5419320e68182085535367b2c58297177`.
The runner exited 0 with GO against the unchanged v3 threshold. Its
16 scored rows gave learned **8/8** task cells and **24/24** examples,
frozen-hand **8/8** and **24/24**, no-library **0/8**, and size-matched
non-transfer **0/8**; learned tied, not beat, the frozen-hand arm.

| Arm, seeds 1/7/23/101 | Passed task cells | Final entries / canonical bytes (growth over 131) | Generations | Queries / examined per seed | Retrieval-only monotonic ns by seed |
|---|---:|---:|---:|---:|---|
| No library | 0/8 | 0 / 131 (+0) | 1 | 4 / 0 | 2504, 1062, 1183, 1182 |
| Frozen hand | 8/8 | 2 / 892 (+761) | 2 | 4 / 8 | 8125, 5410, 5322, 5069 |
| Accumulated learned | 8/8 | 2 / 1815 (+1684) | 3 | 12 / 12 | 8777, 10689, 8614, 8978 |
| Size-matched non-transfer | 0/8 | 2 / 1944 (+1813) | 3 | 12 / 12 | 11290, 8835, 9077, 12343 |

Every scored row recorded zero teacher/proposer **attempts and calls**,
matching restart identities and zero earlier-task regressions. Four separate
fresh-library negative controls each attempted one proposer and one teacher
request: all eight refused `Unsupported` before callback invocation, retained
zero records and unchanged identity manifests. All break-even invocation
counts are null with the recorded reason that per-invocation execution-cost
differences were not measured. This is bounded held-out transfer under the
frozen criterion, **not** lifelong learning, a speedup, a proof of arbitrary
composition, or an improvement over a frozen hand-written library.

## Finite byte-encoder correspondence, not universal refinement

The kernel-checked `NobleFoundation/ProcedureLibraryWire.lean` models a
restricted source-record encoding with length-prefixed UTF-8, big-endian
integers, fixed chunk IDs and revisions, typed interfaces, body node tags,
three provenance branches and bounded ordered library-dependency edges. Its
three inventoried theorems check two source byte lengths and an encoder
prefix. The earlier clean-source Lean emitter produced
`receipts/encoder-corpus-v1.json` (SHA-256
`3936d12dc1ee017ce6b0d10ec7ec22cee60e29868ffb9814e5b15cb772e1dd4e`);
its verifier binds the model/emitter and Rust encoder/fixture digests.
That earlier differential compared two Rust records against their full
Lean-generated immutable-subject bytes; the test has since been replaced by
the expanded matrix. The separately retained
`receipts/encoder-corpus-v2.json` was generated from clean commit `6905332`;
its source-digest verifier and
`transfer::encoder_corpus::finite_subject_matrix_matches_lean_generated`
check **36 full-byte cases**, spanning four producer-derived I64 source
bodies, three provenance tags and zero/one/two ordered library edges. The
Rust tests also check dependency-order sensitivity and over-limit refusal.
The explicit **universal obligation remains open**: no Rust/Lean refinement
for arbitrary chunks, captures, dependency closures, hash injectivity or
proof-demanding promotion follows from this finite differential matrix.
`NobleFoundation/ProcedureLibraryAlias.lean` independently proves equality
of wrapping-I64 outputs from equal formal polynomial normal forms; it does
not prove the Rust bounded sparse normalizer implements that model.

## Acceptance and remaining obligations

| Item | Status | Evidence / release condition |
|---|---|---|
| Content-addressed `procedure-record/1`, body/dependency identity, versioned promotion/cost, deterministic encoding and the shared #73 chunk view | DONE | `crates/noble-contracts/src/source/procedures.rs`, `procedures/persistence.rs`; `tests/editor/procedures/identity.rs`, `admission.rs`. Identities are experimental and scoped to one encoding/toolchain, not language-wide canonical hashes. |
| Reuse, two-record composition, ordinary source/kernel re-admission, invalid contract/effect/capability/policy refusals | DONE | `tests/editor/procedures/composition.rs`, `admission.rs`; `procedures/composition.rs`. Resulting authority is checked under the caller's budgets. |
| Exact-record deduplication and refusal of digest-only alias claims | DONE | `tests/editor/procedures/identity.rs` and `admission::an_alias_digest_cannot_replace_a_verified_exact_pair_certificate`; no claim that a digest proves semantic equivalence. |
| Positive semantic alias of distinct records in the bounded pure-I64 fragment (LPL-04) | DONE | `procedures/alias.rs` recomputes bounded polynomial forms and empty closure digests from both retained records on admission and reload; `admission/alias/{mod,forgery}.rs` tests distribution, wraparound, success, replay and forged/unsupported refusals. The distinct `validation/passed` evidence does not claim a #56 proof, proof-demanding promotion or formal Rust/Lean refinement. |
| Two-process reload equality, deterministic dependency order, mutation/encoder-drift refusals, replacement and rollback | DONE | `tests/editor/procedures/identity.rs`, `reload-negative.rs`, `generations.rs`. Two child processes compare the exact identity manifest against the parent. |
| Frozen held-out composed-task evaluation and per-seed library growth/retrieval overhead | DONE for the **repaired fixture** | Criterion SHA-256 above; both attempts retained separately; one repaired repeat meets 8/8 and 24/24. Per-arm rows, bytes, query counts, restart, retention, regression and null break-even reasons are in `transfer-repeat.json`. This is not an incremental benefit over the frozen-hand library. |
| #116-generated held-out **transfer measurement**, with grammar/budget/trace/verifier status and no target leakage | DONE for frozen v3 | `criteria-producer-targets-v3.json`/`producer-targets-v3.txt` were mechanically derived and committed before target generation. One clean-source #116 producer receipt retains candidates, budgets, trace, external solver answers, held-out producer examples and normal CLI re-admission. One clean-source four-arm receipt `producer-transfer-v3.json` plus raw log gives GO, learned 8/8 and 24/24, hand 8/8, other controls 0/8, all restart/regression checks passed. Generated target bodies are oracle-only; acquired source procedures alone enter the library. Neither v1 nor defective v2 was rerun. |
| No runtime teacher/proposer fallback, including an attempted-call refusal | DONE for v3; original frozen clause NOT DONE | V3's 16 scored rows have zero attempts and calls; its four isolated negative controls each deliberately attempted both producer types and refused before any callbacks or library mutation. The original frozen repeat missed the attempted-call clause and is not retroactively repaired. |
| Proof obligations O1–O6 and proof-demanding promotion | PARTIAL | The abstract ProcedureLibrary model proves typed identity, reload, append/dedup and rollback; Rust tests exercise O5/O6. ProcedureLibraryAlias proves the formal wrapping-I64 polynomial equivalence claim. ProcedureLibraryWire and `receipts/encoder-corpus-v2.json` compare a 36-case source/provenance/closure subprofile against actual Rust subject bytes. No **universal** Rust encoder refinement, digest injectivity or Rust normalizer refinement theorem exists; do not promote under a proof-demanding policy on those finite/model-only witnesses. |
| Cairn validation and archive | PARTIAL | The bounded alias and valid #116-produced v3 measurement are done; `.cairn/changes/learned-procedure-library/tasks.md` retains the unchecked universal byte-encoder/proof-demanding-promotion obligation. The original frozen run's missed teacher-attempt clause is historical, not retroactively discharged; v3 independently exercises the boundary. Keep this change active until the universal obligation is proved and independently checked. |

Model proof, implementation tests, producer provenance, observed results and
correspondence assumptions are separate evidence classes. The original
fixture chose its composition order and references; v3 derived
its order and expected references from retained source-program semantics
before target generation. Do not infer lifelong learning, generalization
across task families, speedup, equivalence of same-signature bodies outside
the bounded verifier fragment, or a checked universal Rust/Lean refinement
from these numbers.
