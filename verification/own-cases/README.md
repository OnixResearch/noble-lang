# OWN-CASE acceptance gate

`gate.mjs` builds the production `noble` binary from a clean checkout and runs the committed lanes
for OWN-CASE-01, 02, 03, 04 (non-borrow variants), 05, 06 and 07. It binds the source snapshot,
the built binary (hashed before and after the run), the exact case projections and every raw output
file it wrote, with each file's sha256. Promotion verdicts (`promotable`, `failed_cases`,
`receipts[*].case`) are derived from the observations: a case is promotable only when its lane
observed every registered expectation. A failing lane still writes the receipt, with the failure.
It promotes nothing by itself.

## Retained runs

| Run | Command | Result |
|---|---|---|
| [`runs/20261120c`](runs/20261120c) | superseded gate at commit `5c9e38f4` | `completed`; OWN-CASE-03 misregistered (review F-01), OWN-CASE-04 rows constant (F-03) |
| [`runs/20261120-failed`](runs/20261120-failed) | superseded gate before its row parser was fixed | `failed`; OWN-CASE-04 rows were misparsed after the test harness prefix |
| [`runs/20261120-r2`](runs/20261120-r2) | `node verification/own-cases/gate.mjs --out <new dir>` at commit `81139111` | `completed` / `unmet-cases`; OWN-CASE-01, 02, 04, 05, 06, 07 promoted; OWN-CASE-03 is an executed failure (deep chains refused at compose 5) |
| [`runs/20261120-r3`](runs/20261120-r3) | `node verification/own-cases/gate.mjs --out <new dir>` at commit `33318b7c` | `completed` / `unmet-cases`; OWN-CASE-01, 02, 04, 06, 07 promoted, OWN-CASE-03 failed, OWN-CASE-05 withheld after review round 2 |
| [`runs/20261121-b1`](runs/20261121-b1) | `node verification/own-cases/gate.mjs --out <new dir>` at commit `05bac95a` (Cairn batch-1 tree) | `completed` / `unmet-cases`; the same verdicts as r3: OWN-CASE-01, 02, 04, 06, 07 promotable, OWN-CASE-03 failed, OWN-CASE-05 withheld |
| [`runs/20261121-b2`](runs/20261121-b2) | `node verification/own-cases/gate.mjs --out <new dir>` at commit `b088cfc7` (Cairn batch-2 tree, after review round-3 notes R3-1 and R3-4) | `completed` / `unmet-cases`; the same verdicts: OWN-CASE-01, 02, 04, 06, 07 promotable, OWN-CASE-03 failed, OWN-CASE-05 withheld |
| [`runs/20261121-b3`](runs/20261121-b3) | `node verification/own-cases/gate.mjs --out <new dir>` at commit `c9343972` (combined Cairn tree: batch 2 plus LIVE round 3) | `completed` / `unmet-cases`; the same verdicts, projection `7aa15017…` and gate `828ff88e…` as b2; the case records keep citing b2 |
| [`runs/20261122-b4`](runs/20261122-b4) | `node verification/own-cases/gate.mjs --out <new dir>` at commit `88e90add` (Cairn batch-4 tree, with the OWN-CASE-03 amendment of owner decision 35) | `completed` / `all-passed`; OWN-CASE-01, 02, 03, 04, 06, 07 promotable, OWN-CASE-05 withheld |
| [`runs/20261122-b5`](runs/20261122-b5) | `node verification/own-cases/gate.mjs --out <new dir>` at commit `8b62827a` (Cairn batch-5 tree, with LREF-EXIT-01 and the OWN-CASE-03/05 amendments of integrator decision 39) | `completed` / `all-passed`; OWN-CASE-01..07 promotable |

[`acceptance.json`](acceptance.json) is the **first** run's receipt (`sha256 e873df58…`, tested
commit `5c9e38f4`), kept byte-for-byte as the immutable path the first OWN promotions cite. Later
runs live under `runs/` and are cited by their own paths; superseded receipts stay immutable and
rework receipts are added beside them, never edited. The current promotion facts come from
[`runs/20261122-b5/acceptance.json`](runs/20261122-b5/acceptance.json) (`sha256 c9bbeb56…`,
`source_revision sha256:897a5ebf…`, binary `7f32e4e1…` before and after, tested commit
`8b62827a`, projection digest `3130b4c6…`). The OWN-CASE-03/05 amendments (integrator decision
39, after LREF-EXIT-01) changed the projection and the gate, so every case record moved to this
run; OWN-CASE-03 is re-promoted and OWN-CASE-05 promoted on it, every registered field observed.
An earlier run of the same tree at `da8d475e` is not retained: it reported OWN-CASE-05 `unmet`
only because the gate compared objects by key order (the CLI host sorts reply keys), which
`8b62827a` fixes. The batch-4 run [`runs/20261122-b4/acceptance.json`](runs/20261122-b4/acceptance.json)
(`sha256 1a5abea4…`, tested commit `88e90add`) observed the earlier session termination after
OWN-CASE-03's save-time refusals and was the previous current run. The batch-2 run
[`runs/20261121-b2/acceptance.json`](runs/20261121-b2/acceptance.json) (`sha256 f5c2d53a…`,
tested commit `b088cfc7`) was current before that; its verdicts matched r3 except for the
binary digests and OWN-CASE-05's policy-denied `1 test.abort` command, which the frontend refuses
as `word test.abort is not available in this environment` instead of an internal error. The r3
receipt (`sha256 3d7666bf…`, tested commit
`33318b7c`), the r2 receipt (`sha256 ae533fdd…`, tested commit `81139111`) and the batch-1 re-run
[`runs/20261121-b1/acceptance.json`](runs/20261121-b1/acceptance.json) (`sha256 3d311f50…`, tested
commit `05bac95a`) remain beside it.

## Cases this gate executes

- **OWN-CASE-03** runs the design's captured-I64 leaves (`quote [ + ] compose`: the factory input
  I64 is captured by `quote`, and `[ + ]` adds it to the running value). The balanced tree runs and
  returns 36 before and after the 14 owners other than the root are released. The left- and
  right-deep 8-leaf chains are refused at compose 5 by the anonymous summary's 32-node per-child
  bound (`checked captured value exceeds host observation bound`), as amended by owner decision
  35. That refusal happens while saving the root's output, after the guest settled, so the host's
  release is exact and the session continues (LREF-EXIT-01, owner decision 6; amended by
  integrator decision 39): the lane releases all 12 earlier owners and runs a new root. The 40-leaf
  control behaves alike at compose 5 (44 owners released). The 4-leaf balanced, left-deep and
  right-deep trees each return 10 before and after their 6 non-root owners are released.
- **OWN-CASE-04** registers the design's `Account@1` opaque nominal and its source programs, and the
  lane installs them through the real CLI. The six non-borrow variants are refused at the check
  stage with the exact `required S Data` eligibility diagnostic; the reply itself reports zero
  guest requests, zero protected operations and no emitted artifact. Two controls (an I64 program
  and a resource pass-through) install and emit an artifact, so the harness is observed to emit
  whenever the checker accepts.
- **OWN-CASE-05** records an abnormal exit (a policy-denied effect inside a dispatched frame after
  the target produced a captured Program): no stack (no saved-owner token, and none existed before
  the root), the request trace (`dispatch allowed`, `effect denied`) and the frame-pin ledger. Since
  LREF-EXIT-01 the host releases every frame pin and the map pin from its own ledger, exactly once,
  and reports how many it released itself: the CLI root shows `entered 2 / released 2 / active 0 /
  released_on_abort 0` (the guest's unwinding released both through `leaveFrame`), and the
  in-process root aborted inside both frames shows `released_on_abort 2` with its map pin released
  once. The exit release is not exact, because the guest's recorded failure is terminal for the
  shared guest state, so the session terminates (owner decision 6, poison branch) and the CLI
  exits 2. `test.abort` is still not installable in this profile: the live-slot test hosts grant
  only `test.emit`, and the source is refused as invalid input (`word test.abort is not available
  in this environment`). That refusal is a witness, never the case's success condition.
- **OWN-CASE-01, 02, 06, 07** run the lanes already described in the change's `tasks.md`; their
  observations are read from the lanes' JSONL evidence and printed values, not declared by the gate.
  Every observed field is compared with the case's registered `expected` values; a mismatch is
  recorded as `unmet`, and a case whose design cannot execute is recorded as a failure with the
  observation that refused it.

## The anonymous capture summary bound

A saved Program's anonymous output summary is an exact representation bounded at 32 nodes and 4096
bytes **per child** (`captureValue`), the same per-child limit the selected-origin path encodes.
Anonymous results may therefore carry two such summaries (about 65 cells); selected results are
also capped as a whole by `verifySelectedGraph`. The node limit is what refuses a deep composite
here, before `observeProgramGraph`'s separate 4096-node retained-graph walk can bind. Before this
per-child limit the anonymous summary was capped at 1000 bytes / 64 nodes, whose byte cap bound
first at about ten nodes; admission is now about 32 nodes per child. Roots between the two limits
used to end in `execution-failed` with a poisoned session and now receive an owner.

## Non-claims

- Reclamation is observed through owner refusal and survivors running alike; the runtime exposes no
  live-cell count.
- The exposed per-root frame-pin count is the host ledger of frame entries, exits and abnormal-exit
  releases; the backend's own pin counter is not exposed. A frame pin released on abort keeps its
  backend reference, which is why such an exit never continues the session.
- The receipt is finite behavioral evidence for one binary built from the recorded commit, not a
  universal proof of ownership, reclamation or compiler correctness.
