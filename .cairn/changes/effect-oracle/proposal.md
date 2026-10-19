## Why

Noble's source checker solves finite host-effect constraints by narrowing a
greatest positive-union solution and by a separate symbolic rigid pass
(`crates/noble-contracts/src/inference/effects.rs:334`,
`crates/noble-contracts/src/inference/effects/solve.rs:37`). Acceptance
controls effect claims that the host treats as authorization inputs
(K-EFFECT-01, K-EFFECT-04), so a solver that loses a host bit, narrows
through a non-neutral union, or treats a declaration binder as a free hole
would accept a program whose latent bound is wrong.

The current evidence is a single inline regression with hand-built private
state (`solve.rs:416`) plus end-to-end source cases that assert acceptance or
refusal but not the solved assignment. There is no independent semantic
oracle, no full-assignment comparison, no rigid-valuation check, no seed
replay, no shrinking, and no seeded faulty-solver control. V-CHECK-04 already
requires distinguishable rejection, unsupported, exhaustion and acceptance;
V-CHECK-05 requires positive coverage and a termination argument for the
checked fragment; PO-04/PO-09/PO-10 remain open.

Issue #28 asks for a bounded, independent finite-model oracle for the
**existing** solver. It is a testing subtask of #15, not a soundness report.
No finite oracle proves the solver; bounded differential evidence
characterizes the tested domain. This change is design-only: it selects the
reference semantics, the independent oracle, the corpus and bounds, the
outcome classification, the mutation controls and the receipt shape. It
implements nothing, changes no acceptance behavior and does not depend on any
subtyping or merge extension.

## What Changes

- Select reference semantics `finite-effects/v1` for the existing grammar of
  `Hole`, `Constant`, `Union`, `Rigid`, effect equations and rigid goals. The
  reference uses an independently represented finite universe, exact set
  union, exhaustive assignment enumeration within a declared bound, a proved
  greatest-solution reading for concrete systems, and an explicit
  universal-rigid/universal-hole reading for rigid goals. Inconsistent systems
  are distinguished from vacuously true implications. Cyclic union graphs are
  outside the supported reference fragment. r[FXORACLE-SEM-01]
- Specify a test-local differential oracle that builds production arenas only
  through the public effect constructors, runs the production solver, computes
  every expected result in the independent representation, compares the
  complete final bound table, and classifies production outcomes (`accepted`,
  `invalid`, `unsupported`, `exhausted`, `internal`) separately from reference
  outcomes (`satisfiable` with a greatest assignment, `inconsistent`,
  `goals-valid`, `goal-counterexample`, `rigid-inconsistent`,
  `unsupported-reference`, `oracle-exhausted`). It uses no production
  narrowing, symbolic propagation or `EffSet` helper as an expected result.
  r[FXORACLE-DIFF-01]
- Specify a deterministic corpus, replayable SplitMix64 seeds, explicit
  universe/node/equation/rigid/work caps, order/renaming/duplicate-equation
  metamorphic checks, and bounded outcome-preserving shrinking.
  r[FXORACLE-CORPUS-01]
- Specify seeded test-local faulty-solver variants that must be caught, the
  reproducible runner and receipt fields, and the non-claims.
  r[FXORACLE-CTRL-01]

## Impact

- **Files**: `.cairn/changes/effect-oracle/` only, with ADDED `verification`
  deltas `FXORACLE-SEM-01`, `FXORACLE-DIFF-01`, `FXORACLE-CORPUS-01` and
  `FXORACLE-CTRL-01`.
- **Code**: none. The future implementation is test-only (`#[cfg(test)]`); no
  production solver, public API, CLI, kernel or specification behavior changes.
- **Testing**: document validation only. No oracle case is executed, no
  counterexample or solvability result is claimed, and no existing source or
  kernel regression is replaced.
- **Coordination**: #15 owns the broader checker correctness foundation. This
  change supplies bounded differential evidence and a characterization of
  conservative rejections; it does not close PO-04/PO-09/PO-10 or any other
  proof obligation.
