# M8 projected service component boundary

Canonical authority is `.cairn/specs/wit-wasi/spec.md`; the accepted M7 `noble:syndicate@1.0.0` WIT world and four imports/exports remain unchanged.

## MODIFIED Requirements

### Requirement: WI-SYN-05
r[WI-SYN-05]

**WI-SYN-05.** The bounded M8 choreography descriptor
`noble:choreography/service@1.0.0` SHALL be versioned *data*, not a new WIT
world, WIT resource, authority or replacement for the unchanged
`noble:syndicate@1.0.0` `service` imports/exports in WI-SYN-04.
Its publisher and subscriber projections MUST dispatch those existing
exports under S-CONC-09's trusted pre-export and per-import checks.
The host MUST reject mismatched WIT versions/signatures, wrong export,
role, facet, call order, exact `(name,ready)` pair, import phase and replay
before the unauthorized operation. The pre-export rejection MUST preserve
cursor, dataspace, events, rights and request/protected-operation counts;
post-start refusal MUST still perform required facet trap cleanup, never
claiming that previously owned assertions or their typed removals persist
unchanged. `publish-and-trap` MUST exhibit an actual *successful* `publish`
import followed by `fail`, a real failed compiled invocation and facet retirement with typed
add/remove and subscriber-visible absence; an ordinary `publisher` export
with the same first import or an earlier authorization trap does not satisfy
that step. Direct trusted Rust calls to legacy M7 `Profile` outside an M8
session are not guest-accessible M8 bypasses.
If `publish-and-trap` returns normally or omits `fail` after publication,
the host MUST retire its facet as a protocol failure without advancing
the choreography or reporting a successful publisher invocation.

Selected acceptance MUST use separately compiled Noble participants and
an independently linked typed component peer that routes *every* M8 import
through production monitored policy, not directly around it to M7.
It MUST distinguish pure deterministic projection, host/admission behavior,
compiled execution, source-bound proof and trusted engine/host assumptions.
The M7 synchronous world remains unchanged; no user-defined protocols,
native async, transport, durability, fairness, full Syndicate or universal
compiler/host/engine correspondence is implied.

<!-- cairn:scenario-links:start -->
#### Scenario: WI-19 for WI-SYN-05

- GIVEN the `Choreography-Service-M8` profile and every field of `input` in [WI-19](../../../../../specs/conformance/wit-wasi-cases.json)
- WHEN the `runtime` procedure for case `WI-19` runs against those inputs
- THEN the observations match every field of `expected` in case `WI-19`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: WI-20 for WI-SYN-05

- GIVEN the `Choreography-Service-M8` profile and every field of `input` in [WI-20](../../../../../specs/conformance/wit-wasi-cases.json)
- WHEN the `runtime` procedure for case `WI-20` runs against those inputs
- THEN the observations match every field of `expected` in case `WI-20`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->
