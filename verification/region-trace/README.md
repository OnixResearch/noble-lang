# Region trace relation pilot (WIT-TRACE-01)

Model-theorem check for `.cairn/changes/region-trace-relation`. It builds
`proofs/m5/RegionTrace.lean` in an isolated project with the pinned Lean 4.31 toolchain, then checks
every inventoried theorem's compiled statement and transitive axioms, so a finite instance, an extra
premise or a weaker statement cannot satisfy the inventory by mentioning a dependency.

## Command

```sh
env -i HOME="$HOME" USER="$USER" LOGNAME="$USER" PATH=/run/current-system/sw/bin:/usr/bin:/bin \
  "$(selected node)" verification/region-trace/check.mjs --output /tmp/region-trace-check
```

The harness refuses a caller environment carrying `LEAN*`, `LAKE*`, `BASH*`, `GIT_*`, `NIX_*` or
`ELAN*` variables, refuses an output directory inside the source tree, and refuses a dirty worktree
unless `--allow-dirty` is passed explicitly (a dirty receipt records it, it does not hide it).

## What is proved

The source alphabet is `RegionProtocol-v1`'s `Req`/`Resp`/`Commit`; the target alphabet is the
emitted ABI step sequence this host performs (`importCrossing`, `hostDecision`, `protectedCopy`,
`returned`, `normalReturn`, `abnormalExit`, `cleanup`, and the two named erasable bookkeeping steps).
The relation is the projection of a **well-formed** target trace onto the source alphabet, where
well-formedness itself is a condition of the relation:

* every commit identity was decided by a `hostDecision` first (a rep number or handle class can never
  stand in for the exact identity);
* every protected read commit is preceded by the crossing that asked for exactly that region;
* a trace commits at most one release or retirement (`guestRelease`, `destructorRelease` or
  `hostRetire`), so a guest release followed by a host retirement of the same region is refused.
  Until 2026-10-29 the condition bounded each kind separately, so that pair was well formed even
  though the definition's own docstring said "at most one release or retirement per trace";
  `related_rejects_release_then_retirement` is the control for the corrected condition.

| Theorem | Claim |
|---|---|
| `erase_preserves_projection`, `erase_preserves_{decisions,commit_identities,read_crossings,release_members,read_commits}` | erasing the two named bookkeeping classes changes none of the projected trace's scan inputs |
| `erase_preserves_well_formedness` | erasure cannot repair a forged target trace |
| `related_erase_invariant` | the relation is erasure-invariant |
| `denial_projects_to_request_only` | a refused request stays visible and gains no response |
| `normal_success_projects_request_and_commit` | a normal success projects its request and its protected commit |
| `identity_is_decision_identity` | the projection keeps the exact identity it was emitted with |
| `retirement_is_not_a_guest_release` | host retirement projects to its own commit kind |
| `abnormal_exit_class_is_not_in_the_projection`, `abnormal_exit_classes_collapse_in_projection` | the trap/quota/cleanup distinction is a target-side observation |
| `append_projection` | prefix compatibility: projections compose over concatenation |
| `take_projection` | a cut trace relates only when its retained prefix is itself well formed |
| `related_rejects_{substituted_identity,unexplained_read_commit,two_guest_releases,release_then_retirement,missing_protected_commit,host_cleanup_as_guest_release,denial_erased_into_success}` | the forged target traces are refused, each by the condition that names it |
| `related_accepts_well_formed_success` | the refusals are not vacuous: a well-formed success relates |
| `retainFrom_prefix`, `retainFrom_cost_bounded` | a trace retained before fuel exhausts is a prefix of the full trace and costs no more than the budget |
| `filterMap_prefix`, `exhaustion_keeps_committed_prefix`, `exhaustion_keeps_commit_identities` | the retained trace's projected commits and identities are prefixes of the full trace's: exhaustion may cut a trace but cannot invent or drop a commit |
| `exhaustion_cannot_relate_an_unaffordable_commit`, `exhaustion_relates_when_affordable` | the exhaustion rule bites, and the same trace relates when the budget affords it |
| `retained_projection_is_permitted_prefix` | WIT-TRACE-03 corollary: a retained projection is a permitted ordered prefix of the *source* trace |
| `source_commits_are_decided`, `source_release_is_at_most_once` | WIT-TRACE-03 corollaries: exact identity and at-most-once release are inherited by the source alphabet |
| `fixture_{ordinary_error,host_bounds_refusal,trap_after_protected_copy,cleanup_failure_after_protected_copy,quota_prefix_stops_before_the_copy,quota_prefix_keeps_the_affordable_commit}` | WIT-TRACE-05 fixtures at model level: the design's observable executions relate as designed, and the quota boundary is measured rather than assumed |
| `observed_read_and_release_lowers_to_a_related_trace`, `observed_abnormal_exit_lowers_to_a_related_trace`, `lowered_projection_starts_with_the_request` | WIT-TRACE-05: the ordered trace the host actually records (a read, its protected commit, the response, a consuming release; or an abnormal exit's own retirement) lowers onto the target alphabet and relates, in the host's order |
| `observed_traces_are_paired`, `swapped_response_is_unpaired` | WIT-TRACE-03: every response follows a request of its own operation, tolerating only that request's own protected commits, and a response answering another operation is unpaired |
| `related_rejects_swapped_response`, `related_rejects_substituted_resource_identity`, `related_rejects_observed_retirement_as_guest_release` | the relation refuses a target trace whose response answers the wrong operation, whose commit carries an identity the host never authorised, or whose host retirement is presented as a guest release |
| `related_implies_read_commits_explained` | the corollary the pilot was missing: every read commit a related trace projects is explained by the import crossing that asked for that exact region |

`verification/region-trace/receipt.json` is the retained receipt of one run: **49** model
theorems, `propext` and `Quot.sound` the only reported axioms, both inside the standard allow-list.

## Revisions the verdict depends on (WIT-TRACE-06)

The receipt carries a `bindings` block: the model revision (pinned Lean version and executable hash,
plus the source and inventory hashes) is bound, and `component`, `helper` and `engine` are recorded as
**`null` with the reason stated** — no component, lowering helper or engine instance is a subject of
these theorems, so binding one would imply a correspondence this package does not establish. A
`claims` block labels the verdict: 49 model theorems, 0 implementation theorems, correspondence none,
liveness none, cancellation unsupported and unmodelled.

## Non-claims

The target alphabet is a **handwritten machine**, not an emitted step sequence: there is no
Canonical-ABI, Wasmtime, Rust host or lowering correspondence here, no liveness claim and no
cancellation model. Pairing is stated over projected source traces, not proved for the emitted
component's own steps, and the ordered trace's lowering maps an *observation* onto the target
alphabet rather than corresponding to the emitted steps. WIT-TRACE-02 through WIT-TRACE-06 remain
open.
