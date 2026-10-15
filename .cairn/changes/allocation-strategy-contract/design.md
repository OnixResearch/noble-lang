## Context

Issue #5 asks for research and design only. It does not select an allocator, change semantics or request an implementation. Every citation below was re-read at `44329dc`; the issue's links point at `0ea10d3`. Labels used throughout:

- **Design argument**: written reasoning.
- **Planned test**: an unexecuted design for a test.
- **Benchmark plan**: an unexecuted measurement design.
- **[INFERENCE]**: a claim not observed in source.

Nothing here is measured, executed or proved.

### Drift from the issue's 0ea10d3 links

| Issue citation | At 44329dc |
|---|---|
| `linear-storage.wat`: "up to 4,096 fixed 48-byte cells" | Address mapping moved to `runtime/linear-address.wat:1-8` (ordinary cells 1..4096) and `runtime/calculator-linear-address.wat:1-20` (Calculator-v1 cells 1..72,362 across three disjoint regions). `storage_ensure` now checks `$cell_limit` and computes pages from the physical address (`linear-storage.wat:45-69`). |
| `accounting.wat` reservation/quotas and suffix rollback | Unchanged except the constant 4096 became `$cell_limit` (`accounting.wat:16-28`). `$rollback` (`:30-36`) has only one caller, the M3 experiment's `cleanup_internal` (`common.wat:31-37`). The source ABI never rolls back (`source.wat:1-3, 23`). |
| `live-linear-storage.wat` free list, epochs, root counts, deferred RC | Unchanged |
| `gc-storage.wat` engine structs and root array | Unchanged. It is included only by the M3 `wasm-experiment` emitter (`crates/noble-wasm/src/emit.rs:11-20, 36-41`; `crates/noble-cli/src/backend.rs:27-31`). |

### Allocation inventory at 44329dc (source reading)

| Site | Profile/selection | Strategy | Release/reuse | Failure path |
|---|---|---|---|---|
| A1 ordinary source cells | `Compiler::new`/`new_live`/`new_text_cursor` (`source/mod.rs:476-521`); runtime fragments `accounting.wat` + `linear-address.wat` + `linear-storage.wat` (`source/emit/imports.rs:143-163`); memory fixed at 16/16 pages (`imports.rs:11-17`) | fixed 4,096 × 48-byte bump pool (`accounting.wat:16-28`) | never within a session. `begin` sets `heap_baseline` to the live count (`source.wat:23`); README "Persistent sessions" says the arena "does not reclaim individual cells" | `$quota(1)` on cell/byte limit or failed `storage_ensure` (`accounting.wat:19-24`); terminal failure poisons (`engine.mjs:1012`) |
| A2 Calculator-v1 cells | `Compiler::new_calculator`; `cell_limit` 72,362, `allocation_ceiling` 1,376,208 (`imports.rs:56-60`); memory 16→64 pages | three-region bump pool with `memory.grow` (`linear-storage.wat:45-69`) | order-preserving sliding compaction at submission boundaries. Roots are operand slots only, and nothing moves if a reachable Program runs compiled code (`calculator.wat:136-150, 289-372`; `calculator-v1/design.md:60`) | as A1. A `memory.grow` failure is reported as the same `$quota(1)` |
| A3 in-process live (Live-Wasm-Draft) | `new_live`; same fragments as A1 | A1 bump pool in a shared fixed-size arena | host whole-instance `snapshotLive`/`rollbackLive` of memory, table, globals and counters. It refuses a grown arena (`engine.mjs:693-742`) | publication failure rolls back and still poisons (`engine.mjs:744-793`, per audit) |
| A4 live-slot cells | `Compiler::new_live_slots`; `live-accounting.wat`, `live-linear-storage.wat`, `live-borrows.wat`, `live-stack.wat` (`imports.rs:122-134`); memory 20/20 (`slot-engine.mjs:52-55`) | 4,096-slot pool. Free list, handle `(epoch<<12)|(slot-1)`, u16 sidecar counters (`live-linear-storage.wat:1-10, 78-104`) | deferred RC: zero-count scan plus cascade (`:245-287`). An epoch-exhausted slot retires permanently (`:88-91, 113-118`) | `$quota(1)` (`live-accounting.wat:33-38`). Ownership refusals return 4 (`live-linear-storage.wat:153-240`). Invariant breaches hit `unreachable` |
| A5 M3 managed linear | `noble wasm-experiment managed-linear-memory` | A1 pool | `begin`/`cleanup` roll back to the module-init baseline (`common.wat:31-37, 68-86`) | as A1 |
| A6 M3 Wasm GC | `noble wasm-experiment wasm-gc` | engine `struct.new` plus a 4,096-entry root array (`gc-storage.wat:1-16, 35-81`) | discard nulls the root. "Dropping a root does not promise immediate collection or physical reclamation" (`:46-52`) | logical 48-byte charge only (`:54-55`). Engine allocation failure is outside the guest's accounting |
| A7 component canonical ABI | component runtime | byte bump `cabi_realloc` with align ≤ 8, power of two (`component/runtime.wat:33-49, 58-70`) | `noble$cleanup` resets the heap to 65,536 (`:23-26`) | quota sets `$quota_exceeded` and then traps (`:42-46`) |
| A8 function-table code spans | `Compiler::retire_code` (`source/mod.rs:855-909`) | coalescing free list of table spans in a 16,384-entry shared table (`abi.json:6`) | only after `code_owners == 0` and table clear (`slot-engine.mjs:705-733`); `retention_revision` refuses at `u64::MAX` | partial table clear poisons the session (`slot-engine.mjs:719-727`) |
| A9 host process | Node 24.13.0 / V8 13.6 (`runtime/config.json:8-11`); Rust CLI | engine and system allocators | engine GC | process/worker death |
| A10 prospective native | `c-backend` CB-VALUE-01/CB-LIMIT-01 (no allocator named); #4 routes | none selected | — | — |

The `accounting.wat` seam is real but narrow. `$reserve` covers quota and accounting. `$storage_ensure`, `$storage_take_slot`, `$storage_discard` and `$new` belong to the storage fragment. The `metric` export reads both. This is a compile-time fragment boundary, not a runtime-replaceable interface. Suffix `$rollback` is safe only because M3 has no session-persistent values. **Design argument:** it is not a general safe reset.

## Decisions

### Decision: Minimal capability-specific allocation contract

**Choice:** Each storage strategy declares a fixed capability set. Every operation runs inside one instance (or one native region) and is recorded with the profile's accounting revision.

| Operation | Capability | Lifetime precondition (from #3) | Postcondition / accounting |
|---|---|---|---|
| `alloc(cell48 \| bytes(len, align))` | all | none | Either a fresh handle whose header is published only after every field is initialized (`linear-storage.wat:102-104`), or one classified refusal (next decision). Charges logical cells/bytes before any publication. No child handle is inspected after a failed reserve (`linear-storage.wat:80-87`). |
| `is_live(h)` / field reads | all | none | A stale, foreign-epoch or out-of-range handle fails closed. It is never reinterpreted as a different live cell. |
| `release(h)` | **R** reclaimable cells/slabs | `OWN-RECLAIM-01`: no counted owner of any kind | Handle is stale forever: epoch advance or permanent slot retirement. Logical release counted. Physical memory unchanged. |
| `mark() → m`, `reset(m)` | **Z** arena/rollback | `OWN-SCRATCH-01`: no retained owner references storage above `m` | Marks are LIFO. `reset(m)` also invalidates every inner mark. It is all-or-nothing. It never rolls back host requests or effects. |
| `checkpoint()`, `restore(c)` | **K** instance snapshot | no in-flight frame; the host ledger is snapshotted with the memory | Byte-exact memory, table, globals and host counters restored and verified (`engine.mjs:710-742`). Refused if size changed. Never rolls back external effects. |
| `compact(roots)` | **C** relocating | every root is known and rewritable; no raw handle is held outside them; no reachable compiled Program reads a moved cell through a global | Order-preserving forwarding keeps edges backward (`calculator.wat:136-150`). Every pre-compaction handle not returned as a root becomes stale. |
| `drop_root(h)` | **G** engine-managed | `OWN-RECLAIM-01` for the logical slot | Logical release only. Physical reclamation is unobservable and unpromised (`gc-storage.wat:46`). |

An unsupported capability MUST refuse explicitly. For example, no strategy pretends that `release` is individual free. **Selection granularity:** the strategy is fixed per *profile at compile time*, as today through the `Compiler` constructor and runtime fragment assembly. Numeric limits are fixed per *instance* at `begin`. Regions inside one instance belong to the same declared strategy, like the calculator's three address ranges. A per-region native strategy is allowed only if each handle carries a region provenance. **Provenance:** every live object keeps the strategy and instance that issued it. `release`, `reset` and `compact` refuse a handle of other provenance. Wasm handles are instance-private indices, and the id-word check does not identify the instance. **[INFERENCE]** Cross-instance misuse protection therefore depends on the host never passing raw handles across instances, as the host's per-runtime maps do (`slot-engine.mjs:1040-1043`). r[ALLOC-CAP-01]

**Layout and handles:** keep the existing 48-byte cell (`kind, id, payload:i64, a, b, c, x, y, z, w, n`; `engine.mjs:208`). Cells start at 65,536 with a stride of 48, so every cell is 16-byte aligned and the `i64` payload sits at offset 8. Range checks run before any subtraction or multiplication (`linear-address.wat:1-8`). Relocation is allowed only for capability C, and handles are then valid only until the next boundary, as in the calculator README. Epoch-qualified handles never wrap. A 32-bit handle carries a 20-bit epoch, and exhaustion permanently retires that slot (`live-linear-storage.wat:1-3, 88-91, 113-118`). A different layout requires a separate profile, consistent with BE-LAYOUT-01.

**Reuse statement (r[ALLOC-OBS-01]):** each strategy documents when released capacity becomes reusable:

| Strategy | Released capacity becomes reusable |
|---|---|
| A1 | never within a session |
| A2 | after the next boundary compaction |
| A3 | only through restore |
| A4 | immediately after collection, except epoch-retired slots |
| A5 | at `cleanup` |
| A6 | logical slot after rollback; physical reuse is engine-defined |
| A7 | at `noble$cleanup` |
| A8 | after table clear and coalescing |

**Rationale:** Real strategies differ in what they can safely do. A single `free` interface would force arenas and GC to fake operations, which the issue rejects. Lifetime preconditions belong to #3, so allocation never decides that storage is dead.

### Decision: Failure classes, atomicity and cleanup

**Choice:** Distinguish four failure classes in diagnostics and metrics (r[ALLOC-FAIL-01]):

| Class | Example source today | Determinism |
|---|---|---|
| F1 logical quota refusal | `next > cell_limit` or bytes over `allocation_limit` (`accounting.wat:19-21`) | deterministic per accounting revision |
| F2 strategy exhaustion | live-slot pool full or epoch-retired slot (`live-accounting.wat:33-35`; `live-linear-storage.wat:88-91`) | deterministic for the same allocation/release sequence |
| F3 memory-growth failure | `memory.grow` returning −1 below the declared maximum (`linear-storage.wat:66-69`) | engine/OS dependent |
| F4 engine/host OOM | V8 heap exhaustion, process kill, engine GC allocation failure (`gc-storage.wat:54-55`) | outside guest accounting |

**Source finding:** F1, F2 and F3 are all reported today as quota reason 1 (`accounting.wat:23-24`; `live-accounting.wat:36-38`). The contract requires distinct classes. It does not require any particular code values.

Atomicity rules:

- A failed `alloc` publishes no cell and inspects no child.
- A multi-cell construction that fails leaves no partially built value reachable from a root. Today this holds because failure is terminal (`source.wat:1-3`; `engine.mjs:1012`).
- The observable host-request prefix is retained and never rolled back.
- Cleanup after a trap, cancellation or failed construction follows `OWN-SCRATCH-01`, through host discard of the instance or region.
- Storage cleanup never performs a host-resource release (`BE-RESOURCE-01`).

**Rationale:** Benchmarks and replay claims (`H-LIMIT-03`) need deterministic classes F1 and F2 kept apart from nondeterministic F3 and F4. Otherwise allocator changes could silently change reported outcomes.

### Decision: Isolation, determinism and observational equivalence

**Choice:**

- Successful values, recipes, identities, effect order and host request/response traces MUST be identical across compatible strategies for the same accepted program and inputs.
- Quota outcomes may differ only through the declared accounting revision (`BE-LIMIT-01` precedent).
- Logical quotas stay deterministic and independent of physical allocation timing.
- Instance and region isolation and checked access are preserved.
- Reentrancy: one guest thread per instance; the host serialises ownership operations (`storage_busy`, `live-linear-storage.wat:45-47`).
- A new native allocator is a new trusted native boundary and needs #4 approval.

Changing the host process allocator (system malloc versus jemalloc) changes only A9 or a native A10 heap. It never changes guest cells in Wasm linear memory. A guest jemalloc would need a Wasm build of jemalloc, or a separately designed host/guest boundary. Wasm GC objects stay engine-managed. r[ALLOC-ISO-01]

**Rationale:** The issue requires that allocator choice never change program values or host-effect semantics. Making equivalence normative is what lets differential tests check any later strategy.

### Decision: Backend/profile × strategy compatibility matrix

**Choice:** Legend: **E** exists at 44329dc · **P** prospective/compatible, gated by the named precondition · **U** unsupported (reason) · **—** not applicable.

| Strategy ↓ / Backend → | Ordinary source (A1) | Calculator-v1 (A2) | In-process live (A3) | Live slots (A4) | M3 linear (A5) | M3 Wasm GC (A6) | Component ABI (A7) | #4 Cranelift via Wasmtime | #4 direct native / C (A10) |
|---|---|---|---|---|---|---|---|---|---|
| Fixed-size pool, no reuse | E | E (3 regions, growable) | E | E (cursor before free list) | E | E (logical root array) | E (byte bump) | P (same Wasm guest semantics; the engine changes, not the strategy) | P (48-byte pool for representation parity) |
| Pool with slot reuse + stale-handle epochs | P (`OWN-RECLAIM-01`; needs counted owners) | U (compaction model; handles valid until boundary) | P (needs RC; BE-LIVE-01 forbids overwriting old callable slots) | E | P | — (engine) | U (bump ABI) | P | P |
| Invocation-local arena, bulk reset | P (`OWN-SCRATCH-01`; requires a scratch/retained split, absent today) | P | P (old Program cells must stay put) | P (redundant with RC) | E (reset to baseline) | P (null roots) | E (`noble$cleanup`) | P | P |
| Checkpoint/rollback marks (nested) | U today (source ABI never rolls back); P only for a proven non-escaping suffix | U today | U (shared arena; old slots are retained) | U (free-list reuse breaks suffix order) | E (single mark only) | P | U (no marks) | P | P |
| Individually reclaimable cells/slabs | P (`OWN-RECLAIM-01`) | U (compaction instead) | P | E (deferred RC) | P | — (engine) | U | P | P |
| Compacting/relocating | P (same calculator preconditions) | E (boundary only) | U (host-held handles; old table slots fixed) | U (host saved tokens are raw handles) | P | — (engine may move) | U | P | P |
| Whole-instance snapshot/restore | P | P (#17 proposal 2A; grown memory refused today) | E (`engine.mjs:693-742`) | P (needs registry snapshot) | — | U (GC references are not byte-snapshottable) | U | P (Wasmtime store snapshot is not selected) | P |
| Engine-managed GC objects | U (BE-LIVE-01 feature set excludes WasmGC for live; ordinary is linear) | U | U | U | — | E | U | P (if Wasmtime GC is enabled; unselected) | — |
| Native system allocator (malloc API) | U as guest allocator (host only) | U | U | U | U | U | U | host/engine only (A9-equivalent) | P (backing for pools/arenas, behind the contract) |
| jemalloc | U as guest allocator; P for host process only | U | U | U | U | U | U | P for host/engine process only | P where the platform and ABI permit; never changes the guest representation |

**Rationale:** The matrix shows most combinations as P rather than E. Each P depends on a lifetime precondition owned by #3 or on a route decision owned by #4. That dependency is why no abstraction should be selected before measurement.

### Decision: Reproducible benchmark plan and decision criteria

**Choice (Benchmark plan, unexecuted):**

Workloads:

| ID | Workload |
|---|---|
| W1 | quote/compose-heavy: left-, right- and balanced trees of 8–256 leaves, the BE-LIMIT-01 shapes |
| W2 | 10,000 short repeated roots |
| W3 | N saved/shared Programs released in seeded random orders |
| W4 | long session: 1,000 submissions with definitions and replacements |
| W5 | mixed lifetimes: scratch work interleaved with saved captures |
| W6 | quota exhaustion to each F1/F2 limit |
| W7 | abnormal exits: trap at seeded points; worker kill for cancellation |
| W8 | Calculator-v1 multi-definition session (#17 baseline) |

Controls:

- Hold the 48-byte representation, checked workload bytes, declared limits and accounting revision constant.
- Use the pinned Node/V8 and wasm-tools from `runtime/config.json`.
- For native comparisons, hold the backend and representation constant and vary only the allocator. Run system allocator versus jemalloc within one native route, never across routes.

Record:

- source and tool revisions, profile, strategy and capabilities;
- hardware, OS, governor and core pinning;
- warm/cold state and V8 cache policy;
- at least 30 samples per point, with median, p99 and bootstrap 95% CI.

Measure:

- throughput and tail latency;
- per-operation alloc, release and reset cost;
- peak logical cells/bytes (metrics 2–5);
- committed pages (metric 10) and engine heap and RSS, reported separately;
- fragmentation = (committed − live logical) / committed;
- allocator metadata bytes (for example the A4 sidecar of 24,576 B for 4,096 cells);
- cleanup latency after W7.

A logical release counter is never reported as physical memory return.

**Proposed decision criteria (to be ratified with the measurements):** replaceability earns its complexity only if all of the following hold:

- D1: at least two profiles need the same non-trivial capability with identical lifetime preconditions.
- D2: the candidate improves throughput, p99 or peak retained memory by ≥20% on at least two workloads, with non-overlapping 95% CIs.
- D3: every applicable ALLOC-CASE passes on both strategies with identical values and traces.
- D4: no new trusted native boundary is added without #4 approval.

jemalloc versus the system allocator is considered only inside a selected native route. It needs a ≥10% improvement in RSS or p99 on W4/W5 with no conformance regression; otherwise keep the system allocator.

### Decision: Conformance plan (unexecuted)

**Choice (Planned tests):** every case is `implementation: absent`, `execution: not-run`, `proof: open`, `trust: unassessed`, `evidence: []`. None is registered in `specs/conformance`.

| ID | Input | Expected observation |
|---|---|---|
| ALLOC-CASE-01 alignment and limits | allocate up to `cell_limit`/`allocation_limit` in each profile; mixed `bytes(len, align)` with align 1–8 in A7 | handle-to-address within the region, 8-byte-aligned payloads, no overflow before the range check; exactly the limit succeeds and limit+1 refuses as F1 |
| ALLOC-CASE-02 failure injection | inject F1 (limit), F2 (pool/epoch full), F3 (a harness memory whose growth fails below the declared maximum), F4 (cgroup memory limit on the worker) | four distinct classifications; no partially published cell; host prefix retained |
| ALLOC-CASE-03 nested rollback | marks m1 < m2: reset m2 then m1; reset m1 directly; failure injected mid-reset | LIFO invalidation; m2 refuses after m1 reset; all-or-nothing reset; no host request rolled back |
| ALLOC-CASE-04 reset with escaping captures | root returns a Program capturing scratch data; reset requested | reset refuses or promotion precedes it; the captured value reads identically (`OWN-CASE-08`) |
| ALLOC-CASE-05 shared-owner release order | two owners of one Program released in both orders (`OWN-CASE-02`) under A4 and any R-capable candidate | storage released after the last owner only; identical final live count |
| ALLOC-CASE-06 handle reuse and epoch exhaustion | drive one A4 slot to its maximum epoch by allocate/release cycles | the slot retires permanently; every earlier handle fails liveness; no wrap |
| ALLOC-CASE-07 cross-instance misuse | pass a live handle from instance X to instance Y's host operations | refusal at the host boundary; no access to Y's cell of the same index |
| ALLOC-CASE-08 saved old definitions | `def n [ 1 ]`, saved `[ n ]`, `def n [ 2 ]` under each strategy (`OWN-CASE-07`) | saved Program yields `1`; no code span or cell reclaimed while owned |
| ALLOC-CASE-09 traps mid-construction | trap between reserve and header publication; trap during a multi-cell build | no live partial cell; instance poisoned or region discarded; frame pins released once |
| ALLOC-CASE-10 cancellation | worker kill during W5 | host ledger releases owners; no stale handle is reused by a new worker |
| ALLOC-CASE-11 differential equivalence | W1–W5 programs under each pair of compatible strategies (A1/A5/A4; A3 with restore) | identical results, recipes, identities, effect order and host traces; quota outcomes differ only under declared accounting revisions |
| ALLOC-CASE-12 metric honesty | release cells then read metrics 3, 4 and 10 | logical release grows; committed pages are unchanged in linear profiles; no physical-return claim |

### Decision: Recommendation

**Choice:**

1. Do **not** add a general runtime-replaceable allocator layer now. Keep compile-time per-profile strategy selection. Document the existing `$reserve`/storage-fragment seam as the minimal contract with explicit capability flags. Default allocators are unchanged.
2. First adopt the contract obligations that cost little and close real gaps: the F1–F4 split, provenance and cross-instance refusal, explicit capability refusal, a reuse statement and an accounting revision per strategy.
3. Measure A1–A7 under W1–W8 before adding strategies. Candidates, in order:
   - (a) a whole-instance checkpoint/restore for Calculator-v1 (#17), which needs no lifetime change;
   - (b) a scratch/retained split for ordinary sessions, gated by `OWN-SCRATCH-01` and #3's promotion choice;
   - (c) slot reuse with RC outside live slots, only if W4 shows the 4,096-cell bound binding.
4. Defer jemalloc. It is relevant only to a selected #4 native route and to host-process memory, never to Wasm guest cells.
5. Wasm GC remains an M3 experiment and is incompatible with the live feature set.

**Rationale:** The current strategies already cover the issue's strategy families. The missing pieces are the contract, classification and evidence, not another allocator.

## Risks / Trade-offs

- Splitting F3 from F1 requires an engine-visible distinction that `memory.grow` returning −1 provides only for growable profiles (A2). Fixed-size profiles cannot observe F3.
- Provenance by host maps is only as strong as the host. A native per-region allocator would need tagged handles.
- Snapshot/restore costs O(memory) per checkpoint. With growable memory it needs restore support for size changes, which is refused today.
- The proposed thresholds D1–D4 are placeholders until measurements exist.

### Remaining evidence before any recommendation becomes a selection

1. Executed W1–W8 baselines for A1–A7 with the recorded controls.
2. ALLOC-CASE-01..12 executed against each strategy claimed compatible.
3. #3 decisions on promotion form and OWN-CASE-06.
4. #4 route selection before any native or jemalloc evaluation.
5. Separate proofs or correspondence for any strategy that claims `OWN-RECLAIM-01`. Bounded tests are not proofs.

## Acceptance mapping

| Issue #5 checkbox | Section |
|---|---|
| Backend/profile × strategy compatibility matrix, explicitly marking unsupported and prospective combinations | Decision: Backend/profile × strategy compatibility matrix; Context → Allocation inventory |
| Minimal allocation contract with capability-specific operations, lifetime preconditions, and failure/accounting semantics | Decision: Minimal capability-specific allocation contract; Decision: Failure classes, atomicity and cleanup; Decision: Isolation, determinism and observational equivalence; r[ALLOC-CAP-01] r[ALLOC-FAIL-01] r[ALLOC-OBS-01] r[ALLOC-ISO-01] |
| Reproducible benchmark plan and decision criteria showing when replaceability earns its complexity | Decision: Reproducible benchmark plan and decision criteria |
| Conformance plan covering alignment and limits, allocation-failure injection, nested rollback, reset with escaping captures, shared-owner release order, handle reuse/epoch exhaustion, cross-instance misuse, saved old definitions, traps/cancellation, and matching values/host traces across compatible strategies | Decision: Conformance plan (ALLOC-CASE-01..12) |
| Recommendation and remaining evidence needed, coordinated with ownership/lifetimes in #3 and backend evaluation in #4 | Decision: Recommendation; Risks / Trade-offs → Remaining evidence; r[ALLOC-EV-01] |
