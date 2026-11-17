# Inventory: analyses, retained state and order sensitivity (#134)

This inventory was made by reading the analysis crates at the issue's base commit `62485730`; the
order-variation harness ([README.md](README.md)) then exercised the parts marked *harness* below.
Line references are to that tree. It classifies every piece of retained state as **semantic** (it
can change a verdict, a value or an artifact) or **opportunistic** (memoization or metadata that
must never change one), and separates semantic results from presentation such as which diagnostic
is reported first.

## Pipeline

Source bytes enter the `noble-contracts` frontend (parse, local lowering, aliases, resolution,
preflight, stack/effect inference, symbolic checking, emission). Its candidate is rechecked by the
independent `noble-kernel` acceptance checker, then lowered and emitted to Wasm by `noble-wasm`, or
extracted and admitted as a numeric graph by `noble-ngraph`, whose checked graph feeds the
evaluators, the interaction-net reducer, the specializer and its cache, certificate rewrites and
the packed-schedule validators. `noble-cli` is the effectful shell; `noble-syndicate` is runtime.

No analysis crate (`noble-kernel`, `noble-contracts`, `noble-ngraph`, `noble-wasm`) uses a
`HashMap`/`HashSet`, a global or `static` mutable, a thread, an atomic or a clock. Every loop is an
indexed sweep or an explicit LIFO/FIFO worklist over a `Vec`, or a `BTreeMap` walk in key order, so
each analysis is deterministic for a given input and call history. The only interior mutability is
the opt-in K-CHECK-07 request-work pool (`Rc<Cell<u32>>`, `noble-contracts/src/metering.rs`), which
is single-threaded and session-local. Order sensitivity therefore
comes only from four sources: the order of the *input* (definition, node, export and dependency-list
order), session state that assigns identities in call order, budgets charged in visit order, and
"first refusal wins" diagnostics.

## noble-kernel (independent checker; `no_std`, no caches)

| Analysis | Entry point | Retained state | Order sensitivity | Cycles and diagnostics |
|---|---|---|---|---|
| Acceptance checking of one candidate | `acceptance::check`, `check_with_work`, `check_signed_structure*` (`acceptance/mod.rs:55-94`) | none across calls; per call a `Machine` with the work meter, derivations (visit order) and live sites (encounter order) | preflight stages run in a fixed order (`acceptance/preflight.rs:13-145`); arena scans name the first offending node in arena order, including unreachable nodes (`preflight.rs:490-505`); the body checker names the first failing node in depth-first order | one `Diagnostic` per refusal; effect sets are canonical (sorted), so effect diagnostics name the smallest identity |
| Dependency walk (B-CHECK-02) *harness F1, F2* | `acceptance/validate.rs` `dependencies`, `dep_root`, `dep_step` | per call colour arena and LIFO stack | named definition depends on numbering and edge-list order (presentation); work exhaustion point depends on visit order | **defect fixed (#134):** the root loop continued after a failure over a stale stack and on-path marks, so a later root could name a definition on no cycle and the last error won; the walk now stops at the first failure |
| Schema recursion scan | `validate.rs:207` `schemas` | none | declaration order (first recursive schema) | `RecursiveSchema(id)` |
| Witness resolution (B-CHECK-05) | `words/resolve.rs:54` `bindings`, `follow_chain` | none; every reference re-walks its chain (no memo) | the cycle test precedes the fuel test, so a cyclic witness is `Invalid` or `Exhausted` depending on remaining work | `CyclicWitness` at the node being checked |
| Async task, resource, dataspace and authority tables | `async_tasks::Table`, `resources::Table`, `dataspace::Table`, `authority::Authority` | **semantic** runtime state: slots, generations, facet identities, event queues | identities follow call order; the dataspace `Removed` event order follows `swap_remove` layout; the async primary failure is sticky by design | covered by their own interleaving lanes (`tests/m6-interleaving.rs`, `tests/m7-dataspace.rs`, `tests/m5-authority.rs`), not by this harness |

## noble-contracts (frontend)

| Analysis | Entry point | Retained state | Order sensitivity | Cycles and diagnostics |
|---|---|---|---|---|
| Core source pipeline *harness F3, F4, F9* | `source::Session::prepare(&self, ..)` (`source/preparation.rs:18`), `commit(&mut self, ..)` (`source.rs:1407`) | **semantic**: committed definitions, history, generation (`source.rs:652-666`); definition identity = install index + 1 (`resolution/comparison.rs:210-247`) | `prepare` does not mutate the session; identities and named slots follow commit order (numbering); lookup charges one unit per scanned name (newest first) | first refusal wins in every stage; holes render anonymously (`?stack`/`?value`), so fresh-variable numbering never reaches a message |
| Inference and unification | `inference::infer` (`source/inference/mod.rs:164-231`), `Arena::unify` (`inference/unify.rs:100-125`) | per query arena; fresh terms numbered by visit order (never rendered) | with several defects, which unification failure is popped first can change the message | occurs check: "recursive type or stack equation is not supported" |
| Effect solving | `effects/solve.rs` (`run`, `pass`, `rigid`) | per query | the fixpoint is monotone, so the solution is order-independent; only charged work varies | "cyclic effect equation" |
| Declared-module registration *harness F5* | `ModuleSession::prepare` (`source/declared/state/prepare.rs:12`), `commit` (`:315`) | **semantic**: modules, aliases, `next_module_identity`; work pool `Rc<Cell<u32>>` shared by snapshots (`source/declared.rs:47`) | the definition scheduler installs ready definitions in repeated left-to-right sweeps (`register/definitions/graph.rs:55-155`), so slots, identities and ordinals follow source order; scheduler and lookup charges grow with reverse-ordered dependency chains | cyclic local definitions: one fixed-span `Resolve` refusal that names no definition; type errors: first failing definition in install order |
| Shared request-work pool *harness F3* | `ModuleSession::share_request_work`, `remaining_request_work` (`state.rs:1626-1633`) | **semantic budget** (K-CHECK-07): a refused query still spends it | results depend on query history under scarcity; with ample work only the remaining balance differs | `Exhausted` |
| Size-change closure and proof-dependency walk | `sizechange.rs:1229-1283`, `intrinsic.rs:2220-2370` | per query | verdicts are set-based; the reported cycle or path follows call-site order | "cyclic proof dependency" names the first repeated node met |
| Companion registry | `companion::Core::{admit, begin_admission, complete_admission, derive_compose, replay}` | **semantic** registry: contract and evidence identities are insertion indices; `observations` is an inspection-only cache returning the first entry per subject (`companion/registry/entries.rs:101-113`) | completed proof admissions mint a new evidence identity each time; of two identical derivations exactly one gets `DuplicateEntry`; which caller meets the capacity cap depends on arrival order | premise ancestry must have smaller identities (`rules/premises.rs:33-95`); not covered by this harness |

## noble-ngraph (numeric graphs)

| Analysis | Entry point | Retained state | Order sensitivity | Cycles and diagnostics |
|---|---|---|---|---|
| Graph checking *harness F6, F9* | `check::check`, `check_with` (`check.rs:453-553`) | none; the result is an immutable `CheckedGraph` (holds an `Rc`, so not `Send`) | canonical node order is part of the graph and of its logical identity (`ir.rs` `logical_id`); guards refine later consumers (flow-sensitive); the graph-byte budget counts `Vec` capacity (`check.rs:313-329`) | 3-colour DFS `find_cycle` (`check.rs:630-661`) names a node met again on the current path; one `Diagnostic` per refusal, first in stage then node order |
| Reference and compiled evaluation *harness F6* | `eval::reference` (`eval.rs:370`), `compiled::Compiled::evaluate` (`compiled.rs:441`) | caller-owned `Control` meters; `Workspace` registers (opportunistic, bound to their owner) | reference: first failing guard in canonical order; compiled: the minimum canonical failing index (normalized); fuel is charged per node versus per layer (evaluator-specific by design) | `GuardFailed { node, fact }` |
| Interaction-net reduction *harness F7* | `net::lower`, `validate`, `reduce` (`net.rs:597-643`) | per call arena and active-pair worklist | pair selection is a caller choice (`Schedule::{Lifo, Fifo, SplitMix64}`); observations, interaction counts, final wires and peak live agents are schedule-independent; `PlanStats` and the live queue length are not, so `max_worklist_entries` admission depends on the schedule (documentation corrected for #134) | `SelfWire`; refusals carry no site |
| Specialization and its cache *harness F8, F9* | `specialize::specialize` (`specialize.rs:470`); `specialize::cache::Session::specialize` (`specialize/cache.rs`) | cache entries: **opportunistic**; exact manifests bind recipe bytes, graph identity, interface, static map, closure, revisions and every budget and limit; FIFO eviction; positive-only; hits re-admitted under current limits | statuses, lookup counts and retained bytes depend on request history; results must not | **defect fixed (#134):** a declined publication could still evict entries; publication is now all-or-nothing |
| Certificate rewrites, layout, packed-schedule and permutation validators, belt targets | `cert::{propose, check}`, `layout::check`, `schedule::check`, `permutation::check`, `belt::target::check` | none | rewrite steps must be in strictly increasing node order; the packed validator accepts only the canonical stream; the permutation profile accepts any within-layer order, and the belt target checker sorts events | covered by `tests/schedule_correspondence_dir/permutation.rs`, `tests/belt_target_dir/controls.rs`, `tests/cert/chained/mod.rs` |
| Tick runner and batch rows | `tick::Runner`, `batch::Prepared::run` | **semantic** sequential state (epochs, ticks, run fuel) | sequential by design; the runner is not a concurrency subject (`verification/tick-runner/README.md`) | covered by `tests/tick_runner/` and `tests/batch_rows/` |

## noble-wasm (lowering and emission)

| Analysis | Entry point | Retained state | Order sensitivity | Cycles and diagnostics |
|---|---|---|---|---|
| Kernel-checked lowering and emission | `noble_wasm::compile` (`lib.rs:97`) | none; a fresh signature pool per call | function and type indices follow the candidate arena (Kahn sweep in arena order) and derivation order | Kahn round without progress: `Defective`; one class-only `Diagnostic` |
| Source compiler | `source::Compiler::prepare*` (`source/mod.rs:881-1405`), `commit` (`:1463`) | **semantic** session: function table cursor, first-fit free list, static literal addresses (first seen, never rewound), signature pool, identity ledger | commit order assigns function indices and literal addresses, which also enter the reuse epoch; definition-admission work grows with reverse-ordered chains (`source/admission/dependencies.rs:10-133`) | a stale preparation is refused at `commit`; existing determinism oracles: `tests/fuzz/oracles.rs` |
| Component compilation | `component::compile` (`component/mod.rs:48`) | none | the export slice order sets function order, data offsets and the build-context framing; equivalent export sets in another order give different bytes | first refusing export in slice order |

## noble-cli and noble-syndicate (shell and runtime)

- **Calculator module cache** (`noble-cli/src/calculator/cache.rs:61-200`): opportunistic,
  `HashMap<Vec<u8>, Vec<Entry>>` keyed by source and matched on the exact canonical epoch and input
  types; least-recently-used eviction picks `min_by_key(used)` over unique ticks, so hash iteration
  order cannot choose the victim; positive-only; `Rc<RefCell<..>>`, one per process.
- The other hash containers (`core/live/encoding.rs:312-313`, `core/worker_program_guest.rs:1015,
  1081`) are only inserted into and queried, never iterated into an output.
- Threads exist only for process I/O, sandbox monitoring and the live-slot protocol; no analysis
  runs on them.
- The rules-library loader (`workflow/rules.rs:57-196`) reports the first failing file in
  `read_dir` order, which is file-system dependent; its successful result is sorted by module name.
- `noble ngraph specialize --cache` builds one session per process (`ngraph.rs:636-667`), so the
  CLI is always cold.
- `noble-syndicate` serializes every `Profile` operation under one `Mutex`; its event log follows
  lock order and its choreography `Session` admits exactly one global order. This is runtime
  interleaving, outside the analysis subject of #134.

## Semantic versus opportunistic state

| State | Class | How entries are keyed, invalidated and bounded | Failures retained? |
|---|---|---|---|
| Kernel per-call machine, colour arena, work meter | semantic, per call | not retained | n/a |
| Core `Session` definitions, history, generation | semantic | append-only commits; preparations from another generation are refused at commit | no |
| `ModuleSession` modules, aliases, identities | semantic | committed as one stale-checked transition | no |
| Declared request-work pool | semantic budget | shared by snapshots; debited by every query | consumption, by design |
| Companion registry contracts and evidence | semantic | insertion-ordered; capacity bound | duplicates refused |
| Companion `observations` | opportunistic (inspection only) | first entry per subject digest | no |
| Specialization cache entries | opportunistic | exact manifests; FIFO eviction under entry, variant, byte, lookup and eviction-work bounds; all-or-nothing publication | no (positive-only) |
| Compiled `Workspace`, batch `Prepared` registers | opportunistic | bound to their owner, refused by other compilations | n/a |
| Net `PlanStats`, cache statuses, stage events | presentation metadata | not retained | n/a |
| `noble-wasm` `Compiler` session | semantic | commit only from an unchanged base | no |
| Calculator module cache | opportunistic | exact source, epoch and input types; LRU by unique ticks | no |

## Concurrency surface

There is no concurrent analysis API. Kernel `Env`, `Request`, `Candidate` and `Outcome` hold only
owned data and are `Send + Sync`; F2 checks one shared environment from four threads. The source
`Session`, `ModuleSession` and `ModulePrepared` are `!Send` (they hold an `Rc` pool), as are
`CheckedGraph`, `Compiled`, `Workspace` and `ParamSnapshot`; `IdentityScheme` has no `Sync` bound.
The specialization cache `Session` is plain data but is driven through `&mut self`. A query can
therefore run in parallel only with per-worker state built from plain inputs (`Graph`, source
bytes, requests), which is what F9 does.
