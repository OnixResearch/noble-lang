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
* a release or retirement kind commits at most once.

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
| `related_rejects_{substituted_identity,unexplained_read_commit,two_guest_releases,missing_protected_commit,host_cleanup_as_guest_release,denial_erased_into_success}` | the forged target traces are refused, each by the condition that names it |
| `related_accepts_well_formed_success` | the refusals are not vacuous: a well-formed success relates |
| `retainFrom_prefix`, `retainFrom_cost_bounded` | a trace retained before fuel exhausts is a prefix of the full trace and costs no more than the budget |
| `filterMap_prefix`, `exhaustion_keeps_committed_prefix`, `exhaustion_keeps_commit_identities` | the retained trace's projected commits and identities are prefixes of the full trace's: exhaustion may cut a trace but cannot invent or drop a commit |
| `exhaustion_cannot_relate_an_unaffordable_commit`, `exhaustion_relates_when_affordable` | the exhaustion rule bites, and the same trace relates when the budget affords it |
| `retained_projection_is_permitted_prefix` | WIT-TRACE-03 corollary: a retained projection is a permitted ordered prefix of the *source* trace |
| `source_commits_are_decided`, `source_release_is_at_most_once` | WIT-TRACE-03 corollaries: exact identity and at-most-once release are inherited by the source alphabet |

`verification/region-trace/receipt.json` is the retained receipt of one clean-source run: **33** model
theorems, `propext` and `Quot.sound` the only reported axioms, both inside the standard allow-list.

## Non-claims

The target alphabet is a **handwritten machine**, not an emitted step sequence: there is no
Canonical-ABI, Wasmtime, Rust host or lowering correspondence here, no liveness claim and no
cancellation model. Response-to-request pairing stays a premise (a target response step carries its
own operation, but which request it answers is not proved), and so does the host's own supply of the
identity the relation then checks against its decisions. WIT-TRACE-02 through WIT-TRACE-06 remain
open.
