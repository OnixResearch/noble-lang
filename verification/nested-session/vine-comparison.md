# Vine source-form comparison for the nested session model (design note)

Status: **design examples only**. Nothing in this note is implemented, no source form exists in Noble,
no cross-language session test was executed, and no emitted-Wasm or transport theorem is implied. The
model in `proofs/m6/NestedSession.lean` stays the only semantics; this note asks what two candidate
source notations would have to express and what they would cost.

Grounding:

- The model: `proofs/m6/NestedSession.lean` (`Endpoint` at lines 113-122, `clientProtocol`/
  `serverProtocol` at 2619-2623, `labelSetProtocol` at 2699-2701, protocol equality and duality at
  1490-2572, the table check and the two default-off adjuncts at 2776-3342).
- The addendum: the 11-11 comment on issue #67, including the reviewed Vine commit `cf0c3258` and its
  `tests/programs/par.vi` and `vine/src/features/inverse.rs`. That comment is a source review; the Vine
  code was not re-read or executed for this note, so every Vine statement below is reported from the
  addendum rather than independently observed.
- The profile boundary: `.cairn/changes/nested-session-protocol` (WIT-SESSION-01..06). No general
  multiparty session types, no endpoint delegation, no network transport, no general inverse type.

## 1. The two fixtures, written in two source forms

The fixtures are the model's own closed tables, so the comparison adds no protocol.

**Fixture A (scalar handshake).** `clientProtocol` sends an `i64`, receives a `bool` and finishes;
`serverProtocol` is its dual (`client_and_server_are_dual` decides the duality).

**Fixture B (finite choice).** `labelSetProtocol` declares an internal `select` with labels `0 → 1`
(`send i64`) and `1 → 2` (`recv bool`) at state `0`; its dual is a `branch` node over the same labels
(`label_set_duality`).

### Form A — explicit `Endpoint<Protocol, Role>`

```text
type Handshake = protocol { 0: send I64 -> 1 ; 1: recv Bool -> 2 ; 2: end }
type Choice    = protocol { 0: select [0 -> 1 ; 1 -> 2] ; 1: send I64 -> 3 ;
                            2: recv Bool -> 3 ; 3: end }

-- proposed signatures; no effect identifier is allocated by this change
pair        : S -- S Endpoint<P, Client, 0> Endpoint<P, Server, 0> ! {session.open}
generation  : S Endpoint<P, Role, k> -- S Endpoint<P, Role, k> I64 ! {session.generation}
send        : S Endpoint<P, Role, Send T k> T -- S Endpoint<P, Role, k> ! {session.send}
recv        : S Endpoint<P, Role, Recv T k> -- S Endpoint<P, Role, k> T ! {session.recv}
choose      : S Endpoint<P, Role, Select bs> -- S Endpoint<P, Role, bs[l]> ! {session.choose}
receiveBranch : S Endpoint<P, Role, Branch bs> -- S Endpoint<P, Role, bs[l]> ! {session.branch}
finish      : S Endpoint<P, Role, End> -- S ! {session.close}
```

The protocol state is a type argument here; the model carries it as the runtime field
`Endpoint.state`. The protocol identity `P` is a checked definition identity; the model carries it as
the table index `Endpoint.protocol`.

### Form B — paired / inverse-endpoint library (Vine-inspired)

Transcribed from the mechanism reported in the addendum:

```text
type Channel<T, k>            -- one message position and a continuation
type Recv<T, k> = Channel<T, k>
type Send<T, k> = Channel<~T, k>

-- proposed signatures; `~T` is a type-level inverse, `Session<A, B>` constrains `B` to `~A`
pair        : S -- S Channel<T, k> Channel<~T, k> ! {session.open}
send        : S Channel<~T, k> T -- S Channel<k> ! {session.send}
recv        : S Channel<T, k> -- S Channel<k> T ! {session.recv}
choose      : S Channel<Select bs, k> -- S Channel<bs[l]> ! {session.choose}
receiveBranch : S Channel<Branch bs, k> -- S Channel<bs[l]> ! {session.branch}
```

In the reported Vine implementation the inverse is the value/space role swap in
`vine/src/features/inverse.rs`, not an arbitrary function inverse; that is prior art for endpoint
polarity and continuation composition. It is not a Noble facility. The reported `par.vi` also passes
endpoints as payloads and uses recursive choices; endpoint delegation exceeds this profile's initial
boundary, so only the scalar handshake and the finite choice above are compared.

## 2. Ordered-stack and latent-effect signatures, and the operation map

Both forms keep the model's operation vocabulary exactly; neither adds an operation. The latent effect
names below are **proposed**: this change allocates no effect identifier and adds no host import.

| Operation | Ordered stack (Form A) | Latent effect | Model state (node) | Continuation | Side / role | Generation |
|---|---|---|---|---|---|---|
| `pair` | `S -- S Endpoint<P, Client, 0> Endpoint<P, Server, 0>` | `{session.open}` | fresh: table index `0` | `0` | one `Client`, one `Server` | one fresh host nonce per endpoint |
| `generation` | `S Endpoint<P, Role, k> -- S Endpoint<P, Role, k> I64` | `{session.generation}` | unchanged `k` | `k` | preserved | observed, never advanced |
| `send T` | `S Endpoint<P, Role, Send T k> T -- S Endpoint<P, Role, k>` | `{session.send}` | `send T k` | `k` | preserved (`responder`) | unchanged |
| `recv T` | `S Endpoint<P, Role, Recv T k> -- S Endpoint<P, Role, k> T` | `{session.recv}` | `recv T k` | `k` | preserved | unchanged |
| `choose l` | `S Endpoint<P, Role, Select bs> -- S Endpoint<P, Role, bs[l]>` | `{session.choose}` | `select bs` | `lookup l bs` | internal choice (the decider) | unchanged |
| `receiveBranch l` | `S Endpoint<P, Role, Branch bs> -- S Endpoint<P, Role, bs[l]>` | `{session.branch}` | `branch bs` | `lookup l bs` | external choice (the peer selects) | unchanged |
| `enter` (nested) | `S Endpoint<P, Role, nested n p> -- S Endpoint<P, Role, n>` | `{session.enter}` | `nested n p` | `n`, saving `p` | preserved | unchanged |
| `finish` | `S Endpoint<P, Role, End> -- S Endpoint<P, Role, popped>` | `{session.finish}` | `finished` | popped index, or orderly end | preserved | unchanged |
| `close` | `S Endpoint<P, Role, End> -- S` | `{session.close}` | finished with an empty return stack | — | preserved | unchanged |

Each row is the model's `protoStep`/`stackStep` rule (`stackStep_eq_protoStep` proves they agree on an
open endpoint), not a second semantics. The endpoint fields compared by `sameIdentity` — channel, side,
protocol identity and generation — are the four that every step preserves
(`advance_preserves_the_endpoint_identity`, `steppers_preserve_the_endpoint_identity`).

Split of the two forms by obligation:

- **Library conventions** (no checker change): naming the paired constructor and its two roles; the
  `Send`/`Recv` aliases over one channel shape; the label order convention inside a label set.
- **Checker or representation changes** (both forms): a move-only endpoint resource *type*; a checked
  protocol-definition identity (the model's table index needs a source name); type-level or
  checked-alias protocol state; and the type-equality/duality question asked of the checker, which the
  model already decides (`protocolEquality_decides`, `protocolDuality_decides`).
- **Checker changes only for Form B**: a type-level inverse `~` with an involutive law (`~~T = T`) and a
  `Session<A, B>` constraint that relates the two endpoints. The model has no inverse operator and no
  obligation of that shape, so these would be new Core machinery rather than library sugar.

## 3. What either form must not change

The addendum's constraints, and the model facts that already hold them:

- **Move-only ownership.** Copy, discard and capture are refused with their own classes
  (`endpoint_copy_is_refused`, `endpoint_discard_is_refused`, `endpoint_capture_is_refused`), and no
  operation duplicates an endpoint (`no_endpoint_duplication`,
  `every_operation_preserves_the_endpoint_inventory`).
- **Host-issued authority.** A paired *type* is a type: it creates no live endpoint, grants no authority,
  and a matching protocol type never authenticates the peer. The model states the second half as a
  scope limit rather than a theorem because it models no peer at all; `pair` is a host operation in both
  forms.
- **Queue and failure events.** Send is not receipt (`send_step_does_not_observe_a_peer`), and the
  admission/full/dequeue/close/cancellation/cleanup events keep their meanings
  (`admission_at_the_limit_is_refused`, `dequeue_restores_admission_room`,
  `boundary_preserves_the_committed_prefix`). A notation change adds no delivery.
- **Generation.** The host nonce is explicit; a stale use is refused (`stale_generation_is_refused`) and
  the matching generation is the non-vacuous control (`matching_generation_is_accepted`).
- **No capture through a paired type.** `endpoint_capture_is_refused` is a model refusal; a source form
  must not reintroduce capture by making a channel a closure or a copyable handle.

## 4. Comparison and recommendation

| Dimension | Form A: explicit `Endpoint<Protocol, Role>` | Form B: paired/inverse library |
|---|---|---|
| Authoring burden | one protocol definition and one endpoint type per role; each operation states its current and next state | one library shape (`Channel`) for both directions, so a program names fewer types, but every operation type mentions `~` |
| Diagnostic clarity | a wrong message type or order is a mismatch against the current state alias, which the checker can print; the state index localizes the error | a wrong continuation can surface as a failed inverse equation; the diagnostic mentions `~T`, and an unchecked `~~T = T` can hide an equal-signature wrong-behaviour error |
| Checking work | one equality/duality question per pair of protocol states, for which `workBound` is already proved sufficient | the same questions plus the involution equations and the `Session` constraints; those obligations do not exist in the model, so they are new work with no proved bound here |
| New Core machinery | a resource type, a checked protocol identity and a state alias; no new type operator | a general type-level inverse and its laws. The reported Vine mechanism swaps value and space roles, a notion Noble does not have |

**Recommendation: the explicit `Endpoint<Protocol, Role>` form, with no general inverse type.** The
addendum itself records this as a valid outcome. The reasons are the model's: protocol type equality and
duality are already decided by a work-bounded exploration, so a second notation must not introduce a
second oracle; the inverse operator is the one part of Form B with no existing obligation or bound; and
Form B's only practical gain — naming one library shape instead of two endpoint types — is available as
library aliases over Form A.

## 5. Negative controls

Each existing control is reused, not restated. The one new control is the continuation mismatch the
addendum asks for.

| Control | Model theorem |
|---|---|
| reversed send/receive order | `order_separates_types` |
| wrong payload type / wrong order | `wrong_message_type_is_refused`, `wrong_order_is_refused` |
| wrong choice or undeclared branch | `trace_wrong_branch_is_refused`, `undeclared_label_is_refused`, `branch_zero_refuses_a_bool_send` |
| stale generation | `stale_generation_is_refused` (`matching_generation_is_accepted` is its non-vacuous pair) |
| **wrong-side endpoint** | **no model control.** Side is identity data (`responder`): every step preserves it (`advance_preserves_the_endpoint_identity`) but no step is gated on it. A source form that claims role safety needs a new obligation, recorded here rather than assumed |
| duplication | `no_endpoint_duplication`, `every_operation_preserves_the_endpoint_inventory`, `duplicating_operation_breaks_the_bound` |
| discard / capture | `endpoint_discard_is_refused`, `endpoint_capture_is_refused` |
| double close / receive after close | `double_close_is_refused`, `closed_endpoint_cannot_receive` |
| **source vs model continuation mismatch (new)** | `vine_source_continuation_mismatch_is_refused`: the paired form's slip (`send i64` then `recv i64`) against the model's `send i64` then `recv bool` is `.differ [.send .i64, .recv .bool]` at eight units of work. `vine_source_continuation_match_is_accepted` is the matched control, so the refusal measures the mismatch and not the comparison |
| both source forms denote one fixture | `vine_source_forms_denote_the_same_fixture`: no second semantics is introduced |

## 6. Smallest future source-to-model correspondence obligation

If the explicit form is adopted, the smallest useful correspondence is a **source-to-table lowering for
one closed protocol definition**: a checked source protocol definition `P` and its two role endpoints
lower to the model's definitions table plus an endpoint record, such that

1. the name→index map preserves protocol identities and continuation indices;
2. each source operation's pre- and post-state equal the model's `protoStep` successor for the same
   action (so `stackStep_eq_protoStep` applies unchanged);
3. side/role and generation provenance are recorded, including which host operation issued them;
4. the negative controls in §5 are replayed on the lowered table.

That is a trace relation between emitted transitions and the model's step relation. It reuses the
RegionProtocol trace discipline (#50) and the interleaving infrastructure (#46), and it is exactly what
WIT-SESSION-06 would bind to a lowering, runtime and helper revision. It is not established here: this
change ships no lowering, so the requirement records the binding as required once one exists rather
than binding a revision that does not.

## Scope and coordination

- This note is a source and prior-art comparison. It establishes no emitted-Wasm, runtime, transport,
  multiparty, delegation or liveness result, and it selects no production adoption.
- Duplication/discard permissions are #112's; this note keeps today's must-use policy and does not wait
  on #112. Interaction-net execution stays #37's; a source endpoint notation is not an IVM decision.
- The Lean control lives with the rest of the model (default off, design examples):
  `vine_source_forms_denote_the_same_fixture`, `vine_source_continuation_mismatch_is_refused`,
  `vine_source_continuation_match_is_accepted`.
