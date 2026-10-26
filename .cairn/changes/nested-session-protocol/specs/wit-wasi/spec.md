## ADDED Requirements

### Requirement: WIT-SESSION-01
r[WIT-SESSION-01]

The profile MUST specify one finite, versioned protocol grammar containing
`End`, typed `Send`, typed `Recv`, internal and external finite choice, and named
nested constructors with explicit parameters. Definition count, constructor arity,
unfolding depth and duality work MUST be bounded, and the type-equality,
unfolding and duality algorithm MUST terminate under those declared limits. A
timeout or exhaustion MUST be refused and MUST NOT be reported as acceptance.

#### Scenario: exhaustion is not acceptance

- GIVEN a duality check that exhausts its declared unfolding work
- WHEN the checker reports
- THEN the outcome MUST be a refusal rather than an accepted duality

#### Scenario: bounded constructor arity

- GIVEN a named nested constructor with more parameters than the grammar admits
- WHEN the definition is checked
- THEN it MUST be refused as malformed, and no partial unfolding MUST be retained

### Requirement: WIT-SESSION-02
r[WIT-SESSION-02]

An endpoint MUST be an opaque, move-only resource carrying the channel identity,
the side or role, the protocol identity and the current protocol state. Copy,
discard, capture, stale-generation use and double close MUST be rejected unless
an explicit operation owns the transition. A descriptive handle MUST NOT be
treated as authority, and a matching protocol type MUST NOT be treated as
evidence of peer authenticity.

#### Scenario: copy is not authority

- GIVEN a copied endpoint handle claiming the same channel identity
- WHEN an operation is attempted through it
- THEN the operation MUST be rejected because authority was not moved by an
  explicit operation

#### Scenario: type match is not authenticity

- GIVEN a peer whose declared protocol type matches the local type
- WHEN the session is admitted
- THEN peer authenticity MUST remain unestablished and MUST NOT be inferred from
  the type match

### Requirement: WIT-SESSION-03
r[WIT-SESSION-03]

The operational model MUST treat guest send and receive requests, bounded mailbox
admission, host commit or observation, dequeue, close, cancellation and cleanup
as distinct events. It MUST define when payload ownership transfers, and the
outcomes of refusal, full mailbox, peer close, trap, deadline and budget
exhaustion. A successful send MUST NOT be modelled as peer receipt.

#### Scenario: send is not receipt

- GIVEN a send that is admitted and committed by the host
- WHEN the trace is classified
- THEN it MUST contain a committed send and MUST NOT contain a peer receipt

#### Scenario: full mailbox is its own outcome

- GIVEN a send refused because the bounded mailbox is full
- WHEN the outcome is reported
- THEN it MUST be reported as full rather than as a generic refusal or as success

### Requirement: WIT-SESSION-04
r[WIT-SESSION-04]

Checked operations MUST carry exact ordered-stack and latent-effect signatures,
and sending or receiving MUST advance only the owned endpoint to its continuation
type. The profile MUST state how a protocol-typed program composes with ordinary
`quote`, `compose` and `run`. Endpoint delegation MUST be excluded until its
ownership proof is explicit.

#### Scenario: continuation advances once

- GIVEN a `Send` operation on an endpoint in state `S`
- WHEN the operation is checked
- THEN only that endpoint MUST advance to its continuation, and the operation
  MUST NOT be applied again with the same authority

#### Scenario: delegation is excluded

- GIVEN a program passing an endpoint as a message
- WHEN it is checked
- THEN it MUST be refused as unsupported rather than accepted with an implicit
  ownership assumption

### Requirement: WIT-SESSION-05
r[WIT-SESSION-05]

For the selected closed model, Lean obligations MUST be named for: preservation
of endpoint duality and state, no communication action outside the current
protocol, and no endpoint duplication. Any progress or deadlock statement MUST be
precisely scoped under explicit queue, peer, scheduler and host premises.
Termination, fairness, eventual delivery and network reliability MUST remain
separate and MUST NOT be reported as established by a communication-safety
result.

#### Scenario: progress stays scoped

- GIVEN a communication-safety proof for the selected model
- WHEN a report describes delivery or termination
- THEN the delivery or termination claim MUST remain unclaimed unless its own
  premises are proved

#### Scenario: action outside the protocol

- GIVEN a checked operation whose message type is not admitted by the current
  protocol state
- WHEN the model is applied
- THEN the obligation MUST fail and the trace MUST NOT be recorded as safe

### Requirement: WIT-SESSION-06
r[WIT-SESSION-06]

Correspondence between the handwritten channel machines and actual Noble
transitions MUST be bound to source protocol definitions, endpoint identities,
queue limits and lowering, runtime and helper revisions, and MUST reuse #50's
trace-relating discipline and #46's interleaving infrastructure where applicable.
A theorem over handwritten channel machines MUST NOT be presented as a theorem
about emitted Wasm or a native transport.

#### Scenario: labelled correspondence

- GIVEN a proved model theorem for the channel machines
- WHEN the evidence is reported
- THEN it MUST be labelled a model theorem and MUST NOT be cited as an
  implementation correspondence

#### Scenario: unresolved cleanup stays unresolved

- GIVEN an execution that ends with cancellation or a trap
- WHEN the trace is retained
- THEN the committed prefix MUST be preserved and unresolved native or host
  cleanup facts MUST be recorded rather than replaced by protocol success
