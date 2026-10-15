## Phase 1: Design only (this change)

- [x] [serial] Ground the baseline at `44329dc` and record drift from the issue's `0ea10d3` links. The baseline covers MC1 `verify`/`explain-proof`, `verify-module`, the source checker forms, the template recognizers, the diagnostic kinds and the Lean recheck. r[PAUTO-MAP-01] (related: VC-TOOL-01, VC-TOOL-02, DX-PROOF-01)
- [x] [serial] Design the source-located obligation map and the workflow/API sketch from source span through producer and independent check to diagnostics. r[PAUTO-MAP-01] (related: VC-INPUT-01)
- [x] [serial] Draw the trust and dependency boundary with pinned checker, model and rule revisions and disjoint producer/checker budget pools. r[PAUTO-PRODUCER-01] (related: VC-TOOL-03, VC-INTRINSIC-05)
- [x] [serial] Compare the tactic/SMT, reviewed-template and helper producer approaches on coverage, latency, diagnostic usefulness and evidence size, keeping unsupported cases explicit. Recommend first profile `PAUTO-P1`. r[PAUTO-PRODUCER-01]
- [x] [serial] Design the goal/context view with a worked source-to-goal sketch, checker-derived status chips and staleness. Design the helper expansion and the counterexample/vacuity pipeline. r[PAUTO-GOAL-01] r[PAUTO-CEX-01] (related: VC-LOGIC-02)
- [x] [serial] Author the unexecuted PAUTO-CASE-01 to PAUTO-CASE-16 scenario designs in `design.md`, all absent/not-run/open/unassessed, and the ADDED `PAUTO-*` requirement deltas. Validate the documents with the pinned Cairn runner only. r[PAUTO-MAP-01] r[PAUTO-PRODUCER-01] r[PAUTO-CEX-01] r[PAUTO-GOAL-01]

## Phase 2: Future implementation and evidence gates (not undertaken by this change)

- [ ] [serial] Implement the obligation map and `noble-goal-view/v1` from prepared contract data. Replace the `verify-module` `"unproved"` string with VC-TOOL-02 outcomes and add diagnostic categories. r[PAUTO-MAP-01] r[PAUTO-GOAL-01] (related: VC-TOOL-02)
- [ ] [serial] Implement sandboxed template and helper producers with a disjoint budget pool, routed only through `verify-module`/`verify`. Execute PAUTO-CASE-01, 07 to 09 and 12 to 16 with source-bound receipts. r[PAUTO-PRODUCER-01]
- [ ] [serial] Add editor-only proof-hole transport that is rejected on every admission path, plus stale-status invalidation that retains old evidence. Execute PAUTO-CASE-11 and 15. r[PAUTO-GOAL-01] (related: DX-HOLE-01, VC-ADMIT-04)
- [ ] [serial] Review and prove a tactic-free unary-`I64` Lean refutation template. Build compiled replay bound to the exact artifact. Execute PAUTO-CASE-02 to 06 and report correspondence discrepancies to the artifact-correspondence workstream. r[PAUTO-CEX-01] (related: VC-ADMIT-03)
- [ ] [serial] Measure the approach-comparison criteria on the evaluation matrix. Decide independently whether an external SMT/tactic profile is added. Register scenarios in conformance JSON and sync requirements only after review. r[PAUTO-PRODUCER-01] (related: EV-STATE-02)
