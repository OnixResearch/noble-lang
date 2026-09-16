# Bounded resource adapter contract

<!-- cairn:purpose:start -->
## Purpose

This accepted specification records Noble draft contracts, not completed implementation.
Original requirement IDs, explanatory prose, examples, and open decisions remain authoritative.
Scenario clauses refer to unexecuted designs in the conformance ledger.

## Requirements

<!-- cairn:purpose:end -->


Document: SPEC-R001  
Revision: 0.1.0-draft.5  
Depends on: SPEC-0001 and SPEC-S001 at 0.1.0-draft.5  
Status: First synchronous subset specified; implementation and proofs open

## Scope

### Requirement: RA-SCOPE-01
r[RA-SCOPE-01]

**RA-SCOPE-01.** Implementations of `Component-Sync-Bootstrap` MUST support synchronous WIT imports/exports and owned resources. Borrow support is limited to adapter-local borrows for imported calls. This is not full `Component-Draft` conformance.

The subset excludes borrowed exports, borrowed values in guest storage, and suspension during a borrowed call. It adds no source borrow type or general lifetime-polymorphic program.


<!-- cairn:scenario-links:start -->
#### Scenario: RA-CASE-04 for RA-SCOPE-01

- GIVEN the `Component-Sync-Bootstrap` profile and every field of `input` in [RA-CASE-04](../../../specs/conformance/resource-cases.json)
- WHEN the `static` procedure for case `RA-CASE-04` runs against those inputs
- THEN the observations match every field of `expected` in case `RA-CASE-04`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: RA-SCOPE-02
r[RA-SCOPE-02]

**RA-SCOPE-02.** The compiler MUST reject unsupported borrow or async patterns before component emission. A boundary that unexpectedly suspends or reenters a busy owner must fail closed under the cleanup protocol.


<!-- cairn:scenario-links:start -->
#### Scenario: RA-CASE-02 for RA-SCOPE-02

- GIVEN the `Component-Sync-Bootstrap` profile and every field of `input` in [RA-CASE-02](../../../specs/conformance/resource-cases.json)
- WHEN the `static` procedure for case `RA-CASE-02` runs against those inputs
- THEN the observations match every field of `expected` in case `RA-CASE-02`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: RA-CASE-09 for RA-SCOPE-02

- GIVEN the `Component-Sync-Bootstrap` profile and every field of `input` in [RA-CASE-09](../../../specs/conformance/resource-cases.json)
- WHEN the `adapter` procedure for case `RA-CASE-09` runs against those inputs
- THEN the observations match every field of `expected` in case `RA-CASE-09`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

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

### Requirement: RA-TYPE-01
r[RA-TYPE-01]

**RA-TYPE-01.** The adapter MUST remove the owner from the active guest stack during the call. On either normal result alternative, it returns that same owner exactly once. It does not create another owning handle.

For multiple borrowed parameters, the adapter returns their owners in original parameter order before the declared results. Owned WIT parameters follow their separate transfer contract and are not implicitly returned.


<!-- cairn:scenario-links:start -->
#### Scenario: RA-CASE-01 for RA-TYPE-01

- GIVEN the `Component-Sync-Bootstrap` profile and every field of `input` in [RA-CASE-01](../../../specs/conformance/resource-cases.json)
- WHEN the `adapter` procedure for case `RA-CASE-01` runs against those inputs
- THEN the observations match every field of `expected` in case `RA-CASE-01`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: RA-TYPE-02
r[RA-TYPE-02]

**RA-TYPE-02.** The borrow token exists only inside the adapter. It MUST NOT satisfy `Data` or `Capture`, enter guest aggregates or recipes, cross another unapproved boundary, or survive its call scope.

An imported interface can mention a borrowed parameter without exposing that borrow token in Noble source. This is an explicit generated adapter, not a silent change to WIT ownership.


<!-- cairn:scenario-links:start -->
#### Scenario: RA-CASE-02 for RA-TYPE-02

- GIVEN the `Component-Sync-Bootstrap` profile and every field of `input` in [RA-CASE-02](../../../specs/conformance/resource-cases.json)
- WHEN the `static` procedure for case `RA-CASE-02` runs against those inputs
- THEN the observations match every field of `expected` in case `RA-CASE-02`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

## State and authority

The host tracks kind, owner context, generation, rights, call scope, and outstanding native access. A scalar index is never sufficient authority.

| State | Meaning |
|---|---|
| `Live` | Valid owner available to its guest context |
| `Busy(scope)` | Owner held by the adapter; one approved borrow scope is active |
| `Retiring(scope)` | Guest access revoked; native work still has an accounted pin |
| `Retired` | No guest access and no outstanding native access |

### Requirement: RA-STATE-01
r[RA-STATE-01]

**RA-STATE-01.** A `Live` to `Busy` transition MUST validate kind, context, generation, and rights before protected work. It consumes guest availability atomically with scope registration.


<!-- cairn:scenario-links:start -->
#### Scenario: RA-CASE-01 for RA-STATE-01

- GIVEN the `Component-Sync-Bootstrap` profile and every field of `input` in [RA-CASE-01](../../../specs/conformance/resource-cases.json)
- WHEN the `adapter` procedure for case `RA-CASE-01` runs against those inputs
- THEN the observations match every field of `expected` in case `RA-CASE-01`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: RA-STATE-02
r[RA-STATE-02]

**RA-STATE-02.** A busy owner MUST reject release, transfer, new borrows, and reentrant operations from guest entry points. Sequential invariants do not justify access during a callback.


<!-- cairn:scenario-links:start -->
#### Scenario: RA-CASE-03 for RA-STATE-02

- GIVEN the `Component-Sync-Bootstrap` profile and every field of `input` in [RA-CASE-03](../../../specs/conformance/resource-cases.json)
- WHEN the `adapter` procedure for case `RA-CASE-03` runs against those inputs
- THEN the observations match every field of `expected` in case `RA-CASE-03`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: RA-STATE-03
r[RA-STATE-03]

**RA-STATE-03.** A normal completion can return `Busy` to `Live` only for the matching live scope and generation. Duplicate, late, or wrong-context callbacks MUST NOT return an owner or revive retired state.


<!-- cairn:scenario-links:start -->
#### Scenario: RA-CASE-01 for RA-STATE-03

- GIVEN the `Component-Sync-Bootstrap` profile and every field of `input` in [RA-CASE-01](../../../specs/conformance/resource-cases.json)
- WHEN the `adapter` procedure for case `RA-CASE-01` runs against those inputs
- THEN the observations match every field of `expected` in case `RA-CASE-01`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: RA-CASE-06 for RA-STATE-03

- GIVEN the `Component-Sync-Bootstrap` profile and every field of `input` in [RA-CASE-06](../../../specs/conformance/resource-cases.json)
- WHEN the `adapter` procedure for case `RA-CASE-06` runs against those inputs
- THEN the observations match every field of `expected` in case `RA-CASE-06`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

## Completion and failure

| Event | Required transition |
|---|---|
| Validation fails before call admission | No protected work; no ownership transfer |
| Normal success or domain `Result` error | End borrow, release pin, return owner exactly once |
| Trap, cancellation, or unexpected suspension | Revoke guest access and begin retirement |
| Native completion after retirement request | Release the final pin without returning guest ownership |
| Repeated retirement or late callback | No second release and no resurrection |

### Requirement: RA-CLEAN-01
r[RA-CLEAN-01]

**RA-CLEAN-01.** Abnormal cleanup MUST not depend on guest continuation. The host retires all invocation-owned obligations under the execution profile. It does not return a guessed pre-call session stack.


<!-- cairn:scenario-links:start -->
#### Scenario: RA-CASE-05 for RA-CLEAN-01

- GIVEN the `Component-Sync-Bootstrap` profile and every field of `input` in [RA-CASE-05](../../../specs/conformance/resource-cases.json)
- WHEN the `adapter` procedure for case `RA-CASE-05` runs against those inputs
- THEN the observations match every field of `expected` in case `RA-CASE-05`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: RA-CASE-08 for RA-CLEAN-01

- GIVEN the `Component-Sync-Bootstrap` profile and every field of `input` in [RA-CASE-08](../../../specs/conformance/resource-cases.json)
- WHEN the `adapter` procedure for case `RA-CASE-08` runs against those inputs
- THEN the observations match every field of `expected` in case `RA-CASE-08`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: RA-CASE-09 for RA-CLEAN-01

- GIVEN the `Component-Sync-Bootstrap` profile and every field of `input` in [RA-CASE-09](../../../specs/conformance/resource-cases.json)
- WHEN the `adapter` procedure for case `RA-CASE-09` runs against those inputs
- THEN the observations match every field of `expected` in case `RA-CASE-09`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: RA-CLEAN-02
r[RA-CLEAN-02]

**RA-CLEAN-02.** Cancellation MUST revoke guest access immediately. Native storage MUST remain pinned until no callback or native operation can access it. Unstoppable work remains owned and charged as `Retiring`, not falsely reported as fully cleaned up.


<!-- cairn:scenario-links:start -->
#### Scenario: RA-CASE-05 for RA-CLEAN-02

- GIVEN the `Component-Sync-Bootstrap` profile and every field of `input` in [RA-CASE-05](../../../specs/conformance/resource-cases.json)
- WHEN the `adapter` procedure for case `RA-CASE-05` runs against those inputs
- THEN the observations match every field of `expected` in case `RA-CASE-05`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: RA-CASE-06 for RA-CLEAN-02

- GIVEN the `Component-Sync-Bootstrap` profile and every field of `input` in [RA-CASE-06](../../../specs/conformance/resource-cases.json)
- WHEN the `adapter` procedure for case `RA-CASE-06` runs against those inputs
- THEN the observations match every field of `expected` in case `RA-CASE-06`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: RA-CASE-09 for RA-CLEAN-02

- GIVEN the `Component-Sync-Bootstrap` profile and every field of `input` in [RA-CASE-09](../../../specs/conformance/resource-cases.json)
- WHEN the `adapter` procedure for case `RA-CASE-09` runs against those inputs
- THEN the observations match every field of `expected` in case `RA-CASE-09`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: RA-CLEAN-03
r[RA-CLEAN-03]

**RA-CLEAN-03.** Retirement and callback handling MUST be idempotent with respect to local release. The invariant includes hidden adapter state and native pins, not only the guest stack.


<!-- cairn:scenario-links:start -->
#### Scenario: RA-CASE-06 for RA-CLEAN-03

- GIVEN the `Component-Sync-Bootstrap` profile and every field of `input` in [RA-CASE-06](../../../specs/conformance/resource-cases.json)
- WHEN the `adapter` procedure for case `RA-CASE-06` runs against those inputs
- THEN the observations match every field of `expected` in case `RA-CASE-06`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

## Owned transfer

### Requirement: RA-OWN-01
r[RA-OWN-01]

**RA-OWN-01.** Before committing an owned WIT transfer, the adapter MUST finish argument validation and establish a cleanup owner for each obligation. A failed preflight leaves ownership with the caller.

After commit, the sender cannot use the transferred handle. A domain error does not reverse the transfer unless the declared result explicitly returns ownership. The receiver or host retains cleanup responsibility after a trap.

This contract describes local handle ownership. It does not prove physical exclusivity of an external object or exactly-once remote cleanup.


<!-- cairn:scenario-links:start -->
#### Scenario: RA-CASE-07 for RA-OWN-01

- GIVEN the `Component-Sync-Bootstrap` profile and every field of `input` in [RA-CASE-07](../../../specs/conformance/resource-cases.json)
- WHEN the `adapter` procedure for case `RA-CASE-07` runs against those inputs
- THEN the observations match every field of `expected` in case `RA-CASE-07`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: RA-CASE-08 for RA-OWN-01

- GIVEN the `Component-Sync-Bootstrap` profile and every field of `input` in [RA-CASE-08](../../../specs/conformance/resource-cases.json)
- WHEN the `adapter` procedure for case `RA-CASE-08` runs against those inputs
- THEN the observations match every field of `expected` in case `RA-CASE-08`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: RA-OWN-02
r[RA-OWN-02]

**RA-OWN-02.** Memory reclamation MUST remain separate from resource retirement. GC reachability MUST NOT make a resource capturable or remove its release obligation. Guest operations MUST NOT convert a live owner into freely duplicable unmanaged authority.

A WIT borrow does not imply a Rust shared-reference contract or an immutable external object. No new guest `Unmanaged`, `disown`, or `adopt` regime is selected. Ordinary user-defined names do not bypass these semantic restrictions.


<!-- cairn:scenario-links:start -->
#### Scenario: RA-CASE-05 for RA-OWN-02

- GIVEN the `Component-Sync-Bootstrap` profile and every field of `input` in [RA-CASE-05](../../../specs/conformance/resource-cases.json)
- WHEN the `adapter` procedure for case `RA-CASE-05` runs against those inputs
- THEN the observations match every field of `expected` in case `RA-CASE-05`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: RA-CLEAN-04
r[RA-CLEAN-04]

**RA-CLEAN-04.** Any later scoped cleanup convenience MUST specify cleanup order, declared effects, and all ownership outcomes. It MUST preserve host retirement on abnormal exit and accounting for outstanding native pins. Shielding cancellation MUST NOT imply eventual host completion.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

## Async extension gate

### Requirement: RA-ASYNC-01
r[RA-ASYNC-01]

**RA-ASYNC-01.** This subset MUST reject an async call that retains a borrow. Full async support requires an explicit task/owner transfer contract, cancellation points, completion rules, and backpressure limits.

Later support must satisfy the retirement and pinning invariants here. It must also define live `stream<T>` and `future<T>` operations. Native async ABI availability alone does not supply those semantics.


<!-- cairn:scenario-links:start -->
#### Scenario: RA-CASE-04 for RA-ASYNC-01

- GIVEN the `Component-Sync-Bootstrap` profile and every field of `input` in [RA-CASE-04](../../../specs/conformance/resource-cases.json)
- WHEN the `static` procedure for case `RA-CASE-04` runs against those inputs
- THEN the observations match every field of `expected` in case `RA-CASE-04`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Async task ownership

The following states define local ownership obligations for the later async extension. They do not enable suspension in the synchronous subset. A task record can remain host-owned for a direct-style async import. Any exposed task handle denotes a move-only resource with a context and generation. It is not a capturable continuation or a new kernel type.

### Requirement: RA-ASYNC-02
r[RA-ASYNC-02]

**RA-ASYNC-02.** Before async admission, the adapter MUST validate arguments, authority, ownership, and available task/buffer quotas. A failed preflight leaves owners with the caller and starts no protected work. Successful admission MUST atomically assign every transferred obligation to a task before the caller loses access.

A task in `Pending` owns the inputs and outstanding native pins. A task in `Ready` owns its terminal result and every resource in that result. Only explicit result delivery transfers those result obligations to the receiver. Task creation does not copy, serialize, or grant additional authority over the inputs.


<!-- cairn:scenario-links:start -->
#### Scenario: WORKER-01 for RA-ASYNC-02

- GIVEN the `Worker-Design` profile and every field of `input` in [WORKER-01](../../../specs/conformance/worker-cases.json)
- WHEN the `runtime` procedure for case `WORKER-01` runs against those inputs
- THEN the observations match every field of `expected` in case `WORKER-01`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: WORKER-08 for RA-ASYNC-02

- GIVEN the `Worker-Design` profile and every field of `input` in [WORKER-08](../../../specs/conformance/worker-cases.json)
- WHEN the `adapter` procedure for case `WORKER-08` runs against those inputs
- THEN the observations match every field of `expected` in case `WORKER-08`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: RA-ASYNC-03
r[RA-ASYNC-03]

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


<!-- cairn:scenario-links:start -->
#### Scenario: WORKER-01 for RA-ASYNC-03

- GIVEN the `Worker-Design` profile and every field of `input` in [WORKER-01](../../../specs/conformance/worker-cases.json)
- WHEN the `runtime` procedure for case `WORKER-01` runs against those inputs
- THEN the observations match every field of `expected` in case `WORKER-01`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: WORKER-08 for RA-ASYNC-03

- GIVEN the `Worker-Design` profile and every field of `input` in [WORKER-08](../../../specs/conformance/worker-cases.json)
- WHEN the `adapter` procedure for case `WORKER-08` runs against those inputs
- THEN the observations match every field of `expected` in case `WORKER-08`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: RA-ASYNC-04
r[RA-ASYNC-04]

**RA-ASYNC-04.** If cancellation commits before delivery, the task MUST NOT later deliver a successful result or restore an input owner. Cancellation of a `Ready` task MUST retire resource-bearing results that the receiver never obtained. If delivery commits first, later cancellation through the consumed task handle MUST NOT revoke the delivered owners implicitly.

A cancellation acknowledgement means guest access was revoked. It does not mean native work stopped, cleanup finished, or an external action failed. Pending external outcomes remain unknown until the host observes a terminal outcome. Retry requires a separate operation contract. Local transition order does not establish distributed exactly-once behavior.


<!-- cairn:scenario-links:start -->
#### Scenario: WORKER-01 for RA-ASYNC-04

- GIVEN the `Worker-Design` profile and every field of `input` in [WORKER-01](../../../specs/conformance/worker-cases.json)
- WHEN the `runtime` procedure for case `WORKER-01` runs against those inputs
- THEN the observations match every field of `expected` in case `WORKER-01`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: WORKER-08 for RA-ASYNC-04

- GIVEN the `Worker-Design` profile and every field of `input` in [WORKER-08](../../../specs/conformance/worker-cases.json)
- WHEN the `adapter` procedure for case `WORKER-08` runs against those inputs
- THEN the observations match every field of `expected` in case `WORKER-08`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: OCTET-04 for RA-ASYNC-04

- GIVEN the `Octet-Adoption-Design` profile and every field of `input` in [OCTET-04](../../../specs/conformance/octet-adoption-cases.json)
- WHEN the `adapter` procedure for case `OCTET-04` runs against those inputs
- THEN the observations match every field of `expected` in case `OCTET-04`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: RA-ASYNC-05
r[RA-ASYNC-05]

**RA-ASYNC-05.** The async profile MUST define task-handle disposition, await/result/error interfaces, cancellation observation, and cleanup ownership before implementation acceptance. It MUST enforce limits for tasks, buffered results, outstanding pins, and retirement work. A full queue MUST return an explicit bounded failure without a hidden ownership transfer. Admission MUST reserve capacity for a terminal outcome and bound buffered result storage. An oversized result MUST trigger retirement with a budget failure rather than an unaccounted allocation. Wakeup and callback paths MUST preserve the same state/generation checks.

The concrete ABI mapping, interruption latency, stream buffering policy, and scheduler remain open. [Worker conformance](../../../specs/WORKER-CONFORMANCE.md) supplies both race orders and late-callback rejection designs. These local rules do not select a second concurrency model.


<!-- cairn:scenario-links:start -->
#### Scenario: WORKER-08 for RA-ASYNC-05

- GIVEN the `Worker-Design` profile and every field of `input` in [WORKER-08](../../../specs/conformance/worker-cases.json)
- WHEN the `adapter` procedure for case `WORKER-08` runs against those inputs
- THEN the observations match every field of `expected` in case `WORKER-08`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: WORKER-09 for RA-ASYNC-05

- GIVEN the `Worker-Design` profile and every field of `input` in [WORKER-09](../../../specs/conformance/worker-cases.json)
- WHEN the `runtime` procedure for case `WORKER-09` runs against those inputs
- THEN the observations match every field of `expected` in case `WORKER-09`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

## Conformance

[conformance/resource-cases.json](../../../specs/conformance/resource-cases.json) covers normal errors, escape attempts, reentrancy, suspension, cancellation, and late callbacks. [Worker cases](../../../specs/conformance/worker-cases.json) add async ownership races and admission limits. These are unexecuted harness designs.
