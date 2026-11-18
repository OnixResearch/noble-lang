# Schedule-independent compiler analyses (#134)

Noble's analyses must give the same semantic results whatever order equivalent inputs are
evaluated in, and memoization or dependency tracking must not make results depend on mutation order.
This lane inventories the analyses, states the invariants, tests them with a deterministic
order-variation harness and records whether bounded parallel execution may be proposed.

| File | Content |
|---|---|
| [criteria.md](criteria.md) | frozen comparison rules (revision 1), committed before any measurement |
| [criteria-oracle.md](criteria-oracle.md) | frozen table-level oracle rules (revision 2), committed before its measurement |
| [inventory.md](inventory.md) | analyses, retained state (semantic or opportunistic), order sensitivity, cycles, diagnostics |
| [invariants.md](invariants.md) | invariants I1-I9, allowed differences A1-A5, unsupported cases U1-U10 |
| [decision.md](decision.md) | frozen NO-GO for parallel execution, gate evaluation and reopen conditions |
| [run.mjs](run.mjs) | source-bound receipt runner |
| [receipts/2026-11-16-422283f.json](receipts/2026-11-16-422283f.json) | the harness receipt on clean commit `422283f9` |
| [receipts/2026-11-18-8f360fc.json](receipts/2026-11-18-8f360fc.json) | the table-oracle receipt on clean commit `8f360fc0` |

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
- `retained.rs`: the minimized schedules that diverged before their fixes, replayed as fixtures, and
  the table oracle's planted control.

Every observation of every family also emits one row of the table oracle that
[criteria-oracle.md](criteria-oracle.md) revision 2 freezes: the input digest, the schedule, the
normalized-output digest and the presentation digest. When a family emits its tally it groups its
rows by input digest and requires one normalized output per input; a row whose observation is a
permitted A3 exhaustion carries no normalized output and is counted by class; presentation
differences are counted per input as allowed. A conflict retains exactly two rows of one input with
two schedules and prints them with their replay tokens. The planted control
`oracle-control/position-leak` observes the position of the queried item: the serial and reversed
orders agree on it and only the seeded orders expose it, so the oracle must catch it with exactly
two retained rows.

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

Each family also prints one `NOBLE_SCHEDULE_ORACLE` line with its row, input and
allowed-difference counts, and the planted control prints one `NOBLE_SCHEDULE_ORACLE_CONTROL` line
with the two rows it retains.

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

## Results

### The table-level oracle

The owner follow-up of 2026-11-17 asked for the comparison to be stated as a table. On the clean
commit `8f360fc0` the harness emitted 7,290 rows over 160 input digests: no input digest maps to more
than one normalized output, 52 input digests present an allowed presentation difference, and 479
rows are exempt A3 exhaustions counted by class (275 from the F1 tight-work budgets and 204 from the
F7 worklist cap). The seeded control is caught once and retains exactly two rows, one of them
seeded; both rows re-execute to their recorded digests and the seeded row replays the divergence
through `NOBLE_SCHEDULE_REPLAY=<case>@<order>`.

| Family | Rows | Inputs | Normalized conflicts | Presentation-difference inputs | Exempt rows (A3) |
|---|---:|---:|---:|---:|---:|
| F1 kernel-dependencies | 730 | 32 | 0 | 8 | 275 |
| F2 kernel-threads | 528 | 16 | 0 | 0 | 0 |
| F3 contracts-queries | 828 | 12 | 0 | 4 | 0 |
| F4 contracts-commits | 110 | 10 | 0 | 5 | 0 |
| F5 contracts-modules | 75 | 3 | 0 | 1 | 0 |
| F6 graph-orders | 204 | 6 | 0 | 2 | 0 |
| F7 net-schedules | 1,224 | 18 | 0 | 12 | 204 |
| F8 cache-sequences | 2,898 | 42 | 0 | 20 | 0 |
| F9 threads | 693 | 21 | 0 | 0 | 0 |

A pass is evidence about the sampled table only: the finite relation from input digest to normalized
output over the rows the harness ran. It is not a proof of schedule independence, and it says nothing
about orders outside the admitted set of criteria section 1.

[`receipts/2026-11-16-422283f.json`](receipts/2026-11-16-422283f.json) was produced by `run.mjs` on
the clean commit `422283f9` with the pinned 1.96 nightly toolchain, under criteria revision 1
(SHA-256 `92df81a30516c543af1336b3ae76b54d45f3f585e36cb498963efa61513ffd14`):
`test result: ok. 12 passed; 0 failed`, nine families, zero divergences, two retained minimized
fixtures. *Schedules* counts the admitted orders run, the serial
baseline included (F7: distinct pair-selection schedules; F2 and F9: worker runs).

| Family | Cases | Schedules | Comparisons | A1 | A2 | A3 | A4 | A5 | Divergences |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| F1 kernel-dependencies | 40 | 730 | 730 | 0 | 122 | 30 | 0 | 0 | 0 |
| F2 kernel-threads | 16 | 32 | 512 | 0 | 0 | 0 | 0 | 0 | 0 |
| F3 contracts-queries | 2 | 70 | 828 | 0 | 0 | 0 | 0 | 259 | 0 |
| F4 contracts-commits | 1 | 11 | 110 | 31 | 0 | 0 | 0 | 0 | 0 |
| F5 contracts-modules | 3 | 75 | 75 | 0 | 12 | 0 | 0 | 0 | 0 |
| F6 graph-orders | 6 | 204 | 204 | 0 | 27 | 0 | 0 | 0 | 0 |
| F7 net-schedules | 6 | 204 | 1224 | 0 | 0 | 204 | 126 | 0 | 0 |
| F8 cache-sequences | 6 | 210 | 2898 | 0 | 0 | 0 | 340 | 0 | 0 |
| F9 threads | 21 | 96 | 672 | 0 | 0 | 0 | 0 | 0 | 0 |

## Evidence boundary

The harness is finite implementation evidence over the listed cases and orders. It is not a proof
of confluence, not a Rust-to-Lean refinement, and not a performance measurement. The Lean examples
are kernel-checked facts about the reference model on concrete environments, not a general theorem
about the walk. Analyses outside the harness (inventory rows marked as covered elsewhere) keep their
own lanes; see the unsupported cases in [invariants.md](invariants.md).

The table oracle is the same kind of evidence: a pass is the sampled relation from input digest to
normalized output, not a confluence theorem. Two proved results already cover the net level and stay
separate from it: issue #124's selected revision-3 row (`i64_normal_form_unique`,
`i64_equal_interaction_count`, `parallel_sub_schedules_agree`) and, in the `inets` development,
`MRun.confluent`, which proves the same memory and the same claim count across schedules and thread
counts. Both are about reducing interaction nets, not about compiler analyses, so they do not
discharge this issue and this table does not stand in for them.
