# M7 selected synchronous Syndicate component service

This delta extends SPEC-W001 only for the local M7 synchronous profile. M5
synchronous and M6 native-async acceptance remain separately bounded and
complete; this selection is not M7 execution evidence.

## MODIFIED Requirements

### Requirement: WI-SYN-01
r[WI-SYN-01]

**WI-SYN-01.** Syndicate/Synit remains the standard Noble concurrency/service model. Where the Syndicate layer crosses Component Model boundaries, its host/component operations SHOULD be exposed through versioned WIT packages rather than bespoke native ABIs.

The first M7 local synchronous selection uses the versioned
`noble:syndicate@1.0.0` `service` world with a `dataspace` import. This is a
bounded WIT boundary on the M5 synchronous component profile, not a native
async service, a general Syndicate implementation or a WASI package.
Participant component instances have separate Noble memory and host-bound
facet identities. The host, not a guest WIT argument or Preserves field,
assigns each facet and its runtime rights.
For acceptance, the independently built Rust/Wasmtime peer supplies
Component Model linking and dynamic typed `Val` conversion; it uses the
accounted Noble production policy and dataspace decisions. This is a
trusted bounded test host, not evidence of generated peer bindings or a
deployable general Syndicate runtime.


<!-- cairn:scenario-links:start -->
#### Scenario: WI-18 for WI-SYN-01

- GIVEN the `Syndicate-Sync-Service-M7` profile and every field of `input` in [WI-18](../../../../../specs/conformance/wit-wasi-cases.json)
- WHEN the `runtime` procedure for case `WI-18` runs against those inputs
- THEN the observations match every field of `expected` in case `WI-18`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: WI-SYN-02
r[WI-SYN-02]

**WI-SYN-02.** Preserves and WIT are complementary. Preserves is the default protocol/schema representation for Syndicate conversational data; WIT defines component interfaces, resources, and ABI-visible operations. A Preserves value MUST NOT become a WIT resource/capability without an explicit validated adapter.

The selected adapter uses only the canonical Preserves text subset and
`service(name:Text,ready:Bool)` schema in S-CONC-04/S-CONC-08. The versioned
WIT methods carry `string` and `bool` through the checked component ABI;
typed `Service` data comes only from the bounded schema-checked decoder, not
from a forged WIT resource representation. Encoding then decoding each actual
published service in the selected compiled-component scenario MUST roundtrip
the name and either Boolean value. Neither roundtrip, schema acceptance nor
component validity establishes authority, proof, or a live resource.


<!-- cairn:scenario-links:start -->
#### Scenario: WI-14 for WI-SYN-02

- GIVEN the `Syndicate-Sync-Service-M7` profile and every field of `input` in [WI-14](../../../../../specs/conformance/wit-wasi-cases.json)
- WHEN the `adapter` procedure for case `WI-14` runs against those inputs
- THEN the observations match every field of `expected` in case `WI-14`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: WI-18 for WI-SYN-02

- GIVEN the `Syndicate-Sync-Service-M7` profile and every field of `input` in [WI-18](../../../../../specs/conformance/wit-wasi-cases.json)
- WHEN the `runtime` procedure for case `WI-18` runs against those inputs
- THEN the observations match every field of `expected` in case `WI-18`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: WI-SYN-03
r[WI-SYN-03]

**WI-SYN-03.** A componentized Syndicate implementation MUST preserve the standard concurrency profile's scope, assertion-lifetime, isolation, capability, and hostile-input safety requirements across the WIT boundary.

The M7 host MUST serialize selected dataspace operations against host-bound
facet ownership. It MUST validate each instance's publication, observation or
retraction right independently of the well-typed WIT call and reject
cross-facet mutation. Trapping a compiled publisher after a successful import
MUST retire its assertion and subordinate facet-owned state without requiring
a second guest call. The independently active observer's exact-pair interest
MUST see the local removal in the host event trace; a failed return from the
trapping export MUST NOT be reported as a successful invocation, even though
its earlier publication was admitted.


<!-- cairn:scenario-links:start -->
#### Scenario: WI-14 for WI-SYN-03

- GIVEN the `Syndicate-Sync-Service-M7` profile and every field of `input` in [WI-14](../../../../../specs/conformance/wit-wasi-cases.json)
- WHEN the `adapter` procedure for case `WI-14` runs against those inputs
- THEN the observations match every field of `expected` in case `WI-14`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: WI-18 for WI-SYN-03

- GIVEN the `Syndicate-Sync-Service-M7` profile and every field of `input` in [WI-18](../../../../../specs/conformance/wit-wasi-cases.json)
- WHEN the `runtime` procedure for case `WI-18` runs against those inputs
- THEN the observations match every field of `expected` in case `WI-18`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: WI-SYN-04
r[WI-SYN-04]

**WI-SYN-04.**

The selected synchronous `noble:syndicate@1.0.0` boundary SHALL use this
versioned WIT surface:

```wit
package noble:syndicate@1.0.0;
interface dataspace {
  publish: func(name: string, ready: bool) -> bool;
  observe: func(name: string, ready: bool) -> bool;
  retract: func(name: string) -> bool;
  fail: func(value: bool) -> bool;
}
world service {
  import dataspace;
  export publisher: func(name: string, ready: bool) -> bool;
  export observer: func(name: string, ready: bool) -> bool;
  export withdraw: func(name: string) -> bool;
  export publish-and-trap: func(name: string, ready: bool) -> bool;
}
```

The compiled `publisher`, `observer` and `withdraw` exports call their
corresponding import directly. `publish-and-trap` calls `dataspace.publish`
then `dataspace.fail`; the selected host's `fail` deliberately traps after
the successful publication to exercise failure cleanup. `observe` installs
an exact `(name,ready)` interest owned by the calling facet and returns
whether that exact assertion is currently present; false is absence of the
queried pair, NOT the service's `ready` value. A separate host event trace
records typed `(name,ready)` add/remove events. All four imported words MUST
retain their exact fully qualified versioned WIT operation identities in
Noble request effects; the WIT signatures alone convey no host right.
Unsupported versions and mismatched signatures MUST be rejected before
starting imports. This synchronous boundary does not implicitly select M6
native async, transport protocols or a general WIT resource conversion.

Acceptance for this selection MUST execute two separately compiled Noble
participant instances with an independent typed component peer: register the
observer's interest, publish and observe each of `ready=true` and
`ready=false`, encode/decode the admitted Preserves values, and observe
removal after retraction and after the compiled publish-then-trap path.
Host-only transition tests, a hand-written component or a byte decoder
roundtrip alone MUST NOT be reported as that compiled scenario. Host
serialization and facet binding, decoder behavior, actual component
execution, kernel correspondence and trusted engine/host boundaries retain
distinct evidence claims; none proves universal component or concurrency
refinement.

<!-- cairn:scenario-links:start -->
#### Scenario: WI-18 for WI-SYN-04

- GIVEN the `Syndicate-Sync-Service-M7` profile and every field of `input` in [WI-18](../../../../../specs/conformance/wit-wasi-cases.json)
- WHEN the `runtime` procedure for case `WI-18` runs against those inputs
- THEN the observations match every field of `expected` in case `WI-18`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

