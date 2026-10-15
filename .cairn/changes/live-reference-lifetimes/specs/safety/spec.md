# Live-reference version lifetimes

## ADDED Requirements

### Requirement: LREF-PKG-01
r[LREF-PKG-01]

In `Live-Slot-Design`, the retention unit of each admitted, published or pinned slot version MUST be its complete executable package. That package comprises:

- compiled code and table entries;
- exact named dependencies read through module globals or table entries, transitively;
- captures and recipes;
- static data and signature entries;
- metadata.

Registry maps, root and frame pins, saved-owner tokens, and saved Programs that escaped from the version's execution MUST each count as owners of every package member they reach.

Retirement MUST proceed in this order:

1. An obsolete map's last pin releases it.
2. The version loses its last map, frame and saved owner.
3. Each package member loses every owner, after which module roots may be released.
4. Zero code-span owners is confirmed.
5. The table span is cleared and the retained-version charge reclaimed.

No step MAY remove a member that a surviving owner still reaches. The registry map swap is the sole commit point. A retirement failure after that point MUST NOT revert the publication and MUST fail closed, for example by poisoning the session. Capacity admission MUST evaluate post-commit reachability over packages.

#### Scenario: LREF-CASE-03 escaped result keeps its version package

- GIVEN a version v1 whose returned Program reads a named helper through its module global, saved as an owner before v2 publishes and the root ends, as in LREF-CASE-03
- WHEN the module sweep runs and the saved Program is invoked and later released
- THEN it returns the checked result while owned, its code span is not cleared before release, and the span retires with its charge reclaimed after release

### Requirement: LREF-EXIT-01
r[LREF-EXIT-01]

Every abnormal root exit MUST release the root's registry map pin and every active frame pin exactly once from the host ledger, without resuming the guest. Abnormal exits include trap, quota exhaustion, host abort, cancellation or worker loss, poisoned transport, and failure while saving outputs.

The root's `LiveRef` inputs MUST expire. No saved-owner token MAY be issued for that root's outputs, and owners provisionally created inside the root MUST be released. Already performed effects MUST NOT be rolled back. A host that continues the session after such an exit MUST keep retention accounting exact: no permanent quota leak, and no surviving root or saved Program invalidated.

#### Scenario: LREF-CASE-01 trapped nested root does not leak its pin

- GIVEN a quota-1 slot B and a root pinned at epoch 1 whose nested B dispatch traps, or exits by quota, host abort, cancellation or a save-time failure, as in LREF-CASE-01
- WHEN the host performs abnormal root cleanup
- THEN the map and frame pins are released exactly once, no saved owner is issued, and a continuing host can publish a freshly authorized B-v2 that retires B-v1
