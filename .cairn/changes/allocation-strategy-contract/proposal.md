## Why

Noble issue #5 asks for research on a replaceable, backend-specific allocation layer. The layer would cover fixed pools, arenas, reclaimable cells and malloc-compatible native allocators such as jemalloc. The issue wants a small contract and a compatibility matrix before any implementation complexity is added. At 44329dc Noble already has several distinct storage strategies:

- the ordinary source session bump arena;
- the Calculator-v1 three-region arena with boundary compaction;
- the in-process live whole-instance snapshot;
- the live-slot free list with epoch handles and deferred reference counting;
- M3 Wasm GC structs;
- the component `cabi_realloc` bump allocator;
- the compiler's function-table span allocator.

Each strategy is selected per compiler profile. No requirement states which operations a strategy must or may support, or how quota refusal, allocator exhaustion, memory-growth failure and engine OOM differ. Nor does any requirement say how allocator provenance and observational equivalence are preserved. `S-MEM-04` permits any internal strategy that preserves the invariants. `BE-CONFIG-01`/`BE-REPORT-01` already separate logical from engine memory, and `H-LIMIT-03` requires an accounting revision. This change adds the missing allocation contract, kept separate from the ownership and lifetime contract of #3. r[ALLOC-CAP-01] r[ALLOC-FAIL-01]

## What Changes

- Inventory every allocation site at 44329dc and give a backend/profile × strategy compatibility matrix. Each cell is marked existing, prospective, unsupported or not applicable. Prospective native routes from #4 appear in separate columns. r[ALLOC-CAP-01]
- Define a minimal capability-specific contract. `alloc` and liveness are common to every strategy. `release`, `mark`/`reset`, instance checkpoint/restore, `compact` and engine-managed release are optional capabilities. Each optional operation has its own lifetime precondition, supplied by the #3 contract (`OWN-RECLAIM-01`, `OWN-SCRATCH-01`), and an explicit refusal when unsupported. The contract also fixes selection granularity, allocator provenance, layout, handle and epoch rules, publication order and reuse statements. r[ALLOC-CAP-01] r[ALLOC-OBS-01]
- Separate four failure classes: logical quota refusal, strategy exhaustion, memory-growth failure and engine/host OOM. Fix their atomicity and cleanup rules. Require instance isolation, cross-instance handle refusal and identical successful values and host traces across compatible strategies. Host jemalloc is never treated as a guest-cell allocator. r[ALLOC-FAIL-01] r[ALLOC-ISO-01]
- Give a reproducible benchmark plan, proposed decision criteria, a conformance plan (ALLOC-CASE-01..12, unexecuted), a recommendation and the remaining evidence. r[ALLOC-EV-01]

## Impact

- **Files**: only this active change: `proposal.md`, `design.md`, `tasks.md`, and ADDED deltas under `specs/backend-experiments`, `specs/safety` and `specs/evidence`. No canonical spec, conformance JSON, view, ledger, roadmap or status file is edited.
- **Coordination**: lifetime preconditions come from `ownership-lifetime-contract` (#3). Native-route columns defer to #4. The calculator snapshot/restore proposal (#17) is a whole-instance checkpoint candidate. Retained-version budgets for live slots (#1) are host-ledger quotas, not guest-cell allocation.
- **Testing**: document validation and the proposal/design/tasks gates only. No benchmark, allocator, conformance case or proof is executed or claimed. No default allocator changes.
