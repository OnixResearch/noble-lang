# Version-bound live-target contracts

## ADDED Requirements

### Requirement: VC-LSLOT-01
r[VC-LSLOT-01]

Optional behavioral evidence required by host policy for a live call MUST independently match the exact selected target ProgramValueId and DefinitionId, captures, instantiated ordered interface, semantic context, claim, assumptions and admitted executable artifact correspondence under P-ID-06. An unchanged generic caller's type-safety or proof, an older target's proof, a name or compatible effect interface MUST NOT transfer behavioral claims to a replacement. Missing or inapplicable evidence MUST refuse the proof-required selection without running the candidate; ordinary typed execution does not acquire a universal proof requirement. Pinned old versions MAY retain independently applicable old evidence only for that exact old subject and context, subject to current host policy.

#### Scenario: LSLOT-05 exact selected-target evidence

- GIVEN wrong-version, capture, context, claim, assumption, artifact and wrapper-only evidence in LSLOT-05
- WHEN proof-required target admission checks each against the selected candidate
- THEN only independently applicable exact subject/artifact evidence can pass
