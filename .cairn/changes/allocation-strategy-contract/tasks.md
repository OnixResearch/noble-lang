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

## Progress (2026-11-23)

Neither Phase 2 box above is complete. The committed harness is `verification/allocation-baselines/`
(commit `9ce308ca`): `run.mjs` drives each strategy's own host (A1/A3 shipped `session`/`live repl`,
A4 `live slot`, A5/A6 `wasm-experiment`, A7 the component runtime fragment, A2 the bounded
`calculator` session), records pinned Node/wasm-tools, source hashes, host affinity and load with
every sample, and writes a source-bound receipt plus raw JSONL. `cases.mjs` gates
ALLOC-CASE-02/09/10 and is expected to be red until the F1-F4 split is implemented.

Measured so far (development runs of the harness against a clean base tree, 1 sample per point,
not yet a retained receipt): A1/A3 runtime composition trees compile at 8 and 16 leaves in one
submission and build 32/128 leaves across submissions; 144 leaves is refused with quota reason
`recipe leaves` (`data.wat`'s 256-atom bound), so 256-leaf BE-LIMIT-01 trees are not reachable in
the A1/A3 frontend. A5/A6 reach 255/256 leaves and refuse 257 at construction. One 10,000-root
shipped session stops at submission 513 with quota reason `allocation` (196,608 logical bytes,
384 bytes per short root, A1 never reclaims); the same expression in one fresh root costs 384 bytes.
A4 released 64 distinct saved Program owners in five seeded orders (all `program-released`).
The live-slot pool and a lowered-memory linear harness currently report the same quota reason 1 as
the logical limit (the pre-split gap the second box addresses).

Remaining before the first box can be ticked: run the full sampled matrix (30 samples per point
where the workload is cheap; the 10,000-root point is cost-bounded by the pinned host's ~12.7 ms
per submitted module), retain the receipt and raw samples in this change's verification directory,
and ratify or revise D1-D4 with those receipt citations. The second box's F1-F4 split, per-strategy
accounting revisions, and ALLOC-CASE-02/09/10 execution remain unstarted.
