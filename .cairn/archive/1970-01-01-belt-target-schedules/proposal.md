## Why

Issue #113 defines a target-neutral checked belt artifact. Issue #114 extends
that work with two explicit target timing profiles and an independently checked
schedule model. A target schedule is a different artifact and legality relation
from the sequential belt stream; it does not change #113's portable semantics.

r[NGRAPH-BELT-TARGET-01] r[NGRAPH-BELT-TARGET-02]
r[NGRAPH-BELT-TARGET-03] r[NGRAPH-BELT-TARGET-04]

## What Changes

- **ADDED** `NGRAPH-BELT-TARGET-01`: the versioned `ngraph-belt-machine/0`
  descriptor family with fixed T1/T2 resource, belt, and
  latency descriptors.
- **ADDED** `NGRAPH-BELT-TARGET-02`: issue, operand-read, completion, ordering,
  and final output timing rules.
- **ADDED** `NGRAPH-BELT-TARGET-03`: independent target-schedule legality and
  semantic-preservation obligations.
- **ADDED** `NGRAPH-BELT-TARGET-04`: controlled fixtures, mutation classes,
  and evidence boundaries.

This is a design package only. It chooses a precise model so a later scoped
implementation can be evaluated; it adds no target machine checker, scheduler,
Rust behavior, or performance claim.

## Impact

- Affected native spec: `backend-experiments` (ADDED deltas only).
- Depends on the target-neutral checked artifact in `belt-backend-v0`.
- T1 and T2 are abstract timing models, not Mill or hardware specifications.
- #37 retains net semantics and #43 retains guard-aware packed execution; no
  dependency or completion claim for either issue is introduced.
