## Phase 1: Design selection (no implementation or execution)

- [x] [serial] Re-read the cited runtime and emitter sources at 44329dc, record drift from 0ea10d3, and keep the module-root correspondence question as an open reproducer rather than a defect claim. r[OWN-PKG-01]
- [x] [serial] Recommend the three-class ownership model with the bounded scope-tagged borrow form, stack-operation transitions, and rejected escape/capture examples. r[OWN-BORROW-01]
- [x] [serial] Define the executable package lifetime unit, compare scratch/retained, closed-copy and sharing/deferred-RC strategies, and state the reclamation conditions C1–C7. r[OWN-PKG-01] r[OWN-RECLAIM-01] r[OWN-SCRATCH-01]
- [x] [serial] Record proof obligations OWN-PO-1..7 by layer with assumptions, design OWN-CASE-01..12 as unexecuted scenarios, and list remaining decisions; run document validation and the proposal/design/tasks gates only. r[OWN-EV-01]

## Phase 2: Future implementation and evidence gates (open; not undertaken here)

- [ ] [serial] Execute the OWN-CASE-06 module-root retirement reproducer against the live-slot runtime. Decide between counting code-to-global edges and extending module lifetime, and record source-bound evidence. r[OWN-PKG-01] r[OWN-RECLAIM-01]
- [ ] [serial] Register the OWN-CASE designs in a reviewed conformance change and execute OWN-CASE-01..05 and 07 against the actual runtimes, without inheriting LIVE or LSLOT acceptance. r[OWN-EV-01]
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
