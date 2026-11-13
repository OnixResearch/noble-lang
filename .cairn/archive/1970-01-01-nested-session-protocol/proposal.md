## Why

Noble's `async_tasks` provides serialized lifecycle and ownership decisions for
one-shot asynchronous work, and `M6Async.lean` already checks the lifecycle and
reference/extracted transition correspondence. Neither is a reusable message
channel or a source-level protocol type, and closed #9 deliberately excluded
general concurrent protocols. The open question is whether Noble should gain a
separate endpoint resource and protocol checker, and which narrow
semantics/proof obligations would justify it.

The research basis cuts both ways: nested recursive session types can describe
non-regular protocol structure, and they are expressive enough to encode
two-counter machines — so communication safety implies neither termination, nor
decidable behaviour, nor bounded queues, nor practical resource safety.

r[WIT-SESSION-01] r[WIT-SESSION-02] r[WIT-SESSION-03] r[WIT-SESSION-04]
r[WIT-SESSION-05] r[WIT-SESSION-06]

## What Changes

- **ADDED** `WIT-SESSION-01`: one finite, versioned protocol grammar (`End`,
  typed `Send`, typed `Recv`, internal/external finite choice, named nested
  constructors with explicit parameters), bounded definitions and constructor
  arity, and a terminating type-equality/unfolding/duality algorithm where
  timeout or exhaustion is never acceptance.
- **ADDED** `WIT-SESSION-02`: an opaque move-only endpoint resource carrying
  channel identity, side/role, protocol identity and current protocol state, with
  copy, discard, capture, stale-generation use and double close rejected unless an
  explicit operation owns the transition; descriptive handles are never authority
  and a matching type never establishes peer authenticity.
- **ADDED** `WIT-SESSION-03`: the asynchronous operational model — guest
  send/receive requests, bounded mailbox admission, host commit/observation,
  dequeue, close, cancellation and cleanup as distinct events, with payload
  ownership transfer and refusal/full/peer-close/trap/deadline/exhaustion
  outcomes defined, and a successful send never modelled as peer receipt.
- **ADDED** `WIT-SESSION-04`: typing and composition — exact ordered-stack and
  latent-effect signatures, advancement of only the owned endpoint, composition
  with ordinary `quote`/`compose`/`run`, and endpoint delegation excluded
  initially.
- **ADDED** `WIT-SESSION-05`: the Lean properties for the selected closed model
  (endpoint duality/state preservation, no action outside the current protocol,
  no endpoint duplication, and a precisely scoped progress/deadlock statement
  under explicit queue, peer, scheduler and host premises), with termination,
  fairness, eventual delivery and network reliability kept separate.
- **ADDED** `WIT-SESSION-06`: implementation correspondence, reusing #50's
  trace-relating discipline and #46's interleaving infrastructure, where a
  theorem over handwritten channel machines is not a theorem about emitted Wasm
  or a native transport.

No runtime pilot is added, `Task` is not retrofitted into a channel, and no
production adoption is authorized.

## Impact

- Rust source subjects: none. The endpoint resource and mailbox are design
  subjects, not code.
- Affected specs: `.cairn/specs/wit-wasi/spec.md` (delta only).
- Coordination: #3 keeps ownership/lifetime, #15 keeps whole-fragment semantics,
  #46 keeps M6 schedule exploration, #50 keeps RegionProtocol trace
  correspondence. None of them is reopened.
- Adoption: none.

## Non-goals

No general multiparty session types, unrestricted type-level recursion,
arbitrary endpoint delegation, network transport, distributed consensus,
automatic actor runtime, universal deadlock freedom, fairness, exactly-once
delivery or whole-language verification.
