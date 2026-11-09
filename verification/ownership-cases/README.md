# Ownership cases on the live-slot runtime (OWN-CASE-01 to 03)

`.cairn/changes/ownership-lifetime-contract` designs the OWN-CASE scenarios. This directory
executes three of them against the live-slot runtime. They are executed, not registered: the task
reserves registration for a reviewed conformance change.

| Case | Lane | What it checks |
|---|---|---|
| OWN-CASE-01 | `crates/noble-cli/tests/live_slot_direct_quote.mjs` | `def q [ quote ]` with 5 saves a Program; after that root exits, another root runs it with 20 via `swap [ run ] dip +` and gets 25 |
| OWN-CASE-02 | `crates/noble-cli/tests/live_slot_ownership_cases.mjs` | `[ 1 + ] dup` saves two owners of one Program. In both release orders, the other owner still runs after the first release, a second release of the first is refused, and both are refused after the second release |
| OWN-CASE-03 | the same lane | a balanced 8-leaf `compose` tree, each leaf and intermediate saved: the root gives 8 before and after the 14 other owners are released, every released owner is refused, and the root is refused after its own release |

`cargo test -p noble-cli --test live_slot_direct_quote` runs both lanes.

## The observation bound

The host observes the graph of every anonymous saved Program, and bounds that observation at
1000 bytes and 64 nodes (`captureValue` in `slot-engine.mjs`). A saved composite crosses that
bound early:
- with one static leaf, `drop [ 1 + ]`, a left- or right-deep chain is refused at its fifth compose;
- with captured leaves such as `k quote [ + ] compose`, it is refused at the second.

The refusal comes after the composing root ran. It is `execution-failed` with the diagnostic
"checked captured values exceed bounded exact trace representation". No owner is issued, and
the session ends with `terminated-after-guest-failure`, as every failed root does in this profile.
The lane checks this for the left-deep chain. So OWN-CASE-03's left- and right-deep 8-leaf trees
cannot run on this runtime as designed; the balanced tree is the one that fits. Whether a failed
root should end the session is the poison-or-continue decision that
`.cairn/changes/live-reference-lifetimes` leaves open.

## Evidence

[`receipt.mjs`](receipt.mjs) runs both lanes against one CLI built from a clean commit. The retained run is [`receipt.json`](receipt.json), with the lanes' output in [`lane-1.stdout`](lane-1.stdout) and [`lane-2.stdout`](lane-2.stdout), from the clean commit `dd96f88c`. Every case passed as described above, and the left-deep chain was refused at its fifth compose.

## Non-claims

Reclamation is observed only through owners being refused after release and the root running
alike; the runtime exposes no live-cell count. OWN-CASE-04, 05 and 07 are not executed here:
- OWN-CASE-04's borrow variants need a `borrow` word the kernel lacks;
- OWN-CASE-05 needs a root that traps after saving;
- OWN-CASE-07 needs a runtime that rebinds a retained name, which the live-slot CLI refuses.
