# Live-reference design evidence

## ADDED Requirements

### Requirement: EV-LSLOT-01
r[EV-LSLOT-01]

Every LSLOT scenario MUST remain `implementation: absent`, `execution: not-run`, `proof: open`, `trust: unassessed`, `evidence: []` until a separate implementation and actual execution/proof are independently recorded. Later evidence MUST bind the generic caller identity and exact selected target ProgramValueId/DefinitionId, captures, context, claim, assumptions, admitted artifact, pinned slot-registry map/global epoch (distinct from `Live-Wasm-Draft` namespace/source generation), slot incarnation/generation, publication or refusal CAS inputs/outcome, current host policy decisions, nested version selections and actual effect requests/outcomes. Replay evidence MUST also bind exact host-issued typed root inputs and every ordered host request/response/accounting event, and MUST refuse wrong epoch/incarnation/generation, substituted artifact/captures, context or trace order/accounting even with a frozen admitted map. A design gate, wrapper proof, historical LIVE receipt or compatible interface MUST NOT count as live-target execution, replay, behavioral proof or acceptance. Existing LIVE-01..10 and accepted milestone receipts MUST remain unchanged.
