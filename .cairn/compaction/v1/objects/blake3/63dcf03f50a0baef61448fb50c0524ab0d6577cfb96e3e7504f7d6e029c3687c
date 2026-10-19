# M8 finite choreography safety

Canonical authority is `.cairn/specs/safety/spec.md`; this change does not revise accepted M7 requirements or their execution receipts.

## MODIFIED Requirements

### Requirement: S-CONC-09
r[S-CONC-09]

**S-CONC-09.** The selected `Choreography-Service-M8` profile SHALL project only
the versioned data descriptor `noble:choreography/service@1.0.0`, never
interpreting it as a WIT world, facet authority or executable guest code.
Its entire descriptor has exactly `protocol` and `rounds` fields; `rounds`
has one or two records with exactly `name`, `ready`, `exit`. Each `name`
MUST be distinct and match M7's ASCII `[A-Za-z0-9_-]{1,32}`; `ready` MUST be
Boolean; `exit` MUST be `withdraw` or `trap`, with `trap` only in the last
round. The selected untrusted descriptor boundary SHALL accept only strict
UTF-8 JSON bytes, not an arbitrary transport or a new M7 Preserves tag.
Before constructing trusted typed descriptor values it MUST enforce at most
512 input bytes, valid UTF-8 and JSON container nesting depth at most three
(root object, `rounds` array, round object; count `{` and `[` outside
strings). It MUST parse one complete JSON value with no trailing content
except JSON whitespace, reject duplicate object keys at either object
level, unknown fields, unsupported versions, invalid types or names, zero
or more than two rounds, duplicate names and nonterminal traps before
admission or guest effects. JSON strings MAY use standard escapes; validate
the *decoded* protocol/name characters and lengths. The JSON bytes do not
assign a facet, role, WIT resource or authority. This boundary defines
descriptor decoding only; it selects no file/network transport, general
JSON protocol or replacement for M7's service-only Preserves decoder.

For `(name,ready)=(n,b)`, a withdrawal round expands, in order, to
`subscriber.observer(n,b)=false`, `publisher.publisher(n,b)=true`,
`subscriber.observer(n,b)=true`, `publisher.withdraw(n)=true`,
`subscriber.observer(n,b)=false`. A terminal trap round expands to
`subscriber.observer(n,b)=false`,
`publisher.publish-and-trap(n,b)=trap-after-publication`,
`subscriber.observer(n,b)=false`. The trap MUST observe a successful
`publish` import followed by `fail`, commit the publication, produce
typed add/remove on a real invocation error and M7 retirement, and leave the active
subscriber seeing absence, without reporting publisher success. False
readiness is an exact-pair field, not absence. Role-local projections MUST
be the stable ordered subsequences of these global actions, preserving
arguments, expected results and global/round ordinals. Projection is static
data; execution, admission, fairness and compiler correspondence are
separate claims.

Only one M8 session MAY occupy an initially empty M7 dataspace, using
exactly two separately compiled participant instances and host-issued
publisher/subscriber facets. Before creating facets or invoking a guest,
admission MUST validate the complete descriptor, projections, exact
`noble:syndicate/service@1.0.0` WIT signatures, distinct host-bound roles,
rights and capacity for two facets, one simultaneous assertion, two
interests and four retained add/remove events, including immediate
notifications and owed removal/cleanup on trap. M7's preflight still
applies. No competing observer/session/activity is admitted.

The trusted host SHALL pre-admit each export's identity, arguments,
caller role/facet, next global step, exact pair and expected pre-state
before invoking the export. This MUST reject a wrong `publisher` export
where `publish-and-trap` was required even though both first import
`publish`. Pre-export refusal MUST leave the session cursor, dataspace,
events, rights and guest-request/protected-operation counts unchanged.
Every imported operation MUST additionally pass an owner-bound,
phase-specific check before forwarding to M7. The monitor MUST hold at most
one in-flight export with expected import substeps and refuse interleaved
exports. Pre-export admission, each import precheck/M7 commit/phase update
and post-call cursor finalization MUST use the same serialized critical
section, released while component code runs to permit host callbacks.
The lock order MUST be monitor then M7 dataspace for admission, imports and
retirement; it MUST NOT be reversed or held across a guest call. The
pre-export check atomically reserves the in-flight phase, each import
updates its pending substate atomically with its M7 effect, and finalization
after `function.call` plus `post_return` success or trap plus mandatory
retirement commits the next global cursor. There is no unmonitored M8
route to M7. Wrong order/role/replay, forged facet/rights,
wrong pair or an unexpected import MUST NOT cause an unauthorized M7
operation, protected effect or session cursor advance. An in-flight refusal
MAY trap and trigger mandatory M7 retirement of previously owned state and
typed removal events; it MUST NOT be misreported as unchanged dataspace
or as a completed choreography. Only an observed expected import sequence,
return value (or successful compiled `publish` import, subsequent `fail`
import, invocation error and facet retirement/removal, not an earlier
authorization failure) and
final subscriber absence advance the corresponding step and finish the plan.
An unexpected successful return, including one omitting `fail` after a
successful trap publication, MUST terminate the publisher facet as a
protocol failure, retract its assertion and not advance or report success.
No durability, remote transport, global progress or general Syndicate
claim follows.

<!-- cairn:scenario-links:start -->
#### Scenario: S-CASE-18 for S-CONC-09

- GIVEN the `Choreography-Service-M8` profile and every field of `input` in [S-CASE-18](../../../../../specs/conformance/safety-cases.json)
- WHEN the `static` procedure for case `S-CASE-18` runs against those inputs
- THEN the observations match every field of `expected` in case `S-CASE-18`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: S-CASE-19 for S-CONC-09

- GIVEN the `Choreography-Service-M8` profile and every field of `input` in [S-CASE-19](../../../../../specs/conformance/safety-cases.json)
- WHEN the `runtime` procedure for case `S-CASE-19` runs against those inputs
- THEN the observations match every field of `expected` in case `S-CASE-19`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: WI-19 for S-CONC-09

- GIVEN the `Choreography-Service-M8` profile and every field of `input` in [WI-19](../../../../../specs/conformance/wit-wasi-cases.json)
- WHEN the `runtime` procedure for case `WI-19` runs against those inputs
- THEN the observations match every field of `expected` in case `WI-19`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: WI-20 for S-CONC-09

- GIVEN the `Choreography-Service-M8` profile and every field of `input` in [WI-20](../../../../../specs/conformance/wit-wasi-cases.json)
- WHEN the `runtime` procedure for case `WI-20` runs against those inputs
- THEN the observations match every field of `expected` in case `WI-20`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->
