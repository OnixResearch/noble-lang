## MODIFIED Requirements

### Requirement: K-PROG-09
r[K-PROG-09]

Under the separately opted Worker Program v2 boundary, a runtime Program handle MUST retain the source-checked ordered instantiated input/output/effect witness and nominal identities of its retained session. A package row or guest-reported handle MUST NOT substitute a checked Program. Runtime lists and selection require one exact common witness, established through explicit checked adaptation rather than erasure.

#### Scenario: v2 runtime list rejects nominal lookalikes

- GIVEN two independently source-checked Programs with equal data layout but different nominal schema identities, and an explicit common list witness from the host-selected session
- WHEN an opted Worker Program v2 guest tries to insert both without an adapter
- THEN the incompatible insertion is refused before invocation or protected operation; the previously admitted Program remains unchanged

### Requirement: K-EFFECT-05
r[K-EFFECT-05]

For Worker Program v2, effect widening MUST check subset inclusion in the host-selected destination ceiling, retain the source recipe unchanged, and retain the original and adapted interface witnesses separately. Neither a package effect claim nor widening creates host authority.

#### Scenario: v2 effect widening and selection

- GIVEN two runtime-selected checked `I64 -- I64` Programs with `{}` and `{test.emit}` effects and an independently selected `{test.emit}` destination witness
- WHEN the pure Program is explicitly adapted, both Programs are stored, selected, duplicated and composed after compilation with no source compiler service
- THEN their common list interface is the selected witness, original recipe and witness are unchanged, composition retains checked effects, and no host request is made before invocation

### Requirement: K-CHECK-07
r[K-CHECK-07]

The Worker Program v2 package decoder, dependency closure, source checker, builder, and import MUST charge a request-shared finite budget; nested validation MUST NOT reset remaining work. Exhaustion must preserve an exhausted primary outcome and publish no partial Program.

#### Scenario: v2 shared finite package admission

- GIVEN a finite source-bound package whose decoded nodes, captures, transitive dependencies and source check fit a host-selected finite budget exactly, and the same package with one additional charged node
- WHEN both are imported under identical independently selected schema and effect ceilings
- THEN the exact-limit candidate is admitted and the over-limit candidate exhausts without a published Program, implicit fetch, candidate-body request or diagnostic overflow changing the primary outcome

### Requirement: P-PACK-01
r[P-PACK-01]

Worker Program v2 uses a separately versioned, bounded portable package: indexed finite Program recipe graph, exact captured typed data, original and adapted interface witnesses, checked definition/schema/builtin/host-contract versions, and scoped recursive owner. Package digests and artifact bytes describe untrusted correspondence, not installed executable authority. Import MUST reconstruct the checked closure under a locally selected ModuleSession and reject missing or substituted transitive dependencies, contract versions and owners without implicit fetching or publication.

#### Scenario: v2 hostile package closure

- GIVEN a valid package with a checked captured Text and named owner, and alternatives with a missing transitive definition, substituted host-contract version, foreign self owner or unrelated artifact
- WHEN import is attempted with no dependency fetch authority
- THEN only the locally source-checked exact dependency graph and witness can be admitted; each hostile alternative rejects before body execution, artifact installation or partial publication

### Requirement: P-PACK-02
r[P-PACK-02]

An opted Worker Program v2 exporter MUST require explicit authorization for both destination and exact captured information before emitting any package bytes; reflected capturable data supplies no export authority. Import MUST recheck semantic source and locally selected contracts even for a signed, digest-matching or formerly accepted package.

#### Scenario: v2 capture export denied

- GIVEN a valid checked Program capturing synthetic private Text and a denied destination/capture-specific export grant
- WHEN the guest requests export
- THEN exactly the request is observable, no package or capture bytes are published and no protected export operation is admitted; denial cannot become source-check failure or a successful no-op
