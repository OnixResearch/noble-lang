# Arrow-inspired, resource-safe wiring library and law suite

## Why

Issue #11 asks for a small library of composition and routing combinators over existing `Program<S,T,e>` values. It also asks for the laws those combinators satisfy, an executable test corpus, and a proof plan. Arrows are the guide, but Noble's ordered stacks, latent effects, move-only resources, reflection and exact proof subjects decide which laws are admissible.

The baseline already provides typed `compose`/`run`/`dip`/`case`/`if`, `TypedPC.compose`, and Lean rules for `dip`, `run`, `if` and `case`. Several gaps remain:

- No library vocabulary exists.
- No law states its observation relation.
- Arrow-style claims can silently overreach. Examples are commuting effects, duplicating resources, and equating `[ 1 1 + ]` with `[ 2 ]`.
- Source signatures can name only one latent effect slot and cannot state `Data`/`Capture` premises.

These gaps make the problem concrete. The specification boundaries are K-EVAL-04, K-RES-02, P-RECIPE-03/04, VC-COMPOSE-02, VC-REWRITE-02 and V-OBS-01. r[WIRE-API-01] r[WIRE-LAW-01]

## What Changes

- Specify a `wire@1` library sketch. Every combinator has a complete stack and effect signature and is derived from existing words. Every combinator also has explicit eligibility rules: move-only, or requiring `Data` or `Capture`. The sketch states the evaluation order. It says how checked Programs enter without an `arr` lifting operation. It separates routing owned inputs from fanout of a shared input. r[WIRE-API-01]
- Specify a law matrix over named observation relations: normal result, recipe, ordered trace, ownership, failure/cleanup, quota, and identity/proof subject. Each law lists its premises, its supported profile and any unsupported case. Effect union is a bound, not commutation. Laws are stated modulo quota, and behavioral equality never establishes recipe or subject identity. r[WIRE-LAW-01]
- Specify an executable-law corpus plan with independently stated expectations, negative cases and abnormal-cleanup coverage. Add a proof plan that maps each law to existing Lean rules and lists new model, implementation and correspondence obligations. Finite tests are never reported as proofs. r[WIRE-EVID-01]
- Specify an optional known-graph analysis profile, and analyze one candidate optimization (compose-tree flattening) against reflection, proof identity, ordering and quota behavior. r[WIRE-GRAPH-01]
- Package the first pure laws as an explicit #7 law bundle, `wire@1.seq-laws`, with a worked consumer and refusal cases. Keep the resource-preserving frame bundle separate until its ownership premises and evidence exist. r[WIRE-LAW-01]

## Impact

This change is design-only. It touches only `.cairn/changes/resource-safe-wiring-laws/`: proposal, design, tasks and ADDED deltas for `developer-experience`, `program-contracts` and `verification`.

It does not change the compiler, kernel, runtime, Lean library, conformance JSON, canonical spec, roadmap or status. It creates no library file.

The `wire@1` sketch and the other design-time runs used the prebuilt `noble-44329dc-release` binary on throwaway files. They are recorded as observations, not evidence. All WIRE-CASE designs remain unexecuted. A future sync needs a `WIRE` route in `tools/check-specs.mjs` `ROUTES`. The bundle schema comes from #7 (`reusable-contract-calls`). r[WIRE-EVID-01]
