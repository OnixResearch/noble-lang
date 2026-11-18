# Decision: bounded parallel analysis execution is NO-GO (#134)

**Decision (frozen): NO-GO.** This change adds no parallel execution mode, no concurrent API and
no shared cache to any Noble analysis. Parallel execution stays unavailable; a future mode must be
opt-in and must first qualify under the four gates of [criteria.md](criteria.md) section 7, which
were committed before any measurement. Correctness of the existing serial analyses across
alternative orders is a separate result and is GO: the order-variation harness passes after two
defects it found were fixed (README, "Defects found").

## Gate evaluation

| Gate | Requirement (criteria section 7) | Evidence | Result |
|---|---|---|---|
| G1 correctness | zero divergences for F1-F9 at the section 5 minimums, threaded families included | [receipts/2026-11-16-422283f.json](receipts/2026-11-16-422283f.json) (`run.mjs` on clean commit `422283f9`): nine families, zero divergences, two retained minimized fixtures; and [receipts/2026-11-18-8f360fc.json](receipts/2026-11-18-8f360fc.json) (clean commit `8f360fc0`): the table oracle of [criteria-oracle.md](criteria-oracle.md), 7,290 rows over 160 input digests with zero normalized-output conflicts | **pass** |
| G2 isolation | parallel analyses take only shared references to `Sync` inputs; shared memos are single-owner or single-flight; no new interior mutability | the kernel checker qualifies (F2 shares one environment between four threads). The frontend `Session`/`ModuleSession` and the checked graph types are `!Send` (`Rc`), `IdentityScheme` has no `Sync` bound, and the specialization cache is a single-owner `&mut` session with no single-flight (inventory, "Concurrency surface") | **fail** for any mode that shares frontend, graph or cache state; met only by per-worker state |
| G3 throughput | a preregistered optimized-build benchmark with a median speedup of at least 1.5x at four workers and at most 5% serial regression | no such benchmark exists and none was run; debug-build test timings are not a throughput measurement | **not measured: fail** |
| G4 reproducibility | byte-identical artifacts between serial and parallel runs of identical ordered inputs; only semantic equality for reordered equivalent inputs | F9 compares residual graph text, residual source, graph identities and cache keys across four threads byte for byte; reordered equivalent inputs change identities by design (invariants, "Semantic determinism versus bit-for-bit reproducibility") | **pass** for the covered artifacts |

GO requires all four gates; G2 and G3 fail, so the decision is NO-GO.

The table oracle is evidence for G1 only, and only about the sampled table: 7,290 rows over 160
input digests, no input with more than one normalized output, 52 inputs with an allowed presentation
difference and 479 exempt A3 exhaustion rows. It is not a confluence proof, it does not cover orders
outside criteria section 1, and it changes no gate: G2 and G3 still fail, so the decision stays
NO-GO. The proved net-level results (issue #124's revision-3 row and `inets` `MRun.confluent`) are
about interaction-net reduction, not about compiler analyses, so they neither qualify a gate here nor
discharge issue #134.

## Semantic determinism and bit-for-bit reproducibility

The qualification separates two properties. **Semantic determinism** (G1) is required of every
analysis: all admitted orders of equivalent inputs give the same verdicts, values, interfaces and
sound diagnostics, with only the classified differences A1-A5. **Bit-for-bit reproducibility** (G4)
is required only for identical ordered inputs; artifacts that encode input order (graph logical
identities, definition and module numbering, component export order) are not expected to be equal
across reordered inputs and are compared after mapping or not at all.

## What a qualifying proposal must bring

A bounded proposal is limited to independent queries over per-worker state built from plain inputs
(for example: checking or specializing several independent graphs, each worker owning its own
checked graph, compiled form and cache session), with scoped threads, no work stealing between
sessions and the serial path unchanged. It must:

1. add a preregistered benchmark (its own frozen file) and measure G3 in an optimized pinned build;
2. extend the harness with the proposal's own parallel family and rerun F1-F9 at the section 5
   minimums on one clean commit (G1);
3. for any shared memo, specify per-key single-flight, its failure and cancellation behaviour, and
   add a concurrent-request family for it (G2);
4. keep the mode opt-in and record a new versioned decision.

## Reopen conditions

Reopen this decision only with a new versioned decision file when all of the following hold: a
concrete bounded design as above exists; the G3 benchmark is preregistered before it is run; the
harness passes on the same clean commit; and any shared state has a single-flight design with its own
order-variation family. A change to the frozen criteria needs a new criteria revision; this revision
and its result stay retained.
