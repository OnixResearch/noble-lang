Hard dependency: #73 Phase 2 MUST first implement and independently admit the
exact `typed-chunk/1` envelope, immutable chunk library, `library_revision`,
one-hole stack-equation solver, deterministic shortlex queue, ordinary
source/kernel re-admission, shared budgets and four outcome classes. No task
below may substitute a new chunk store, a second body encoding or a
signature-only index. #116 supplies synthesized bodies; the local bounded
pure-I64 exact-pair verifier supplies the alias route exercised by this slice.
The separate #56 certificate lane is not an alias-admission dependency.

## Phase 1: Design (this change)

- [x] [serial] Ground the package in the verified substrate at `fed77835`: the #73 envelope, request, ordering, budgets and outcomes (`.cairn/changes/typed-chunk-completion/design.md:44-195`), P-ID-04 (`.cairn/specs/language/spec.md:2138-2141`), VC-ID-03 (`.cairn/specs/program-contracts/spec.md:469-481`), the MC2 core/rules/registry/refusals (`crates/noble-contracts/src/companion/`), the CLI companion surface (`crates/noble-cli/src/core/companions.rs:25-38`), the evidence classes and binding rules, and #73's Phase 1-complete/Phase 2-untouched status. r[LPL-01]
- [x] [serial] Fix the extension: one store, two views; `procedure-record/1` over the unmodified `typed-chunk/1` envelope; separate `chunk_library_revision` and `procedure_library_revision` derived from one generation; no second body encoding, snapshot, enumerator or admission route. r[LPL-01]
- [x] [serial] Fix the record field set, canonical field order, integer-only cost key, derived retrieval signature, per-obligation evidence binding, and the rule that identity is recomputed from retained fields and never supplied by a digest, id, signature, name or path. r[LPL-01]
- [x] [serial] Fix identity status: experimental, versioned, library-local; encoder id and encoding revision recorded in every record and export; no canonical encoding, hash algorithm or cross-implementation stability selected (P-ID-04, VC-ID-03). r[LPL-01] r[LPL-07]
- [x] [serial] Fix composition and capability admission onto the MC2 path: fresh candidate, ordinary `Session::prepare`, `acceptance::check`, `ComposeV1` with both premises, joint endpoint as the intermediate implication, authority-subset rule, and the escalation table naming exact refusals. r[LPL-02]
- [x] [serial] Fix promotion: `candidate`/`tested`/`promoted`/`superseded`, consumer-selected obligation policy, existing seven evidence classes reused, tested and proved permanently distinct, no promotion transfer across subject or policy, demotion recorded in a new generation. r[LPL-03]
- [x] [serial] Fix deduplication: exact-record identity reuse, derivation restatement refused as `duplicate-entry`, equal signatures kept distinct, semantic alias only from a checked exact-pair certificate retaining both bodies and closures. r[LPL-04]
- [x] [serial] Fix deterministic retrieval: total order over interface, effects, promotion rank, integer cost key and identity bytes; filters as ordered subsequences; bounded index and counted examination; candidates are identities, never authority. r[LPL-05]
- [x] [serial] Fix append-only generations, `supersedes` replacement, rollback exactness, rollback refusal on drift or unavailable toolchain, and the no-deletion rule for referenced generations. r[LPL-06]
- [x] [serial] Fix reload identity: persisted canonical form, recomputation order, `invalid` at the first differing field, prohibited nondeterministic inputs, canonical collection order, and the two-process manifest comparison with four negative controls. r[LPL-07]
- [x] [serial] Fix the two export records: deterministic `noble-procedure-library-export/1` identity manifest, separate `noble-procedure-library-metrics/1` bound to its digest, RSI Lab measurement fields, and fail-closed refusal of any runtime teacher fallback with claim separation. r[LPL-08]
- [x] [serial] Fix the acceptance 1–3 test plan naming every #73 and #116 interface consumed, its blocked status, and six planned proof obligations with separate model-proof/implementation-test/correspondence/assumption routes. r[LPL-01] r[LPL-02] r[LPL-03] r[LPL-04] r[LPL-05] r[LPL-06] r[LPL-07] r[LPL-08]
- [x] [serial] Validate the change documents with the pinned Cairn runner; touch no canonical specification, generated view, requirement ledger, source inventory, policy file, receipt or foundation claim. r[LPL-01] r[LPL-08]

## Phase 2: Store implementation and rejection tests (originally blocked on #73 Phase 2)

The #73 substrate is now implemented. This slice uses its admitted chunk
envelopes, pinned `library_revision`, one-hole enumerator and report counters;
the Phase 1 blocker above describes the state when this package was designed.

- [x] [serial] Implement the `procedure-record/1` record over the admitted chunk envelopes, with canonical field order, provenance, ordered library dependency edges, per-obligation evidence binding and the integer cost key; scope identity to the immutable subject fields, keep evidence/promotion/cost as versioned state, recompute identity on every admission, and refuse any supplied digest, id, signature, name or path as authority. r[LPL-01]
- [x] [serial] Implement the two derived revisions from one generation and prove by execution that the chunk view reproduces the #73 `library_revision` exactly for the same chunk set. r[LPL-01] r[LPL-07]
- [x] [serial] Implement composition through `Session::prepare`, `acceptance::check` and `Core::derive_compose`, including the `UnresolvedImplication` case, and implement the authority-subset check with every escalation row as a named refusal. r[LPL-02]
- [x] [serial] Implement the promotion lattice over the existing evidence classes, with consumer policy obligations, per-obligation outcomes, `ForgedStatus`, `StaleContext`, recorded demotion and no evidence inspection that reaches a prover or a proof file. r[LPL-03]
- [x] [serial] Implement exact-record deduplication and the `DuplicateEntry` restatement refusal; implement the alias relation gated on a locally verified bounded pure-I64 exact-pair certificate, retaining both identities and closures. r[LPL-04]
- [x] [serial] Implement the retrieval total order, the four query filters as ordered subsequences, the bounded index, the examined-entry counter and the rule that no retrieval result authorizes a composition. r[LPL-05]
- [x] [serial] Implement append-only generations, `supersedes`, rollback with the exactness and refusal rules, and the no-deletion rule. r[LPL-06]
- [x] [serial] Implement persistence and load recomputation with canonical collection order, the prohibited-input audit and `invalid` at the first differing field; add the two-process manifest comparison and the four negative controls as fixtures inside an existing test target, without adding a compilation unit. r[LPL-07]
- [x] [serial] Implement the identity manifest and metrics records, bind metrics to the manifest digest, and verify that no time, path, process, host, capability, secret, teacher configuration or training policy appears in either record. r[LPL-08]
- [x] [serial] Add the acceptance-1 and acceptance-2 fixtures (`LPL-T01`–`LPL-T12`) to existing test targets, report counts and outcomes as executed library mechanics, and keep them separate from any proof or transfer claim. r[LPL-02] r[LPL-03] r[LPL-04] r[LPL-05] r[LPL-06] r[LPL-07]

Phase 2 execution evidence: `crates/noble-contracts/tests/editor/procedures/{identity,admission,composition,retrieval,generations,reload-negative,metrics}.rs`
and `crates/noble-contracts/src/source/procedures/{composition,retrieval,persistence,alias}.rs`.
`LPL-T07` admits and reloads a verified exact-pair alias for distinct pure-I64
bodies with matching normalized polynomials and independently binds both
empty dependency closures. Unsupported operators, a forged closure and the
legacy digest-only claim refuse; the positive case is not a #56 certificate,
a proof-demanding promotion or an implementation-to-Lean refinement.

## Phase 3: Transfer evaluation (the original #73/#116 blockers have cleared)

- [x] [serial] Freeze the transfer protocol before any run: task families, held-out composition split, four arms (no library, frozen hand library, accumulated learned library, size-matched non-transfer control), paired seeds, success threshold, and the exact library sweeps. Commit the criterion before measuring. r[LPL-08]
- [x] [serial] Generate held-out composed tasks from #116's producer with its grammar revision, budget, witness/counterexample trace and verifier status retained, and prove no held-out answer, body or evaluator state reaches an entry or the retrieval index. r[LPL-08]
- [x] [serial] Run the arms with the proposer and teacher disabled, record `proposer_calls` and `teacher_calls`, make any attempted call fail closed, and withdraw any transfer claim from a run with a nonzero count. r[LPL-08]
- [x] [serial] Report acquisition, retention, transfer, regression, growth, retrieval overhead, restart reuse and the break-even invocation count per arm and per seed, with model/synthesis behavior, executed mechanics, correspondence and assumptions kept separate; claim no lifelong learning, speedup or universal transfer. r[LPL-08]
- [x] [serial] Discharge the planned proof obligations `LPL-O1`–`LPL-O4` and the executed properties `LPL-O5`/`LPL-O6` as their own obligations before any promotion of a record to `promoted` under a proof-demanding policy. r[LPL-01] r[LPL-03] r[LPL-07] Decision D14 (NO-GO for unproved implementation refinement); `verification/procedure-library/obligations.json`.

Phase 3 frozen criteria: `verification/procedure-library/criteria-2026-11-13.json`
(committed before measurement); source candidates from
`verification/synthesis/receipts/2026-11-13/synthesis-v2.json`.
The invalid original attempt and its erratum are retained at
`verification/procedure-library/receipts/2026-11-13/transfer-failed.json`
and `transfer-failed-erratum.json`. A single repaired repeat from a separate
clean source commit under the byte-identical criterion is retained at
`verification/procedure-library/receipts/2026-11-13/transfer-repeat.json`:
8/8 learned task cells (24/24 examples), 8/8 frozen-hand, 0/8 empty and
0/8 size-matched control. `verification/procedure-library/decision.md`
records why this is not a claim of incremental learning or of attempted-call
refusal **in the original frozen run**. The later separately frozen
`verification/procedure-library/receipts/producer-transfer-v2-final.json`
records all 16 scored rows with zero producer/teacher attempts and callbacks.
Its four separate fresh-library negative controls each deliberately attempted
both requests and refused `Unsupported` before either callback; the original
receipt's missed clause remains missed. The v2 target references were paired
to the wrong source-order semantics; v2's 0/8 learned outcome is a
**defective-criterion NO-GO, not evidence against the library**. V3's
criterion and manifest were mechanically derived and committed from the
#116 source programs before new target generation. One clean-source producer
run emitted `verification/procedure-library/receipts/producer-targets-v3.json`;
one clean-source four-arm run emitted `receipts/producer-transfer-v3.json`
with its raw log. Learned and frozen-hand each passed 8/8 paired seed-task
cells and 24/24 examples; both other arms passed 0/8. All 16 scored rows
recorded zero teacher/proposer attempts and calls, and each seed's isolated
attempted-call controls refused before callbacks. Neither older run was
repeated. The final Phase 3 row records a decision, **not** a universal
implementation proof. Decision D14 and the six separate records in
`verification/procedure-library/obligations.json` bind inventoried, audited
abstract-model theorems for O1–O4, generated execution for O5/O6, and
clean-source refusal of proof-demanding promotion when a consumer demands
missing implementation-refinement evidence. The 36-case finite Rust/Lean
encoder comparison is retained at `verification/procedure-library/receipts/encoder-corpus-v3.json`;
it does not prove universal Rust encoder or normalizer correspondence.
Proof validity remains the independent MC2 route, not a trusted status field.
Do not claim proof-demanding promotion for those implementation refinements
until D14's explicit reopening conditions are met; archiving this bounded
change does not change the original frozen run's missed teacher-attempt clause.
