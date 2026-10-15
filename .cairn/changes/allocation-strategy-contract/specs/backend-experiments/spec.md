# Replaceable allocation strategy contract

## ADDED Requirements

### Requirement: ALLOC-CAP-01
r[ALLOC-CAP-01]

Each guest storage strategy MUST declare a fixed capability set. Allocation and liveness checks are common to every strategy. Individual release, mark/reset, instance checkpoint/restore, relocation and engine-managed root release are optional capabilities. An unsupported operation MUST be refused explicitly rather than emulated. For example, no strategy may expose a non-releasing operation as individual free.

Each optional operation MUST be invoked only after the ownership and lifetime layer establishes its precondition: no counted owner for release, and no retained reference above a mark for reset. Relocation additionally requires that every root be known and rewritable.

Strategy selection MUST be fixed per profile before execution, with limits fixed per instance. Every live object MUST keep the provenance of its issuing strategy, instance and region. Release, reset and relocation MUST refuse a handle of other provenance.

A cell MUST be published only after all of its fields are initialized. Handle range checks MUST precede address arithmetic. Epoch-qualified handles MUST NOT wrap; exhaustion MUST permanently retire the affected slot. Any change to the existing 48-byte cell representation requires a separately selected profile.

#### Scenario: ALLOC-CASE-03 nested rollback honors capabilities

- GIVEN nested marks m1 < m2 and a mid-reset failure injection in ALLOC-CASE-03, together with a reset request on a strategy without the reset capability
- WHEN resets are requested in LIFO, out-of-order and unsupported forms
- THEN LIFO resets succeed atomically, an invalidated inner mark refuses, the unsupported strategy refuses explicitly, and no host request is rolled back

### Requirement: ALLOC-OBS-01
r[ALLOC-OBS-01]

Each storage strategy MUST identify its accounting revision. It MUST state when released capacity becomes reusable: never within the session, after a boundary compaction, after restore, immediately, after cleanup, or as engine-defined.

Reports MUST keep the following measurements distinct:

- logical allocation and live-cell budgets;
- committed memory pages;
- allocator metadata bytes;
- fragmentation;
- retained bytes;
- engine heap;
- process RSS.

A logical release counter MUST NOT be reported as physical memory returned to the engine or operating system.

#### Scenario: ALLOC-CASE-12 logical release is not physical return

- GIVEN cells released under a linear-memory strategy in ALLOC-CASE-12
- WHEN logical release, live-byte and committed-memory metrics are read
- THEN logical release and live bytes change while committed pages are reported unchanged, and no physical-return claim is made

### Requirement: ALLOC-ISO-01
r[ALLOC-ISO-01]

For the same accepted program and inputs, compatible storage strategies MUST produce identical successful values, normalized recipes, identities, effect order and host request/response traces. Quota outcomes MAY differ only through declared accounting revisions, and a quota outcome MUST NOT count as success. Logical quotas MUST remain deterministic and independent of physical allocation timing.

Instances and regions MUST stay isolated. A handle presented to a host operation of a different instance MUST be refused before any cell access.

Changing a host-process or native allocator MUST NOT be described as changing Wasm guest-cell allocation, and MUST NOT change the guest representation. A new native allocator is a new trusted native boundary and MUST be identified as such.

#### Scenario: ALLOC-CASE-11 differential equivalence across strategies

- GIVEN the ALLOC-CASE-11 programs executed under each pair of compatible strategies, plus the ALLOC-CASE-07 cross-instance handle
- WHEN results, recipes, identities and host traces are compared
- THEN successful observations are identical, quota differences appear only under declared accounting revisions, and the foreign handle is refused
