Hard dependency: #73 Phase 2 MUST first implement and independently admit the
exact `typed-chunk/1` envelope, immutable chunk library, `library_revision`,
one-hole stack-equation solver, deterministic shortlex queue, ordinary
source/kernel re-admission, shared budgets and four outcome classes. No task
below may substitute a new chunk store, a second body encoding or a
signature-only index. #116 supplies synthesized bodies; #56 supplies the only
certified-equivalence route used for semantic deduplication.

## Phase 1: Design (this change)

- [x] [serial] Ground the package in the verified substrate at `fed77835`: the #73 envelope, request, ordering, budgets and outcomes (`.cairn/changes/typed-chunk-completion/design.md:44-195`), P-ID-04 (`.cairn/specs/language/spec.md:2138-2141`), VC-ID-03 (`.cairn/specs/program-contracts/spec.md:469-481`), the MC2 core/rules/registry/refusals (`crates/noble-contracts/src/companion/`), the CLI companion surface (`crates/noble-cli/src/core/companions.rs:25-38`), the evidence classes and binding rules, and #73's Phase 1-complete/Phase 2-untouched status. r[LPL-01]
- [x] [serial] Fix the extension: one store, two views; `procedure-record/1` over the unmodified `typed-chunk/1` envelope; separate `chunk_library_revision` and `procedure_library_revision` derived from one generation; no second body encoding, snapshot, enumerator or admission route. r[LPL-01]
- [x] [serial] Fix the record field set, canonical field order, integer-only cost key, derived retrieval signature, per-obligation evidence binding, and the rule that identity is recomputed from retained fields and never supplied by a digest, id, signature, name or path. r[LPL-01]
- [x] [serial] Fix identity status: experimental, versioned, library-local; encoder id and encoding revision recorded in every record and export; no canonical encoding, hash algorithm or cross-implementation stability selected (P-ID-04, VC-ID-03). r[LPL-01] r[LPL-07]
- [x] [serial] Fix composition and capability admission onto the MC2 path: fresh candidate, ordinary `Session::prepare`, `acceptance::check`, `ComposeV1` with both premises, joint endpoint as the intermediate implication, authority-subset rule, and the escalation table naming exact refusals. r[LPL-02]
- [x] [serial] Fix promotion: `candidate`/`tested`/`promoted`/`superseded`, consumer-selected obligation policy, existing seven evidence classes reused, tested and proved permanently distinct, no promotion transfer across subject or policy, demotion recorded in a new generation. r[LPL-03]
- [x] [serial] Fix deduplication: exact-record identity reuse, derivation restatement refused as `duplicate-entry`, equal signatures kept distinct, semantic alias only from a #56 certificate for the exact pair with both closures retained. r[LPL-04]
- [x] [serial] Fix deterministic retrieval: total order over interface, effects, promotion rank, integer cost key and identity bytes; filters as ordered subsequences; bounded index and counted examination; candidates are identities, never authority. r[LPL-05]
- [x] [serial] Fix append-only generations, `supersedes` replacement, rollback exactness, rollback refusal on drift or unavailable toolchain, and the no-deletion rule for referenced generations. r[LPL-06]
- [x] [serial] Fix reload identity: persisted canonical form, recomputation order, `invalid` at the first differing field, prohibited nondeterministic inputs, canonical collection order, and the two-process manifest comparison with four negative controls. r[LPL-07]
- [x] [serial] Fix the two export records: deterministic `noble-procedure-library-export/1` identity manifest, separate `noble-procedure-library-metrics/1` bound to its digest, RSI Lab measurement fields, and fail-closed refusal of any runtime teacher fallback with claim separation. r[LPL-08]
- [x] [serial] Fix the acceptance 1–3 test plan naming every #73 and #116 interface consumed, its blocked status, and six planned proof obligations with separate model-proof/implementation-test/correspondence/assumption routes. r[LPL-01] r[LPL-02] r[LPL-03] r[LPL-04] r[LPL-05] r[LPL-06] r[LPL-07] r[LPL-08]
- [x] [serial] Validate the change documents with the pinned Cairn runner; touch no canonical specification, generated view, requirement ledger, source inventory, policy file, receipt or foundation claim. r[LPL-01] r[LPL-08]

## Phase 2: Store implementation and rejection tests (blocked on #73 Phase 2)

Blocked until #73 implements and independently admits TCHUNK-01/TCHUNK-02. The
follow-up slice starts from the interfaces listed in the design's test plan:
the ordered admitted chunk envelopes (or their canonical bytes), the pinned
`library_revision`, the one-hole enumerator and its report counters.

- [ ] [serial] Implement the `procedure-record/1` record over the admitted chunk envelopes, with canonical field order, provenance, ordered library dependency edges, per-obligation evidence binding and the integer cost key; scope identity to the immutable subject fields, keep evidence/promotion/cost as versioned state, recompute identity on every admission, and refuse any supplied digest, id, signature, name or path as authority. r[LPL-01]
- [ ] [serial] Implement the two derived revisions from one generation and prove by execution that the chunk view reproduces the #73 `library_revision` exactly for the same chunk set. r[LPL-01] r[LPL-07]
- [ ] [serial] Implement composition through `Session::prepare`, `acceptance::check` and `Core::derive_compose`, including the `UnresolvedImplication` case, and implement the authority-subset check with every escalation row as a named refusal. r[LPL-02]
- [ ] [serial] Implement the promotion lattice over the existing evidence classes, with consumer policy obligations, per-obligation outcomes, `ForgedStatus`, `StaleContext`, recorded demotion and no evidence inspection that reaches a prover or a proof file. r[LPL-03]
- [ ] [serial] Implement exact-record deduplication and the `DuplicateEntry` restatement refusal; implement the alias relation gated on a #56 certificate for the exact pair, retaining both identities and closures. r[LPL-04]
- [ ] [serial] Implement the retrieval total order, the four query filters as ordered subsequences, the bounded index, the examined-entry counter and the rule that no retrieval result authorizes a composition. r[LPL-05]
- [ ] [serial] Implement append-only generations, `supersedes`, rollback with the exactness and refusal rules, and the no-deletion rule. r[LPL-06]
- [ ] [serial] Implement persistence and load recomputation with canonical collection order, the prohibited-input audit and `invalid` at the first differing field; add the two-process manifest comparison and the four negative controls as fixtures inside an existing test target, without adding a compilation unit. r[LPL-07]
- [ ] [serial] Implement the identity manifest and metrics records, bind metrics to the manifest digest, and verify that no time, path, process, host, capability, secret, teacher configuration or training policy appears in either record. r[LPL-08]
- [ ] [serial] Add the acceptance-1 and acceptance-2 fixtures (`LPL-T01`–`LPL-T12`) to existing test targets, report counts and outcomes as executed library mechanics, and keep them separate from any proof or transfer claim. r[LPL-02] r[LPL-03] r[LPL-04] r[LPL-05] r[LPL-06] r[LPL-07]

## Phase 3: Transfer evaluation (blocked on #73 Phase 2 and #116)

- [ ] [serial] Freeze the transfer protocol before any run: task families, held-out composition split, four arms (no library, frozen hand library, accumulated learned library, size-matched non-transfer control), paired seeds, success threshold, and the exact library sweeps. Commit the criterion before measuring. r[LPL-08]
- [ ] [serial] Generate held-out composed tasks from #116's producer with its grammar revision, budget, witness/counterexample trace and verifier status retained, and prove no held-out answer, body or evaluator state reaches an entry or the retrieval index. r[LPL-08]
- [ ] [serial] Run the arms with the proposer and teacher disabled, record `proposer_calls` and `teacher_calls`, make any attempted call fail closed, and withdraw any transfer claim from a run with a nonzero count. r[LPL-08]
- [ ] [serial] Report acquisition, retention, transfer, regression, growth, retrieval overhead, restart reuse and the break-even invocation count per arm and per seed, with model/synthesis behavior, executed mechanics, correspondence and assumptions kept separate; claim no lifelong learning, speedup or universal transfer. r[LPL-08]
- [ ] [serial] Discharge the planned proof obligations `LPL-O1`–`LPL-O4` and the executed properties `LPL-O5`/`LPL-O6` as their own obligations before any promotion of a record to `promoted` under a proof-demanding policy. r[LPL-01] r[LPL-03] r[LPL-07]
