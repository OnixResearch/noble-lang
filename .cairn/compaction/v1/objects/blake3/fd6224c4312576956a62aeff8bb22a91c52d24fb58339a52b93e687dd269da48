## Why

M7 completed a finite local synchronous service, but its publisher and subscriber are still manually ordered by a host. M8 selects one bounded choreography whose global service actions can be checked and projected to the two existing M7 roles. A projection is a static contract, not evidence that compiled participants obey it; the latter needs separate runtime admission and execution evidence.

## What Changes

- Select `Choreography-Service-M8` with protocol identity `noble:choreography/service@1.0.0`: one or two ordered rounds over M7 canonical `service(name:Text,ready:Bool)`, each with a distinct name, Boolean readiness and `withdraw` or terminal `trap` exit. The only roles are publisher and subscriber. Accept only bounded strict UTF-8 JSON descriptor bytes with exact typed keys, duplicate/unknown-key and trailing non-whitespace rejection, independent of M7 Preserves. Define the exact global action expansion and role-local order-preserving projections.
- Require admission of the entire finite descriptor and projected plan before any M7 facet or guest effect, binding the selected M7 WIT world, host-issued facets and rights, exclusive empty dataspace, complete finite quota reservation, and the next global action to a serialized session cursor. Pre-export mismatch changes no cursor/state/trace; in-flight refusal has no unauthorized operation or cursor advance but still permits mandatory facet retirement/removal. A trap is an observed failure/retirement transition, never a successful export or rollback.
- Add positive S-CASE-18 and WI-19 designs for static projections and separately compiled execution; hostile S-CASE-19 and WI-20 designs cover malformed/over-budget descriptors, wrong role/order/facet, forged authority/version and no partial effects. Preserve M7 execution and proof claims as separate records.

## Impact

- **Contract**: `.cairn/specs/safety/spec.md` and `.cairn/specs/wit-wasi/spec.md` with change deltas; cases in `specs/conformance/{safety,wit-wasi}-cases.json`; generated compatibility views and requirement ledger; supporting roadmap/status/decision records.
- **Later implementation/verification**: projector/admission and session monitor must compose existing M7 production policy, kernel quotas and `noble:syndicate/service@1.0.0` exports. Execute source-bound positive and every hostile variant through a genuinely independent typed component peer before promoting case states. A document check alone is not M8 execution evidence.
- **Non-goals**: user-defined protocols, transport, native async, durability, fairness/progress, distributed delivery, full Syndicate/Preserves, universal compiler/host/engine correctness and closure of M7's open authored-body proof obligations.
