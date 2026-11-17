# Schedule-independence invariants and unsupported cases (#134)

These invariants state what must not depend on evaluation or interleaving order in Noble's
analyses. Each names the mechanism that provides it and the evidence for it: a harness family of
[criteria.md](criteria.md) (F1-F9), an existing test, or a reading of the code recorded in
[inventory.md](inventory.md). A harness family is finite implementation evidence, not a proof.

## Invariants

**I1. Queries are pure.** Every analysis entry point is a function of its explicit arguments and,
for session queries, of the committed session state. The analysis crates hold no global or static
mutable state, no hash-ordered container, no thread, atomic or clock (inventory). Repeating a query
at any position of a schedule, or issuing it concurrently, returns an identical result.
Evidence: F3 (every query twice in every order), F2 and F9 (four barrier-synchronized threads
issuing the same query, eight rounds), F8 (each request twice).

**I2. Session queries are idempotent; commits are the only phase boundary.** `prepare` takes
`&self` and does not change the session; `commit` is the only transition, and it refuses a
preparation made against another generation (`source::Session::commit`,
`ModuleSession::commit`, the `noble-wasm` compiler's `commit`). A query's result depends on the
committed history, never on which other queries ran before it. The one deliberate exception is the
shared request-work pool (I8). Evidence: F3; stale-preparation tests in
`crates/noble-contracts/tests/declared.rs` and `crates/noble-contracts/src/source.rs`.

**I3. Dependency tracking is stable.** A name resolves to the identity committed when its user
was checked. Definition, module, slot and ordinal numbering follows install order and is
presentation (A1). For any two legal commit orders, and for any order of the definitions inside one
module, the per-name results agree: output types, interfaces, the sorted dependents of every
definition, and the results of invoking the exports. Evidence: F4 (all ten legal commit orders of a
diamond plus an independent definition), F5 (all 24 orders of a four-definition module).

**I4. Cycle detection is set-based and names a cycle member.** (a) An input is refused as cyclic
exactly when its dependency graph has a cycle, in every admitted order. (b) A named element lies on
a cycle of the input. (c) The first failure ends the walk. (d) Which member is named, and which of
several cycles, is presentation (A2). Evidence: F1 (eight dependency graphs, every renumbering, both
orders of every dependency list, against independent reachability), F5 (cyclic local definitions:
one fixed-span refusal in all 24 orders), F6 (two-node and self cycles in every admitted node order,
named node checked against reachability). Defect found and fixed: the kernel walk violated (b) and
(c) (README, "Defects found").

**I5. Cache memoization never changes results.** For the specialization cache: (a) keys bind every
input that can affect the result: exact recipe bytes, graph identity, interface, static map,
closure, revisions and every budget and source limit; (b) entries are invalidated only by key
change, so a budget-only change misses rather than hits; (c) eviction changes reuse only, and every
result equals a cold run in a fresh session under the same limits, in every order and under every
limit configuration; (d) refusals are never cached; (e) publication is all-or-nothing, so a declined
publication leaves the retained entries and accounted bytes unchanged; (f) a hit is re-admitted
under the current limits and returns exactly the cold artifacts. Evidence: F8 (six limit
configurations including one-entry, one-variant, tight eviction work, disabled and manifest bypass),
the cache unit tests. Defect found and fixed: (e) (README, "Defects found").

**I6. Exhaustion may replace a verdict, never flip one.** Work, fuel, byte and reservation budgets
are charged in visit order, so where a run gives out depends on the order. An exhausted run in one
order may face a verdict in another; two orders that both complete must agree (A3). Exception: every
complete reduction of an admitted interaction net fires the same number of interactions, so net
interaction fuel refuses identically under every schedule. Evidence: F1 (work 1-3 cases), F7 (fuel
one below the interaction count under all 34 schedules; worklist cap at the smallest high-water
mark).

**I7. One diagnostic per refusal; only its class and soundness are semantic.** Every analysis
returns at most one diagnostic, the first in its fixed stage order and then in input order. The
refusal class (stage and code or kind) and the soundness of the named element are semantic. With two
or more independent defects, which one is reported is presentation (A2), and the reported diagnostic
must equal the diagnostic of one single-defect variant in the same order. Evidence: F5 two-defects
module (12 of 24 orders report the other defect), F6 two-type-defects graph and two-guards rows.

**I8. A shared request-work pool is history-dependent by design.** K-CHECK-07 charges every query,
including refused ones, against one pool, so the remaining balance after a query depends on the
query history (A5); with an ample pool every result agrees. Evidence: F3 module-pool case.

**I9. No mutable analysis state is shared between threads.** Types that hold an `Rc` pool or graph
are `!Send`, and the cache is driven through `&mut self`, so sharing is excluded by the type system,
not by locks. Concurrent requests for one query agree with the serial result both on one shared
kernel environment and on per-worker sessions, graphs and caches. Evidence: F2, F9.

## Allowed differences

| Class | Meaning | Where it occurs |
|---|---|---|
| A1 numbering | identities and slots compared only after mapping | committed definition identities (F4) |
| A2 first defect | which of several defects, or which cycle member, is reported | F1, F5, F6 |
| A3 exhaustion | an exhausted run where another order decided | F1 tight work, F7 worklist cap |
| A4 metadata | cache status, net queue high-water marks | F7, F8 |
| A5 shared pool | remaining pool balance | F3 |

The receipt counts each class; none is a failure.

## Semantic determinism versus bit-for-bit reproducibility

Semantic determinism is the invariant: every admitted order of equivalent inputs yields the same
verdicts, values, interfaces and sound diagnostics. Bit-for-bit reproducibility is a stronger,
narrower property and holds only for **identical ordered inputs**:

- tested byte for byte for identical inputs: printed canonical residual graph text, emitted residual
  source, residual and readmitted graph identities and the specialization and program keys (F8 cold
  versus warm, F9 across threads); the complete `Debug` rendering of a frontend preparation,
  including its history (F3, F9); net observations (F7);
- not reproducible across reordered equivalent inputs, by design: a renumbered graph has a
  different logical identity, layout keys and canonical text; definition and module identities
  follow commit order; component artifacts follow the export slice order (U1).

## Explicit unsupported cases

- **U1 Component artifacts.** `noble_wasm::component::compile` and `noble component compile` keep
  the caller's export order: function order, data-segment offsets and the build-context framing
  follow it, so the same export set in another order yields different bytes for a semantically
  equal component. Only identical ordered inputs are claimed reproducible.
- **U2 Source-compiler session numbering.** Commit order assigns function indices, static literal
  addresses and reuse epochs in the `noble-wasm` compiler; artifacts are comparable only across
  identical commit histories (existing same-input oracles: `crates/noble-wasm/tests/fuzz/oracles.rs`,
  `crates/noble-wasm/tests/calculator_collect.rs`).
- **U3 Node-order identities and capacity accounting.** Graph identities and canonical text follow
  node order, which is why F6 compares only semantic results across node orders. The graph-byte
  budget counts allocated `Vec` capacity, so equal graphs with different allocation histories can
  differ at that boundary (A3).
- **U4 Order-dependent charges.** Declared-module scheduler and lookup charges, `noble-wasm`
  definition-admission rounds, kernel witness fuel and evaluator fuel (per node in the reference
  evaluator, per layer in the compiled one) depend on visit order. Reverse-ordered dependency chains
  cost more scheduler work, so a budget near the boundary can admit one definition order and exhaust
  another.
- **U5 Net reservation caps.** Worklist and arena reservation caps depend on the schedule (A3); the
  `Plan::max_worklist_entries` documentation now says so.
- **U6 Companion registry.** Identities are insertion-ordered, proof admission is not idempotent and
  the capacity cap is met by whichever caller arrives last; not covered by this harness.
- **U7 Runtime state machines.** Kernel async, resource, dataspace and authority tables, the
  syndicate profile and the tick runner treat call order as semantics; their own interleaving lanes
  cover them.
- **U8 Host shell.** The rules-library loader reports the first failing file in `read_dir` order.
- **U9 Concurrency.** No concurrent analysis API, shared session or shared cache exists; a cache
  shared between workers would need per-key single-flight and its own concurrent-request family.
  Parallel execution is not offered ([decision.md](decision.md)).
- **U10 Extracted kernel subject.** The M3 Lean extraction of the kernel (`proofs/m3/NobleKernel`)
  is a historical subject (Rust revision a46f588a) and still contains the pre-fix dependency loop;
  no refinement theorem covers environments with several dependency failures.
