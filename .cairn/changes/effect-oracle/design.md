## Context

This is a design-only change for issue #28. It specifies an independent finite-model oracle for the **existing** frontend effect solver. It implements no oracle, executes no case, changes no production behavior and claims no soundness or completeness result. It is an implementation/testing subtask of #15 and does not depend on the subtyping (#29) or semantic-merge (#30) designs.

**Baseline.** All source references are at `de1044b` (the change's base). The issue cites `66eb7cd`; the cited effect/inference files have moved only by the line numbers recorded below. No test was executed for this design.

### The production solver, exactly

The arena grammar is closed (`crates/noble-contracts/src/inference/effects.rs:8`):

```text
Effect ::= Hole | Constant(u64) | Union(u32,u32) | Rigid(u32)
```

An arena holds `effects`, `effect_bounds: Vec<u64>`, `effect_equations: Vec<(u32,u32)>`, `rigid_effect_goals: Vec<(u32,u32)>` and `program_effects: Vec<(u32,u32)>` (`inference/mod.rs:60-80`). Construction initializes bounds in `add_effect` (`effects.rs:54`):

| Node | Initial bound |
|---|---|
| `Constant(bits)` | `bits` |
| `Hole` | `effect_universe` |
| `Union(_,_)` | `effect_universe` |
| `Rigid(_)` | `effect_universe` |

`effect_universe` is the bit mask of host effect identities known to the selected session, capped at 64 bits (`crates/noble-contracts/src/source.rs:815`; `rigid_effect` refuses a binder identity ≥ 64 at `effects.rs:85`). `effect_constant` refuses an identity outside the universe (`effects.rs:105`). `editor_effect` reports known bits plus an `unknown` flag for a hole or rigid node (`effects.rs:18`).

`solve_effects` (`effects.rs:334`) first runs `solve::rigid` and then `solve::run`.

**Rigid pass** (`effects/solve.rs:37`), when at least one rigid goal exists:

1. Build a symbolic fact per node: `Constant → {concrete:bits}`, `Rigid(i) → {rigid:1<<i}`, `Hole → {unknown:true}`, `Union → empty`.
2. Iterate to a fixed point: every union fact is the componentwise union of its operands (concrete bitwise OR, rigid bitwise OR, `unknown = left.unknown && right.unknown`); every equation propagates the combined fact through a **neutral** side only (`equate_through_pure`, `solve.rs:136`). A union side is neutral only when its fact is exactly empty. If both sides are non-empty, the equation propagates nothing.
3. Reject if any equation has both sides known and different, with `"signature narrows a universal effect"`.
4. Reject every rigid goal whose body or declared fact is unknown or whose facts differ, with `"source body does not preserve its universal effect bound"`.

**Concrete passes** (`solve::run`, `solve.rs:192`), repeated to a fixed point by `pass` (`solve.rs:234`):

- `union` (`solve.rs:277`): `u := u ∩ (a ∪ b)`, then `a := a ∩ u`, `b := b ∩ u`.
- `equation` (`solve.rs:304`): `a := a ∩ b`, `b := b ∩ a`.
- After convergence, `constant` (`solve.rs:325`) rejects any `Constant(bits)` whose final bound differs from `bits`.

`effect_value` (`solve.rs:345`) extracts the final bits of a node as an `EffSet`; this is the exposed latent effect. Program construction records `(program node, effect node)` (`effects.rs:282`), and type unification of two `Program` terms adds an effect equation (`inference/unify.rs:130-155`). Signature application equates the inferred effect to a non-rigid scheme pattern and records a rigid goal for a rigid scheme (`source/inference/mod.rs:233-260`; `completion.rs:5-24`).

The only existing solver regression is `solve.rs:416`: it calls `rigid` directly on a hand-built four-node arena, checks convergence, then appends a host constant and expects refusal. Source regressions cover branch/Program behavior, not the solved assignment (`crates/noble-contracts/tests/source.rs:32,168,226`); the declared-source case rejects a narrowed universal effect end to end (`tests/declared/cases.rs:256`). The kernel property oracle checks candidate acceptance, not this solver (`crates/noble-kernel/tests/property.rs:107`). All of these remain.

## Reference semantics `finite-effects/v1`

### Universe, systems and supported fragment

A system is `(U, nodes, equations, goals)`:

- `U` is a finite set of effect identities, represented as a bit mask in production and as an independent `Vec<bool>`/sorted list in the reference. `|U| ≤ 4` for exhaustive cases.
- Nodes are `hole(i)`, `const(S)`, `union(a,b)`, `rigid(i)`, with `S ⊆ U`.
- Equations are unordered pairs of node IDs; goals are unordered pairs `(body, declared)`.

The **supported reference fragment** is:

1. Union references form a DAG: every child ID is smaller than its union node ID. Production does not require this, so cyclic systems are a separate robustness lane.
2. Every `const` set and every equation/goal ID is in range.
3. Concrete lane: no `rigid` node and no goal.
4. Rigid lane: at least one goal; every `rigid` node has an index below the declared rigid cap; no other constructor is unrepresentable.

Systems outside the fragment are reported `unsupported-reference`; the oracle records the production outcome but makes no semantic comparison.

### Concrete reading

An **assignment** `A` maps every node to a subset of `U` such that:

```text
A(const S)       = S
A(union a b)     = A(a) ∪ A(b)
A(a)             = A(b)            for every equation (a,b)
```

An assignment exists iff the system is **satisfiable**. The set of assignments is closed under pointwise union: constants are fixed, union is preserved by union, and equality is preserved. Therefore a unique **greatest assignment** `A*` exists; it is the pointwise union of all satisfying assignments.

**Theorem (production computes `A*`).** Define the deflationary operator `F` on bound vectors by the three production passes. `F(x) ⊆ x`, and `F` is monotone. A fixed point of `F` is exactly a satisfying assignment: at a fixed point `u = u ∩ (a∪b)`, `a = a ∩ u`, `b = b ∩ u` imply `u = a∪b`; equations imply `a = b`; constants are unchanged. Conversely a satisfying assignment is a fixed point. The greatest fixed point of a monotone deflationary operator on a finite lattice is the union of all post-fixed points, hence `A*`. Production starts at the top vector and iterates `F`, so acceptance implies `effect_bounds = A*` for every node. The design records this theorem as the reason to compare the **complete final bound table**, not only accepted/refused. It will be proved in the future oracle's review notes; finite testing does not replace the proof obligation.

If no assignment exists, the reference reports `inconsistent`. The production result is expected to be `Invalid` (a constant narrowed, or a value extraction impossible). A production `Exhausted` never counts as `inconsistent` or as rejection.

### Rigid reading

Rigid variables denote **arbitrary subsets of `U`**, not holes to narrow. Hole assignments are separate. Define:

- A rigid valuation `ρ : Rigid → 2^U`.
- A hole assignment `H : Hole → 2^U`; derived union nodes are computed exactly; constants are fixed.
- `(ρ,H)` **satisfies the equations** iff all union/constant equalities and all equations hold.

The system is **rigid-consistent** iff for every rigid valuation `ρ` there exists at least one `H` satisfying the equations. If some `ρ` has none, the universally quantified equations are false for that valuation; report `rigid-inconsistent`. Goals are then **not** vacuously accepted.

For a rigid-consistent system, a goal is **valid within the bound** iff, for every `ρ` and every `H` satisfying the equations, `A(body) = A(declared)` under `(ρ,H)`. The reference reports:

- `goals-valid` only after exhausting every `ρ` and every `H` within the declared caps;
- `goal-counterexample` with the first witness `(ρ,H,body,declared)` otherwise;
- `oracle-exhausted` if either enumeration cap is reached first.

This reading deliberately differs from a "solve holes once, then check one assignment" reading. Production's symbolic acceptance is expected to be sound for it: a symbolic fact records only bits that hold in **every** admissible assignment, and `equate_through_pure` descends only through a side whose fact is definitely empty. A production refusal may still be a **conservative refusal** of a semantically valid goal (for example, `union(h,c) = union(c,h)` is valid for every valuation but is not structurally equal). Conservative refusals are characterization results, not automatic defects.

The issue requires distinguishing an inconsistent system from a vacuously true implication. The reference therefore orders its verdicts: `rigid-inconsistent` first, then `goal-counterexample`, then `goals-valid`; an inconsistent system never yields `goals-valid`.

## Oracle and differential comparison

### Representation and independence

The future test-only oracle lives under `crates/noble-contracts/src/inference/effects/` as a `#[cfg(test)]` module (selected location: `effects/oracle.rs`, with `#[cfg(test)] mod oracle;` in `effects.rs`). It:

- drives production only through `effect_hole`, `rigid_effect`, `effect_empty`, `effect_constant`, `effect_union`, `equate_effects`, `require_rigid_effect`, `solve_effects`, `effect_value` and `program`;
- stores its own nodes, universe, assignments, equations, goals, valuations and witness records in ordinary `u64`/`Vec` types;
- computes every expected value by its own union/equality/enumeration code;
- never calls `narrow_effect`, `equate_through_pure`, `symbol`, `Effect::union`, the production pass driver or `EffSet` to compute an expected result. `EffSet` is used only to read an exposed production value if a case exercises `effect_value`.

Building the production arena through the public constructors means the oracle also observes initialization and ID allocation. Malformed arenas are built by direct field assignment only in the separate robustness lane, where no semantic claim is made.

### Outcome classification

Production outcomes are recorded independently of semantic outcomes:

| Production | Meaning |
|---|---|
| `accepted` | `solve_effects` returned `Ok` |
| `invalid` | `DiagnosticKind::Invalid` |
| `unsupported` | `DiagnosticKind::Unsupported` |
| `exhausted` | `DiagnosticKind::Exhausted` |
| `internal` | `DiagnosticKind::Internal` |

Reference outcomes for the concrete lane: `satisfiable(assignment)` (including the greatest assignment for every node), `inconsistent`, `unsupported-reference`, `oracle-exhausted`.

Reference outcomes for the rigid lane: `goals-valid(bound)`, `goal-counterexample(witness)`, `rigid-inconsistent`, `unsupported-reference`, `oracle-exhausted`.

The differential matrix:

| Production | Reference | Class | Defect? |
|---|---|---|---|
| accepted | satisfiable | compare every final bound; equal → `agree-accept`; different → `wrong-assignment` | mismatch is a defect |
| accepted | inconsistent | `unsound-accept` | defect |
| invalid | inconsistent | `agree-refuse` | no |
| invalid | satisfiable | `conservative-refusal` | characterization only |
| unsupported | any | `production-unsupported` | recorded, no comparison |
| exhausted | any | `production-exhausted` | **never** acceptance or rejection |
| internal | any | `production-internal` | defect in the lane |
| accepted | goals-valid | `agree-accept` | no |
| accepted | goal-counterexample or rigid-inconsistent | `unsound-accept` | defect |
| invalid | goal-counterexample or rigid-inconsistent | `agree-refuse` | no |
| invalid | goals-valid | `conservative-refusal` | characterization |
| any | unsupported-reference or oracle-exhausted | `oracle-unavailable` | no semantic claim |

The oracle does not reinterpret a `conservative-refusal` as a completeness bug. A declared **symbolically exact** subfamily (syntactic equality of body and declared after the production neutral propagation, with no unknown) is expected to agree; every case in that subfamily that refuses is a `defect-missed-supported-case`. The design keeps this subfamily explicit so the completeness claim is scoped rather than universal.

### Low-work and sufficient-work runs

Every corpus case runs twice:

1. **Low work**: `Limits { work: 1, .. }` or a budget recorded below the case's measured need. The expected outcome is `exhausted`; the harness MUST fail if the low-work run is recorded as accepted, invalid, unsupported, internal, or any reference semantic verdict.
2. **Sufficient work**: a budget large enough to converge, also bounded by the case cap. Only this run is compared against the reference relation.

The receipt records both budgets; a low-work exhaustion is never counted as a semantic rejection.

## Corpus, bounds, metamorphic checks and shrinking

### Deterministic exhaustive corpus

Universe sizes `|U| ∈ {0,1,2,3,4}`. For each universe, enumerate within:

- at most 5 holes, 24 nodes, 8 equations, 0 rigid nodes/goals (concrete lane);
- at most 3 rigid binders, 4 goals, 4 holes, 20 nodes, 6 equations (rigid lane).

Enumeration is deterministic in generation order (node count, then node kinds in fixed order, then equations in lexicographic order). A case is skipped with `out-of-bound` rather than truncated silently. The corpus MUST include each family:

1. Empty universe, one hole, no constraints.
2. A hole constrained only by a host constant.
3. A shared hole used by two independent equations.
4. Repeated and nested unions (`union(union(h,h),c)`, `union(h,union(h,c))`).
5. A pure neutral operand (`const ∅`) on one side of an equation.
6. Two non-neutral sides: no propagation is permitted.
7. Contradictory constants (`const {a}` and `const {b}` equated, `a ≠ b`).
8. Multiple independent equations that intersect.
9. A constant narrowed by a union/equation (must be `invalid`).
10. Rigid identity goal `(r,r)`.
11. Distinct rigid goal `(r,s)` (must refuse; a counterexample exists).
12. Rigid versus constant, rigid union rigid, and a hole constrained to a constant before a goal.
13. An unconstrained hole in a rigid goal (must be a conservative refusal or counterexample, never `goals-valid`).
14. Inconsistent equations with a rigid goal (must be `rigid-inconsistent`, never `goals-valid`).
15. Equation order reversed, duplicated, and node-renamed isomorphs.
16. A production-effect extraction case: a `Program` effect node flows to `effect_value`.

### Seeded larger cases

Seeds are unsigned 64-bit values from a fixed default list (for example `0x1`, `0x9e3779b97f4a7c15`, `0xdeadbeefcafebabe`, plus two more). The generator is SplitMix64 implemented in the test module, so the schedule is replayable without an external crate. Bounds: 8 holes, 40 nodes, 16 equations, 4 rigid binders, 6 goals, `|U| ≤ 6`; reference enumeration uses exhaustive search when the assignment count is ≤ 2^20 and otherwise a deterministic per-hole domain/DFS with a work cap. The reference MUST report `oracle-exhausted` rather than a verdict when the cap is reached. Larger cases are a separate `#[ignore]` or environment-gated lane with seeds recorded in the receipt; the normal test lane uses the fixed default seeds and small bounds.

### Metamorphic checks

For every case in the deterministic corpus that accepts under a sufficient budget, the harness checks:

- **Equation order**: reversing and randomly permuting the equation list preserves acceptance and the complete final assignment. Work counts and exhaustion points may differ; the harness MUST NOT require equal work.
- **Duplicate equation**: adding an equation already present preserves acceptance and the assignment.
- **Node renaming**: an isomorphic arena with permuted node IDs (and references rewritten) preserves acceptance and the assignment up to the renaming. Rigid binder renaming is included in the rigid lane.
- **Union operand swap**: `union(a,b)` and `union(b,a)` preserve acceptance and the assignment, but not work.
- **Union self-form**: `union(h,h)` and `h` are expected to have the same final assignment; this is a characterization check and is skipped if either run exhausts.

A metamorphic failure is a defect only when both runs have sufficient budget and the original is in the supported fragment.

### Shrinking

When a discrepancy is found, the harness shrinks it by:

1. deleting equations (last first) while all references and the outcome class are preserved;
2. deleting goal pairs;
3. replacing a union child with a hole or a constant of the same universe;
4. renaming nodes to the smallest free IDs.

Every candidate is re-run against the reference and production; shrink steps are capped (256 attempts, 64 accepted steps). Shrinking MUST preserve the original outcome class and the supported fragment. The minimized arena is serialized as a deterministic text record (node kind, operands, constant bits, equation pairs, goal pairs, universe) in the receipt. If shrinking changes the class, the original is retained and the step is discarded.

## Seeded faulty-solver controls

The oracle's detection power is demonstrated with test-local variants that are **not** production code and are never compiled into the shipped crate:

| Variant | Mutation | Required catch |
|---|---|---|
| M1 | skip `constant` exactness | `wrong-assignment` or `unsound-accept` on a narrowed-constant case |
| M2 | skip narrowing union children after narrowing the parent | `wrong-assignment` on a nested-union/shared-hole case |
| M3 | propagate an equation through the left union side even when non-neutral | `wrong-assignment` on a two-non-neutral-side case |
| M4 | allocate `Rigid` as `Hole` (narrowable bound) | `unsound-accept` on a rigid goal case |
| M5 | run a single pass instead of the fixed point | `wrong-assignment` on a multi-pass convergence case |
| M6 | initialize `Hole` bounds to zero instead of the universe | `unsound-accept` or `wrong-assignment` on a host-bit case |

Each variant runs against the same corpus and must be caught by at least one **named** deterministic case; the receipt records the case ID, the expected discrepancy class and the observed class. A variant that is not caught is a design defect in the corpus, fixed by adding a distinguishing family, not by weakening the variant.

## Reproducible runner and receipt

The future implementation selects:

```sh
nix develop --command cargo test -p noble-contracts --lib inference::effects::oracle -- --nocapture
```

The default lane runs the deterministic corpus, the fixed default seeds, the metamorphic checks, shrinking smoke cases and the six variants under a pinned toolchain. A separate `#[ignore]` lane accepts `NOBLE_EFFECT_ORACLE_SEEDS` and `NOBLE_EFFECT_ORACLE_BOUNDS` and is invoked only by the evaluation runner. A Bun runner (future `verification/effect-oracle/run.mjs`) captures stdout and writes `verification/effect-oracle/<revision>/receipt.json` with:

- oracle revision and source revision (`git rev-parse HEAD`), dirty state and exact test binary hash;
- Rust toolchain and package versions;
- universe/hole/node/equation/rigid/goal/assignment caps, low and sufficient work budgets;
- seed schedule;
- counts by production outcome, reference outcome and discrepancy class;
- every `conservative-refusal` case ID;
- every minimized discrepancy record;
- elapsed time and peak recorded allocation, if the harness can observe it;
- the exact command and any environment variables.

A receipt records finite evidence for the tested domain. It does not prove the solver, close PO-04/PO-09/PO-10, or replace the existing source and kernel tests.

## Acceptance mapping (issue #28)

| Issue criterion | Section |
|---|---|
| Document oracle semantics, supported fragment, quantifier order and finite bounds | "Reference semantics `finite-effects/v1`" |
| Independent concrete evaluator and rigid-valuation checker; no production solving helpers as expected results | "Representation and independence"; "Rigid reading" |
| Deterministic corpus, replayable seeds, bounded shrinking, explicit outcome classification | "Corpus, bounds, metamorphic checks and shrinking"; "Outcome classification" |
| Preserve the shared-pure-constraint regression; add neutral-union, hidden-host, shared-constraint and rigid-identity cases | "Deterministic exhaustive corpus" (families 3–13); the existing `solve.rs:416` test is retained unchanged |
| Seeded faulty variants catch lost host bits, bad union/equality propagation, rigid-as-hole | "Seeded faulty-solver controls" |
| Low work returns exhaustion, never semantic rejection; sufficient work checks the reference | "Low-work and sufficient-work runs" |
| Reproducible command and receipt with revision, seed, bounds, counts, conservative discrepancies | "Reproducible runner and receipt" |

## Risks and non-claims

- The greatest-solution theorem is proved on paper here and must be reviewed; a flaw would invalidate the concrete expectation, not production behavior.
- The rigid universal-hole reading is deliberately stronger than "one solved assignment". If review selects a different reading, the reference outcome table changes; production acceptance must still be sound for the selected reading.
- Exhaustive enumeration grows exponentially. The caps are part of the evidence; `oracle-exhausted` is not a verdict.
- Conservative refusals are expected. Treating every false refusal as a bug would demand completeness that #28 does not establish and that the current symbolic fragment does not claim.
- The test-only access to private arena fields is a maintenance cost of the selected location. A future change that makes the solver a separate crate would replace it with an integration harness; the reference semantics remains the same.
