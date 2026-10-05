# Live-reference profile boundary

## ADDED Requirements

### Requirement: B-LSLOT-01
r[B-LSLOT-01]

`Live-Slot-Design` MUST remain unsupported in current `Core-Bootstrap`, ordinary Core sessions and guarded `Live-Wasm-Draft` unless separately implemented, checked and advertised. Such attempts MUST fail explicitly, without interpreting `slot.invoke` as ordinary `run`, defaulting its effects to empty, invoking a candidate, or inheriting LIVE case acceptance. Historical M4, MC2, DXM1 and LIVE status/evidence MUST remain unchanged. This design alone supplies no executable slot representation or accepted profile.

#### Scenario: LSLOT-09 existing profiles remain unsupported

- GIVEN the legacy profile attempts and ordinary captured controls in LSLOT-09
- WHEN profile admission runs
- THEN slot dispatch is explicitly unsupported, while ordinary capture semantics remain unchanged
