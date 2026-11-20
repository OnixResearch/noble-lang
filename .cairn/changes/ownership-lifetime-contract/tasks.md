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
  Progress (2026-11-09): OWN-CASE-01 to 03 run on the live-slot runtime; see
  `verification/ownership-cases/receipt.json`, from clean source `dd96f88c`. OWN-CASE-01 is the
  direct quote of `live_slot_direct_quote.mjs`.
  `live_slot_ownership_cases.mjs` runs OWN-CASE-02 in both release orders and OWN-CASE-03 as a
  balanced 8-leaf tree, whose root gives the same result after the 14 other owners are released.
  The left- and right-deep 8-leaf trees cannot run as designed. The host bounds its observation of
  an anonymous saved Program at 1000 bytes and 64 nodes, so it refuses a chain's fifth compose (the
  second with captured leaves) and the session ends with `terminated-after-guest-failure`.
  Progress (2026-11-20, campaign slice own-cases):
  - **Registration prepared, review pending.** `specs/conformance/ownership-cases.json` registers
    OWN-CASE-01..12 in a new `Ownership-Lifetime-Design` family with canonical requirement
    bindings; `specs/spec-family.json`, the scenario views and the requirement ledger are
    regenerated. OWN-CASE-08 and OWN-CASE-09 record the 2026-11-20 owner decision that no
    scratch-arena or borrow profile is selected. Independent review is still required by the
    integrator before any registration is accepted.
  - **OWN-CASE-01, 02, 06 executed.** The committed lanes pass on the selected CLI: the direct
    quote's capture is live after invocation exit (25); both shared-owner release orders leave the
    survivor runnable and refuse released owners; the saved named caller still yields 42 after
    supersession and the module retires only after the owner is released.
  - **OWN-CASE-03 executed, expectation not met.** The balanced 8-leaf tree gives 8 before and
    after the 14 other owners are released. Both the left- and the right-deep 8-leaf chains are
    refused at their fifth compose by the host's declared observation bound (`captureValue`:
    1000 bytes, 64 nodes) with `execution-failed` and
    `terminated-after-guest-failure`. The registered expectation asks for equal results from all
    three trees; the case is therefore recorded as an executed failure, not silently narrowed.
  - **OWN-CASE-04 non-borrow variants executed.** `crates/noble-cli/tests/core/ownership_cases.rs`
    rejects `dup`, `drop`, `quote`, `Pair` then `quote`, `Sum` then `drop` and `List` then `drop`
    at the kernel check stage with the exact `Eligibility` constraint, before any candidate
    submission or emission. The borrow variants remain inapplicable under the 2026-11-20 owner
    decision.
  - **OWN-CASE-07 executed.** `def n [ 1 ]`, `[ n ]` saved, `def n [ 2 ]` reloaded: the saved
    Program yields 1, its old code span stays installed while the owner lives, and
    `:release-observed-roots` retires the span after the owner is consumed.
  - **OWN-CASE-05 NOT executed as designed.** Every live-slot source containing `test.abort` is
    refused at `install` with the internal diagnostic `inconsistent frontend arena`. The kernel
    bootstrap table ends at `test.emit`; the ordinary Core source environment appends the abort
    row (Definition 23, effect id 1) in `adapt_environment`, but the live-slot session uses the
    kernel environment directly. Appending the row without the effect is refused as
    `unknown source host effect`; appending the effect too is refused as an invalid contract
    because `bootstrap_matches` fixes a declared-module environment's effect list to
    `test.emit` + `live.dispatch`. The host ledger path was measured separately in
    `crates/noble-cli/tests/live_slot_own_trap.mjs`: `abortRoot` clears entered dispatch frames,
    a normal `finishRoot` is refused while frames are entered, and the aborted root's captured
    map pin is retired exactly once. Backend per-root frame-pin counters are not observable, so
    the case is withheld from promotion.
  - **Gate not yet run.** `verification/own-cases/gate.mjs` is committed; it must still run from a
    clean tree before any case promotion is recorded. This task stays open.
- [x] [serial] If a borrow profile is selected, specify syntax separately, implement the checker escape test and obtain checker-refinement evidence for OWN-PO-2. r[OWN-BORROW-01] OWNER DECISION 2026-11-20: no borrow profile is selected, so the condition does not hold and the task closes with that recorded outcome (owner rule: a recorded negative outcome completes a conditional task). Noble keeps only the WIT `own`/`borrow` adapter forms; it has no `borrow` word. OWN-CASE-04's borrow variants are therefore inapplicable, OWN-PO-2 is not applicable, and no checker escape test or refinement evidence is claimed. Selecting a borrow profile later needs a new change carrying its syntax, escape test and OWN-PO-2 evidence.
- [ ] [serial] Mechanize OWN-PO-1/3 in the abstract model, then obtain separate lowering, runtime and host-ledger correspondence for OWN-PO-4..7. r[OWN-EV-01]
  Progress (2026-10-30): in the small-step model `proofs/foundation/NobleFoundation/Effects.lean`,
  OWN-PO-1 holds for composition, branches (`ifte`), `dip` with a frame-held value, `run` and
  dynamic Programs. `owners_step` proves that every typed step permutes the owner multiset
  (stack plus frames), except a permitted `acquire` (fresh id added) or `release` (released id
  consumed). `own_preserve` and `reachable_sound` carry this to every finite prefix.
  OWN-PO-3 is covered in the same model by `ValT.rfree`, `CodeT.rfree` and `typed_rfree`, plus
  the capture negatives.
  The task stays open:
  - the model has no aggregates, named definitions, case forms, borrows or heap;
  - no lowering, runtime or host-ledger correspondence exists for OWN-PO-4..7.
- [x] [serial] Select any scratch-arena promotion form only after measured comparison under the allocation benchmark plan, and verify reset safety with OWN-CASE-08. r[OWN-SCRATCH-01] OWNER DECISION 2026-11-20: no scratch-arena profile is selected, so no promotion form is chosen and OWN-CASE-08 is inapplicable. The task closes on that recorded outcome (owner rule for conditional tasks). Scratch arenas stay a design option. Selecting one later needs the allocation benchmark measurements and an executed OWN-CASE-08 reset-safety receipt in a new change.
