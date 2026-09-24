<!-- Generated compatibility view. Edit .cairn/specs/resource-adapters/spec.md instead. -->
# Bounded resource adapter contract

Document: SPEC-R001  
Revision: 0.1.0-draft.5  
Depends on: SPEC-0001 and SPEC-S001 at 0.1.0-draft.5  
Status: Bounded synchronous implementation delivered; broader contracts and milestone acceptance remain separate

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

**RA-ASYNC-01.** This subset MUST reject an async call that retains a borrow. Full async support requires an explicit task/owner transfer contract, cancellation points, completion rules, and backpressure limits.

Later support must satisfy the retirement and pinning invariants here. It must also define live `stream<T>` and `future<T>` operations. Native async ABI availability alone does not supply those semantics.

### Async task ownership

The following states define local ownership obligations for the later async extension. They do not enable suspension in the synchronous subset. A task record can remain host-owned for a direct-style async import. Any exposed task handle denotes a move-only resource with a context and generation. It is not a capturable continuation or a new kernel type.

**RA-ASYNC-02.** Before async admission, the adapter MUST validate arguments, authority, ownership, and available task/buffer quotas. A failed preflight leaves owners with the caller and starts no protected work. Successful admission MUST atomically assign every transferred obligation to a task before the caller loses access.

A task in `Pending` owns the inputs and outstanding native pins. A task in `Ready` owns its terminal result and every resource in that result. Only explicit result delivery transfers those result obligations to the receiver. Task creation does not copy, serialize, or grant additional authority over the inputs.

**RA-ASYNC-03.** The host MUST serialize local completion, result delivery, cancellation, and retirement decisions for each task generation. These decisions MUST follow this transition table:

| Prior state | Event | Next state and owner |
|---|---|---|
| Caller owns inputs | Preflight failure | Caller retains inputs, with no protected work |
| Caller owns inputs | Admission commit | `Pending`, with task-owned inputs and pins |
| `Pending` | Matching normal completion | `Ready`, with task-owned typed result and resource disposition |
| `Ready` | Result delivery commit | `Delivered`, with result owners transferred once to the receiver |
| `Pending` or `Ready` | Cancellation commit | `Retiring`, with guest access revoked and all obligations retained by the host |
| `Pending` or `Ready` | Trap, deadline, budget exhaustion, or internal failure | `Retiring`, with the primary failure retained and obligations owned by the host |
| `Retiring` | Native completion or cleanup progress | Remain `Retiring` until all pins and retirement obligations finish |
| `Retiring` | All retirement obligations complete | `Retired`, with the primary cancellation/failure outcome and no live guest owners |
| `Delivered` or `Retired` | Duplicate delivery, cancellation, or completion | No ownership change and no second delivery or release |

The table abbreviates a complete state/event matrix under DX-PROTOCOL-03. It does not authorize wildcard fallback in a designated lifecycle implementation. Protected admission additionally follows H-AUTH-01 through H-AUTH-04. Witness consumption and input-resource transfer remain distinct obligations at the same admission boundary.

Each completion MUST account for every input obligation through a returned owner, explicit consumption, or host retirement. A domain error has the same obligation. An event not permitted in the current state MUST NOT change ownership or publish a result. Repeated cancellation during retirement returns the existing status without restarting cleanup.

A stale or wrong-context event MUST NOT change the current generation. The host still accounts for any native pin associated with the original event. A duplicate callback does not justify releasing that pin twice.

**RA-ASYNC-04.** If cancellation commits before delivery, the task MUST NOT later deliver a successful result or restore an input owner. Cancellation of a `Ready` task MUST retire resource-bearing results that the receiver never obtained. If delivery commits first, later cancellation through the consumed task handle MUST NOT revoke the delivered owners implicitly.

A cancellation acknowledgement means guest access was revoked. It does not mean native work stopped, cleanup finished, or an external action failed. Pending external outcomes remain unknown until the host observes a terminal outcome. Retry requires a separate operation contract. Local transition order does not establish distributed exactly-once behavior.

**RA-ASYNC-05.** The async profile MUST define task-handle disposition, await/result/error interfaces, cancellation observation, and cleanup ownership before implementation acceptance. It MUST enforce limits for tasks, buffered results, outstanding pins, and retirement work. A full queue MUST return an explicit bounded failure without a hidden ownership transfer. Admission MUST reserve capacity for a terminal outcome and bound buffered result storage. An oversized result MUST trigger retirement with a budget failure rather than an unaccounted allocation. Wakeup and callback paths MUST preserve the same state/generation checks.

The concrete ABI mapping, interruption latency, stream buffering policy, and scheduler remain open. [Worker conformance](WORKER-CONFORMANCE.md) supplies both race orders and late-callback rejection designs. These local rules do not select a second concurrency model.

## Conformance

[conformance/resource-cases.json](conformance/resource-cases.json) covers normal errors, escape attempts, reentrancy, suspension, cancellation, and late callbacks. The M5 runtime gate executes RA-CASE-01 through RA-CASE-09 in the bounded synchronous scope above; each case's state and evidence fields remain the authority for recorded acceptance. [Worker cases](conformance/worker-cases.json) add later async ownership races and admission limits; they are not delivered by these M5 controls. Full native async, borrowed exports, general lifetime support and universal host/backend refinement remain open.
