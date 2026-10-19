# M7 bounded local dataspace and Preserves adapter

This delta extends SPEC-S001 for the first synchronous service, not the full
Syndicate/Synit concurrency profile. The canonical source is
`.cairn/specs/safety/spec.md`; the scenarios remain designs until independently
executed evidence is retained.

## MODIFIED Requirements

### Requirement: S-CONC-01
r[S-CONC-01]

**S-CONC-01.** Concurrent Noble participants in the standard profile MUST NOT obtain direct shared mutable Noble memory. Cross-participant interaction occurs through immutable data, typed dataspace assertions/interests/messages, explicit host operations, or profile-defined ownership transfer of resources.

The selected M7 profile MUST reject an actual submitted component/core module
requesting shared mutable guest memory before creating a facet, starting a
guest import or granting a participant. The selected component loader MUST
also reject that shared-memory module. Admission MUST bind the rejection
decision to inspected module memory facts, not only a caller-supplied Boolean
or the observation that separately instantiated Stores did not share memory.


<!-- cairn:scenario-links:start -->
#### Scenario: S-CASE-11 for S-CONC-01

- GIVEN the `Syndicate-Sync-Service-M7` profile and every field of `input` in [S-CASE-11](../../../../../specs/conformance/safety-cases.json)
- WHEN the `admission` procedure for case `S-CASE-11` runs against those inputs
- THEN the observations match every field of `expected` in case `S-CASE-11`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: S-CASE-17 for S-CONC-01

- GIVEN the `Syndicate-Sync-Service-M7` profile and every field of `input` in [S-CASE-17](../../../../../specs/conformance/safety-cases.json)
- WHEN the `runtime` procedure for case `S-CASE-17` runs against those inputs
- THEN the observations match every field of `expected` in case `S-CASE-17`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: S-CONC-02
r[S-CONC-02]

**S-CONC-02.** The absence of guest shared mutable memory SHALL imply **Noble-visible data-race freedom** for the standard concurrency profile. This claim does not automatically prove the Rust runtime itself race-free; runtime synchronization must separately refine the abstract concurrency model.


<!-- cairn:scenario-links:start -->
#### Scenario: S-CASE-11 for S-CONC-02

- GIVEN the `Syndicate-Sync-Service-M7` profile and every field of `input` in [S-CASE-11](../../../../../specs/conformance/safety-cases.json)
- WHEN the `admission` procedure for case `S-CASE-11` runs against those inputs
- THEN the observations match every field of `expected` in case `S-CASE-11`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: S-CONC-03
r[S-CONC-03]

**S-CONC-03.** Assertions, interests, reactions, and subordinate conversational activity MUST have explicit owners/scopes. Facet/actor termination SHALL retract or retire scope-owned conversational state according to the standard profile, including failure paths.

In the selected M7 local synchronous service, the host binds each participant
instance to a facet and checked rights; guest-provided names and protocol bytes
are not facet identities. An active facet owns its assertions, interests, and
children. Normal exit and trap MUST retire its descendants, assertions and
interests once, without requiring another guest call or retaining an assertion
as visible after its owner has terminated. A different active observer's
matching interest can observe the local retraction. This is local scope
cleanup, not remote rollback or exactly-once external cleanup.


<!-- cairn:scenario-links:start -->
#### Scenario: S-CASE-12 for S-CONC-03

- GIVEN the `Syndicate-Sync-Service-M7` profile and every field of `input` in [S-CASE-12](../../../../../specs/conformance/safety-cases.json)
- WHEN the `adapter` procedure for case `S-CASE-12` runs against those inputs
- THEN the observations match every field of `expected` in case `S-CASE-12`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: S-CASE-17 for S-CONC-03

- GIVEN the `Syndicate-Sync-Service-M7` profile and every field of `input` in [S-CASE-17](../../../../../specs/conformance/safety-cases.json)
- WHEN the `runtime` procedure for case `S-CASE-17` runs against those inputs
- THEN the observations match every field of `expected` in case `S-CASE-17`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: S-CONC-04
r[S-CONC-04]

**S-CONC-04.** Network or process-boundary protocol data MUST be decoded under bounded resource limits and validated against its Preserves schema/protocol before it becomes a typed Noble protocol value. Preserves embedded/native extension mechanisms MUST NOT by themselves create Noble resources or authority.

The M7 adapter accepts only the selected, canonical Preserves text subset for
`service(name:Text,ready:Bool)`: `<service "NAME" #t>` or
`<service "NAME" #f>`, with exactly the shown delimiters and spaces, and an
ASCII `NAME` of one to 32 characters from `[A-Za-z0-9_-]`. Admission MUST
enforce at most 64 input bytes and nesting depth four, reject malformed UTF-8,
overlong input, excess nesting, noncanonical spelling, escaped names,
annotations, embedded/native extensions, trailing content and schema
mismatches, and create a typed `Service` only after all checks succeed.
Encoding an admitted `Service` MUST emit that same canonical subset and
respect the same byte and depth limits. The source
[Preserves: Text Syntax, v0.996.3 (June 2025)](https://preserves.dev/preserves-text.html)
defines records as `<` followed by label `Value` and field `Value`s then
`>`, permits bare symbol `service`, JSON-style strings and `#t`/`#f`
Booleans; it also permits syntax this profile intentionally rejects.
These limits bound the selected adapter's admitted input and traversal; they
are not a heap bound or general Preserves implementation.


<!-- cairn:scenario-links:start -->
#### Scenario: S-CASE-09 for S-CONC-04

- GIVEN the `Syndicate-Sync-Service-M7` profile and every field of `input` in [S-CASE-09](../../../../../specs/conformance/safety-cases.json)
- WHEN the `adapter` procedure for case `S-CASE-09` runs against those inputs
- THEN the observations match every field of `expected` in case `S-CASE-09`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: S-CASE-10 for S-CONC-04

- GIVEN the `Syndicate-Sync-Service-M7` profile and every field of `input` in [S-CASE-10](../../../../../specs/conformance/safety-cases.json)
- WHEN the `adapter` procedure for case `S-CASE-10` runs against those inputs
- THEN the observations match every field of `expected` in case `S-CASE-10`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: S-CONC-05
r[S-CONC-05]

**S-CONC-05.** Typed protocol validity and runtime authority are distinct. A participant may only publish, observe, message, or transfer what both its static protocol interface and its runtime capability permit.

For the selected two-participant service, the publisher's assertion and the
subscriber's exact-pair interest belong to distinct host-bound facets.
Publication and observation require separate checked facet rights. The
`observe(name,ready)` result is exact assertion membership, not the `ready`
field: querying a missing `(name,false)` returns false; querying a published
`(name,false)` returns true. A valid wire value or WIT signature cannot supply
the rights, impersonate another facet, or turn an untrusted resource-shaped
payload into a resource.


<!-- cairn:scenario-links:start -->
#### Scenario: S-CASE-10 for S-CONC-05

- GIVEN the `Syndicate-Sync-Service-M7` profile and every field of `input` in [S-CASE-10](../../../../../specs/conformance/safety-cases.json)
- WHEN the `adapter` procedure for case `S-CASE-10` runs against those inputs
- THEN the observations match every field of `expected` in case `S-CASE-10`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: S-CASE-17 for S-CONC-05

- GIVEN the `Syndicate-Sync-Service-M7` profile and every field of `input` in [S-CASE-17](../../../../../specs/conformance/safety-cases.json)
- WHEN the `runtime` procedure for case `S-CASE-17` runs against those inputs
- THEN the observations match every field of `expected` in case `S-CASE-17`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: S-CONC-06
r[S-CONC-06]

**S-CONC-06.** Deadlock freedom, global progress, fairness, and service availability MUST remain separate claims. They require explicit protocol/profile assumptions or proofs and are not implied by race freedom or type safety.

The bounded M7 local synchronous service promises serialized local decisions,
not progress for absent participants, fairness, transport delivery, durability,
distributed exactly-once publication, or native-async task semantics.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: S-CONC-07
r[S-CONC-07]

**S-CONC-07.**

The selected M7 dataspace SHALL serialize local facet creation, checked
interest registration, assertion publication, exact-pair observation,
retraction and facet termination. Only an active host-bound facet with the
applicable runtime right may mutate its owned state; an invalid owner, retired
facet, failed preflight or exceeded finite capacity MUST leave retained state
and its visible assertions unchanged. The selection admits at most eight
facets, eight assertions, eight interests and 32 retained events; capacity
for a state change, immediate notifications and every already-owed future
removal notification MUST be reserved before commitment. This reservation
MUST leave enough event capacity for mandatory facet retirement even when no
further guest call can run; retirement MUST NOT be refused solely because
the event buffer later becomes full.
`observe` registers one interest per `(facet,name,ready)` idempotently even
when that assertion is absent. A registered exact-pair interest belongs to
its observer facet and records a local add when a matching assertion becomes
visible and a local remove when it ceases to be visible. Publishing the same
`(facet,name,ready)` again is idempotent; publishing changed `ready` for the
same `(facet,name)` atomically removes the prior assertion and adds its
replacement after preflight. `retract(name)` removes only that facet's
assertion and reports whether one existed. The host MUST distinguish these
events from an `observe` membership return and MUST NOT report a stale
queued add as a currently live assertion after retraction.

| Local operation | Required state and rights | Serialized outcome |
|---|---|---|
| Open facet | New root or live parent; free facet capacity | Active host-bound facet, child owned by parent when present |
| Observe `(name,ready)` | Active facet with observation right; reserved interest/event capacity | Idempotent exact-pair interest and current membership Boolean |
| Publish `(name,ready)` | Active facet with publication right; validated service and reserved assertion/event capacity | One owned assertion per `(facet,name)`; identical publish does nothing, changed readiness removes old then adds new |
| Retract `name` | Active facet with publication right | Remove only its owned assertion and return true, or preserve state and return false if missing |
| Retire facet, including trap | Active facet and its descendants | Remove descendant interests and assertions once, emit owed removals to still-live matching observers, retire the subtree without guest resumption |

An unadmitted operation, foreign facet, revoked right or retired facet MUST
NOT mutate the dataspace, create a notification or transfer ownership.
Notifications are local serialized observations, not remote delivery
guarantees.

For the first service scenario, two separately compiled Noble component
instances share no mutable Noble memory. A publisher facet publishes
`service(name:Text,ready:Bool)`; a subscriber facet registers an exact-pair
interest and observes publication. A trap after publication terminates the
publisher facet and retracts its assertion, while the still-live subscriber
observes removal. Both `ready=true` and `ready=false` MUST be exercised as
distinct exact-pair assertions. The abstract local transitions do not assert
that a trap rolls back an admitted operation or that a remote peer receives an
event exactly once.

<!-- cairn:scenario-links:start -->
#### Scenario: S-CASE-12 for S-CONC-07

- GIVEN the `Syndicate-Sync-Service-M7` profile and every field of `input` in [S-CASE-12](../../../../../specs/conformance/safety-cases.json)
- WHEN the `adapter` procedure for case `S-CASE-12` runs against those inputs
- THEN the observations match every field of `expected` in case `S-CASE-12`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: S-CASE-17 for S-CONC-07

- GIVEN the `Syndicate-Sync-Service-M7` profile and every field of `input` in [S-CASE-17](../../../../../specs/conformance/safety-cases.json)
- WHEN the `runtime` procedure for case `S-CASE-17` runs against those inputs
- THEN the observations match every field of `expected` in case `S-CASE-17`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: S-CONC-08
r[S-CONC-08]

**S-CONC-08.**

The selected Preserves subset adapter MUST reject inputs exceeding 64 bytes,
nesting depth four, the declared service schema or the exact canonical
encoding before creating a typed protocol value. Its input traversal and
output encoding MUST be bounded by those limits, with overflow and UTF-8
validation before allocation or typed admission. An authenticated host-bound
facet and its rights remain distinct from decoded `Service` data. A payload
field, embedded/native extension, or successful byte roundtrip MUST NOT
manufacture a facet, WIT resource, authority or proof claim. A positive
roundtrip MUST exercise actual published values from compiled Noble
components, not only a host-only test vector.

<!-- cairn:scenario-links:start -->
#### Scenario: S-CASE-09 for S-CONC-08

- GIVEN the `Syndicate-Sync-Service-M7` profile and every field of `input` in [S-CASE-09](../../../../../specs/conformance/safety-cases.json)
- WHEN the `adapter` procedure for case `S-CASE-09` runs against those inputs
- THEN the observations match every field of `expected` in case `S-CASE-09`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: S-CASE-10 for S-CONC-08

- GIVEN the `Syndicate-Sync-Service-M7` profile and every field of `input` in [S-CASE-10](../../../../../specs/conformance/safety-cases.json)
- WHEN the `adapter` procedure for case `S-CASE-10` runs against those inputs
- THEN the observations match every field of `expected` in case `S-CASE-10`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: S-CASE-17 for S-CONC-08

- GIVEN the `Syndicate-Sync-Service-M7` profile and every field of `input` in [S-CASE-17](../../../../../specs/conformance/safety-cases.json)
- WHEN the `runtime` procedure for case `S-CASE-17` runs against those inputs
- THEN the observations match every field of `expected` in case `S-CASE-17`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: WI-14 for S-CONC-08

- GIVEN the `Syndicate-Sync-Service-M7` profile and every field of `input` in [WI-14](../../../../../specs/conformance/wit-wasi-cases.json)
- WHEN the `adapter` procedure for case `WI-14` runs against those inputs
- THEN the observations match every field of `expected` in case `WI-14`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

