## ADDED Requirements

### Requirement: VC-LIVE-01
r[VC-LIVE-01]

A kernel-checked live-dispatch reference MUST define bounded roots, ordered entry maps, single-slot publications and an ordered guest trace with separately identifiable control events.

It MUST prove that a root's live selections come from its pinned map, that a single-slot publication changes only its own slot, that execution equals a frozen-entry-map reference for the same request sequence, and that two separate single-slot publications are not one atomic replacement. It MUST state that authorized execution alone carries the event-count conclusion, and MUST keep snapshot consistency distinct from coordinated-version coherence.

#### Scenario: VC-LIVE-CASE-01 pinning and mixed maps

- GIVEN a root pinned between two separate single-slot publications
- WHEN it selects the slot both publications touched
- THEN the selection is the second publication's generation, while a root pinned before either publication keeps the first, so the pair is mixed rather than atomically replaced

### Requirement: VC-LIVE-02
r[VC-LIVE-02]

Receipts MUST bind the live-dispatch model to a clean source commit with the inventoried theorem statements and an audited axiom set.

The reference MUST NOT promote canonical live-slot status, and MUST NOT claim runtime correspondence from kernel checking alone.

#### Scenario: VC-LIVE-CASE-02 receipt scope

- GIVEN the retained kernel receipt
- WHEN its scope statement is read
- THEN it names the clean commit, the theorem count, the live-dispatch theorems, the axiom allow-list and the absence of any runtime-correspondence claim

### Requirement: VC-LIVE-03
r[VC-LIVE-03]

Implementation comparison MUST bind an existing supported observation or trace rather than inventing a host API, and MUST include a broken per-dispatch-current-map control that exposes the duplicate or omitted event.

An event-count conclusion MUST NOT apply when a denied dispatch or effect stopped execution before the event, and committed prefixes MUST NOT be presented as rollback.

#### Scenario: VC-LIVE-CASE-03 relocation witness

- GIVEN two interface-compatible versions where the later one moves a distinguished event from the caller into the callee
- WHEN the broken per-dispatch-current-map control runs
- THEN the mixed selection duplicates or omits the event, and the coherent all-old or all-new entry maps produce exactly one event under successful authorized execution
