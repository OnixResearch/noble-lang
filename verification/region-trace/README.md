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
The relation is the projection of a target trace onto the source alphabet.

| Theorem | Claim |
|---|---|
| `erase_preserves_projection` | erasing the two named bookkeeping classes never changes the projected source trace |
| `related_erase_invariant` | the relation is erasure-invariant |
| `denial_projects_to_request_only` | a refused request stays visible and gains no response |
| `normal_success_projects_request_and_commit` | a normal success projects its request and its protected commit |
| `identity_is_decision_identity` | the projection keeps the exact identity it was emitted with |
| `retirement_is_not_a_guest_release` | host retirement projects to its own commit kind |
| `abnormal_exit_class_is_not_in_the_projection` | the trap/quota/cleanup distinction is a target-side observation |
| `abnormal_exit_classes_collapse_in_projection` | trap and quota exhaustion are indistinguishable in the projection |
| `append_projection` | prefix compatibility: projections compose over concatenation |
| `take_projection` | a trace cut by an exhaustion boundary relates to the projection of its retained prefix |

`verification/region-trace/receipt.json` is the retained receipt of one clean-source run: 10 model
theorems, `propext` the only reported axiom and inside the standard allow-list.

## Non-claims

The target alphabet is a **handwritten machine**, not an emitted step sequence: there is no
Canonical-ABI, Wasmtime, Rust host or lowering correspondence here, no liveness claim and no
cancellation model. Two premises stay premises rather than theorems: that the host supplies the
exact resource identity, and response-to-request pairing (a target response step carries its own
operation, but which request it answers is not proved). WIT-TRACE-02 through WIT-TRACE-06 remain
open.
