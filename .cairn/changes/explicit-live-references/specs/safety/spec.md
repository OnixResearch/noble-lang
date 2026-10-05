# Live-reference registry and authority

## ADDED Requirements

### Requirement: S-LSLOT-01
r[S-LSLOT-01]

Each root call MUST atomically pin one immutable persistent slot-registry map and strictly monotonically increasing global epoch at entry using an O(1) map reference; every nested/transitive live dispatch within that root MUST select from that same pinned map, even after another map publishes. At each dispatch the host MUST independently check current invocation authority for the exact selected slot/version, and each actual requested effect MUST pass current host authorization; a LiveRef, slot ID, effect ceiling, proof, or previously granted policy MUST NOT act as authority. Candidate admission/publication MUST check exact ordered input/output and nominal resource/schema identity and a candidate effect bound within the slot ceiling before any candidate execution. Failure, quota exhaustion or stale publication MUST leave the published registry unchanged and execute zero candidate bodies. Pinned old code and independently saved exact Programs MUST remain usable under their current host policy and bounded retention; publication MUST refuse when capacity cannot retain them, not invalidate either. No implicit resource/state migration or rollback of already performed effects is selected.

The host MUST account independently for current registry reachability, each old root/in-flight frame's map pin, and each saved exact Program's code ownership. After obsolete maps lose their last root/frame pin and old code loses its last map/frame/saved-Program owner, the host MUST retire that code and reclaim its retained-version budget; releasing only one of several owners MUST NOT invalidate the others. Post-commit reachable-version budgeting MUST permit a newly authorized publication after the last old owner releases without a permanent quota leak.

#### Scenario: LSLOT-02 one snapshot for nested dispatch

- GIVEN root A pinned at epoch 1 with explicit host-issued A/B refs and publication of B-v2 at epoch 2 during A in LSLOT-02
- WHEN A invokes B again and a later root invokes B
- THEN A selects B-v1 throughout and the next root selects B-v2

#### Scenario: LSLOT-03 ordered interface and effect admission

- GIVEN the exact and hostile type/schema/effect candidates in LSLOT-03
- WHEN admission checks them without candidate execution
- THEN only the exact compatible candidate publishes and dispatch includes `live.dispatch`

#### Scenario: LSLOT-07 current authorization after pin

- GIVEN independent publication, dispatch and actual-effect revocations in LSLOT-07
- WHEN an old pinned root reaches the nested request
- THEN revoked authority denies protected work despite a ref, slot ID or prior permission
