## Phase 1: Design selection building on canonical Live-Slot-Design (no runtime acceptance)

- [x] [serial] Identify which issue #1 semantics the canonical LSLOT contract already selects. Separate the experimental runtime facts at 44329dc and isolate four lifetime gaps without changing `explicit-live-references`. r[LREF-PKG-01] r[LREF-EXIT-01]
- [x] [serial] Answer each "Questions to settle" item. Define the version package retention unit and retirement order, abnormal-exit pin release, non-wrapping counters and escaped-result semantics, related to the #3 borrow and reclamation contract and the #5 quotas. r[LREF-PKG-01] r[LREF-EXIT-01] r[LREF-CTR-01] r[LREF-ESCAPE-01]
- [x] [serial] Map the issue outcome scenarios to the registered LSLOT designs and design LREF-CASE-01..06 as unexecuted. Run document validation and the proposal/design/tasks gates only. r[LREF-EV-01]

## Phase 2: Future implementation and evidence gates (open; not undertaken here)

- [ ] [serial] Implement abnormal-exit pin release with exact ledger accounting for a continuing host, and execute LREF-CASE-01. r[LREF-EXIT-01]
- [ ] [serial] Implement package-level version retention, resolve #3 `OWN-CASE-06`, and execute LREF-CASE-03 through slot replacement. r[LREF-PKG-01]
- [ ] [serial] Execute LREF-CASE-02, 04, 05 and 06 together with LSLOT-01..09 against an implemented profile, recording source-bound evidence separately per case. r[LREF-CTR-01] r[LREF-ESCAPE-01] r[LREF-EV-01]
