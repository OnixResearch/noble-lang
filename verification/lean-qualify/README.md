# Lean dependency qualification

`qualify.mjs` is an explicit developer/CI command for deciding whether a reviewed immutable Noble proof/dependency revision set still passes an existing consumer profile. It does not schedule, merge, publish, change pins, repair specifications or axioms, or replace Nix, Cairn, or tool-policy gates.

## Two-step invocation

Run the policy-selected Node from `policy/tool-selection.json`. Each revision argument is a full immutable Git commit SHA for the Noble repository; its complete Lean package set is read from that commit's Lake manifests, Lakefiles, toolchain files, policy and vendored-source records.

```sh
NODE="<absolute path to policy-selected Node>"
BASELINE="<full baseline commit SHA>"
PROPOSAL="<full proposal commit SHA>"
PLAN_OUT="<new directory outside the repository>"
PLAN_TMP="<new empty private scratch directory outside the repository>"

"$NODE" verification/lean-qualify/qualify.mjs plan \
  --baseline "$BASELINE" --proposal "$PROPOSAL" \
  --profile core-bootstrap --output "$PLAN_OUT" --scratch "$PLAN_TMP"
```

Review `candidate.patch`, the four classified patches under `change-classes/`, every `package-diffs/*.patch`, the package/toolchain/checker snapshots, and the pre-update plan. The plan prints `review_sha256`; then explicitly approve those exact reviewed bytes:

```sh
RUN_OUT="<another new directory outside the repository>"
RUN_TMP="<another new empty private scratch directory outside the repository>"
TARGET="<absolute temporary Cargo target outside the repository>"
"$NODE" verification/lean-qualify/qualify.mjs run \
  --plan "$PLAN_OUT/plan.json" --approve-diff "<review_sha256>" \
  --output "$RUN_OUT" --scratch "$RUN_TMP" --target-dir "$TARGET"
```

The run writes `qualification.json`, exact commands and logs under `stages/`, checker receipts, and manifest-comparison details. Exit 0 means the proposal passed every required stage in the selected profile; a failed baseline, unavailable tool/checker, resolver discrepancy, cancellation, or changed source tree is unqualified. Plan/run output directories must be new. Scratch is removed after candidate worktrees and package workspaces are cleaned up. Never place output or scratch inside the repository.

## Current profiles and required checks

`core-bootstrap` runs the existing `verification/ci/lean-proofs.sh` (M3 build and proof gate, foundation build, theorem inventory and axiom audit), then the existing 80-case `verification/foundation/differential.mjs` replay and its receipt verifier. It requires retained exact-revision Mathlib/Aeneas caches and fails if they are missing, changed, or would need rebuilding.

`inets-foundation` is the bounded compatible-update profile for the vendored inets library. It keeps `proofs/m3` and `proofs/foundation` toolchain/manifests as compatibility evidence, but builds only the foundation package with `verification/foundation/check.mjs`, runs the same seeded differential replay and verifies its receipt. This rebuilds the vendored `InteractionNets` modules and their foundation consumers without invoking the M3/Aeneas consumer build or Mathlib. Use it for a no-op run and compatible vendored-inets updates when the M3/Aeneas artifacts are not in the checked-in build context; it does not claim to qualify M3/Aeneas consumers.

`mc1-export` is intentionally reported unqualified: the current MC1 exporter/checker runner requires Bun, which has no version/output pin in `policy/tool-selection.json`. To qualify that profile, first establish an owner-reviewed tool-selection pin and checker profile; this command will not silently substitute Node, omit the checker, or infer that a Lake build is sufficient.

Before an approved candidate update, the planner uses a native, data-only parser for the Noble Lakefile declarations and `git show` at immutable commits. It never evaluates a newly fetched package's Lakefile during planning. Once the source diff is approved, changed proof-package manifests/Lakefiles are resolved in the disposable proposal worktree and the full package entries in the resulting manifest are compared exactly to the reviewed prediction. Any extra, missing, or changed package identity is labelled and refuses qualification. Unchanged/no-op locks are compared as locked manifests without re-resolving moving upstream branches.

The report separates proposed pin changes, proof-only repairs, semantic/proof-source changes, and verifier-policy changes. Changes to Lean proof source are conservatively classed semantic until a reviewer establishes that they are proof-only. Nothing automatically repairs specs, inventories, or axiom policy. New receipts identify the proposal commit/tree and never relabel historical evidence.

## Regression localization

A failing proposal can be investigated with an ordered JSON array of immutable commit SHAs, from the verified good baseline through the failed proposal:

```sh
"$NODE" verification/lean-qualify/qualify.mjs localize \
  --baseline "$BASELINE" --proposal "$PROPOSAL" \
  --history "<path to ordered full-SHA JSON array>" --mode bisect \
  --plan "$PLAN_OUT/plan.json" --approve-diff "<review_sha256>" \
  --output "<new directory outside the repository>" \
  --scratch "<new empty private scratch directory outside the repository>" \
  --target-dir "$TARGET"
```

Native bisection verifies the first revision before probing and records each tested SHA, actual toolchain, outcome, and retained check result. It assumes monotonicity. Use `--mode linear` to test and record every revision; this is required for non-monotonic histories or when every tested revision must be certified. No hopscotch or paranoid-lake-update adapter is used, so there are no external adapter pins to trust. A static plan cannot see dependency logic that is computed dynamically by an upstream Lakefile; the post-resolution exact manifest comparison is the required check for that case.

## Isolation and preserved evidence

The runner requires a clean starting worktree, snapshots the tracked proof-package sources and their external package identities/trees, `lean-toolchain` and manifest files, `policy/tool-selection.json`, checker/profile sources, and the vendored inets record/bytes. It records the starting HEAD/tree/status, creates detached disposable Git worktrees under caller-provided scratch, and copies each candidate project's `.lake/build` plus every manifest package checkout (including its `.lake` cache) into the disposable worktree using `cp -a --reflink=always`. The runner never points candidate Lake package entries back into a shared checkout. It refuses if it cannot prove the package cache is private, if reflinks are unavailable, or if a heavy package cache is missing. Mathlib/Aeneas caches used by `core-bootstrap` are made read-only; any attempted rebuild fails closed. The retained report lists cache files added/changed/removed, per-package rebuilt-byte estimates, and candidate worktree apparent/allocated bytes; reflink-shared blocks can make allocated-byte totals overstate unique filesystem use. It removes candidate/package workspaces and restores temporary cache permissions on success, failure, tool error, or SIGINT, then verifies that the original worktree HEAD/tree/status are unchanged. Rust replay builds use the caller's target directory.

The candidate proof suite reuses the existing checked-in Lean/Mathlib/Aeneas source and output identities. This lane does not rebuild Mathlib or Aeneas: a changed pin to either heavy package is refused unless a future profile supplies independently admitted, exact-revision prebuilt artifacts. The pre-update scan treats upstream package Lakefiles as data-only observations; unparsed or newly introduced upstream requirements are explicitly recorded, then the approved proposal resolver's complete manifest is compared exactly to the candidate's pinned manifest. Vendored inets updates are realistic when the Noble source commit contains the matching upstream source bytes and `UPSTREAM.json` identity; the imported InteractionNets and dependent foundation modules are checked in the same way as the baseline.

A no-op source revision need not be a no-build execution: Lake's existing `*.setup.json` dependency traces contain absolute import-artifact paths from the package context in which the cache was produced. A private copied package tree has a different path and `LEAN_PATH`, so Lake may regenerate foundation modules and their setup traces even when every pin is identical. The run report records changed cache paths and rebuilt bytes separately for each package; the no-op claim is about unchanged committed inputs and qualification results, **not** a zero-rebuild claim. Choose `inets-foundation` rather than `core-bootstrap` when the latter's context would recompile Aeneas/Mathlib; any heavy-package rebuild attempt is refused, not counted as a successful compatibility check. The report also records the disposable worktree's apparent and allocated bytes, with reflink accounting caveats.

## Self-tests

The test suite creates small local Git-backed Lean package fixtures under caller-provided private scratch; it does not fetch packages or rebuild Mathlib/Aeneas.

```sh
NOBLE_QUALIFY_TEST_TMP="<new private scratch directory outside the repository>" \
  "$NODE" --test verification/lean-qualify/qualify.test.mjs
```

Coverage includes data-only planning, exact detection of a dependency added by an upstream package, an incremental synthetic rebuild that leaves an unrelated consumer module untouched, a seeded mid-range regression, a fail-then-pass history showing why bisection alone is insufficient, full linear scan evidence, change-class separation, private reflink package-cache copies with source-byte preservation and rebuild-byte reporting, refusal controls for missing theorem roots/changed roots/extra axioms, missing negative-control detection, stale Lean artifact and missing-module builds, profile boundaries, and byte-identical fixture state after tool-spawn failure or active-child cancellation. The suite writes `regression-evidence.json` into its caller-selected private scratch root, retaining the fixture's SHA history and every bisection/linear probe; its declared outcomes are synthetic and must not be mistaken for a full proof/replay qualification.

## Ownership boundaries

Noble owns its Lean pins, selected tool policy, theorem/axiom policy, and consumer acceptance. The vendored inets source remains governed by its own upstream revision and identity record. #62 owns independent replay/semantic-binding work; this qualification command only invokes existing replay controls and does not duplicate their semantics. #97 owns Aeneas re-extraction; this command does not replace extraction or treat its success as a proof of exporter correctness.
