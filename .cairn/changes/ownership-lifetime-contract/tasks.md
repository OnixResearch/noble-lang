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
  Still open:
  - registration in a reviewed conformance change;
  - OWN-CASE-04's borrow variants, which need a `borrow` word;
  - OWN-CASE-05, which needs a root that traps after saving;
  - OWN-CASE-07, which needs a runtime that rebinds a retained name.
- [ ] [serial] If a borrow profile is selected, specify syntax separately, implement the checker escape test and obtain checker-refinement evidence for OWN-PO-2. r[OWN-BORROW-01]
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
- [ ] [serial] Select any scratch-arena promotion form only after measured comparison under the allocation benchmark plan, and verify reset safety with OWN-CASE-08. r[OWN-SCRATCH-01]
