## Phase 1: Design

- [x] [serial] Specify the finite versioned grammar with named nested constructors, the bounded equality/unfolding/duality algorithm and its refusal classes. r[WIT-SESSION-01]
- [x] [serial] Specify the opaque move-only endpoint resource with identity, role, protocol identity and state, and reject copy, discard, capture, stale generation and double close. r[WIT-SESSION-02]
- [x] [serial] Specify the asynchronous operational model with distinct request, admission, commit, observation, dequeue, close, cancellation and cleanup events and no send-as-receipt. r[WIT-SESSION-03]
- [x] [serial] Specify ordered-stack and latent-effect signatures, single-endpoint continuation advancement, composition with quote, compose and run, and excluded endpoint delegation. r[WIT-SESSION-04]
- [x] [serial] Specify the Lean obligations for duality/state preservation, no action outside the protocol, no duplication, and the scoped progress statement. r[WIT-SESSION-05]
- [x] [serial] Specify the correspondence route reusing the trace discipline and interleaving infrastructure, keeping model theorems separate. r[WIT-SESSION-06]
- [x] [serial] Record the explicit comparison with the existing Task and callback ownership showing reused and new capabilities. r[WIT-SESSION-02]

## Phase 2: Model and fixtures (future, not started)

- [x] [serial] Implement the grammar, the terminating equality/unfolding/duality algorithm and the endpoint resource in Lean for the selected closed model. r[WIT-SESSION-01] r[WIT-SESSION-02]
- [ ] [serial] Prove duality/state preservation, no action outside the current protocol and no endpoint duplication. r[WIT-SESSION-05]
  Partial: duality is proved definitional and involutive, one step is proved to advance exactly to the
  send node's continuation with identity fields unchanged, and copy/discard/capture are refused. No
  endpoint duplication is not yet a general theorem over every transition, so the task stays open.
- [ ] [serial] Prove the scoped progress statement under the explicit queue, peer, scheduler and host premises. r[WIT-SESSION-05]
- [ ] [serial] Add the positive traces for send/receive sequencing, both choices, recursive entry/return, orderly close and bounded asynchronous buffering. r[WIT-SESSION-03]
- [ ] [serial] Add one ordinary finite-handshake control beside the non-regular nested fixture and record whether the nested fixture executes or is only a model trace. r[WIT-SESSION-05]
- [ ] [serial] Add the negative controls for wrong type or order, wrong branch, endpoint copy or discard, stale identity, double close, receive-after-close, full-queue misaccounting, forged peer response and omitted cleanup. r[WIT-SESSION-02] r[WIT-SESSION-03]
- [ ] [serial] Preserve the committed trace prefix across cancellation, trap, quota exhaustion and peer disappearance, retaining unresolved cleanup facts. r[WIT-SESSION-06]
- [ ] [serial] Bind protocol definitions, endpoint identities, queue limits and lowering/runtime/helper revisions in a retained correspondence receipt. r[WIT-SESSION-06]
