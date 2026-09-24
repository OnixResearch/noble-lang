# M6 selected native-async WIT boundary

## MODIFIED Requirements

### Requirement: WI-WASI-02
r[WI-WASI-02]

**WI-WASI-02.** The preferred standard host profile SHALL target the stable WASI 0.3 family. Exact WIT/WASI package patch versions MUST be pinned in a reproducible build/profile and MUST participate in compatibility checks and build identity.

`Component-Async-Bootstrap` selects Wasmtime 40.0.2, `wasm-tools` 1.245.1, `wit-parser`/`wit-component` 0.243.0, `wasmtime-wit-bindgen` 40.0.2 and official WASI `0.3.0-rc-2025-09-16` from the pinned Wasmtime source. This explicitly named prerelease selection MUST NOT be represented as stable WASI 0.3 conformance. Its selected linker MUST reject a request for `wasi:clocks/monotonic-clock@0.3.0` rather than silently substitute the prerelease interface. The independent peer-only clock probe MUST remain distinguished from Noble-compiled component evidence and MUST NOT establish Noble support for `u64` or the full WASI interface family.

#### Scenario: Pinned prerelease clock compatibility

- GIVEN the selected Wasmtime 40.0.2 source, its official WASI `0.3.0-rc-2025-09-16` clock linker, and the peer-only clock component
- WHEN the component invokes `wasi:clocks/monotonic-clock@0.3.0-rc-2025-09-16#wait-for` for 1,000,000 ns and the peer awaits task exit
- THEN the peer records the native clock operation and an elapsed interval at least as large as requested, without classifying the component as Noble output or the observation as stable WASI conformance

#### Scenario: Stable version is not substituted

- GIVEN the same selected linker and a clock component requesting `wasi:clocks/monotonic-clock@0.3.0`
- WHEN the peer attempts component instantiation
- THEN linking rejects the unsupported stable version before any import starts, rather than resolving it to the prerelease package

### Requirement: WI-ASYNC-01
r[WI-ASYNC-01]

**WI-ASYNC-01.** A WIT `async func` imported into Noble SHALL be callable through direct-style Noble code. The runtime MAY suspend and resume the invocation according to the Component Model async ABI; source-level sequential evaluation order MUST remain preserved.

In `Component-Async-Bootstrap`, imports MUST retain their exact versioned WIT request effects and checked stack interfaces. The emitted memory32/UTF-8 adapter MUST lower async imports through native async calls, suspend using `waitable-set.new`, `waitable.join` and `waitable-set.wait`, and discharge pending subtask/waitable-set handles with `subtask.drop` and `waitable-set.drop`. An eagerly completed import needs no subtask handle. Result storage and any indirect argument tuple MUST remain valid across suspension; the next sequential Noble word MUST NOT execute until the import has returned and its result has been validated.

In this selection, async exports MUST use stackful lifting and `task.return`; canonical result lifting precedes guest cleanup, and successful observation MUST await task exit. Async exports MUST NOT use synchronous post-return. Synchronous members in a mixed world MUST retain their synchronous ABI, including applicable post-return cleanup, rather than being relabelled async merely because another member suspends. The selected lowerer passes up to four flat async-import argument lanes directly and uses a canonical tuple pointer above four; the profile still rejects signatures exceeding sixteen flat parameter lanes. These are bounded adapter choices, not a universal Canonical ABI claim.

#### Scenario: Direct-style native suspension preserves results and order

- GIVEN a checked Noble export `host.first host.second` and the selected async world
- WHEN `host.first` suspends until its host-controlled native completion and execution then resumes
- THEN `host.second` starts only after the first import completes, the export returns the independently expected result, and the peer awaits task exit before reporting successful observation

#### Scenario: Mixed members and indirect arguments retain their contracts

- GIVEN a selected world containing synchronous live-value factories and async consumers, and a checked five-`s64` async import
- WHEN Noble compiles those members and the independent peer executes the native component
- THEN synchronous members retain synchronous calling semantics, the five arguments survive indirect tuple lowering and suspension without narrowing, and async results use task-return rather than synchronous post-return

### Requirement: WI-ASYNC-02
r[WI-ASYNC-02]

**WI-ASYNC-02.** Suspension of an async import MUST NOT imply concurrency of subsequent sequential Noble words, speculative execution, retry, transactionality, or reordering.

The selected adapter MUST allow at most one outstanding direct-style async import per compiled invocation and MUST exclude guest reentrancy while that invocation is active. Host-owned future/stream producers and bounded external native workers MAY progress independently; their existence MUST NOT license executing a later sequential guest word early. Failure, cancellation, trap or quota exhaustion MUST NOT automatically replay an import, restore a consumed authorization witness, roll back an admitted effect, or promote a late operation success to invocation success.

#### Scenario: Cancellation cannot replay admitted work

- GIVEN an admitted protected operation with consumed authority, an outstanding native pin and a later sequential guest word
- WHEN invocation cancellation is acknowledged before authentic native completion
- THEN guest continuation and result delivery remain revoked, the consumed witness cannot authorize replay, and a late operation-success observation cannot change the cancelled invocation to success or roll back the admitted operation

### Requirement: WI-ASYNC-03
r[WI-ASYNC-03]

**WI-ASYNC-03.** WIT `stream<T>` and `future<T>` SHALL be represented by typed profile-level Noble values with explicit completion/cancellation/error semantics. They are not additional expression forms.

The selected live boundary types are exactly `stream<u8>`, `future<s64>` and `future<result<s64,string>>`, each mapped to a distinct opaque resource kind. The selected ordinary result alternatives are `result<s64,string>` and `result<list<u8>,string>`, mapped respectively to `Sum<I64,Text>` and `Sum<List<I64>,Text>` with checked byte conversion. Supported `bool`, `s64`, UTF-8 `string`, `list<u8>` and declared owned resources retain their existing exact boundary mappings. The profile MUST reject other WIT live payload/aggregate combinations, other result shapes, async borrowed parameters, borrowed exports, escaping borrows and guest-defined resource exports rather than erase distinctions or infer an adapter. Synchronous scoped borrowed imports remain subject to SPEC-R001.

The selected world's synchronous `make-future` and `make-stream` return live values; `finish-future` and `drain-stream` consume them through async imports. A typed domain error is a normal declared result, distinct from cancellation, trap, deadline, budget exhaustion or internal failure. `future<s64>` has no typed domain-error alternative. A stream's bytes and read-end closure MUST NOT encode or imply its producer's terminal success/error: `drain-stream` returns the separately recorded terminal `result<list<u8>,string>`. `cancel-future` consumes and cancels the future task, whereas `close-stream` consumes only the read end and leaves producer terminal observation/cleanup to the host. Neither explicit endpoint operation alone is invocation cancellation.

#### Scenario: Typed terminal error is not stream data or cancellation

- GIVEN a compiled `future<result<s64,string>>` consumer or a compiled `stream<u8>` consumer whose host records a producer domain error
- WHEN the live value is consumed and its declared terminal result is delivered
- THEN the consumer receives the typed string-error branch as a normal return, stream bytes are not interpreted as errors, and ownership cleanup remains separate from invocation cancellation

#### Scenario: Explicit endpoint termination preserves distinct obligations

- GIVEN an owned live endpoint returned by a synchronous factory
- WHEN the guest invokes the endpoint's typed `cancel-future` or `close-stream` operation
- THEN the sender loses the endpoint, future cancellation suppresses future delivery, stream read-end closure leaves terminal producer observation to the host, and neither operation alone cancels the invocation

#### Scenario: Unsupported live shapes and borrows are refused

- GIVEN a WIT signature containing `future<list<u8>>`, `stream<s64>`, `result<own<counter>,string>` or an async borrowed parameter
- WHEN the selected profile attempts WIT admission
- THEN it rejects the unsupported exact shape before component emission instead of narrowing it to an admitted payload or permitting the borrow to survive suspension

### Requirement: WI-ASYNC-04
r[WI-ASYNC-04]

**WI-ASYNC-04.** Until duplication/capture semantics are proven and standardized for a particular async value, `stream<T>` and `future<T>` boundary values SHALL conservatively be treated as non-`Data` and non-`Capture` when they carry live runtime state.

For the selected live resource kinds, this exclusion MUST apply recursively through Noble `Pair`, `Sum` and `List` payload types, including unselected alternatives. `dup`, generic `drop`, `quote`, and the `quote reflect` route to generic serialization MUST reject a live value or an aggregate containing one before a component is published or any candidate-body host request occurs. Constructing, moving and unpacking such an aggregate for a typed consumer MAY remain valid; a Noble `Pair` does not imply a WIT tuple adapter. A live handle MUST NOT become capturable inert syntax merely because its machine representation is an integer.

Passing a live endpoint or owned resource to a consuming boundary operation MUST transfer its single ownership obligation; the sender MUST NOT retain a reusable alias. A normally returned owner belongs to the receiver only after delivery. The selected string-error result shapes do not themselves return an owner; broader owner-bearing error results require an explicit boundary contract. Typed close/cancel operations and abnormal cleanup MUST discharge outstanding obligations without treating generic guest `drop` or GC reachability as resource retirement.

#### Scenario: Recursive eligibility fails before guest requests

- GIVEN an admitted live `stream<u8>` or `future<s64>` type, or a Noble `Pair<Text,stream<u8>>` constructed from a supported factory
- WHEN source checking reaches `dup`, generic `drop`, `quote` or `quote reflect` on that value
- THEN kernel eligibility rejects the operation before publishing a component or issuing a candidate-body host request, rather than relying on an unsupported-WIT or lowering failure

#### Scenario: Moving an aggregate into a typed sink is valid

- GIVEN a checked Noble body that obtains a stream, pairs it with text, unpairs it and invokes `close-stream`
- WHEN the emitted component executes in the independent peer
- THEN the typed close consumes the single live endpoint while the text remains ordinary data, with no duplication, capture, generic live drop or implied WIT tuple support

### Requirement: WI-ASYNC-05
r[WI-ASYNC-05]

**WI-ASYNC-05.** The standard async profile MUST specify cancellation propagation, outstanding resource ownership, completion errors, quotas/backpressure where applicable, and cleanup after abnormal termination. Native Component Model async is an ABI mechanism, not a complete concurrency policy.

`Component-Async-Bootstrap` uses host-owned task records under SPEC-R001 RA-ASYNC-02 through RA-ASYNC-05. Admission MUST preflight task, terminal-result, payload, pin, wakeup and retirement capacity before transferring input custody or consuming protected authority. Completion and delivery MUST remain separate: a ready result is task-owned until one delivery transfers custody. Cancellation before delivery MUST revoke guest access and retire undelivered input/result obligations without resurrection; cancellation after delivery MUST NOT reclaim receiver-owned results. Trap, deadline, budget and internal failure MUST preserve their abnormal-cleanup obligations rather than masquerade as typed domain errors.

The selected driver is a single-thread cooperative engine with bounded external blocking workers. Its pinned limits are 100,000 guest fuel, a 1,000-fuel yield quantum, 4,194,304 bytes of Store memory, 32 native jobs, 8 live values and 256 driver events. The stream producer uses a one-byte engine buffer and the consumer reserves a 64-byte payload; excess payload MUST fail explicitly, not create an unbounded queue. Live-value admission reserves 128 result-payload bytes, 128 locally parked payload bytes and 128 externally parked payload bytes. These reservations cover payload storage, not all engine allocations, task metadata or worker-thread stacks; they MUST NOT be reported as whole-process heap accounting. Task-table capacity and transition decisions remain independently enforced by the production kernel.

In this selection, invocation cancellation MUST combine logical task-table revocation with stopping guest access through invocation-isolated Store teardown. It MUST NOT imply that blocking native work stopped. Native pins and retained host storage MUST survive cancellation acknowledgement and Store destruction until authentic native-stop/settlement permits release; no per-task Wasmtime cancellation API is claimed for 40.0.2. Stream read-end closure MUST leave the host responsible for the producer's terminal outcome. Cleanup and task-exit observation MUST precede successful invocation observation, and late native completion MUST NOT restore cancelled delivery or consumed authority.

The selected profile classifies every admitted outbound operation under H-AUTH-01. These are local fixture policies, not permissions inferred from the WIT type or the presence of a native async ABI:

| Operation | Classification | Selected scope and rationale |
|---|---|---|
| `noble-test:async-boundary/host@1.0.0#first` | Protected | The exact argument and local operation plan require a current host grant, one-shot witness consumption and attempt registration before native work; a later observation and receipt are separate. |
| `noble-test:async-resources/counters@1.0.0#transfer`, `#consume-error` | Protected | Transfer or consume an already-owned local fixture counter only after resource/argument preflight and exact protected admission. A domain error does not restore the sender or the witness. |
| `#second`, `noble-test:async-wide/host@1.0.0#sum5` | Unprotected local arithmetic | Compute bounded local scalar results without accessing an external service or granting authority; task and progress limits still apply. |
| `#make-future`, `#make-stream` in the selected bootstrap and eligibility worlds | Unprotected local producer creation | Create bounded peer-local future/stream endpoints and native worker obligations; no remote operation or permission is implied. |
| `#finish-future`, `#drain-stream` in the selected bootstrap and eligibility worlds | Unprotected local endpoint consumption | Consume a previously owned live endpoint and account for its terminal result or error; this is not fresh authorization for a protected operation. |
| `#cancel-future`, `#close-stream` in the selected bootstrap and eligibility worlds | Unprotected local revocation or read-end closure | Revoke guest access or close the stream reader without claiming native stop, operation failure or release of an outstanding pin. |
| Fixture counter creation, canonical resource destruction and invocation cleanup | Unprotected local ownership and retirement | Maintain the existing host-owned fixture's custody and physical cleanup obligations; GC reachability and generic guest `drop` do not discharge them. |
| `wasi:clocks/monotonic-clock@0.3.0-rc-2025-09-16#wait-for` | Unprotected, peer-only compatibility probe | Wait for a fixed 1,000,000 ns against the trusted local clock in a hand-written component; this is not an import of Noble output or a stable-WASI grant. |

The protected set is exactly `first`, `transfer` and `consume-error`. All other selected imports above are explicitly scoped unprotected operations; unapproved interfaces or operations fail closed. Protected admission, authentic current facts and observations, and local task custody remain distinct contracts even when one peer lock serializes their decisions.

The concrete selected ABI and endpoint interfaces are specified above and implemented by the [component emitter](../../../../../crates/noble-wasm/src/component/emit.rs), [waitable adapter](../../../../../crates/noble-wasm/src/component/async.wat), [peer driver](../../../../../verification/m6/peer/src/driver.rs) and [live-value transfer adapter](../../../../../verification/m6/peer/src/transmit.rs). Broader payload shapes, general async borrowing, cross-component concurrency policy, full WASI integration and universal compiler/ABI/engine refinement remain open. Local task-transition correspondence does not prove host callback authenticity, physical native release or OS scheduling.

#### Scenario: Quota refusal preserves pre-admission custody

- GIVEN caller-owned inputs and an unused applicable authorization witness
- WHEN task or payload reservation fails before admission commits
- THEN custody remains with the caller, the witness remains unused, and no protected operation starts

#### Scenario: Ready-result cancellation and delivered-result cancellation differ

- GIVEN a completed task whose result remains ready but undelivered
- WHEN cancellation wins before delivery
- THEN the result remains an undelivered retirement obligation and cannot be published or resurrected; if delivery won first, a later cancellation cannot reclaim receiver custody

#### Scenario: Guest stop does not release native pins

- GIVEN an invocation with blocking native work and a retained native pin
- WHEN cancellation or abnormal termination stops guest access and destroys the invocation Store before the worker stops
- THEN host storage and the pin remain retained until authentic native-stop settlement, cleanup occurs without duplicate release, and any late completion cannot publish a cancelled result

#### Scenario: Bounded stream storage is not a heap proof

- GIVEN the selected one-byte producer buffer, 64-byte consumer payload and explicit live payload reservations
- WHEN the stream transfers data or exceeds its payload reservation
- THEN it remains within the selected buffers or fails explicitly, and the observation is not reported as accounting for engine metadata, worker stacks or the entire process heap
