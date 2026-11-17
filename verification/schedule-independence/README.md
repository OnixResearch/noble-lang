# Schedule-independent compiler analyses (#134)

Noble's analyses must give the same semantic results whatever order equivalent inputs are
evaluated in, and memoization or dependency tracking must not make results depend on mutation order.
This lane inventories the analyses, states the invariants, tests them with a deterministic
order-variation harness and records whether bounded parallel execution may be proposed.

| File | Content |
|---|---|
| [criteria.md](criteria.md) | frozen comparison rules (revision 1), committed before any measurement |
| [inventory.md](inventory.md) | analyses, retained state (semantic or opportunistic), order sensitivity, cycles, diagnostics |
| [invariants.md](invariants.md) | invariants I1-I9, allowed differences A1-A5, unsupported cases U1-U10 |
| [decision.md](decision.md) | frozen NO-GO for parallel execution, gate evaluation and reopen conditions |
| [run.mjs](run.mjs) | source-bound receipt runner |

## The harness

The harness is a set of modules of the existing `noble-ngraph` test target `schedule_correspondence`
(`crates/noble-ngraph/tests/schedule_independence/`); it adds no Cargo unit and no production code.
`noble-ngraph` sits on top of the frontend and the kernel, so one test target reaches every pure
analysis crate except `noble-wasm`.

- `mod.rs`: the scheduler. A *case* is a list of items and a runner. Query cases run queries in an
  order (every order repeats every query); renumbering cases rebuild one equivalent input from a
  permutation of its items. The serial order is the baseline. Admitted orders are the reversed
  order, 32 seeded orders (SplitMix64 from base seed `0x5eed_0134`), every permutation for small
  cases, and Kahn topological orders for graphs. Each observation is compared with the baseline and
  is the same, an allowed difference of a class A1-A5 (counted), or a divergence. A divergence is
  minimized greedily (drop queries, or move items back to their serial position while it persists)
  and reported as a replay token `<case>@<order>`.
- `kernel.rs` (F1): the kernel dependency walk under every renumbering of the extra definitions
  and both orders of every dependency list, with work budgets that do and do not suffice, judged
  against an independent reachability reference.
- `contracts.rs` (F3-F5): frontend queries in many orders on one session (and on a shared
  request-work pool), every legal commit order, and every definition order inside a declared module.
- `graph.rs` (F6, F7): graph checking and both evaluators under every admitted node order; the
  interaction-net reducer under Lifo, Fifo and 32 seeded schedules, uncapped, with fuel one below the
  interaction count and with a worklist cap.
- `cache.rs` (F8): specialization requests through one cache session in many orders under six limit
  configurations, each result compared with a cold run, and the cache invariants checked after every
  request.
- `threads.rs` (F2, F9): four barrier-synchronized threads issuing the same query, eight rounds.
- `retained.rs`: the minimized schedules that diverged before their fixes, replayed as fixtures.

The issue's counterexample families map onto the cases as follows.

| Counterexample family | Cases |
|---|---|
| shared dependencies | F1 diamond, wide-diamond, cycle-and-shared-sink; F3/F4/F5 a definition used by two others; F6 shared-diamond; F8 two static maps over one original graph |
| cycles | F1 self-loop-and-entry, two-cycles, chain-to-self-loop, shared-entry-cycle; F5 cyclic local definitions; F6 two-node-cycle and self-cycle |
| partial errors | F3 ill-typed and unresolved queries among valid ones; F5 two-defects module; F6 two-type-defects graph and two-guards rows; F8 two refusing requests among successful ones |
| memoization invalidation | F8 one-entry, one-variant and tight-eviction limits (eviction), a budget-only key change, disabled retention, manifest bypass, declined publication |
| concurrent requests for one query | F2 one shared kernel environment; F9 frontend, graph and cache queries on per-worker state |

### Running it

With the repository's pinned toolchain (for example inside `nix develop`):

```sh
cargo test --offline --locked -p noble-ngraph --test schedule_correspondence --all-features \
  -- schedule_independence --nocapture --test-threads=1
```

Each family prints one `NOBLE_SCHEDULE_FAMILY` line with its case, schedule and comparison counts
and its allowed differences by class. To replay one schedule, run the replay test with the token a
divergence printed:

```sh
NOBLE_SCHEDULE_REPLAY='<case>@<order>' cargo test --offline --locked -p noble-ngraph \
  --test schedule_correspondence --all-features \
  -- schedule_independence::replay_requested_order --nocapture
```

A receipt is produced from a clean commit only:

```sh
NOBLE_CARGO='<pinned cargo>' NOBLE_RUSTC='<pinned rustc>' \
  node verification/schedule-independence/run.mjs '<new file outside the repository>'
```

## Defects found

The inventory pointed at two order-dependent defects in the analyses and one in the Lean reference
model. The harness reproduced both analysis defects before they were fixed, in development runs on
working trees that held the harness and the unfixed source of the analysis (these runs are
observations, not receipts); the minimized schedules are retained in `retained.rs`, and the owning
crates' regression tests failed on the unfixed sources and pass on the fixed ones.

1. **Kernel dependency walk** (`crates/noble-kernel/src/acceptance/validate.rs`, B-CHECK-02). The
   root loop kept going after a failure. The aborted walk left on-path marks and a non-empty stack,
   so a later root could report `RecursiveDependency` naming a definition on no cycle, and a later
   error (including `Exhausted(Work)`) replaced an earlier one. Before the fix the harness printed
   `NOBLE_SCHEDULE_DIVERGENCE {"token":"kernel-dependencies/self-loop-and-entry/forward/work-10000@0,1,2",...,"reason":"named item 0 lies on no cycle"}`
   (the serial order itself: definitions 0→1, 1→1, 2→0). The walk now stops at the first failure,
   as the Lean reference model always did. Regressions: `crates/noble-kernel/tests/recursion_schema_cycle/orders.rs`
   (exact outcomes of the minimized environments and an exhaustive renumbering check) and the
   retained token.
2. **Lean reference model of the same walk** (`proofs/m3/NobleM2/Check.lean`, `depWalk`). A
   finished definition stayed in the on-path list, so an edge to an already finished shared
   dependency was reported as a cycle and the verdict for an acyclic diamond depended on the order of
   its edges: evaluated with the pinned Lean, `validateDeps` of the environment `[[1, 2], [2], []]`
   returned `.depCycle 2` while `[[2, 1], [2], []]` returned `.ok 97`. Finished definitions are now
   skipped before the on-path test, mirroring the kernel's colour test. Kernel-checked examples:
   `proofs/m3/NobleM2/DependencyOrder.lean`.
3. **Specialization-cache publication** (`crates/noble-ngraph/src/specialize/cache.rs`). A
   publication that ran out of eviction work after evicting some entries was declined but kept the
   evictions, destroying reuse it did not replace. Before the fix the harness printed
   `NOBLE_SCHEDULE_DIVERGENCE {"token":"cache-sequences/tight-eviction@2,3,1",...,"reason":"memoization invariant: Some(MissUncached) changed the session from (2, 5834) to (1, 2917)"}`,
   minimized from a seeded order of 14 requests. Publication is now all-or-nothing. Regressions: the
   cache unit tests `declined_publication_keeps_entries_and_accounting` and
   `fitting_eviction_plan_evicts_subject_then_oldest`, and the retained token.

The documentation of `net::Plan::max_worklist_entries` described a per-plan bound; the cap includes
the live queue length, which depends on the schedule, so it now says that worklist admission can
differ between schedules (F7 exercises it as an A3 difference).

## Evidence boundary

The harness is finite implementation evidence over the listed cases and orders. It is not a proof
of confluence, not a Rust-to-Lean refinement, and not a performance measurement. The Lean examples
are kernel-checked facts about the reference model on concrete environments, not a general theorem
about the walk. Analyses outside the harness (inventory rows marked as covered elsewhere) keep their
own lanes; see the unsupported cases in [invariants.md](invariants.md).
