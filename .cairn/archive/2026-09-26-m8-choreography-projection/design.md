## Context

M7's accepted `S-CONC-07/08` and `WI-SYN-04` define a host-serialized eight-facet/eight-assertion/eight-interest/32-event dataspace, exact `(name,ready)` membership, and the four synchronous `noble:syndicate/service@1.0.0` exports. Its host binds facet identities and rights; guest strings, Preserves bytes and WIT types grant no authority. M8 adds only a finite ordering contract above this service. No M7 WIT/world change or M6 async assumption is selected.

## Decisions

### Finite typed protocol and projection

`noble:choreography/service@1.0.0` is a versioned **data-descriptor** identity, **not** a WIT package/world, authority, resource or general language syntax. The existing `noble:syndicate@1.0.0` imports and exports stay unchanged. An admitted typed descriptor has exactly `protocol` and `rounds` fields. `protocol` is that exact identity; `rounds` is an ordered list of one or two records with exactly `name`, `ready`, `exit`. Names are distinct, ASCII `[A-Za-z0-9_-]{1,32}`; `ready` is Boolean; `exit` is `withdraw` or `trap`, and a trap round must be last. No extensions, additional roles, branches, loops, recursion, free-form assertions, per-round facet IDs or data-dependent choices. The *only* selected untrusted descriptor input is strict UTF-8 JSON: at most 512 bytes and three JSON container levels (root object, rounds array, round objects; count containers outside escaped strings), checked before typed construction. Parse one complete value, allowing only trailing JSON whitespace, rejecting duplicate object keys, unknown fields, non-Boolean readiness, malformed UTF-8/JSON and excess nesting. Standard JSON string escapes are allowed but the decoded identity/name must satisfy the exact protocol and ASCII M7 name rules. An admitted JSON object does not grant rights; do not feed it to M7's service-only Preserves decoder. No file/network transport or general JSON protocol is selected.

The global action order for each `withdraw` round `(n,b)` is `subscriber.observe(n,b)=false`, `publisher.publish(n,b)=true`, `subscriber.observe(n,b)=true`, `publisher.withdraw(n)=true`, `subscriber.observe(n,b)=false`. A terminal `trap` round is `subscriber.observe(n,b)=false`, `publisher.publish-and-trap(n,b)=trap-after-publication`, `subscriber.observe(n,b)=false`; its publisher operation emits the M7 add then cleanup remove to the live subscriber, and is never a successful export. `observe` Boolean is exact pair membership, including when `b=false`; existing interests are idempotent. The typed add/remove trace is distinct from membership returns. Projecting onto a role filters this action sequence without reordering and preserves every action's arguments, expected result, round ordinal and global step ordinal. The projections are data, not a runtime interpreter, independent scheduler or proof that arbitrary compiled code implements them.

Production policy forbids external JSON providers. Implement this one
descriptor grammar with a local, allocation-free, bounded typed parser:
check the 512-byte limit, UTF-8 and JSON container depth before admitting
typed values; parse JSON escapes in keys and values, reject malformed
surrogates and unescaped controls, compare *decoded* keys for duplicates
and unknown fields, and require end of input after optional JSON
whitespace. Fixed-capacity storage suffices for the exact root fields,
at most two round records and their bounded decoded strings. Do not
first parse into a generic map that could collapse duplicate keys or
create an unbounded value tree; add no production provider or Cargo
dependency. The parser sees descriptor bytes only; neither M7 Preserves
nor the independent peer is the admission authority.

### Admission and execution boundary

A selected execution admits one session into an otherwise empty M7
dataspace, with exactly two independently instantiated, separately
compiled Noble components and host-issued active publisher/subscriber
facets. Publisher gets publish/retract, subscriber gets observe; neither
the descriptor nor component arguments select identities or expand rights.
Validate the exact protocol identity and full descriptor, role projections
and world/import signatures of `noble:syndicate/service@1.0.0`,
component/host binding, exclusive empty dataspace and capacity **before**
facet creation or guest import. Reserve two facets, at most one concurrent
assertion, **two exact-pair interests and four add/remove events for either
one or two rounds** (including mandatory removal after trap); never scale
the 2/4 reservation down for a one-round descriptor. M7's retirement
preflight still applies; refuse if any reservation fails. No concurrent
sessions, competing observers or other dataspace activity during a
selected run. These limits do not assert a process-memory bound.

A serialized trusted host/session monitor stores an immutable admitted plan, one next-global-step cursor and at most one in-flight export with expected import substeps. A host-owned **pre-export call admission** checks the projected export identity, arguments, host-bound caller facet/role, current step, exact pair and pre-state **before invoking the guest export**. This is necessary because `publisher` and `publish-and-trap` both begin with the identical `dataspace.publish` import: an import-only guard cannot reject a wrong export before publication. Pre-export denial leaves cursor, dataspace/events, rights and guest-request/protected-operation counts unchanged. Each import then passes a production monitor checking its required import identity/phase/arguments before forwarding to M7; no M8 peer may invoke `Profile` directly around this guard. Each pre-export admission, import precheck+M7 state commitment+phase update and post-call finalization uses the same serialized critical section, **released between callbacks** (holding its mutex across the component call would deadlock on host reentry). While an export is in flight, a different export cannot start; always acquire monitor lock before M7 dataspace lock (including cleanup), never the reverse. Pre-export admission atomically reserves the pending phase, each import re-locks and atomically checks, applies its M7 operation and updates pending substate, and finalization follows `function.call` and `post_return` success or trap plus retirement. Post-call validation checks result and import sequence before cursor completion. A wrong role, order, replay, export, name, readiness, exit, retired/foreign facet, forged rights, extra call or divergent membership is rejected before the prohibited M7 operation/trace or cursor advance. For a **post-start** import mismatch, no unauthorized M7 operation or protected effect occurs and no session cursor advances, but the invocation may trap; mandatory M7 facet retirement can then retract previously owned state and emit legitimate typed removals. Do not promise unchanged dataspace, event log or rights after such cleanup, and distinguish refused imports from admitted guest requests in the receipt. An ordinary permitted M7 operation remains subject to M7 rights and quota checks. The terminal trap export must produce the observed `publish` import followed by the observed `fail` import and real compiled trap: its publication is an admitted effect, then host trap retirement must retract it and emit the owed removal, and only after observing failed invocation plus cleanup may the cursor advance. It must not report publisher success. An interruption after publication is failure and cleanup, never transaction rollback or choreography success. The final subscriber absence observation completes the plan; early termination or missing steps do not. Host serialization, callback authenticity, real facet retirement and engine correctness are explicit assumptions, not inferred from a pure projector.

For the selected terminal-trap outcome specifically, the monitored
`publish` import MUST have succeeded before the subsequent `fail` import,
the compiled invocation MUST report an error, and retirement MUST emit
the owed removal before final subscriber absence. An earlier authorization
error/trap is a refusal, not a successful trap-step observation. Direct
trusted Rust calls to M7 `Profile` outside an M8 session are outside this
monitored branch, not a guest-accessible bypass.

### Acceptance and boundaries

S-CASE-18 checks deterministic static projection. WI-19 checks bounded
runtime order, false-readiness membership distinct from absence, terminal
trap, existing WIT/effects and *separately compiled* publisher/subscriber
execution under independent typed linking; neither a host-only script nor
copied projected data passes. S-CASE-19 and WI-20 exercise every declared
malformed, quota, wrong-role/order and forged-version/control variant at
the actual production admission/monitor boundary, observing no prohibited
effect or cursor change. Static projection, compiled execution, host policy
and pure-function correspondence need independently scoped evidence. Case
states remain absent/not-run until fresh evidence binds the precise
source/configuration and all declared variants. Existing M7 receipts do
not prove M8.

## Risks / trade-offs

- A single exclusive serialized session is intentionally narrower than general concurrent choreographies. Only two participants run concurrently as separate instances; steps are serialized, not asynchronous progress guarantees.
- A trap publication is observable before mandatory cleanup; a failed export is not rolled back. Reservations ensure cleanup remains possible, not fair delivery.
- An independent peer and source-bound extraction still do not prove universal compiler, ABI, engine or host refinement. Durability, remote transport and general Syndicate/Preserves are outside the selection.
