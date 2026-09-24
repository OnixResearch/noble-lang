<!-- Generated compatibility view. Edit .cairn/specs/resource-adapters/spec.md instead. -->
# Bounded resource adapter contract

Document: SPEC-R001  
Revision: 0.1.0-draft.5  
Depends on: SPEC-0001 and SPEC-S001 at 0.1.0-draft.5  
Status: Bounded synchronous M5 and selected native-async M6 implementations accepted; broader resource contracts remain open

## Scope

**RA-SCOPE-01.** Implementations of `Component-Sync-Bootstrap` MUST support synchronous WIT imports/exports and owned resources. Borrow support is limited to adapter-local borrows for imported calls. This is not full `Component-Draft` conformance.

The subset excludes borrowed exports, borrowed values in guest storage, and suspension during a borrowed call. It adds no source borrow type or general lifetime-polymorphic program.

The delivered M5 slice uses the pinned [`noble-test:sync/bootstrap@1.0.0` world](../crates/noble-wasm/wit/bootstrap.wit). Owners are move-only and noncapturable; imported borrows use adapter-local tokens, not guest values or serializable authority. Normal success and domain errors return the borrowed owner once. Owned transfer instead commits only after preflight and does not implicitly return ownership on a domain error.

The [M5 runtime gate](../verification/m5/gate.mjs) exercises the production resource table and an independent Rust/Wasmtime 40.0.2 component peer. Its bounded local cancellation and unexpected-suspension control revokes guest access, retains the native pin through actual guest GC, and handles completion, duplicate callbacks and repeated retirement without resurrection or a second release. This control is not native async execution. The peer independently links and converts component values but reuses Noble's resource/authority decision libraries for host policy.

The [strict resource proof lane](../proofs/m5/M5Resources.lean) relates the extracted Rust transition and checked table-decision functions to their logical transitions, with separate owner-return, busy-owner, cancellation-pin and late-completion properties. The [extraction audit](../verification/m5/extraction.mjs) binds those seven theorem roots to actual Rust definitions; broader authority and component body/dependency coverage is not refinement. Native release, authenticated host facts and callbacks, and engine/ABI correctness remain external assumptions. Fresh complete extraction/check receipts, not these descriptions or a diagnostic proof audit, determine milestone acceptance.

**RA-SCOPE-02.** The compiler MUST reject unsupported borrow or async patterns before component emission. A boundary that unexpectedly suspends or reenters a busy owner must fail closed under the cleanup protocol.

## Typed import adapter

Consider an imported WIT resource method:

```wit
package noble-test:counter@1.0.0;
interface counters {
  resource counter {
    read: func() -> result<s64, string>;
  }
}
```

Its receiver is borrowed at the component boundary. The generated Noble adapter exposes a move-threaded owner:

```text
counter.read : S Counter
               -- S Counter Result<I64,Text> ! {CounterReadOpId}
```

`CounterReadOpId` names the exact versioned operation. `Counter` identifies the selected resource contract and owning instance context.

**RA-TYPE-01.** The adapter MUST remove the owner from the active guest stack during the call. On either normal result alternative, it returns that same owner exactly once. It does not create another owning handle.

For multiple borrowed parameters, the adapter returns their owners in original parameter order before the declared results. Owned WIT parameters follow their separate transfer contract and are not implicitly returned.

**RA-TYPE-02.** The borrow token exists only inside the adapter. It MUST NOT satisfy `Data` or `Capture`, enter guest aggregates or recipes, cross another unapproved boundary, or survive its call scope.

An imported interface can mention a borrowed parameter without exposing that borrow token in Noble source. This is an explicit generated adapter, not a silent change to WIT ownership.

## State and authority

The host tracks kind, owner context, generation, rights, call scope, and outstanding native access. A scalar index is never sufficient authority.

| State | Meaning |
|---|---|
| `Live` | Valid owner available to its guest context |
| `Busy(scope)` | Owner held by the adapter; one approved borrow scope is active |
| `Retiring(scope)` | Guest access revoked; native work still has an accounted pin |
| `Retired` | No guest access and no outstanding native access |

**RA-STATE-01.** A `Live` to `Busy` transition MUST validate kind, context, generation, and rights before protected work. It consumes guest availability atomically with scope registration.

**RA-STATE-02.** A busy owner MUST reject release, transfer, new borrows, and reentrant operations from guest entry points. Sequential invariants do not justify access during a callback.

**RA-STATE-03.** A normal completion can return `Busy` to `Live` only for the matching live scope and generation. Duplicate, late, or wrong-context callbacks MUST NOT return an owner or revive retired state.

## Completion and failure

| Event | Required transition |
|---|---|
| Validation fails before call admission | No protected work; no ownership transfer |
| Normal success or domain `Result` error | End borrow, release pin, return owner exactly once |
| Trap, cancellation, or unexpected suspension | Revoke guest access and begin retirement |
| Native completion after retirement request | Release the final pin without returning guest ownership |
| Repeated retirement or late callback | No second release and no resurrection |

**RA-CLEAN-01.** Abnormal cleanup MUST not depend on guest continuation. The host retires all invocation-owned obligations under the execution profile. It does not return a guessed pre-call session stack.

**RA-CLEAN-02.** Cancellation MUST revoke guest access immediately. Native storage MUST remain pinned until no callback or native operation can access it. Unstoppable work remains owned and charged as `Retiring`, not falsely reported as fully cleaned up.

**RA-CLEAN-03.** Retirement and callback handling MUST be idempotent with respect to local release. The invariant includes hidden adapter state and native pins, not only the guest stack.

## Owned transfer

**RA-OWN-01.** Before committing an owned WIT transfer, the adapter MUST finish argument validation and establish a cleanup owner for each obligation. A failed preflight leaves ownership with the caller.

After commit, the sender cannot use the transferred handle. A domain error does not reverse the transfer unless the declared result explicitly returns ownership. The receiver or host retains cleanup responsibility after a trap.

This contract describes local handle ownership. It does not prove physical exclusivity of an external object or exactly-once remote cleanup.

**RA-OWN-02.** Memory reclamation MUST remain separate from resource retirement. GC reachability MUST NOT make a resource capturable or remove its release obligation. Guest operations MUST NOT convert a live owner into freely duplicable unmanaged authority.

A WIT borrow does not imply a Rust shared-reference contract or an immutable external object. No new guest `Unmanaged`, `disown`, or `adopt` regime is selected. Ordinary user-defined names do not bypass these semantic restrictions.

**RA-CLEAN-04.** Any later scoped cleanup convenience MUST specify cleanup order, declared effects, and all ownership outcomes. It MUST preserve host retirement on abnormal exit and accounting for outstanding native pins. Shielding cancellation MUST NOT imply eventual host completion.

## Async extension gate

The selected `Component-Async-Bootstrap` implementation uses the `noble-test:async-boundary/bootstrap@1.0.0` world and Wasmtime 40.0.2 native async support, with the toolchain, ABI and driver selections recorded in [M6 pins](../verification/m6/pins.json). This bounded selection extends, rather than relaxes, the resource-lowering obligation in [WI-RES-04](WIT-WASI.md#requirement-wi-res-04). It does not enable suspension in `Component-Sync-Bootstrap` or imply full `Component-Draft` support.

**RA-ASYNC-01.** The synchronous subset MUST reject an async call that retains a borrow. Full async support requires an explicit task/owner transfer contract, cancellation points, completion rules, and backpressure limits. The selected `Component-Async-Bootstrap` profile MUST also reject borrow-retaining async calls before component emission; native suspension does not extend an adapter-local borrow's lifetime.

Every async extension MUST satisfy the retirement and pinning invariants here and define its live `stream<T>` and `future<T>` operations. Native async ABI availability alone does not supply those semantics. The selected bounded implementation supplies task admission, completion, delivery, cancellation and retirement under RA-ASYNC-02 through RA-ASYNC-05, including live `future<s64>`, `future<result<s64,string>>` and `stream<u8>` values. Those live values remain move-only and ineligible for `Data`, `Capture`, generic duplication, generic discard or serialization. Explicit future cancellation and stream read-end closure have distinct contracts: closing a stream reader does not by itself cancel the invocation or establish the producer's terminal outcome.

#### Scenario: Suspension does not extend an imported borrow

- GIVEN an async call that retains an adapter-local WIT borrow in the selected `Component-Async-Bootstrap` profile
- WHEN the compiler checks the boundary before component emission
- THEN it rejects that call without extending the borrow lifetime, transferring the owner or starting protected work

#### Scenario: Explicit live-value disposal keeps distinct terminal obligations

- GIVEN an admitted live future or stream with an outstanding native producer
- WHEN the caller explicitly cancels the future or closes the stream read end
- THEN future cancellation revokes that task's guest access, while stream reader closure alone neither cancels the invocation nor supplies a terminal producer outcome, and native obligations remain accounted until settlement

**RA-ASYNC-02.** Before async admission, the adapter MUST validate arguments, authority, ownership, and available task/buffer quotas. A failed preflight leaves owners with the caller and starts no protected work. Successful admission MUST atomically assign every transferred obligation to a task before the caller loses access.

A task in `Pending` owns the inputs and outstanding native pins. A task in `Ready` owns its terminal result and every resource in that result. Only explicit result delivery transfers those result obligations to the receiver. Task creation does not copy, serialize, or grant additional authority over the inputs.

The selected admission boundary MUST preflight task, terminal-result, payload, parked-payload, native-pin, wakeup, retirement-work and native-job capacity, and reserve the required bounded payload backing before consuming authorization or transferring caller custody. The production `Table::prepare` checks capacity and generation while holding exclusive access to the table; it does not itself consume an owner, reserve retained capacity, advance the generation or start native work. Abandoning preparation leaves the table unchanged. `Admission::commit` records the reservation and task custody after the independent preflights.

Protected admission additionally follows H-AUTH-01 through H-AUTH-04. One-shot witness consumption and attempt registration, resource transfer, and task admission MUST remain distinct obligations at the same serialized boundary, before protected native work starts. Task admission is not authorization. After commit, a transfer or execution-start failure MUST retain task custody for retirement, not reconstruct caller ownership or restore a consumed witness.

The retained table's opaque `Task` token is move-only; successful result delivery or cancellation consumes it, and rejection returns the supplied token unchanged. Dropping that token does not retire its task. Descriptive kernel `Handle`, `Callback`, `NativeId`, snapshot and decision values are not independently authenticated capabilities, native-access permits, authorization witnesses or receipts. The embedding MUST authenticate provenance, retain actual payload storage and serialize these local decisions with its authority and resource tables.

#### Scenario: Admission refusal preserves caller custody and authorization

- GIVEN a valid caller owner and unconsumed one-shot witness, with one required admission capacity unavailable
- WHEN task, storage and authority preflights run before the serialized admission commit
- THEN the call starts no protected operation, consumes neither caller custody nor the witness, and leaves retained task capacity and generation unchanged

#### Scenario: Post-commit failure is retirement rather than rollback

- GIVEN successful independent preflights and a committed task reservation, witness consumption and task-owned input
- WHEN a later resource-transfer or execution-start step fails at that boundary
- THEN the host retains the input for task retirement, preserves the consumed witness state and does not reconstruct caller ownership or authorize a retry

**RA-ASYNC-03.** The host MUST serialize local completion, result delivery, cancellation, and retirement decisions for each task generation. The selected protocol MUST account for all five states and all fifteen event constructors under DX-PROTOCOL-03:

| Event | `Pending` | `Ready` | `Delivered` | `Retiring` | `Retired` |
|---|---|---|---|---|---|
| `Inspect` | Inspect | Inspect | Inspect | Inspect | Inspect |
| `CompleteSuccess` | Complete | Duplicate | Duplicate | Complete for retirement | Duplicate |
| `CompleteDomainError` | Complete | Duplicate | Duplicate | Complete for retirement | Duplicate |
| `Deliver` | Reject `Pending` | Deliver | Reject `Delivered` | Reject `Retiring` | Reject `Retired` |
| `Cancel` | Retire `Cancelled` | Retire `Cancelled` | Duplicate | Duplicate | Duplicate |
| `Trap` | Retire `Trap` | Retire `Trap` | Duplicate | Duplicate | Duplicate |
| `Deadline` | Retire `Deadline` | Retire `Deadline` | Duplicate | Duplicate | Duplicate |
| `Budget` | Retire `Budget` | Retire `Budget` | Duplicate | Duplicate | Duplicate |
| `InternalFailure` | Retire `Internal` | Retire `Internal` | Duplicate | Duplicate | Duplicate |
| `NativeStopped` | Observe stop | Observe stop | Observe stop | Observe stop | Duplicate |
| `SettlePins` | Settle pins | Settle pins | Settle pins | Settle pins | Reject `Retired` |
| `Cleanup` | Reject `Pending` | Clean eligible obligations | Clean eligible obligations | Clean eligible obligations | Reject `Retired` |
| `Wake` | Queue/coalesce/budget | Queue/coalesce/budget | Reject `Delivered` | Reject `Retiring` | Reject `Retired` |
| `TakeWake` | Take queued wake | Take queued wake | Reject `Delivered` | Reject `Retiring` | Reject `Retired` |
| `Finish` | Reject `Pending` | Reject `Ready` | Finalize `Delivered` | Finalize `Retired` | Duplicate |

These are all 75 constructor pairs, not a wildcard fallback or permission to omit payload guards. Inspect is read-only. Duplicate leaves the record unchanged and carries no second acquisition, delivery, cleanup or release. A rejected pair MUST preserve retained ownership and publish no result. Allowed cells remain subject to the checked identities and completion data below, the settlement guards in RA-ASYNC-04, and the wakeup limits in RA-ASYNC-05.

Before classification, the implementation MUST validate the retained record and match table namespace, slot, owner context and generation. Native completion, stop, pin-settlement and wake events MUST also match the independent native-operation identity. The host MUST NOT reuse a table namespace while an old callback can exist; exhausted generations fail rather than wrap. A stale, foreign-context or wrong-native event MUST NOT change the current generation or release its pins. The embedding still owns any obligation associated with the original event.

Each first accepted completion MUST account for every admitted input through disjoint returned, consumed and retired sets whose union is exactly the admitted input set. Newly produced result owners are accounted separately and MUST fit the reserved result-owner set. Success and domain error have the same obligation. A fitting normal completion changes `Pending` to `Ready`, not to `Delivered`; both returned inputs and newly produced result owners remain task-owned. Invalid disposition leaves the task unchanged. Oversized bytes follow RA-ASYNC-05 rather than being admitted.

A first accepted late completion in `Retiring` remains there: returned inputs and produced results enter host retirement, while explicit consumption can discharge the corresponding input-retirement obligation. It MUST NOT deliver owners or replace the primary failure. Once completion is closed, another matching completion is a duplicate, not another disposition or pin release. Delivery alone changes `Ready` to `Delivered` and transfers result obligations once; host-owned pin and cleanup debt can remain afterward. Repeated cancellation during retirement leaves the existing status and cleanup intact.

#### Scenario: Invalid events preserve the current generation

- GIVEN the retained production task table and its five-state, fifteen-event schema
- WHEN an invalid state/event pair or a callback with a foreign table, context, generation or native-operation identity is submitted
- THEN the declared rejection preserves the current task's ownership and pins, publishes no result and leaves any original-event obligation with its embedding owner

#### Scenario: A ready result is owned before it is delivered

- GIVEN a pending task with admitted inputs and separately reserved result-owner capacity
- WHEN a matching success or domain-error completion supplies a disjoint complete input disposition and fitting result metadata
- THEN the task becomes `Ready` with returned inputs and produced results still in task custody, native stop remains unestablished, and a duplicate completion neither reacquires owners nor releases a pin

**RA-ASYNC-04.** If cancellation commits before delivery, the task MUST NOT later deliver a successful result or restore an input owner. Cancellation of a `Ready` task MUST retire resource-bearing results that the receiver never obtained. If delivery commits first, later cancellation through the consumed task handle MUST NOT revoke the delivered owners implicitly.

A cancellation acknowledgement means guest access was revoked. It does not mean native work stopped, cleanup finished, or an external action failed. Pending external outcomes remain unknown until the host observes a terminal outcome. Retry requires a separate operation contract. Local transition order does not establish distributed exactly-once behavior.

Cancellation, trap, deadline, budget exhaustion and internal failure MUST retain their primary outcome through late completion and cleanup. An authenticated late operation-success or operation-failure receipt may describe that operation under H-RECEIPT-01, but MUST NOT promote a cancelled or failed invocation to success, deliver its abandoned owners, restore a consumed witness or grant retry rights. Local task outcomes and identity checks do not establish receipt provenance; observation and receipt validation remain separate host obligations.

Native completion, native stop, pin settlement, cleanup acknowledgement and final reservation release MUST remain separate decisions. Completion alone releases no pin. `NativeStopped` records the host's observation that native access ended; it does not itself settle pins or release reservations. `SettlePins` requires that observation and a nonempty subset of the remaining pin identities. `Cleanup` requires native stop, zero outstanding pins and a nonempty subset of eligible outstanding obligations; it acknowledges cleanup already performed by the shell. A `Ready` task's returned inputs, produced results and result buffer MUST NOT be cleaned up as undelivered waste while they are still available for delivery. Cancellation first places them under retirement.

`Finish` MUST refuse while native work, pins or any task-owned input, result or buffer obligation remains. Only a settled `Delivered` or `Retiring` task can release its admission reservation: the former remains `Delivered`, and the latter becomes `Retired`. Repeated finalization releases nothing again. Slot reuse is permitted only after finalization, with a new checked generation. Pure retirement or an accounting delta is not a native destructor call or proof of physical release; the embedding MUST retain storage and charges until actual access and cleanup obligations end.

The selected native peer serializes task, resource and authority decisions under its host lock, isolates an invocation in its Wasmtime `Store`, and retains host state and native jobs independently of that Store. Abnormal invocation exit revokes guest access before Store destruction; native pins survive destruction until actual native accesses end and settlement is acknowledged. Wasmtime 40.0.2 supplies no per-task cancellation API for this selection. Dropping a Store, a task token or guest GC reachability MUST NOT be treated as native-stop evidence. Authentic callbacks, serialization, native execution and physical cleanup remain trusted embedding obligations.

#### Scenario: Cancellation and delivery preserve their committed order

- GIVEN a `Ready` task whose terminal result contains an owner
- WHEN cancellation commits before delivery, or delivery commits before cancellation
- THEN the first order retires the undelivered owner without subsequent guest delivery, while the second transfers it once to the receiver without implicit revocation by late task cancellation

#### Scenario: Late operation evidence does not revive an invocation

- GIVEN a cancelled or failed invocation with a consumed one-shot witness and a still-pinned native operation
- WHEN the host admits an authenticated late operation-success observation and constructs its operation receipt
- THEN the operation outcome may be reported without restoring invocation success, abandoned owners, the witness or retry rights, and the primary invocation failure remains unchanged

#### Scenario: Native debt survives guest teardown

- GIVEN a retiring task with outstanding native access after invocation Store destruction
- WHEN cleanup or finalization is attempted before native stop and pin settlement, then retried after actual native access and named cleanup obligations end
- THEN the early attempt releases no reservation, while the settled task can become `Retired` and release its reservation once without relying on guest continuation or treating a pure decision as physical cleanup

**RA-ASYNC-05.** The async profile MUST define task-handle disposition, await/result/error interfaces, cancellation observation, and cleanup ownership before implementation acceptance. It MUST enforce limits for tasks, buffered results, outstanding pins, and retirement work. A full queue MUST return an explicit bounded failure without a hidden ownership transfer. Admission MUST reserve capacity for a terminal outcome and bound buffered result storage. An oversized result MUST trigger retirement with a budget failure rather than an unaccounted allocation. Wakeup and callback paths MUST preserve the same state/generation checks.

The selected peer's ordinary task-table limits are eight tasks, eight terminal-result reservations, 4,096 aggregate reserved payload bytes, eight parked payloads, eight native pins, 64 wakeup credits, 128 retirement-work units and 128 admitted generations. An ordinary request reserves one native pin and four wakeups. Every admitted task reserves a terminal outcome even when its result has no bytes. Retirement-work reservation accounts for declared input and result owners, each nonempty input/result/parked buffer, and terminal cleanup. The complete admission reservation remains charged through `Ready`, `Delivered` and `Retiring` until `Finish`; logical cancellation or early native stop does not make that capacity reusable.

For each selected live future or stream, admission reserves 128 result bytes and 256 parked bytes, split into 128 local and 128 external native-worker/channel payload bytes. The driver limits live values to eight and retained native jobs to 32. The stream producer uses a one-byte engine buffer; the consumer has a 64-byte bounded payload, with terminal success/domain error observed separately from read-end closure. These are payload-storage reservations, not whole-process heap accounting: engine allocation, task metadata and bounded worker thread stacks are not thereby charged as payload. The selected Wasmtime linear-memory limit is 4,194,304 bytes, and driver event observation is bounded to 256 entries.

The host MUST check completion metadata before retaining result bytes. A result exceeding its byte reservation is not admitted; its bounded owner obligations enter retirement and its rejected bytes remain the shell's rejection obligation. A pre-existing cancellation or failure remains primary. Wakeups have at most one queued notification per task: repeated wake events coalesce, a newly queued wake consumes one reserved credit, and `TakeWake` without a queued notification rejects. Exhaustion retires the task with `Budget` without starting new native work.

The selected driver uses a single-thread cooperative engine with 100,000 guest fuel and a 1,000-fuel yield quantum, fuel exhaustion and epoch interruption enabled, and explicitly awaits task exit before successful export observation. Blocking native work runs on bounded external workers, not on that cooperative thread. Separate progress controls use their declared probe budgets to exercise continuously runnable fuel exhaustion, host-driven epoch interruption and a driver deadline while native work remains pinned. An epoch deadline depends on host epoch advancement, not on an autonomous wall clock. These mechanisms and measured probes do not promise a universal interruption latency, scheduler fairness, eventual native completion or eventual cleanup. Wider scheduler and interruption contracts remain open; this selection is not a second concurrency model or full worker service.

#### Scenario: Bounded result storage cannot escape retirement accounting

- GIVEN an admitted task with a fixed result-byte reservation and reserved terminal and retirement capacity
- WHEN its first accepted completion reports an oversized payload
- THEN the excess bytes are not retained, bounded result owners enter retirement with a budget failure unless an earlier failure is already primary, and admission capacity stays charged until final settlement

#### Scenario: Wakeups coalesce without unbounded callback work

- GIVEN a live task with one queued wake and a finite remaining wakeup reservation
- WHEN duplicate wakes arrive, the queued wake is taken, and subsequent newly queued wakes exhaust the reservation
- THEN duplicates do not consume extra credits, taking an absent wake rejects, and exhaustion retires the task with `Budget` without new native work or ownership resurrection

#### Scenario: Guest interruption is distinct from native completion

- GIVEN the cooperative native driver, bounded external blocking work and the declared fuel, epoch or deadline progress-control budget
- WHEN runnable guest work exhausts fuel or receives a host-driven epoch interrupt, or an invocation deadline expires while native work remains active
- THEN guest access is revoked independently of native completion, retained pins remain charged until actual stop and settlement, and measured progress is not promoted to a wall-clock, fairness or eventual-cleanup guarantee
