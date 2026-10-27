## Phase 1: Design

- [x] [serial] Define the source and target event alphabets and the explicit relation, covering operation, arguments, exact identity, response, protected commit, return, abnormal-exit class and host-cleanup commit. r[WIT-TRACE-01]
- [x] [serial] Specify erasure only for named internal ABI steps with per-class justification, preserving ordering, pairing, identity and commits, never erasing denials or re-writing retirement as a guest release. r[WIT-TRACE-02]
- [x] [serial] Specify the derived RegionProtocol corollaries with declared premises and a normal-return postcondition conditional on normal return. r[WIT-TRACE-03]
- [x] [serial] Specify the cost or accounting relation for quota and fuel boundaries and refuse an unrestricted trap clause. r[WIT-TRACE-04]
- [x] [serial] Specify the finite observable pilot, the instrumentation needed to separate trap, quota and cleanup failure, and unknown observations staying unknown. r[WIT-TRACE-05]
- [x] [serial] Specify the correspondence route from the target model to actual lowering and runtime steps with pinned revisions. r[WIT-TRACE-06]

## Phase 2: Pilot (future, not started)

- [x] [serial] Implement the Lean source and target alphabets and the relation definition, with the target model separate from any real engine instance. r[WIT-TRACE-01]
- [ ] [serial] Prove one justification per erased ABI bookkeeping class and show the relation preserves ordering, pairing, identity and every protected commit. r[WIT-TRACE-02]
  Partial: `proofs/m5/RegionTrace.lean` proves the erasure justification over every scan input
  (`erase_preserves_*`), the denial and retirement controls, the exact-identity projection and
  prefix/ordering composition (`append_projection`, `take_projection`). Exact identity, read-commit
  explicability and at-most-once release are now **conditions of the relation** rather than claims:
  the six `related_rejects_*` controls refuse the forged traces and `related_accepts_well_formed_success`
  keeps them non-vacuous. Pairing and the host's own identity supply stay premises, so the task stays
  open.
- [ ] [serial] Derive and prove prefix compatibility and the RegionProtocol corollaries under the declared host, identity, cost and cleanup premises. r[WIT-TRACE-03]
- [ ] [serial] Implement the cost or accounting relation and prove the committed prefix is preserved across an exhaustion boundary. r[WIT-TRACE-04]
- [ ] [serial] Add host-side observations that separate trap, quota exhaustion and cleanup failure at the existing WIT interface, recording unknown where the observation is absent. r[WIT-TRACE-05]
- [ ] [serial] Add the normal-success, ordinary-error, host-refusal, trap and quota-prefix fixtures, keeping cancellation unsupported. r[WIT-TRACE-05]
- [ ] [serial] Add the negative controls for swapped requests, substituted responses and resources, an erased denial, a missing protected commit and host cleanup disguised as a guest release. r[WIT-TRACE-02] r[WIT-TRACE-03]
  Model-level controls done: substituted identity, unexplained read commit, second release, missing
  protected commit, host cleanup as guest release and a denial traded for a success are refused.
  Swapped requests and substituted responses at the host/WIT interface still need the instrumentation
  this package does not add, so the task stays open.
- [ ] [serial] Bind source, component, helper, engine and model revisions in a retained receipt and label every proved model theorem separately from correspondence. r[WIT-TRACE-06]
