## Context

Grounded at worktree HEAD `9dd78ebb` (issue #67 cites `eff2f23`; the cited
facts still hold):

- `crates/noble-kernel/src/async_tasks/mod.rs` is "serialized async ownership
  decisions, independent of native storage and authority". The shell owns all
  payloads, validates arguments and native provenance, and serializes the table
  with resource transfer and one-shot witness consumption. `prepare` reserves and
  starts nothing; its exclusive borrow makes a subsequent `commit` infallible,
  and dropping that preparation preserves the table. Cancellation acknowledges
  revoked guest access only — native terminal observation, pin settlement,
  cleanup and finalization are separate decisions. `Task` is an opaque move-only
  result/cancellation capability whose handle may be copied as identity but not
  as the ability to receive a result. Descriptive handles, callbacks, snapshots
  and decisions **grant no authority**.
- `proofs/m6/M6Async.lean` already models that lifecycle with `Snapshot`,
  `Action`, `Decision`, `transition`, `stateValid`, `recordCheck`,
  `dispositionCheck`, `handleCheck`, `nativeCheck`, `settled`, bounded
  `Request`/`Limits` footprints (`MAX_BYTES`, `MAX_OBLIGATIONS`, `MAX_SLOTS`,
  `MAX_WAKEUPS`) and reference/extracted transition correspondence.
- #46 owns bounded interleaving exploration of those records; #50 owns the
  synchronous `RegionProtocol` trace relation; closed #9 excluded general
  concurrent/distributed protocols.

## Decisions

### Decision: one finite grammar with named nested constructors, not a process calculus

**Choice:** `End`, typed `Send`, typed `Recv`, internal/external finite choice,
and **named nested constructors with explicit parameters**, under a versioned
grammar. Definition count, constructor arity, unfolding depth and duality work
are bounded, and timeout or exhaustion is a refusal rather than acceptance. The
paper's process calculus is adapted as protocol ideas, not imported.

**Rationale:** Noble needs rules for its own ordered stacks, `Program<S,T,e>`,
move-only resources, effects, quotas, cancellation, reflection and exact proof
subjects. Importing a calculus would carry assumptions Noble has not established,
and an unbounded unfolding check could be read as a decision it never made.

### Decision: `Task` is not retrofitted; a channel is a distinct abstraction

**Choice:** the design adds no behaviour to `Task`. The comparison is explicit:

| Capability | Existing `Task`/callback layer | Required channel abstraction |
|---|---|---|
| Serialized lifecycle decisions, one-shot witness consumption | **reused** (`async_tasks`, `M6Async.lean`) | — |
| Exclusive `prepare` → infallible `commit` pattern | **reused** as the admission shape | — |
| Move-only opaque capability with copyable identity, uncopyable authority | **reused** as the endpoint shape | — |
| Bounded request/limits accounting (`Request`, `Limits`) | **reused** for mailbox admission bounds | — |
| Ordered message types, protocol state, duality | — | **new**: protocol grammar and per-endpoint state |
| Mailbox with peer, order and full/refusal semantics | — | **new**: bounded queue + observation events |
| Receive/send advancing a continuation type | — | **new**: typing rules with continuation |

**Rationale:** reusing the lifecycle and admission shapes keeps the design
small; retrofitting `Task` into a channel would put message ordering into a table
whose contract explicitly covers one-shot work, cancellation revocation and
native settlement only.

### Decision: endpoints are move-only resources, and type equality is not authority

**Choice:** an endpoint carries channel identity, side/role, protocol identity
and current protocol state, and is opaque and move-only. Copy, discard, capture,
stale-generation use and double close are rejected unless an explicit operation
owns the transition. A descriptive handle is never authority, and a matching
protocol type never establishes peer authenticity.

**Rationale:** this mirrors the existing resource and task discipline (handle
identity is not authority) and prevents the classic confusion of a type check
with a peer-identity check.

### Decision: send is not receipt; every asynchronous boundary is an event

**Choice:** guest send/receive requests, bounded mailbox admission, host
commit/observation, dequeue, close, cancellation and cleanup are distinct
events. Payload ownership transfer is defined, and refusal, full mailbox, peer
close, trap, deadline and budget exhaustion each have their own outcome. A
successful `send` is never modelled as peer receipt.

**Rationale:** if a send implied receipt, the model would silently claim
delivery, which the issue keeps separate and generally unclaimed. Separating the
events also lets the trace discipline of #50 preserve committed prefixes across
trap and cancellation.

### Decision: excluded initially

**Choice:** endpoint delegation (passing an endpoint as a message) is excluded
until its ownership proof is explicit. Recursion is confined to named nested
constructors under bounded unfolding.

**Rationale:** delegation multiplies the ownership obligations beyond what a
first profile can justify, and unrestricted recursion removes the bounds that
make the checker terminate.

### Decision: progress is scoped, and the negative controls are part of the claim

**Choice:** the only progress statement is a precisely scoped one under explicit
queue, peer, scheduler and host premises. Termination, fairness, eventual
delivery, exactly-once delivery and network reliability are separate and
generally unclaimed, and no result of one kind is reported as another.

**Rationale:** the expressiveness result warns that well-typed nested session
processes can encode two-counter machines. A communication-safety proof is not a
termination proof, and saying so is part of the design rather than a footnote.

## Risks / Trade-offs

- A bounded unfolding limit may refuse protocols that are well-typed but deep;
  the refusal class is explicit so it cannot be mistaken for an acceptance.
- The mailbox model needs scheduler and host premises before deadlock statements
  mean anything; without them the statement is unconditional and false.
- Reusing `Request`/`Limits` accounting for mailbox admission may not fit
  multi-message queues exactly; the design names the reuse but Phase 2 must
  verify it rather than assume it.
- The nested recursive fixture must be executable and bounded; if it cannot be
  executed within the quota model, the acceptance target becomes a model trace
  and that must be stated.

## Proof obligations (named, not discharged here)

- preservation of endpoint duality and state across every checked operation;
- no communication action outside the current protocol state;
- no endpoint duplication (authority is not copyable);
- the scoped progress/deadlock statement under its explicit premises;
- the correspondence route from the handwritten channel machines to actual
  transitions, kept separate from the model theorems and labelled as such.

## Fixtures and controls (future, unchecked)

Positive: send/receive sequencing, both choices, recursive entry/return, orderly
close, bounded asynchronous buffering, and one ordinary finite-handshake control
alongside the non-regular nested fixture.

Negative: wrong message type or order, wrong branch, endpoint copy or discard,
stale identity or generation, double close, receive-after-close, full-queue
misaccounting, forged peer response, omitted cleanup obligation.

Cancellation, trap, quota exhaustion and peer disappearance must preserve the
committed trace prefix and retain unresolved native/host cleanup facts rather
than manufacturing protocol success.

## Non-claims

- No implementation, proof, test or benchmark result is claimed by this package.
- No general multiparty session types, unrestricted type-level recursion,
  endpoint delegation, network transport, consensus, actor runtime, universal
  deadlock freedom, fairness, exactly-once delivery or whole-language
  verification.
- Communication safety is not termination, liveness or bounded resource use.
