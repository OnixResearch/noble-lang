# Live-reference diagnostics and replay

## ADDED Requirements

### Requirement: DX-LSLOT-01
r[DX-LSLOT-01]

Reflecting a checked generic live caller MUST expose its inert typed `slot.invoke` recipe/interface/effect, not resolve a target, expose a session slot ID/credential, execute a dispatch or claim selected-target behavior. Runtime traces MUST separately record the pinned root registry epoch, slot incarnation/generation, exact selected ProgramValueId/DefinitionId, captures and admitted artifact, nested dispatch selections and actual authorized/denied effects. Portable replay MUST require an explicitly frozen, independently admitted registry map and matching context/policy inputs; a bare LiveRef or slot ID MUST NOT identify portable behavior. Diagnostics MUST distinguish interface, evidence, authorization, stale CAS, unsupported profile and retention-budget refusals without treating the global registry epoch as `Live-Wasm-Draft` namespace/source generation.

Replay admission MUST reject a frozen map whose global epoch, slot incarnation/generation, exact selected artifact or captures differ from the recorded invocation, and MUST reject a changed semantic context or any missing, substituted, reordered or differently accounted host request/response. All selected versions and artifact correspondence MUST be independently admitted in the frozen context. An eligible deterministic replay is not a behavioral proof and MUST NOT replay protected real-world effects without independent current host authorization.

#### Scenario: LSLOT-08 inert reflection and frozen replay

- GIVEN pinned old roots and saved code, exhausted quota and replay variants in LSLOT-08
- WHEN publication, generic reflection and replay are attempted
- THEN publication refuses while pins remain, succeeds after last-pin retirement and fresh authorization, reflection stays inert, and hostile replay bindings refuse
