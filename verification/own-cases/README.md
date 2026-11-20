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

[`acceptance.json`](acceptance.json) is the latest receipt. Superseded receipts stay immutable;
the rework receipts are added beside them, never edited.

## Cases this gate executes

- **OWN-CASE-03** runs the design's captured-I64 leaves (`quote [ + ] compose`: the factory input
  I64 is captured by `quote`, and `[ + ]` adds it to the running value). The balanced tree runs and
  returns 36 before and after the 14 owners other than the root are released. The left- and
  right-deep 8-leaf chains are refused at compose 5 by the anonymous summary's 32-node per-child
  bound (`checked captured value exceeds host observation bound`), which terminates the session.
  The design's deep trees are therefore not executed; the case records the refusal and is not
  promoted on that receipt.
- **OWN-CASE-04** registers the design's `Account@1` opaque nominal and its source programs, and the
  lane installs them through the real CLI. The six non-borrow variants are refused at the check
  stage with the exact `required S Data` eligibility diagnostic; the reply itself reports zero
  guest requests, zero protected operations and no emitted artifact. Two controls (an I64 program
  and a resource pass-through) install and emit an artifact, so the harness is observed to emit
  whenever the checker accepts.
- **OWN-CASE-05** uses a policy-denied effect inside a dispatched frame after the target produced a
  captured Program. The failing root's reply carries the request trace (`dispatch allowed`,
  `effect denied`), no stack (no saved-owner token), the session state and the per-root frame-pin
  ledger: every frame pin the root held is released by `abortRoot` exactly once (`entered ==
  released`, `active == 0`). `test.abort` is still not installable in this profile; its internal
  `inconsistent frontend arena` refusal is recorded as a frontend-defect witness, never as the
  case's success condition.
- **OWN-CASE-01, 02, 06, 07** run the lanes already described in the change's `tasks.md`; their
  observations are read from the lanes' JSONL evidence and printed values, not declared by the gate.

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
- The exposed per-root frame-pin count is the host ledger of frame entries and exits; the backend's
  own pin counter is not exposed.
- The receipt is finite behavioral evidence for one binary built from the recorded commit, not a
  universal proof of ownership, reclamation or compiler correctness.
