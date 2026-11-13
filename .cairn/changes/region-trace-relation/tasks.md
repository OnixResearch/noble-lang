## Phase 1: Design

- [x] [serial] Define the source and target event alphabets and the explicit relation, covering operation, arguments, exact identity, response, protected commit, return, abnormal-exit class and host-cleanup commit. r[WIT-TRACE-01]
- [x] [serial] Specify erasure only for named internal ABI steps with per-class justification, preserving ordering, pairing, identity and commits, never erasing denials or re-writing retirement as a guest release. r[WIT-TRACE-02]
- [x] [serial] Specify the derived RegionProtocol corollaries with declared premises and a normal-return postcondition conditional on normal return. r[WIT-TRACE-03]
- [x] [serial] Specify the cost or accounting relation for quota and fuel boundaries and refuse an unrestricted trap clause. r[WIT-TRACE-04]
- [x] [serial] Specify the finite observable pilot, the instrumentation needed to separate trap, quota and cleanup failure, and unknown observations staying unknown. r[WIT-TRACE-05]
- [x] [serial] Specify the correspondence route from the target model to actual lowering and runtime steps with pinned revisions. r[WIT-TRACE-06]

## Phase 2: Pilot

- [x] [serial] Implement the Lean source and target alphabets and the relation definition, with the target model separate from any real engine instance. r[WIT-TRACE-01]
- [x] [serial] Prove one justification per erased ABI bookkeeping class and show the relation preserves ordering, pairing, identity and every protected commit. r[WIT-TRACE-02]
  Complete: `RegionTrace.lean` proves each named erasure preserves projection and every relation
  scan, and `related_projects_only_paired_traces` requires each response to answer the preceding
  request of the same operation (while permitting an unanswered final request at a cut prefix).
  Exact identity, read-commit explanation and at-most-once release/retirement are relation
  conditions with negative and positive controls. The runtime observation retains the host's
  ordered requests, commits, responses and identities. The emitted-component correspondence remains
  explicitly open under WIT-TRACE-06 and is not claimed by these model theorems.
- [x] [serial] Derive and prove prefix compatibility and the RegionProtocol corollaries under the declared host, identity, cost and cleanup premises. r[WIT-TRACE-03]
- [x] [serial] Implement the cost or accounting relation and prove the committed prefix is preserved across an exhaustion boundary. r[WIT-TRACE-04]
- [x] [serial] Add host-side observations that separate trap, quota exhaustion and cleanup failure at the existing WIT interface, recording unknown where the observation is absent. r[WIT-TRACE-05]
  Complete: the opt-in v2 report retains ordered host events, separate `exit_class`, cleanup state
  and reason, fuel remaining, and exact signed request arguments. Runtime execution distinguishes
  `normal`, `rejected`, `trap`, `quota-exhausted`, `cancelled`, `adapter-invalid` and
  `outside-model`; absent exit classes lower to `unknown`. A refused retirement is representable in
  the host report and model, but the existing guest interface cannot induce it. Cleanup selection,
  invocation and failure retention are therefore evidenced at the host method by
  `HostLocality.lean::cleanup_failure_is_selected_invoked_and_retained` and
  `verification/host-locality/evidence-1bf39a52.json:cleanup-release-refusal-retains-owner`;
  no production fault-injection seam is added. The ordered observation lowers to the handwritten
  model only; emitted-step correspondence remains open.
- [x] [serial] Add the normal-success, ordinary-error, host-refusal, trap and quota-prefix fixtures; keep cancellation scoped to its real host test control rather than claiming a general API. r[WIT-TRACE-05]
  Complete: the runtime receipt supplies normal success, ordinary refusal, the scope-exhaustion
  refusal of the 65th read, traps, quota exhaustion, all four cancellation controls, omitted release,
  adapter-invalid outcomes and outside-model results. The v2 bound records the entire scope-ceiling
  trace (64 admitted reads, the refused request/response and guest release) without truncation.
  `runtime-fixtures.mjs` generates the Lean fixtures and inventory from the new dated effect-protocol
  receipt; the region-trace checker requires byte-for-byte agreement. Two early host-retirement traces
  relate only under the reported registration premise; forged retirement controls remain refused.
  Cancellation is a real `--cancel-after-reads` host control, not a general asynchronous cancellation
  API. Cleanup refusal is modelled and generator-ready but not guest-reachable.
  The checker pins the generator's refused-`host.retire` path with a synthetic observation control;
  it is not counted as a recorded runtime fixture.
- [x] [serial] Add the negative controls for swapped requests, substituted responses and resources, an erased denial, a missing protected commit and host cleanup disguised as a guest release. r[WIT-TRACE-02] r[WIT-TRACE-03]
  Model-level controls done: substituted identity, unexplained read commit, second release, a guest
  release followed by a host retirement of the same region (refused since `atMostOnce` bounds
  releases and retirements together; it previously bounded each kind separately and accepted that
  pair), missing protected commit, host cleanup as guest release and a denial traded for a success
  are refused.
  Swapped responses are now refused twice over at both levels: the projected trace is unpaired
  (`swapped_response_is_unpaired`) and the relation refuses the target trace
  (`related_rejects_swapped_response`). At the host/WIT interface the ordered trace binds every
  response to the request it answers and every protected commit to the identity that request named,
  and the execution lane refuses a release that observes an identity the read did not retain. No
  emitted-step correspondence is claimed; that remains the separate WIT-TRACE-06 boundary.
  A request argument replaced by its unsigned reinterpretation (#90) is refused in three places:
  - by the relation (`related_rejects_unsigned_reinterpretation`, with
    `related_accepts_the_signed_refused_request` as its positive control);
  - as a laundered read commit (`related_rejects_commit_of_reinterpreted_region`);
  - by the execution lane's checker (`reinterpreted-unsigned-request-argument`, forged into the real
    refused reports).
  Swapped requests and a substituted response value are refused at model level
  (`related_rejects_swapped_requests`, `related_rejects_substituted_response_value`). The trace in the
  host's order relates. The four Langlib-addendum controls each pair a refusal with a positive case:
  - fuel stability (`retainFrom_fuel_stable`) refuses a semantics right at one budget only;
  - an omitted protected read whose value is unused is refused although the final answer agrees, and
    a related source trace claims exactly the target's read crossings
    (`related_source_reads_are_target_reads`);
  - a source trace that drops a commit made before a trap is refused;
  - `lower_correct_on_every_input` quantifies over identity, region and value, and refuses a lowering
    right on the empty input only.
  Complete: swapped responses are now refused by the relation itself, with a positive correctly paired
  response and an explicit incomplete-prefix control. The runtime generator applies the same pairing
  rule to every retained trace.
- [ ] [serial] Bind source, component, helper, engine and model revisions in a retained receipt and label every proved model theorem separately from correspondence. r[WIT-TRACE-06]
  Partial: the retained receipt binds the source and model revisions (pinned Lean version and executable
  hash, source and inventory hashes) and labels the verdict (model theorems 68 at `918db86a`, Lean 4.35.0,
  implementation theorems 0, correspondence none, liveness none, cancellation unsupported). `component`, `helper` and `engine`
  are recorded as null with the reason, because no correspondence route exists yet; the task stays open
  until those three are bound to real revisions.
