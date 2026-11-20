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
- [ ] [serial] Register the OWN-CASE designs in a reviewed conformance change and execute OWN-CASE-01..05 and 07 against the actual runtimes, without inheriting LIVE or LSLOT acceptance. r[OWN-EV-01]
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
    final commit; results are recorded in the receipt run log below.
  - **OWN-CASE-04 registered as executed.** The registered input is the design's `Account@1` opaque
    nominal (module 110, ordinal 0, resource kind 41) with its source programs, and the lane
    installs them through the real CLI: `dup`, `drop` and `quote` on `Account@1`, `quote` on
    `Pair<I64,Account@1>`, `drop` on `Sum<Account@1,I64>` and `drop` on `List<Account@1>`. Each is
    refused at the check stage with the exact `required S Data` eligibility diagnostic; the refusal
    reply reports zero guest requests, zero protected operations and no artifact. Two controls (an
    I64 program and a resource pass-through) install and emit an artifact. The six borrow escape
    variants of `design.md` are registered verbatim as inapplicable under OWNER DECISION 2026-11-20
    (decision 7: no borrow profile is selected).
  - **OWN-CASE-05 executed with an observable frame-pin ledger.** The design names no trap
    mechanism, so the lane uses a policy-denied effect inside a dispatched frame after the target
    produced a captured Program: the reply has no stack (no saved-owner token), the request trace is
    `[dispatch allowed, effect denied]`, and the session ends `terminated-after-guest-failure`. The
    live-slot registry now keeps a per-root frame-pin ledger (`entered`/`released`/`active`),
    `abortRoot` closes every abandoned pin exactly once and reports the counts, `finishRoot` refuses
    a root with unreleased pins, and both the invoke replies and the `trace` operation expose the
    counts (also needed by LREF-EXIT-01). The failing root observed `entered 2 / released 2 /
    active 0`. Expected state follows OWNER DECISION 2026-11-20 (decision 6: continue the host
    session when the pin ledger proves an exact release; poison otherwise): the exposed count is the
    host ledger of frame entries and exits, the backend's own pin counter is not exposed, so the
    poison branch applies. `test.abort` remains uninstallable in this profile; its internal
    `inconsistent frontend arena` refusal is recorded as a frontend-defect witness and is not the
    case's success condition.
  - **OWN-CASE-07 bound to H-LIVE-03.** The saved old definition yields 1 after the reload, its old
    code span stays installed while the owner lives, and `:release-observed-roots` retires the span
    after the release.
  - **Gate rework.** `verification/own-cases/gate.mjs` derives `promotable`, `failed_cases` and
    `receipts[*].case` from the observations, writes a receipt even when a lane fails, hashes every
    raw output file, re-hashes the executable after the run, reads the observed OWN-CASE-01/07
    values from the lanes, pins the bounded control's refusal point and diagnostic, takes the
    OWN-CASE-04 lane path from the tree, and never requires an internal diagnostic as success.
  - **Receipt and promotions.** The gate ran from a clean tree at `81139111`:
    `verification/own-cases/runs/20261120-r2/acceptance.json` sha256 `ae533fdd…`, source revision
    `sha256:f09c0d…`, binary `8020eb86…` (re-hashed after the `cargo test` steps, unchanged), 17
    hashed raw output files. `result: completed`, `verdict: unmet-cases`: OWN-CASE-01, 02, 04, 05,
    06 and 07 are promoted implemented/passed/open/explicit from this receipt; OWN-CASE-03 is
    recorded implemented/failed with the observed compose-5 refusal. The earlier receipts
    (`runs/20261120c`, `runs/20261120-failed`) stay immutable beside it.
  - **Consumer tests.** Every noble-cli live-slot target plus `live_repl` was run at the receipt's
    commit: `live_slot`, `live_slot_capture_legacy`, `live_slot_cas_policy`,
    `live_slot_coherent_maps`, `live_slot_direct_quote`, `live_slot_late_control`,
    `live_slot_quota_replay`, `live_slot_relocation_witness`, `live_slot_resource_ref`,
    `live_slot_selected_origin`, `live_slot_selected_retention`, `live_slot_typed_cases` and
    `live_repl` all pass. The run also covers the new OWN-CASE-04 lane through
    `live_slot_resource_ref`.
  - Independent review of this reworked registration is required by the integrator before the
    promotions are accepted (owner decision 1).
  - Independent review of the registration is still required, and OWN-CASE-05 is not executed as
    designed, so this task stays open.
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
- [x] [serial] Select any scratch-arena promotion form only after measured comparison under the allocation benchmark plan, and verify reset safety with OWN-CASE-08. r[OWN-SCRATCH-01] OWNER DECISION 2026-11-20: no scratch-arena profile is selected, so no promotion form is chosen and OWN-CASE-08 is inapplicable. The task closes on that recorded outcome (owner rule for conditional tasks). Scratch arenas stay a design option. Selecting one later needs the allocation benchmark measurements and an executed OWN-CASE-08 reset-safety receipt in a new change.
