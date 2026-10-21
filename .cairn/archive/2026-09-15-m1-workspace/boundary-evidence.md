# Task 2.2: boundary controls in progress

Task 2.2 remains unchecked. M1 remains five complete tasks and sixteen open tasks.
This document records the historical failed boundary check. The [published-provider adoption](body-owner-adoption-evidence.md) supersedes that blocker.
All eleven current flake checks now pass, but task 2.2 and M1 acceptance remain open.

## Implemented scope

- Added Nickel-owned expected fixtures, a checked JSON export, and a policy-registry entry.
- Added pure Nix mutation and observation checks with a separate file adapter and bounded execution helper.
- Added thirty synthetic observation controls and six source-mutation controls.
- Added exact CLI results for five successful inputs, ten invalid argument cases, and a Unix non-UTF-8 argument.
- Kept the production kernel, CLI implementation, Cargo manifests, architecture policy, and all targets unchanged.
- Kept Nix and pre-commit on published Octet `f8b12bea3ba5d72de74df9f569afcdf396871325`.
- Explicitly refreshed the reviewed flake hash and selection export after the flake change.

The [boundary document](../../../verification/boundary-controls.md) describes each fixture and its evidence role.
The auxiliary default-lint run cannot replace the required full deny-all result.

## Observations

Pueue 2314 passed the initial workspace and Octet baseline plus Cairn planning gates.
Pueue 2320 passed the expanded shell tests. Pueue 2351 passed five workspace Rust tests and the ten earlier Nix checks.
That expanded flake run failed its new eleventh check.

The first fixture build, 2338, failed because the comparator used `callee_definition` instead of `resolved_definition`.
The synthetic regression in 2346 reproduced the error. The corrected historical fixture set passed in 2347.
Those controls used exact compiler aliases rather than source API aliases.

The earlier local-cell fixture did not demonstrate hidden state. Its result is historical harness evidence only.
The replacement mutates a thread-local cell whose initializer contains an inline constant.
In 2351, that fixture caused an internal compiler error and lacked a replayable JSON bundle.
The check correctly refused to accept this failure as boundary enforcement.

Pueue 2379 reproduced the crash with human diagnostics. Pueue 2385 checked the same fixture with plain Rust before the Octet backtrace probe.
Plain Rust checking passed with all targets and features. The Octet probe failed.
The backtrace identifies `signature_shape_identity`, `collect_item`, and `collect_call` in the architecture collector.
The `caller_item_id` fallback requests a function signature for a previously unseen non-closure owner. An inline constant is not a function.

## Retention and next work

Persistent evidence is under the primary repository's `.pi/m1-boundary/` directory.
It retains source snapshots, raw compiler context, full command records, matched historical fixtures, current failures, and the operator result.
The evidence is not an independently authenticated acceptance receipt.

At this snapshot, the next step was a provider repair with actual compiler regressions for non-function body owners.
That repair has since been published and adopted. It preserves identities, observations, reference closure, and production obligations.
The required fixture was not removed or changed. The separate adoption record retains the new results.

Randomness, relocated-body exceptions, foreign ownership, complete source/native accounting, extraction accounting, integrated controls, CI, and independent acceptance remain open.
