# Independently admitted runtime Program template identity

## MODIFIED Requirements

### Requirement: P-ID-03
r[P-ID-03]

Different captured data MUST remain distinguishable in portable program-value identity. Function-template identity alone is insufficient.

When an anonymous closed `Program` returned by accepted `quote` and `compose` operations is presented for an identity-bearing boundary, an independently checked admission MUST derive the target's own definition-template `DefinitionId` from that returned program, rather than reuse the source/guest builder's `DefinitionId`, a generic caller's identity, a slot label, a claimed ID, or executable bytes. Ordinary `quote`, `compose`, `run`, and `reflect` retain their existing behavior; they do not require eager identity derivation, source recompilation, proof search, or host authorization merely to construct or execute a program.

Admission MUST reify a bounded immutable snapshot of the actual selected program's VM recipe, captures, complete instantiated ordered interface, effects, resolved builtin/host/dependency identities, and any scoped owner. It MUST independently validate the graph, types, capture eligibility and owner references against the accepted operations and the selected context. A reflected recipe or guest-supplied event stream, ID, source name, memory address, signature descriptor, or artifact manifest is not authentication. Missing, forged, ambiguous, unbounded or stale capture/owner provenance MUST refuse identity-bearing admission before publication or an identity-dependent claim.

The definition-template projection MUST normalize `compose` association and preserve quotation boundaries as in P-RECIPE-03/04. It MUST distinguish fixed source literals from ordered, typed capture slots backed by authenticated runtime `quote` occurrences, preserve repeated-slot aliasing and owner scope, and include the resolved body shape, interface/schema/effect witnesses and semantic dependencies required by P-ID-01. This projection does not erase the actual captured literal from P-RECIPE-04's exact, inert recipe. Source names/spans, VM handles, allocation or optimizer layout MUST NOT determine the template. The same checked template with captured `I64(2)` versus `I64(3)` therefore has the same target template identity but different `ProgramValueId`; changing a fixed literal, slot shape/aliasing, quotation structure, owner or semantic dependency changes the template identity. A named source definition and an anonymous template have distinct identity domains, not an interchangeable ID supplied by the builder.

The selected target `ProgramValueId` MUST bind this independently derived template identity, its exact ordered typed capture values and instantiated interface/schema/effect and dependency identity, rather than the builder's identity or a template alone. Any installed Wasm candidate MUST separately correspond to that accepted target and its exact capture environment; template equality or a matching compiler artifact label is insufficient. P-ID-04 still leaves canonical ID bytes, hashing and cross-implementation stability open: this amendment chooses structural relations, not digest strings. P-ID-06/07, VC-LSLOT-01 and EV-LSLOT-01 continue to govern applicable proof, replay, host authority and artifact evidence separately; deriving an ID establishes none of them.

#### Scenario: ID-05 captured anonymous template

- GIVEN the accepted `quote [ + ] compose` builder in ID-05 with runtime `I64(2)` and `I64(3)` captures and independently authenticated returned programs
- WHEN identity-bearing admission checks each actual returned program and derives its target template and program-value identities
- THEN the definition template is the same while the two program values differ; neither result borrows the builder's identity or obtains a behavioral proof from this identity relation

This draft scenario is a proposed interpretation of the existing unexecuted ID-05 record, not executed ID-05 evidence or a promoted LSLOT-05/08 result.
