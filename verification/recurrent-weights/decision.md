# Recurrent-weight evaluation — in progress

## Frozen rule

The continuation criteria are fixed in `criteria.md` (commit `0962b348`) and have not been changed since measurements began. U2 has been implemented; the final go/no-go judgement is intentionally pending completion of U3 adversarial controls and U4 unrolled/cost evidence. The earlier provisional NO-GO based on missing units was withdrawn: incomplete work is not an evaluation result.

## Unit disposition so far

| Brief unit | Status | Evidence |
|---|---|---|
| U1 — frozen criteria and design note | DONE | `criteria.md`, `design.md`, commit `0962b348`. |
| U2 — host-loop baseline | DONE | The emitter-independent `CandidateChecker` accepts a program specification, graph, and weights without calling the emitter. Both accumulator and stack-swap graphs pass tiny-domain exhaustive checks; one clean-source test receipt records 1,000-step teacher-forced and free-running accumulator trajectories separately. See `receipts/2026-11-13/host-loop.json`, bound to source commit `94e35636938c15b9d757e1463f91c5776fde3579`. |
| U3 — adversarial controls | IN PROGRESS | Not yet complete. |
| U4 — bounded unrolling and cost comparison | NOT STARTED | Not yet complete. |
| U5 — final decision | PENDING | Must apply the unchanged criteria after U3/U4 evidence exists. |

## Reopen conditions

Not applicable until the final outcome is recorded. Any final NO-GO must distinguish measured criterion failures from unavailable evidence and name concrete evidence that would reopen the decision. Stock-runtime coverage remains “none” unless an actual runtime is independently exercised.
