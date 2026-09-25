<!-- Generated compatibility view. Edit .cairn/specs/wit-wasi/spec.md instead. -->
# WIT, Component Model, and WASI Integration Profile

Document: SPEC-W001  
Revision: 0.1.0-draft.5  
Project: noble  
Status: Canonical working profile with accepted bounded synchronous M5 and native-async M6 slices; broader implementation and conformance remain open
Depends on: SPEC-0001, SPEC-S001, SPEC-V001 and SPEC-R001 at 0.1.0-draft.5

The first delivery is a named synchronous subset, not full `Component-Draft` conformance. See [RESOURCE-ADAPTERS.md](RESOURCE-ADAPTERS.md) and [ROADMAP.md](ROADMAP.md).

The delivered `Component-Sync-Bootstrap` slice compiles the pinned [`noble-test:sync/bootstrap@1.0.0` world](../crates/noble-wasm/wit/bootstrap.wit), with typed imports/exports for arithmetic, UTF-8 strings, byte lists, owned counters and host-established authorization. Generated imports retain exact fully qualified versioned request effects, including for reviewed-pure operations; each export's complete stack interface and effects are independently checked before emission. Unsupported signatures and bodies are refused rather than treated as full WIT support.

The [M5 gate](../verification/m5/gate.mjs) exercises emitted components against an independent Rust/Wasmtime 40.0.2 peer under the [pinned synchronous memory32/UTF-8 ABI](../verification/m5/pins.json). The peer independently supplies component linking and typed conversion, while using Noble's production resource/authority decisions for host policy. Protected work requires a host-established one-shot witness bound to the exact plan and current applicable facts; admission consumes the witness and records the attempt before work. Receipts distinguish admission, operation observations and invocation outcomes: a plan, request, handle, effect or digest does not establish authority or success, and late operation success cannot reverse invocation cancellation.

The [M5 completion record](../verification/m5/evidence.json) owns acceptance of this bounded slice. It does not implement WASI, native async/future/stream execution, full `Component-Draft`, or an arbitrary first-class `Program` ABI. Actual resource-transition correspondence is a separate [strict proof lane](../proofs/m5/M5Resources.lean); it does not prove native release, host fact/callback authenticity, or universal component compiler, ABI or engine refinement.

### Bootstrap host-operation classification

The synchronous bootstrap execution profile classifies every admitted outbound
operation below. These are scope-specific host policies, not permissions inferred
from a WIT declaration or guest effect annotation. Import words refer to the
`noble-test:sync@1.0.0` interfaces in the pinned world.

| Operation | Classification | Scope and review rationale |
|---|---|---|
| `arithmetic.inc` | Unprotected | Wrapping arithmetic on the supplied `s64`; no protected external operation. |
| `echo.text`, `echo.bytes` | Unprotected | Copy and return the supplied string or byte list within the bounded invocation; no external authority or zero-copy claim. |
| `counter.read` | Unprotected | Read an already-owned local fixture counter through a scoped borrow; kind, context, generation, rights and ownership checks remain mandatory. |
| `counters.transfer` | Unprotected | Transfer the local fixture counter to the receiver and discharge that receiver's cleanup obligation, including on its domain-error result; no remote operation or permission is implied. |
| `authorization.prepare` | Unprotected authorization request | Request the trusted local authorizer's decision without executing the protected operation. Only independently supplied current host facts can establish the returned one-shot witness. |
| `authorization.protected` | Protected | Bind the exact argument and operation plan, revalidate current facts, consume the witness and record the attempt before updating the local protected counter; observation and receipt construction follow separately. |
| Canonical counter destruction and invocation cleanup | Unprotected retirement | Discharge existing local ownership obligations; cleanup grants no new access and does not turn GC reachability into resource retirement. |
| Canonical authorization destruction and invocation cleanup | Unprotected revocation | Retire an unused witness without executing, restoring or duplicating its authority. This emergency boundary cleanup is not generic guest `drop` permission. |

The independent-peer compatibility aliases
`noble-test:math/arithmetic@1.0.0#inc` and
`noble-test:counter/counters@1.0.0` use the same bounded arithmetic/counter
classifications. The separate `noble-test:store/api@1.0.0#read` negative probe is
protected and always denied because this profile supplies no host grant.
Unapproved interfaces and operations are rejected.

Authorization is synchronized within one serialized synchronous host invocation.
Its configured checkpoint, authenticated authority/observation sources and
current credential, revocation, policy and quota facts are trusted embedding
inputs, not guest claims. The profile promises neither remote authorization
atomicity nor a distributed revocation service. Ordinary CLI filesystem,
process, environment, clock, supervision and diagnostic I/O have the separately
scoped unprotected classifications and rationales in
[`policy/architecture.ncl`](../policy/architecture.ncl).

### Selected native-async boundary and compatibility

`Component-Async-Bootstrap` is the completed bounded M6 selection alongside, not a replacement for, `Component-Sync-Bootstrap`. Its primary world is [`noble-test:async-boundary/bootstrap@1.0.0`](../crates/noble-wasm/wit/async.wit). The implementation compiles direct-style Noble exports with native async imports, live future/stream transfer, and mixed synchronous members. The [M6 completion record](../verification/m6/evidence.json) establishes only its separately scoped acceptance; it does not narrow the general `Component-Draft` requirements below.

The reproducible selection is recorded in [`verification/m6/pins.json`](../verification/m6/pins.json):

| Boundary | Selected contract |
|---|---|
| Canonical ABI | memory32, UTF-8, native async lowering, stackful async lifting and `task.return`; waitable-set/subtask primitives suspend the compiled invocation rather than interpreting source or recipes |
| Component tooling | `wasm-tools` 1.245.1 with `cm-async`, `cm-async-stackful` and `cm-async-builtins` |
| Independent peer | Wasmtime 40.0.2 and `wasmtime-wit-bindgen` 40.0.2; selected `wit-parser` and `wit-component` dependencies 0.243.0; source/vendor closures are pinned separately from Noble semantic identity |
| Engine features | `async_support`, `component_model`, `component_model_async`, `component_model_async_builtins`, `component_model_async_stackful`, fuel consumption and epoch interruption |
| WASI compatibility | Official `0.3.0-rc-2025-09-16` interfaces from the same pinned Wasmtime source; only `wasi:clocks/monotonic-clock@0.3.0-rc-2025-09-16#wait-for` is executed in the compatibility probe |

The [M6 gate](../verification/m6/gate.mjs) keeps distinct evidence lanes:

- **Noble compilation and execution:** real `.noble` bodies pass export checking and independent kernel acceptance, emit core Wasm and a component, and undergo independent `wasm-tools` validation/interface inspection before the Rust peer executes them. The selected world exercises sequential imports, `future<result<s64,string>>`, `stream<u8>`, explicit future cancellation and stream closure. Additional compiled worlds cover `future<s64>`, a Noble `Pair<Text,stream<u8>>`, owned-resource transfer/domain-error cleanup, and five-argument async lowering. The Pair case does not claim WIT tuple support.
- **Independent compatibility probes:** the [peer](../verification/m6/peer/src/compatibility.rs) admits the Noble-emitted component with the selected native features and rejects it with async or stackful support disabled, before starting imports. A separate, hand-written [clock component](../verification/m6/peer/src/clock.wat), not Noble output, executes the official prerelease `wait-for` for 1,000,000 ns and awaits task exit. Changing that component's import to stable `0.3.0` is rejected at link, with no imports started. This is not Noble `u64` surface support, stable WASI 0.3 linkage, or coverage of other WASI interfaces.
- **Local protocol and progress probes:** task lifecycle/schema controls and runnable-fuel, epoch and blocking-deadline probes exercise the peer and production kernel decisions separately from Noble compilation. Deterministic kernel tests and the [strict async proof source](../proofs/m6/M6Async.lean) are also separate assurance scopes; they do not turn native execution into a universal ABI or engine proof.

The peer independently supplies Component Model linking, binding generation and typed conversion, but deliberately reuses Noble's production task, resource and authority decisions. Selected tooling, host callback/fact authenticity, physical native retirement, clocks and OS scheduling remain trusted boundaries. The [independent source-bound extraction/check](../verification/m6/extraction.json), [formal archives](../verification/m6/formal-archives.json), [full regressions](../verification/m6/regressions.json) and [thirteen Nix checks](../verification/m6/nix-checks.json) passed in separate lanes; the [completion record](../verification/m6/evidence.json) binds their scoped results without discharging trusted host obligations.

## 1. Purpose

This specification makes the WebAssembly Component Model a first-class Noble platform boundary without making Noble's internal language WIT-shaped.

The selected architecture is:

```text
Noble source semantics
    Program<S,T,e>, stack types, effects, resources
                 |
                 | checked specialization/adaptation
                 v
WIT packages and worlds
    external component contracts
                 |
                 v
Component Model / Canonical ABI
                 |
                 v
Wasm components + WASI host profiles
```

The normative distinction is:

> **WIT is Noble's standard external interface language. WASI is Noble's preferred standard host-interface family. Noble's internal types, effects, programs-as-data semantics, and authority model remain richer and independent.**

The standard component profile targets the stable WASI 0.3 family. An implementation MAY support WASI 0.2 through an explicit compatibility profile. Exact package versions and runtime/toolchain versions are build inputs and MUST be pinned for reproducible artifacts. The selected `Component-Async-Bootstrap` prerelease compatibility boundary above is distinct from that stable target; it does not silently substitute prerelease packages for stable WASI.

## 2. Architectural position

**WI-ARCH-01.** A conforming `Component-Draft` implementation SHALL treat WebAssembly components, rather than unadorned core modules, as the primary portable deployment/interoperability artifact. Internal compiler stages MAY use core Wasm modules, and an explicitly named lower-level profile MAY expose them, but that profile is not the standard Noble component contract.

**WI-ARCH-02.** WIT SHALL be the standard language for describing Noble component imports, exports, resource interfaces, and host-facing component contracts. Noble MUST NOT invent a parallel general-purpose component IDL where WIT can express the required contract.

**WI-ARCH-03.** WIT is not Noble's internal type system. It MUST NOT constrain the semantics of `Program<S,T,e>`, stack-tail polymorphism, effect bounds, recipes, syntax reflection, definition identity, proof evidence, or other Noble-only concepts merely because those concepts lack direct WIT equivalents.

**WI-ARCH-04.** Noble MUST interpret a WIT world as describing a component boundary: the functionality a component requires and provides. It does not specify the component's internal behavior, purity, Noble effect bound, proof status, or concrete runtime authority.

## 3. WIT package ingestion and generated bindings

**WI-WIT-01.** The Noble compiler SHALL accept versioned WIT packages/worlds as compiler inputs and generate typed Noble bindings without requiring the user to hand-write equivalent host declarations.

**WI-WIT-02.** Generated bindings MUST preserve the complete externally visible WIT type distinction. Where the Noble kernel has no identical scalar or aggregate type, the binding layer MUST introduce an exact declared/boundary type or checked conversion; it MUST NOT silently narrow, wrap, reinterpret, or otherwise lose information.

**WI-WIT-03.** For a WIT function with ordered parameters `A1 ... An` and ordered results `B1 ... Bm`, a generated Noble import word MUST have the schematic stack interface:

```text
S A1 ... An -- S B1 ... Bm ! {WitOpId}
```

Here `WitOpId` identifies the exact versioned WIT package, interface, and function. These resolved identities participate in semantic identity, not only build identity.

This schematic signature applies to directly lowered parameters and results. Synchronous borrowed imports use the owner-threading adapter in SPEC-R001. Generated bindings publish the adapted signature explicitly.

Every guest-requested import MUST retain its `WitOpId` in the effect bound, including imports with reviewed deterministic or side-effect-free implementations. Such review does not remove the boundary request. Host-operation internals remain separate from request accounting under V-EFFECT-01. This profile defines no pure foreign-call exemption.

**WI-WIT-04.** A WIT import MUST contribute its operation identity to the effect bound. A signature or reviewed purity claim MUST NOT erase that requirement. A WIT signature alone MUST NOT establish purity, determinism, termination, authorization, or absence of external interaction.

**WI-WIT-05.** A Noble export implementing a WIT function MUST have a closed, WIT-lowerable external interface. The component adapter SHALL invoke that program against an isolated adapter stack containing only the declared parameters and SHALL validate that its normal results match the declared WIT result contract. An ambient Noble stack tail MUST NOT cross a component boundary implicitly.

**WI-WIT-06.** Values crossing the component boundary MUST be validated and lowered/lifted according to the selected Component Model/Canonical ABI contract. An implementation MUST NOT expose internal addresses, closure pointers, stack locations, or unspecified language representations as WIT values.

**WI-WIT-07.** The adapter contract MUST account for allocation, copying, and buffer ownership during lifting/lowering and cleanup. A failed conversion MUST NOT publish a partially initialized trusted value. Internal GC support or a byte-view library MUST NOT imply zero-copy component transfer.

M5 checks lossless UTF-8 string and byte-list round trips through the independent component peer. Separate hostile-import probes run the unchanged emitted core Wasm to exercise allocation-quota failures, malformed UTF-8 and truncated ranges, with cleanup and no partially trusted result publication. These are core-boundary probes, not malformed typed Wasmtime values. Success-path canonical post-return and trap-path host cleanup are distinct; core allocation/copy counters do not measure all engine allocations or physical copies. The historical, nonblocking M3 component probe in [SPEC-BE001](BACKEND-EXPERIMENTS.md) is not M5 conformance evidence.

## 4. WIT resources and Noble ownership

**WI-RES-01.** An owned WIT resource handle SHALL map to a Noble move-only resource obligation. It MUST NOT satisfy `Data` or `Capture` merely because the WIT representation is handle-shaped.

**WI-RES-02.** A borrowed WIT resource SHALL be represented as a scoped non-owning boundary value whose lifetime cannot escape the adapter call or declared borrow scope. It MUST NOT be captured, persisted, serialized, stored beyond its scope, or released as though it were owned. This requirement does not introduce a general Rust-style borrow checker into ordinary Noble code.

**WI-RES-03.** WIT resource type identity, ownership mode, component instance/context, and host validation MUST participate in resource-handle validation. Integers, bytes, external IDs, or stale handles MUST NOT be accepted as live resources solely because they have a compatible machine representation.

**WI-RES-04.** Resource lowering MUST preserve the existing Noble normal-path and abnormal-cleanup obligations. A Component Model adapter MUST NOT weaken SPEC-0001's move-only ownership rules.

**WI-RES-05.** The first synchronous subset MUST obey SPEC-R001. It rejects borrowed exports, escaping borrows, reentrant access to busy owners, and suspension with an active borrow. This restriction does not reject those features permanently. Their later admission requires a separate lifetime contract and evidence.

## 5. Worlds, effects, and authority

**WI-WORLD-01.** The selected WIT world and exact imported/exported interface versions SHALL be explicit compilation inputs and SHALL participate in the build key. Replacing an interface version or world is not an invisible linker detail.

**WI-WORLD-02.** The compiler SHALL reject an export whose Noble interface cannot be soundly lowered to the selected WIT contract. It MUST NOT satisfy an incompatible WIT export through `Any`, unchecked casts, implicit resource laundering, or runtime operand guessing.

**WI-AUTH-01.** WIT import availability and a Noble effect bound are not, by themselves, authority grants. Runtime authority SHALL remain controlled by the linked host/component implementation, resource/capability values, and host policy. Some imported functions may themselves convey ambient authority; such authority MUST be documented by the host profile and is not created by the Noble type checker.

**WI-AUTH-02.** Standard Noble profiles SHOULD prefer capability-scoped/resource-scoped WIT and WASI interfaces over ambient global authority where the ecosystem interface permits it.

**WI-AUTH-03.** WIT package/interface/function identity MAY supply stable names for Noble host-operation/effect identities. Such identities describe the operation contract, not the caller's permission to perform it.

## 6. WASI as the standard host-interface family

**WI-WASI-01.** The standard Noble host profile SHALL use stable WASI interfaces where a stable WASI interface adequately represents the desired host capability. A Noble-specific duplicate filesystem, clock, random, socket, HTTP, CLI, or similar interface SHOULD NOT be standardized without a documented semantic reason.

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

**WI-WASI-03.** WASI 0.2 support MAY be provided as an explicitly named compatibility profile or adapter. A compiler/runtime MUST NOT silently reinterpret a 0.2 component as 0.3, or vice versa, without a documented compatibility layer.

**WI-WASI-04.** Noble-specific platform facilities that cross a component boundary SHOULD themselves be expressed as versioned WIT packages so Rust, Noble, and other Component Model languages can participate without language-specific ABI agreements.

## 7. Native async, streams, and futures

WASI 0.3 and the Component Model provide native cross-component `async func`, `stream<T>`, and `future<T>` primitives. Noble adopts these as its standard *component-boundary* async ABI. This choice does not add `async` or `await` expression forms, a task constructor, a guest scheduler, or a spawn/channel language to Noble. The selected implementation is the pinned `Component-Async-Bootstrap` boundary, not the whole standard async surface.

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

**WI-ASYNC-02.** Suspension of an async import MUST NOT imply concurrency of subsequent sequential Noble words, speculative execution, retry, transactionality, or reordering.

The selected adapter MUST allow at most one outstanding direct-style async import per compiled invocation and MUST exclude guest reentrancy while that invocation is active. Host-owned future/stream producers and bounded external native workers MAY progress independently; their existence MUST NOT license executing a later sequential guest word early. Failure, cancellation, trap or quota exhaustion MUST NOT automatically replay an import, restore a consumed authorization witness, roll back an admitted effect, or promote a late operation success to invocation success.

#### Scenario: Cancellation cannot replay admitted work

- GIVEN an admitted protected operation with consumed authority, an outstanding native pin and a later sequential guest word
- WHEN invocation cancellation is acknowledged before authentic native completion
- THEN guest continuation and result delivery remain revoked, the consumed witness cannot authorize replay, and a late operation-success observation cannot change the cancelled invocation to success or roll back the admitted operation

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

The concrete selected ABI and endpoint interfaces are specified above and implemented by the [component emitter](../crates/noble-wasm/src/component/emit.rs), [waitable adapter](../crates/noble-wasm/src/component/async.wat), [peer driver](../verification/m6/peer/src/driver.rs) and [live-value transfer adapter](../verification/m6/peer/src/transmit.rs). Broader payload shapes, general async borrowing, cross-component concurrency policy, full WASI integration and universal compiler/ABI/engine refinement remain open. Local task-transition correspondence does not prove host callback authenticity, physical native release or OS scheduling.

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

**WI-PROG-01.** Noble `Program<S,T,e>` is not automatically a WIT function value, resource, or closure. A component boundary MUST NOT export an internal program as a raw address or unspecified handle.

P-ROUND-01/02 and P-PACK-01/02 in SPEC-0001 define round-trip and dependency-closure obligations. They do not select a portable byte format or make a digest sufficient for admission.

**WI-PROG-02.** Transporting a Noble program across a component, process, or network boundary remains an explicit portable-code/package problem. Such transport MUST preserve recipe/interface/dependency identity, validation/preparation requirements, capture eligibility, and destination-side authorization.

**WI-PROG-03.** A concrete WIT resource interface for prepared Noble programs MAY be standardized later, but it MUST specify ownership, interface specialization, recipe/provenance binding, lifecycle, and authorization independently of the internal `Program<S,T,e>` representation.

## 9. Syndicate, Preserves, and component boundaries

**WI-SYN-01.** Syndicate/Synit remains the standard Noble concurrency/service model. Where the Syndicate layer crosses Component Model boundaries, its host/component operations SHOULD be exposed through versioned WIT packages rather than bespoke native ABIs.

The first M7 local synchronous selection uses the versioned
`noble:syndicate@1.0.0` `service` world with a `dataspace` import. This is a
bounded WIT boundary on the M5 synchronous component profile, not a native
async service, a general Syndicate implementation or a WASI package.
Participant component instances have separate Noble memory and host-bound
facet identities. The host, not a guest WIT argument or Preserves field,
assigns each facet and its runtime rights.
For acceptance, the independently built Rust/Wasmtime peer supplies
Component Model linking and dynamic typed `Val` conversion; it uses the
accounted Noble production policy and dataspace decisions. This is a
trusted bounded test host, not evidence of generated peer bindings or a
deployable general Syndicate runtime.

**WI-SYN-02.** Preserves and WIT are complementary. Preserves is the default protocol/schema representation for Syndicate conversational data; WIT defines component interfaces, resources, and ABI-visible operations. A Preserves value MUST NOT become a WIT resource/capability without an explicit validated adapter.

The selected adapter uses only the canonical Preserves text subset and
`service(name:Text,ready:Bool)` schema in S-CONC-04/S-CONC-08. The versioned
WIT methods carry `string` and `bool` through the checked component ABI;
typed `Service` data comes only from the bounded schema-checked decoder, not
from a forged WIT resource representation. Encoding then decoding each actual
published service in the selected compiled-component scenario MUST roundtrip
the name and either Boolean value. Neither roundtrip, schema acceptance nor
component validity establishes authority, proof, or a live resource.

**WI-SYN-03.** A componentized Syndicate implementation MUST preserve the standard concurrency profile's scope, assertion-lifetime, isolation, capability, and hostile-input safety requirements across the WIT boundary.

The M7 host MUST serialize selected dataspace operations against host-bound
facet ownership. It MUST validate each instance's publication, observation or
retraction right independently of the well-typed WIT call and reject
cross-facet mutation. Trapping a compiled publisher after a successful import
MUST retire its assertion and subordinate facet-owned state without requiring
a second guest call. The independently active observer's exact-pair interest
MUST see the local removal in the host event trace; a failed return from the
trapping export MUST NOT be reported as a successful invocation, even though
its earlier publication was admitted.

**WI-SYN-04.**

The selected synchronous `noble:syndicate@1.0.0` boundary SHALL use this
versioned WIT surface:

```wit
package noble:syndicate@1.0.0;
interface dataspace {
  publish: func(name: string, ready: bool) -> bool;
  observe: func(name: string, ready: bool) -> bool;
  retract: func(name: string) -> bool;
  fail: func(value: bool) -> bool;
}
world service {
  import dataspace;
  export publisher: func(name: string, ready: bool) -> bool;
  export observer: func(name: string, ready: bool) -> bool;
  export withdraw: func(name: string) -> bool;
  export publish-and-trap: func(name: string, ready: bool) -> bool;
}
```

The compiled `publisher`, `observer` and `withdraw` exports call their
corresponding import directly. `publish-and-trap` calls `dataspace.publish`
then `dataspace.fail`; the selected host's `fail` deliberately traps after
the successful publication to exercise failure cleanup. `observe` installs
an exact `(name,ready)` interest owned by the calling facet and returns
whether that exact assertion is currently present; false is absence of the
queried pair, NOT the service's `ready` value. A separate host event trace
records typed `(name,ready)` add/remove events. All four imported words MUST
retain their exact fully qualified versioned WIT operation identities in
Noble request effects; the WIT signatures alone convey no host right.
Unsupported versions and mismatched signatures MUST be rejected before
starting imports. This synchronous boundary does not implicitly select M6
native async, transport protocols or a general WIT resource conversion.

Acceptance for this selection MUST execute two separately compiled Noble
participant instances with an independent typed component peer: register the
observer's interest, publish and observe each of `ready=true` and
`ready=false`, encode/decode the admitted Preserves values, and observe
removal after retraction and after the compiled publish-then-trap path.
Host-only transition tests, a hand-written component or a byte decoder
roundtrip alone MUST NOT be reported as that compiled scenario. Host
serialization and facet binding, decoder behavior, actual component
execution, kernel correspondence and trusted engine/host boundaries retain
distinct evidence claims; none proves universal component or concurrency
refinement.
## 10. Versioning, identity, and reproducibility

**WI-ID-01.** Exact WIT package/world/interface versions, Component Model feature profile, WASI profile, adapters, and relevant runtime/compiler configuration SHALL participate in the `BuildKey` when they can affect generated artifacts or observable boundary semantics.

**WI-ID-02.** WIT or WASI package identity MUST NOT replace `DefinitionId` or `ProgramValueId`. Component interface identity and Noble program identity answer different questions.

**WI-ID-03.** Upgrading a WIT/WASI dependency MUST NOT silently mutate a previously prepared Noble program or running component. Rebuilding against a changed boundary creates a new build context and, where emitted bytes differ, a new artifact identity.

**WI-ID-04.** A changed resolved `WitOpId`, semantic schema, or dependency MUST change the containing definition identity. Dependent program-value identities MUST include that changed dependency. Unrelated definitions do not change merely because another world import changes.

**WI-ID-05.** An adapter-only change can preserve semantic identity only when the resolved Noble operation and schema contracts remain identical. The build key MUST change when the adapter can affect emitted artifacts or boundary behavior. Matching WIT function shapes alone does not establish semantic equivalence.

The [identity scenarios](conformance/identity-cases.json) distinguish semantic changes from build-only changes. They specify relations, not stable digest bytes. Canonical Noble encoding remains open.

## 11. Safety and trust boundary

**WI-SAFE-01.** WIT/component decoding, lifting/lowering, resource-table access, host callbacks, and native adapter entry points are hostile or semi-trusted boundaries under SPEC-S001. Public wrappers MUST validate every precondition not already established by an independently verified caller.

**WI-SAFE-02.** A valid Wasm component or valid WIT world does not prove that Noble recipe metadata, effect metadata, proof evidence, resource authority, or semantic provenance is truthful. Loading/admission MUST preserve SPEC-0001's separate validation and trust checks.

**WI-SAFE-03.** The standard Noble-safe claim for the Component profile MUST include tests and assurance covering WIT type mapping, resource ownership/borrows, malformed component values, wrong-context handles, import/export mismatch, async cancellation/cleanup, and the selected loader/linker boundary.

## 12. Conformance scenarios

The accompanying [conformance/wit-wasi-cases.json](conformance/wit-wasi-cases.json) contains expected scenarios and their recorded state/evidence. The M5 runtime gate executes WI-01, WI-02, WI-04 through WI-10, and WI-15 in the declared synchronous slice, together with resource, boundary-conversion and authorization cases. The M6 gate supplies the selected native-component WI-11/WI-12 lanes, real source-checking WI-16 refusals, and separately classified protocol/compatibility/progress controls described above. The M6 completion record retains separate source-bound closeout and conformance-ledger evidence; neither completed bounded gate closes the remaining `Component-Draft` or WASI obligations. The separately accepted M7 local synchronous service executes WI-14 and WI-18 plus five selected safety cases; the broader `Component-Draft` and WASI obligations remain open.

A conforming Component profile must cover at least:

- importing a WIT function as a statically typed Noble word;
- rejecting a Noble export with an incompatible stack/result interface;
- preserving exact WIT numeric/data distinctions without lossy implicit conversion;
- owned WIT resources remaining move-only/noncapturable;
- borrowed WIT resources being rejected if they escape scope;
- forged or stale resource handles being rejected;
- an imported WIT function contributing a conservative effect requirement;
- lack of runtime authority despite a type-correct effect/import when the host does not grant it;
- native async suspension preserving sequential Noble ordering;
- typed `stream<T>`/`future<T>` boundary handling and cleanup;
- explicit WASI 0.2 compatibility rather than silent version substitution;
- `Program<S,T,e>` not being exported as a raw generic WIT function/address;
- exact WIT/WASI versions affecting build identity;
- Preserves protocol values requiring explicit conversion at WIT resource/capability boundaries.

## 13. Release gates and open work

Before the standard Component profile can be called stable, the project must complete:

The table retains the broader standard-profile gates. M5 supplies a bounded implementation and executable evidence for portions of OW-02, OW-03, OW-04 and OW-08, plus the separately scoped resource correspondence described above. M6 completed its separately retained source-bound acceptance for the selected async lowering/lifting, move-only live values, host task/endpoint ownership and prerelease compatibility probe; that scoped completion does not close any broader standard-profile gate. The selected M7 local synchronous service and Preserves subset have separately accepted bounded execution evidence; broader Component, Syndicate and WASI gates remain open.

| ID | Deliverable |
|---|---|
| OW-01 | Complete lossless WIT-to-Noble type mapping, including exact integer/floating widths and aggregate/schema identities |
| OW-02 | Complete ownership correspondence and broader SPEC-R001 coverage beyond the selected synchronous/native-async boundaries; borrowed exports and general async lifetimes remain separate designs |
| OW-03 | Extend the checked WIT import/export compiler and generated bindings beyond the exact selected worlds, types and bodies |
| OW-04 | Extend Canonical ABI lowering and loader/linker coverage beyond the pinned memory32/UTF-8 synchronous and native stackful-async selections |
| OW-05 | Stable WASI 0.3 package/interface and runtime/tooling compatibility matrix; the official prerelease clock probe is not stable or family-wide support |
| OW-06 | General async/stream/future payloads, borrowing and concurrency policy beyond the selected direct-style calls, typed endpoints and task-retirement protocol |
| OW-07 | Syndicate/Synit WIT package boundary and Preserves conversion profile |
| OW-08 | Broaden independently executed cross-language interoperability beyond the bounded M5/M6 Rust/Wasmtime peer lanes, preserving the distinction between compiled Noble and peer-only probes |
| OW-09 | Safety/trust correspondence for public wrappers, resource tables, lifting/lowering, and component loading |
| OW-10 | Source/recipe-to-component correspondence evidence appropriate to advertised verification claims |

## 14. External reference status

The architectural facts used by this revision were checked against first-party documentation on 2026-09-12:

- WIT worlds describe a component's imports and exports and define its external contract: <https://component-model.bytecodealliance.org/design/wit.html> and <https://component-model.bytecodealliance.org/design/worlds.html>
- WASI 0.3 is the current stable WASI family; 0.3 adds Component Model native async primitives `async func`, `stream<T>`, and `future<T>`: <https://wasi.dev/releases> and <https://wasi.dev/releases/wasi-p3>
- Component Model native async is documented at <https://component-model.bytecodealliance.org/design/async.html>

These references establish ecosystem mechanisms and current release status. Noble's type mapping, effect treatment, ownership rules, standard-profile policy, and safety requirements above are Noble-specific design decisions.
