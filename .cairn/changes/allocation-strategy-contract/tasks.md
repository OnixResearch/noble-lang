## Phase 1: Design selection (no implementation, measurement or allocator change)

- [x] [serial] Audit allocation sites, callers, cleanup paths and profile selection at 44329dc, record drift from 0ea10d3, and build the backend/profile × strategy compatibility matrix. r[ALLOC-CAP-01]
- [x] [serial] Define the minimal capability-specific contract: lifetime preconditions, selection granularity, provenance, layout and handles, reuse statements, and accounting. r[ALLOC-CAP-01] r[ALLOC-OBS-01]
- [x] [serial] Separate failure classes F1–F4 with atomicity and cleanup rules, and state isolation and observational-equivalence obligations. r[ALLOC-FAIL-01] r[ALLOC-ISO-01]
- [x] [serial] Write the W1–W8 benchmark plan, the proposed decision criteria D1–D4, the ALLOC-CASE-01..12 conformance designs and the recommendation. Run document validation and the proposal/design/tasks gates only. r[ALLOC-EV-01]

## Phase 2: Future implementation and evidence gates (open; not undertaken here)

- [ ] [serial] Execute W1–W8 baselines for the existing strategies A1–A7 with the recorded controls, and ratify or revise D1–D4 from measurements. r[ALLOC-EV-01] r[ALLOC-OBS-01]
- [ ] [serial] Implement distinct F1–F4 classification and per-strategy accounting revisions in the runtimes and hosts, then execute ALLOC-CASE-02/09/10. r[ALLOC-FAIL-01]
- [ ] [serial] Register and execute ALLOC-CASE-01, 03–08, 11 and 12 against each strategy claimed compatible, including differential equivalence and cross-instance refusal. r[ALLOC-CAP-01] r[ALLOC-ISO-01]
- [ ] [serial] After #4 selects a native route, compare the system allocator with jemalloc inside that route with representation held constant. Never apply it to Wasm guest cells. r[ALLOC-ISO-01]
