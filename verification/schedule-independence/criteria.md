# Schedule-independence comparison criteria (revision 1, frozen)

Issue #134. This file is committed **before** any order-variation measurement it governs. A receipt
must name this file's SHA-256 and the clean source commit it ran on. Do not revise these rules after
seeing data: a changed comparison needs a new, separately versioned file, and this revision and any
result it produced stay retained. The criteria decide what the order-variation harness compares and
what counts as a divergence; [decision.md](decision.md) applies section 7 to bounded parallel
execution.

## 1. Admitted orders

An *admitted order* is a different evaluation or interleaving order of equivalent inputs that the
repository's public APIs already accept. The harness never changes production code to obtain an
order. The families and their orders are:

| Family | Analysis driven | Admitted orders | Equivalence of inputs |
|---|---|---|---|
| F1 kernel-dependencies | `noble_kernel::acceptance::check` (B-CHECK-02 dependency walk) on an empty program | every renumbering of the extra definitions, each dependency list forward and reversed | the same abstract dependency graph |
| F2 kernel-threads | `noble_kernel::acceptance::check` | the same request issued concurrently from several threads over one shared environment | identical inputs |
| F3 contracts-queries | `noble_contracts::source::Session::prepare` | serial, reversed and seeded permutations, with exact repeats, of a fixed query set on one session | identical session and query bytes |
| F4 contracts-commits | `Session::commit` then `prepare` | every commit order of the session's definitions that respects their dependencies | the same named definitions |
| F5 contracts-modules | `noble_contracts::source::ModuleSession::{prepare, commit}` | every order of the local definitions inside one declared module | the same named definitions and exports |
| F6 graph-orders | `noble_ngraph::check::check_with`, `eval::reference`, `compiled::Compiled` | every topological node order produced by `ir::GraphBuilder` that keeps each guard before its refined consumers | the same abstract graph and inputs |
| F7 net-schedules | `noble_ngraph::net::reduce` | `Schedule::Lifo`, `Schedule::Fifo` and seeded `Schedule::SplitMix64` | the same lowered net and input row |
| F8 cache-sequences | `noble_ngraph::specialize::cache::Session::specialize` | serial, reversed and seeded permutations, with exact repeats, of a request sequence, under several cache `Limits` | identical requests |
| F9 threads | F3, F6 and F8 queries | the same query run concurrently by several threads, each owning its non-`Send` state | identical plain inputs |

## 2. Semantic comparison (must agree)

For every case, the serial order is the baseline and every other admitted order is compared with it.

1. **Verdict class.** Accepted or normal versus each refusal class (stage plus code or kind).
2. **Complete results.** For accepted runs: output values in output order, checked interfaces and
   output types, derivation records after mapping renumbered identities back, interaction counts,
   final wires and peak live agents for nets.
3. **Generated artifacts.** Printed canonical residual graph text, emitted residual source, and the
   specialization and program keys, compared byte for byte with a cold run in a fresh cache session
   (F8), and with the serial run across threads (F9).
4. **Diagnostic soundness.** A refusal that names an element (definition, node, cycle member) must
   name one that independently has the reported defect: a named cycle member lies on a cycle of
   the input graph (reference reachability computed by the harness), and a diagnostic for an input
   with several independent defects equals the diagnostic of one of its single-defect variants run
   in the same order.
5. **Memoization invariants (F8).** Entry count, per-subject variants and accounted bytes stay
   within the configured limits; a request whose publication is declined (`MissUncached`) leaves the
   retained entries and accounted bytes unchanged; a hit returns exactly the cold result.

## 3. Allowed differences (classified and counted, never failures)

- **A1 numbering.** Session-local definition identities and slots, module identities, node and
  agent indices and graph logical identities of renumbered inputs. Compared only after mapping.
- **A2 first defect.** With two or more independent defects, which one is reported, provided
  section 2 item 4 holds.
- **A3 exhaustion.** A run that exhausts a work, fuel, worklist, arena or byte budget may differ from
  a run in another order that completes. Comparison then requires only that the non-exhausted
  verdicts across all orders agree (exhaustion may replace a verdict, never flip one). Exception:
  net interaction fuel (F7) is compared exactly, because every reduction of the selected rule set
  fires the same number of interactions.
- **A4 opportunistic metadata.** Cache status, lookup counts and retained-byte observations, stage
  events and net `PlanStats`.
- **A5 shared request pools.** The remaining balance of a shared request-work pool after a query
  history.

## 4. Divergences and the defect rule

A divergence is any section 2 failure, any panic, or a section 3 class used outside its stated
condition. Every divergence is a defect: the harness reports it with a replay token, the case is
minimized, the minimized case is retained as a committed regression fixture, and the defect is fixed
before a receipt is produced. A receipt with a nonzero divergence count is not retained as passing
evidence.

## 5. Coverage minimums

- F1 and F5: every permutation (cases with at most four permuted items, 24 orders).
- F3, F4, F6 and F8: the serial order, the reversed order and at least 32 seeded permutations per
  case (or every admitted order when there are fewer), with seeds derived from the base seed
  `0x5eed_0134` by SplitMix64. For F6 the serial order is the canonical node order, the reversed
  order is the topological order that always takes the last ready node, and a seeded order takes a
  seeded choice among the ready nodes.
- F7: Lifo, Fifo and at least 32 seeded SplitMix64 schedules per graph, plus a fuel cap one below
  the interaction count.
- F2 and F9: at least four threads, each issuing the same query, in at least eight rounds per case.
- The counterexample families of the issue must each have at least one case: shared dependencies,
  cycles, partial errors, memoization invalidation (eviction, limit and budget changes, declined
  publication) and concurrent requests for the same query.

## 6. Replay and minimization

Every schedule is identified by its family, case, label, seed and explicit order. A failing schedule
prints a `NOBLE_SCHEDULE_DIVERGENCE` line with that token after greedy minimization (dropping
queries, then reducing the order while the divergence persists). Setting `NOBLE_SCHEDULE_REPLAY` to
a token reruns exactly that schedule. Minimized failures are retained as fixtures in the harness and,
for a fixed defect, as a regression test in the owning crate.

## 7. Parallel execution qualification gates

Bounded parallel execution of these analyses may be proposed as an opt-in mode only when, on one
clean committed source, all four gates are measured and pass:

- **G1 correctness.** The harness reports zero divergences for F1-F9 at the section 5 minimums,
  including the threaded families.
- **G2 isolation.** Every analysis that runs in parallel takes only shared references to `Sync`
  inputs; any memo shared between workers is either single-owner or provides per-key single-flight
  with its own concurrent-request family passing G1. No new interior mutability in the analyses.
- **G3 throughput.** In an optimized build of the pinned toolchain, a benchmark preregistered in its
  own frozen file shows a median wall-clock speedup of at least 1.5x with four workers over the
  serial path on the retained corpus (five repetitions, otherwise idle host), and the serial path
  regresses by no more than 5%.
- **G4 artifact reproducibility.** For identical ordered inputs every generated artifact is
  byte-identical between serial and parallel execution. For equivalent but reordered inputs only
  semantic equality is required, unless the artifact is canonicalized.

GO requires all four. Otherwise the decision is NO-GO and parallel execution stays unavailable;
any later mode stays opt-in until a frozen review of G1-G4 qualifies it.

## 8. Non-claims

Finite order-variation tests are implementation evidence for the listed cases, not a proof of
confluence, a Rust-to-Lean refinement, or a performance result.
