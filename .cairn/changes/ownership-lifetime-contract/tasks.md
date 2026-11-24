## Phase 1: Design selection (no implementation or execution)

- [x] [serial] Re-read the cited runtime and emitter sources at 44329dc, record drift from 0ea10d3, and keep the module-root correspondence question as an open reproducer rather than a defect claim. r[OWN-PKG-01]
- [x] [serial] Recommend the three-class ownership model with the bounded scope-tagged borrow form, stack-operation transitions, and rejected escape/capture examples. r[OWN-BORROW-01]
- [x] [serial] Define the executable package lifetime unit, compare scratch/retained, closed-copy and sharing/deferred-RC strategies, and state the reclamation conditions C1–C7. r[OWN-PKG-01] r[OWN-RECLAIM-01] r[OWN-SCRATCH-01]
- [x] [serial] Record proof obligations OWN-PO-1..7 by layer with assumptions, design OWN-CASE-01..12 as unexecuted scenarios, and list remaining decisions; run document validation and the proposal/design/tasks gates only. r[OWN-EV-01]

## Phase 2: Future implementation and evidence gates (open; not undertaken here)

- [x] [serial] Execute the OWN-CASE-06 module-root retirement reproducer against the live-slot runtime. Decide between counting code-to-global edges and extending module lifetime, and record source-bound evidence. r[OWN-PKG-01] r[OWN-RECLAIM-01]
  Ticked 2026-11-08. `crates/noble-cli/tests/live_slot_module_root_retention.mjs` runs the case,
  hosted by a second test in `live_slot_direct_quote.rs`.
  - **Before the fix.** Superseding M's published root released M's module roots while a saved
    `[ g ]` lived, and invoking that owner trapped: `execution-failed`, `RuntimeError: unreachable`.
    A saved child without the named-global read, and `[ g ]` without supersession, both returned 42.
  - **Decision: extend module lifetime to saved callers.** `sweepRetirement` keeps a module's roots
    while `savedOwnerReaches(module)` holds; `discard` and `retireCode` already applied that
    predicate. `releaseProgram`'s existing re-sweep retires the module after its last saved caller
    is released.
  - **Not done.** Code→global owner edges are not counted; the backend counters cannot express them
    today.
  - **Cost.** Retention grows until the saved caller is released.
  - **Evidence.** `verification/module-root-retention/receipt.json`: the fixed CLI passes from clean
    source `27e941a0`, and the CLI from the parent commit `3baaa029` fails at the trap.
- [x] [serial] Register the OWN-CASE designs in a reviewed conformance change and execute OWN-CASE-01..05 and 07 against the actual runtimes, without inheriting LIVE or LSLOT acceptance. r[OWN-EV-01]
  Done 2026-11-22 (owner decision 44): the last open item was review finding F-05 (14 round-1
  commits authored as `brittonr`, already on `main`). The owner waived it: published history is not
  rewritten, and later OWN commits are authored as `noble-maintainers`. OWN-CASE-01..07 are
  implemented/passed from `verification/own-cases/runs/20261122-b5` (OWN-CASE-05 and the amended
  OWN-CASE-03 included); proof stays open.
  Progress (2026-11-09): OWN-CASE-01 to 03 first ran on the live-slot runtime; see
  `verification/ownership-cases/receipt.json`, from clean source `dd96f88c` (historical-only after
  2026-11-20; its bound description is superseded).
  Progress (2026-11-20, campaign slice own-cases, reworked after independent review):
  - **Registration reworked.** `specs/conformance/ownership-cases.json` registers OWN-CASE-01..12 in
    the `Ownership-Lifetime-Design` family with canonical requirement bindings; `specs/spec-family.json`,
    the scenario views and the requirement ledger are regenerated. The independent review of
    2026-11-20 (retained by the integrator, owner decision 1) returned REVISE on the first attempt:
    OWN-CASE-03's promotion was unsupported, OWN-CASE-04's registered input was not what ran,
    OWN-CASE-05's trap and session expectation were wrong, and the gate hard-coded its verdicts.
    This rework executes the design's cases instead of narrowing them and re-derives every verdict.
  - **OWN-CASE-03 executed against the design's case.** The leaves are captured-I64 Programs
    (`quote [ + ] compose`: `quote` captures the factory input, `[ + ]` adds it to the running
    value), and all 14 owners other than the root are released. The balanced 8-leaf tree returns 36
    before and after the releases and refuses every released owner. The left-deep and right-deep
    8-leaf chains are refused at compose 5 by the anonymous summary's 32-node per-child bound
    (`checked captured value exceeds host observation bound`), and the session ends
    `terminated-after-guest-failure`. The design's deep trees therefore do not execute; the case
    records that executed failure and is not promoted on this receipt.
  - **Bound change notes corrected.** The anonymous summary is bounded at 32 nodes / 4096 bytes
    *per child*, not by the same whole-graph bound as selected results, and a refused deep composite
    is stopped by that summary node limit, not by retained-graph accounting (`observeProgramGraph`
    keeps its separate 4096-node walk). Admission rises from about ten nodes per child (the old
    1000-byte cap bound first) to about 32; roots previously ending in `execution-failed` with a
    poisoned session now receive an owner. The consuming live-slot test targets were re-run at the
    receipt's commit; the observed results are recorded below.
  - **OWN-CASE-04 registered as executed.** The registered input is the design's `Account@1` opaque
    nominal (module 110, ordinal 0, resource kind 41) with its source programs, and the lane
    installs them through the real CLI: `dup`, `drop` and `quote` on `Account@1`, `quote` on
    `Pair<I64,Account@1>`, `drop` on `Sum<Account@1,I64>` and `drop` on `List<Account@1>`. Each is
    refused by the checker's eligibility rule with the exact `required S Data` diagnostic; the host
    reports the refusal at its command stage, and the reply carries zero guest requests, zero
    protected operations and no artifact. Two controls (an I64 program and a resource pass-through)
    install and emit an artifact. **Fidelity note:** the design constructs `Pair`/`Sum`/`List` in
    source; the lane feeds those aggregate types as typed root inputs and the diagnostics name the
    exact aggregate type, so the eligibility property is observed without running `pair`/`inl`/`cons`.
    The six borrow escape variants of `design.md` are registered verbatim as inapplicable under
    OWNER DECISION 2026-11-20 (decision 7: no borrow profile is selected).
  - **OWN-CASE-05 withheld (review round 2, N-01).** The lane records the abnormal exit: a
    policy-denied effect inside a dispatched frame after the target produced a captured Program. The
    reply has no stack (no saved-owner token), the request trace is `[dispatch allowed, effect
    denied]`, and the session ends `terminated-after-guest-failure`. The design's "frame pins are
    released exactly once" is **not observed**: on the abnormal path the host forgets the failed
    root's frame records and neither releases nor counts the backend frame pins (the instance is
    discarded). Neither ledger is a release measurement: the in-process abort probe of
    `abortRoot` reports `entered 2 / released 0 / active 2` (the pins the root still held), while
    the CLI root in the r3 receipt reports `entered 2 / released 2 / active 0` from host entry/exit
    calls, which do not prove backend pin release; no `released_by_host` field is emitted. The normal
    path keeps its real counts and the `finishRoot` unbalanced-pin refusal. The registration marks
    `frame_pins_released` and `retained_owner_reaches_failed_root_cell` as not-directly-observable,
    the case returns to `absent/not-run/open/unassessed`, and it is not promoted. Real per-frame
    release accounting on the abnormal path belongs to `live-reference-lifetimes` (OWNER DECISION
    2026-11-20 decision 6; that change's LREF-EXIT-01), which will execute OWN-CASE-05 later. `test.abort`
    remains uninstallable in this profile. It used to be refused with the internal `inconsistent
    frontend arena` diagnostic; the frontend now refuses an unavailable builtin word as invalid
    input (`word test.abort is not available in this environment`), and the refusal stays a
    witness, not the case's success condition.
  - **OWN-CASE-07 bound to H-LIVE-03.** The saved old definition yields 1 after the reload, its old
    code span stays installed while the owner lives, and `:release-observed-roots` retires the span
    after the release.
  - **Gate rework.** `verification/own-cases/gate.mjs` derives `promotable`, `failed_cases` and
    `receipts[*].case` from the observations, writes a receipt even when a lane fails, hashes every
    raw output file, re-hashes the executable after the run, reads the observed OWN-CASE-01/07
    values from the lanes, pins the bounded control's refusal point and diagnostic, takes the
    OWN-CASE-04 lane path from the tree, and never requires an internal diagnostic as success.
  - **Receipts and promotions.** The r2 gate receipt is
    `verification/own-cases/runs/20261120-r2/acceptance.json` (sha256 `ae533fdd…`, tested commit
    `81139111`, source revision `sha256:f09c0d…`); OWN-CASE-01, 02, 04, 06 and 07 are promoted from
    it (OWN-CASE-05 was promoted there and is withheld again after review round 2). The round-3
    receipt is `verification/own-cases/runs/20261120-r3/acceptance.json` (sha256 `3d7666bf…`,
    tested commit `33318b7c`, source revision `sha256:fdf333df…`, binary `5a62b529…` before and
    after, 17 hashed raw outputs; `result: completed`, `verdict: unmet-cases`, `promotable`
    OWN-CASE-01/02/04/06/07, `withheld` OWN-CASE-05, `failed_cases` OWN-CASE-03). Its projection
    digest `26bb4a11…` reflects the round-3 registration edits. OWN-CASE-03 stays
    implemented/failed with the observed compose-5 refusal. The first receipt (`acceptance.json`,
    `e873df58…`) and the intermediate runs (`runs/20261120c`, `runs/20261120-failed`) stay
    immutable. Batch 2 (2026-11-21): after R3-1/R3-4 and the frontend fix merged, the gate ran on
    the batch-2 tree `b088cfc7`; the promoted cases and OWN-CASE-03's executed failure now cite
    [`runs/20261121-b2`](../../../verification/own-cases/runs/20261121-b2/acceptance.json)
    (sha256 `f5c2d53a…`, source revision `sha256:0feaed54…`, binary `8c0844c8…` before and after,
    projection digest `7aa15017…`, gate `828ff88e…`) with the same verdicts.
  - **Consumer tests.** Every noble-cli live-slot target plus `live_repl` was run after the runtime
    change: `live_slot`, `live_slot_capture_legacy`, `live_slot_cas_policy`,
    `live_slot_coherent_maps`, `live_slot_direct_quote`, `live_slot_late_control`,
    `live_slot_quota_replay`, `live_slot_relocation_witness`, `live_slot_resource_ref`,
    `live_slot_selected_origin`, `live_slot_selected_retention`, `live_slot_typed_cases` and
    `live_repl` all pass. The run also covers the new OWN-CASE-04 lane through
    `live_slot_resource_ref` and `core`; all 14 targets pass after the abort-ledger change, and
    the gate run's own `cargo test` steps are among the round-3 receipt's hashed raw outputs.
  - Review status: round 1 returned REVISE, round 2 accepted the registration with notes, accepted
    01/02/06/07, accepted 03 as an executed failure, accepted 04 with notes and rejected the
    OWN-CASE-05 promotion. Round 3 accepted the registration and the promotions with notes
    R3-1..R3-4 and accepted the OWN-CASE-05 withholding. R3-1's task and README wording is fixed;
    R3-2 is applied (the evidence records name the full tested commit
    `33318b7cb518a53efc04e116981fbd70c6aec853`). R3-3: the gate re-ran on the merged Cairn batch-1
    tree `05bac95a` ([`runs/20261121-b1`](../../../verification/own-cases/runs/20261121-b1/acceptance.json),
    sha256 `3d311f50…`) with the same verdicts. R3-1's projected `frame_pins_reported` field and
    lane-header wording and R3-4's two consistency checks change the projection digest, the gate
    and an M4-bound lane file; they merged in batch 2 and the batch-2 run above re-cites the
    promoted cases. This task stays open until OWN-CASE-05 executes (it needs observable
    frame-pin release, shared with LREF-EXIT-01) and the owner decides the round-1 commit identity.
  Progress (2026-11-22, owner decision 35, delegated): **OWN-CASE-03 amended, not narrowed.** The
  32-node/4096-byte summary bound is a reviewed live-slot observation limit, so raising it would
  only move the cliff. The case keeps the 8-leaf balanced tree (36, 14 releases), registers the
  8-leaf left-deep and right-deep chains as refused at compose 5 (`checked captured value exceeds
  host observation bound`, session `terminated-after-guest-failure`), and adds 4-leaf balanced,
  left-deep and right-deep trees that must agree (10 before and after 6 releases), so shape
  independence is observed within the bound. The deep-chain lane now releases every leaf and
  intermediate except the root (`2n - 2`), as the design and the balanced tree do; it had released
  7 of 14. The amendment is recorded in the case file's `amendments` block. Observed (debug CLI
  from the batch-4 branch): balanced-8 36/14, left- and right-deep-8 refused at compose 5,
  balanced-, left-deep- and right-deep-4 all 10/6. Promotion waits for the gate's run on the
  merged tree and an integrator review (decision 28).
  Progress (2026-11-22, batch 4): the gate ran on the batch-4 tree `88e90add`
  ([`runs/20261122-b4`](../../../verification/own-cases/runs/20261122-b4/acceptance.json),
  sha256 `1a5abea4…`, source revision `sha256:50617810…`, binary `ca120a2e…` before and after,
  projection digest `946d97ab…`, gate `65c49dc8…`): verdict `all-passed`, promotable
  OWN-CASE-01/02/03/04/06/07, withheld OWN-CASE-05. OWN-CASE-03 observed every amended variant
  as registered and is promoted to implemented/passed (proof open); the integrator review of the
  amendment (decision 28) accepted it. The amendment changed the projection digest and the gate,
  so every promoted record now cites this run, as the batch-2 move did. The task stays open for
  OWN-CASE-05 (observable frame-pin release, LREF-EXIT-01) and the round-1 commit identity.
  Progress 2026-11-22 (integrator decision 39): LREF-EXIT-01 now releases every frame pin and the
  map pin from the host ledger and reports `released_on_abort`, so OWN-CASE-05's frame-pin release
  is observable; the case is amended (no longer withheld) and stays on the poison branch, because
  its guest's recorded failure is terminal for the shared guest state. OWN-CASE-03's save-time
  refusals now keep the session (an exact release), so its deep-chain and control expectations are
  amended and its batch-4 evidence is withdrawn until the gate re-runs.
  Executed 2026-11-22 (Cairn batch 5): the OWN gate ran at `8b62827a`
  ([`runs/20261122-b5`](../../../verification/own-cases/runs/20261122-b5/acceptance.json), sha256
  `c9bbeb56…`, source revision `sha256:897a5ebf…`, binary `7f32e4e1…` before and after, projection
  `3130b4c6…`): verdict `all-passed`, OWN-CASE-01..07 promotable. OWN-CASE-05 is promoted and
  OWN-CASE-03 re-promoted to implemented/passed (proof open); every other promoted record moved to
  this run because the amendments changed the projection and the gate. The task stays open only
  for the owner's decision on review finding F-05 (14 round-1 commits authored as `brittonr`,
  already on `main`, so they cannot be re-authored without rewriting published history).
- [x] [serial] If a borrow profile is selected, specify syntax separately, implement the checker escape test and obtain checker-refinement evidence for OWN-PO-2. r[OWN-BORROW-01] OWNER DECISION 2026-11-20: no borrow profile is selected, so the condition does not hold and the task closes with that recorded outcome (owner rule: a recorded negative outcome completes a conditional task). Noble keeps only the WIT `own`/`borrow` adapter forms; it has no `borrow` word. OWN-CASE-04's borrow variants are therefore inapplicable, OWN-PO-2 is not applicable, and no checker escape test or refinement evidence is claimed. Selecting a borrow profile later needs a new change carrying its syntax, escape test and OWN-PO-2 evidence.
- [ ] [serial] Mechanize OWN-PO-1/3 in the abstract model, then obtain separate lowering, runtime and host-ledger correspondence for OWN-PO-4..7. r[OWN-EV-01]
  Progress (2026-10-30): `proofs/foundation/NobleFoundation/Effects.lean` proved OWN-PO-1 for
  composition, branches (`ifte`), `dip` with a frame-held value, `run` and dynamic Programs, and
  OWN-PO-3 by `ValT.rfree`, `CodeT.rfree` and `typed_rfree`. That model has no aggregates,
  named definitions, case forms or heap.
  Progress (2026-11-20): the model half is extended in
  `proofs/foundation/NobleFoundation/OwnershipModel.lean`, a second small-step model that
  imports `Effects.lean` without editing it; commit `3b12d1b6`. It adds Pair/Sum/List
  construction and elimination, an exact-interface named-definition table applied by `call`,
  the `cases` and `listCase` forms, and a heap of owner-tracked cells (`alloc`). `run` pins the
  executed Program value in a `pin` frame until its body exits; `dip` keeps its `restore`
  frame.
  - OWN-PO-1: `owners_step` proves every typed step permutes the whole-configuration owner
    multiset (stack, frames and heap). The declared transitions are a permitted `acquire`
    (fresh id added), a permitted `release` (released id consumed) and `alloc` (a fresh cell id
    is consumed; the resource multiset and live set are unchanged). `own_preserve` and
    `own_reach` carry it to every finite prefix.
  - OWN-PO-3: `typed_rfree`, `ValT.rfree`, `ValT.programsRFree`, `CodeT.rfree`,
    `KontT.rfree`, `HeapT.rfree`, `quote_capture_rfree` and `alloc_store_rfree`, with recursive
    eligibility negatives (`dup_pair_res_untypable`, `drop_sum_res_untypable`,
    `drop_list_res_untypable`).
  - Finite prefixes: `preserve`, `preserve_reach`, `progress`, `heap_safe_step`,
    `heap_safe_reach` and `reachable_sound` for typed cell-free programs.
  - Controls: `pair_unpair_round_trip`, `sum_case_selects_arm`, `list_case_selects_arm`,
    `call_runs_named_body`, `call_interface_exact`, `alloc_then_drop_keeps_cell`,
    `run_pins_program`, `dip_holds_aggregate_in_frame`, plus untyped negatives.
  - Model boundary: storage reclamation/RC/collection beyond the no-dangling invariant, heap
    reads, borrows (OWN-PO-2 is not applicable), abnormal cleanup, and all lowering, runtime and
    host-ledger correspondence are not modelled. No theorem here mentions the Rust checker,
    emitted Wasm or the host ledger.
  - Verification: `lake build NobleFoundation` warning-free (2304 jobs). Clean-source
    `verification/foundation/check.mjs --output <scratch>` on `3b12d1b6` reported
    `clean_source: true`, 2708 inventoried theorems, axioms inside the allow-list (`propext`,
    `Classical.choice`, `Quot.sound`) and `implementation_theorem: false`; receipt sha256
    `b104923d216bde9073b4cc75d0e3ae36641b3ab9180185b1098814a26d6b5b48` (retained outside the
    repository, regenerated by the integrator). `check.mjs --self-test` and
    `verification/foundation/claims.mjs` passed.
  The task stays open: no lowering, runtime or host-ledger correspondence exists for
  OWN-PO-4..7, and the retained foundation receipt and claims audit are integrator evidence.
  Owner decision 57 (2026-11-23) selects the extracted Rust route for the
  host ledger. The design records the existing state owners, synchronous worker
  boundary and backend-acknowledgement obligations. An abstract model or a
  differential suite by itself does not complete OWN-PO-6/7.
  Progress (2026-11-24): `proofs/m3/NobleM3/LedgerRefinement.lean` (imported by
  `proofs/m3/NobleM3.lean`) proves the scoped `OWN-PO-6` extracted host-ledger
  refinement directly over Charon/Aeneas-extracted `noble_kernel.ledger`
  (`proofs/m3/NobleKernel/{Types,Funs}.lean`, from `crates/noble-kernel/src/ledger/`),
  strictly under `{propext, Classical.choice, Quot.sound}` with zero `sorry` and zero
  `native_decide` (verified by `lake build NobleM3 NobleM2 NobleM2.Refinement NobleKernel`
  and `verification/m3-proof-gate.sh`):
  - Loop & snapshot validation: `validate_frame_stack_eq`, `validate_frame_stack_ok_iff`,
    `copy_frame_prefix_eq`, `reverse_frames_eq`, `Snapshot.pinned_valid`,
    `Snapshot.pinned_invalid_zero`, `validate_active_root_total`,
    `validate_active_root_ok_iff`, `validate_active_root_refuses_poisoned`,
    `validate_active_root_refuses_zero_root`, `validate_active_root_refuses_zero_claim`,
    `validate_active_root_refuses_foreign`, `validate_active_root_refuses_inactive`,
    and `frame_pin_counts_spec`.
  - LIFO frame stack: `enter_frame_pushes_lifo`,
    `enter_frame_refuses_invalid_token`, `enter_frame_refuses_duplicate_or_unordered`,
    `enter_frame_refuses_bound_exceeded`, `enter_frame_refuses_counter_exhausted`,
    `leave_frame_pops_lifo`, `leave_frame_refuses_pending_dispatch`,
    `leave_frame_refuses_invalid_token`, `leave_frame_refuses_non_lifo`, and
    `enter_leave_roundtrip`.
  - Finish balanced release: `plan_finish_root_iff`, `plan_finish_root_balanced`,
    `plan_finish_root_refuses_active_frames`, `plan_finish_root_refuses_pending_dispatch`,
    and `ack_finish_root_spec`.
  - Abort exact-release vs. fail-closed `Poisoned`: `classify_guest_settled_iff`,
    `classify_guest_total`, `first_plan_unproven_none_iff`, `first_plan_unproven_total`,
    `plan_abort_root_spec` (`released_frames.val = s.frames.val.reverse`),
    `ack_abort_root_spec`, `ack_abort_root_exact_iff`, and
    `ack_abort_root_unexpected_resumption`.
  - Non-wrapping counters: `enter_frame_counter_nonwrapping`,
    `leave_frame_counter_nonwrapping`, `LedgerTrace`, `trace_valid_snapshot`,
    `trace_counters_monotone_nonwrapping`, and `trace_step_counters_monotone`.
  - Pin-frame model correspondence: Mathlib-free `PinState` / `PinStep` simulation
    mirroring `proofs/foundation/NobleFoundation/OwnershipModel.lean` continuation `.pin`
    frames (`toPinState` with innermost-first `s.frames.val.reverse`),
    `pinned_refines_pin_model`, `enter_frame_refines_pin_model`,
    `leave_frame_refines_pin_model`, `finish_root_refines_pin_model`,
    `abort_root_refines_pin_model`, and `retired_or_poisoned_refuses_all`.
  - Non-claims / remaining open scope (`r[OWN-EV-01]`, Decision 57): CAS/retained maps,
    saved-program graphs, code-span retirement, JS worker adapter cutover and adapter
    correspondence, `OWN-PO-7`, and `OWN-PO-4/5` remain unproved and open; this task
    remains unchecked (`[ ]`).
- [x] [serial] Select any scratch-arena promotion form only after measured comparison under the allocation benchmark plan, and verify reset safety with OWN-CASE-08. r[OWN-SCRATCH-01] OWNER DECISION 2026-11-20: no scratch-arena profile is selected, so no promotion form is chosen and OWN-CASE-08 is inapplicable. The task closes on that recorded outcome (owner rule for conditional tasks). Scratch arenas stay a design option. Selecting one later needs the allocation benchmark measurements and an executed OWN-CASE-08 reset-safety receipt in a new change.
