# Protocol-observable bounded region adapter

## ADDED Requirements

### Requirement: EPROTO-OBS-01
r[EPROTO-OBS-01]

Before any `RegionProtocol-v1` runtime observation counts as protocol evidence, the bounded-region host report MUST be a new schema version that keeps the existing `noble-bounded-region/v1` receipts unchanged.

The new report MUST:
- separately count guest releases, destructor releases and host retirements, with abnormal cleanup recording its retirement reason;
- classify guest trap, quota exhaustion, scope exhaustion, cancellation and outside-model responses distinctly (as H-OUTCOME-01 requires), instead of a single `adapter-trap` or `invalid-handle`;
- report any owner or native pin that remains after cleanup;
- record an ordered event trace of requests, responses and committed effects up to the abort point.

Cancellation claims additionally require a real host interruption control that revokes guest access immediately. Until that control exists, cancellation remains unsupported. A finite monitored trace is evidence about that execution only and MUST NOT be reported as a universal protocol proof.
