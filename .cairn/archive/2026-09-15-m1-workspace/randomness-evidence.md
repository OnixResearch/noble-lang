# Task 2.2: randomness controls and provider gap

Task 2.2 remains open. Noble remains 5/21 complete.
The earlier body-owner adoption passed all eleven checks. The expanded boundary check now fails its required randomness observation.

## Fixture scope

The new fixture calls the real `getrandom::fill` entry point. It does not use a local same-named stub.
Nickel selects `getrandom =0.4.3`. Cargo generated a separate fixture lock, with registry checksums for four dependencies.
Nix uses the selected Nixpkgs `importCargoLock` adapter. Fixture compilation runs offline and rejects lock drift.
Production Cargo files, Rust sources, architecture declarations, targets, and tool pins remain unchanged.

The dependency-only mutation compiles with clean lints and fails the exact forbidden kernel-dependency rule.
A first positive-baseline expectation failed because this dependency is not permitted in the core.
The separate negative now requires that actual rejection. No classification or dependency allowance was added.
The actual workspace and acyclic shell remain positive baselines.

The random-call mutation requires its own `ambient_random` error and compiler randomness effects in both kernel production units.
The dependency rejection cannot substitute for those observations.
The harness now has 37 synthetic observation cases and twelve source-mutation cases. These are not compiler executions.

## Observed failure

The full deny-all hook reports the required randomness diagnostic.
The auxiliary compilation succeeds and records the exact `getrandom::fill` call. Full replay is valid.
The published collector emits no randomness effect. Its effect table covers selected standard-library paths but not this registry entry point.
The comparator therefore reports `expected-boundary-observation` and rejects the fixture.
Two positive baselines and twelve negatives match. One of thirteen required negatives remains blocked.

The operator retained the failed dependency-baseline hypothesis, the missing-effect bundle, raw observations, source snapshots, and full commands.
An earlier evaluation also rejected mixed live Nix inputs after an edit during evaluation. That run is not compiler evidence.
Later runs froze the source and checked the staged tree before and after execution.

## Provider handoff

Persistent Noble evidence is under `.pi/m1-boundary-random/` in the primary repository.
The dedicated provider change is `classify-getrandom-effects`, based on published `e3e705a7c04a30b814000b251d245cf083894e4a`.
Its evidence belongs under `.pi/m1-random-effects/`, outside the disposable provider worktree.
The repair must retain callers, spans, identities, production memberships, and each required effect.
Real-crate controls must distinguish exact entry points from local or similar names.
Publication and published-byte checks must precede Noble pin changes.

Call-only evidence, advisory observations, dependency rejection, or fabricated effects cannot replace this control.
Foreign ownership, relocated-body exceptions, Noble-specific test-boundary acceptance, complete inventories, native assurance, CI, and independent acceptance remain open.

## Published repair adopted

Octet published `235255bc4972ced9128fd5b4d1ec66ff7508ded4`, whose tested, final-built, and published collector bytes are identical.
The repair classifies only external `getrandom` root functions `fill`, `fill_uninit`, `u32`, and `u64` by defining crate and root shape, and it preserves the recorded call and effect paths.
Noble regenerated the reviewed selection file hashes, the Octet revision and NAR hash, and the Octet tool derivation and output pairs, then pinned the same revision in Nix and pre-commit.
The published pre-commit deny-all and a direct workspace gate are clean with zero findings, and both retained bundles replay as valid.
All eleven checks pass, including the previously blocked boundary aggregate. The unchanged randomness comparator reports `boundary control randomness: passed (not M1 acceptance)`.
Two baselines and thirteen negatives match; the earlier missing-effect bundle remains retained under `.pi/m1-boundary-random/missing-randomness-effect/`.
The randomness fixture, its comparator, production sources, architecture policy, targets, and extraction inputs are unchanged.
Task 2.2 remains open for full BND-EFFECT coverage, foreign ownership, the relocated-body exception, and Noble-specific test-boundary acceptance.
