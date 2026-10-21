## Context

The accepted twelve-spec family places concurrency safety in SPEC-S001 and
Component/Preserves integration in SPEC-W001. S-CONC-01..06 and WI-SYN-01..03
select direction but do not define a local M7 transition system. M5 supplies
the synchronous component boundary and host admission model. M6 completion
does not make native async a dependency for this local service. The existing
M7 cases remain `absent/not-run/open/unassessed`; design and implementation
are not execution receipts.

## Decisions

### Decision: Bounded serialized dataspace

**Choice:** A host-assigned facet owns assertions, exact `(name,ready)`
interests and child facets. Serialize open, interest registration, publish,
observe, retract and retirement; retain at most eight facets, eight
assertions, eight interests and 32 events. Check rights, liveness and required
capacity, including immediate notifications and already-owed future removal
notifications, before mutation. This reserves enough room for trap/normal
retirement without a later quota refusal. Invalid operations preserve state. Repeated
identical publish/interest is idempotent. Changed readiness for the same
`(facet,name)` atomically emits remove-old then add-new after preflight;
`retract(name)` removes only the caller facet's assertion and reports whether
it existed. Normal exit/trap recursively retires descendants and owned
assertions/interests once. The observer stays in a different facet and sees
typed add/remove; queued history is not current membership. No guest shared
mutable Noble memory, host-only authority from wire, remote rollback, event
fairness, or distributed exactly-once claim follows.

**Rationale:** The first service can prove bounded local lifetime/isolation
properties without an unsupported scheduler or native async abstraction.
S-CASE-11 additionally needs a gate-supplied shared-memory Wasm module whose
memory section is inspected by the peer/loader, then refused by the production
profile admission before facet grant or guest work. A test-supplied shared
flag or two separate Stores alone is not that refusal evidence.

### Decision: Select strict Preserves service subset

**Choice:** The only admitted wire form is `<service "NAME" #t>` or
`<service "NAME" #f>` with exact spacing/delimiters, ASCII `NAME` matching
`[A-Za-z0-9_-]{1,32}`, at most 64 bytes, depth at most four and no trailing
content/extension. Check byte size, bounded traversal, UTF-8 and schema
before constructing `Service`; output the same canonical form. Decode
success never creates an authority or WIT resource. Roundtrip actual
compiled-publisher values as well as hostile bytes through production.

**Rationale:** Explicit byte and depth limits plus exact schema acceptance
allow discriminating S-CASE-09/10 and WI-14 without pretending to implement
general Preserves or native resource conversion.

### Decision: Select synchronous versioned WIT boundary

**Choice:** The selected `noble:syndicate@1.0.0` world `service` imports
`dataspace.publish(name:string,ready:bool)->bool`,
`observe(name:string,ready:bool)->bool`, `retract(name:string)->bool`, and
`fail(value:bool)->bool`. It exports matching `publisher`, `observer`,
`withdraw`, plus `publish-and-trap(name:string,ready:bool)->bool`.
`observe` installs an exact-pair facet-owned interest and returns presence,
not the readiness field; `false` therefore means the queried assertion is
absent, including a query for `(name,false)`. A separate typed host event
trace distinguishes add/remove. The last export compiles
`dataspace.publish dataspace.fail`; the host intentionally traps on `fail`
after publication, then retires the publisher facet. Exact versioned
imports remain Noble request effects. Two separately compiled Noble
participant instances run against an independent typed component peer.

**Rationale:** Exact-pair membership avoids collapsing `ready=false` into
absence, and the compiled trap path prevents host-only lifecycle simulation
from masquerading as a positive source-to-component scenario.
The root workspace owns the production policy/adapter and kernel; the
separate verification peer owns Wasmtime linking and dynamic typed `Val`
conversion. Neither a generated peer binding nor a deployable general
Syndicate runtime is claimed.

### Decision: Keep evidence scopes separate

**Choice:** S-CASE-17 and WI-18 require compiled two-participant execution;
S-CASE-09..12/WI-14 require production hostile-input and cleanup controls.
An independent peer owns typed linking, but it reuses Noble's production
dataspace decisions. A source-bound extraction/check, strict correspondence
if achieved, native runtime execution, physical host cleanup, compiler/ABI
and trusted engine assumptions must be reported separately. Cairn validation
and generated views prove only document structure, not implementation.

## Risks / Trade-offs

- A synchronous query and host trace prove only selected local serialized
  behavior, not asynchronous event delivery or standard Syndicate progress.
- Decoder depth four is a rejection budget, not permission to admit nested
  records outside the one-record service schema. The 64-byte cap is not a
  process-memory bound.
- A successful publication preceding a deliberate trap remains an admitted
  local operation; failed invocation and facet retraction remain distinct.
- M7 case status and milestone status MUST NOT be promoted from plans, file
  presence, source compilation alone, or retroactive M5/M6 evidence.
