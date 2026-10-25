# MW2 worker cancellation slice design

## Context

MW2 depends on completed MW1 and M6. M6 delivered production kernel `async_tasks`, `resources` and `authority` tables and accepted WORKER-08's thirteen local ownership races in an independent peer; it explicitly did not complete WORKER-01, WORKER-09 or a worker service. The selected Worker transition remains pure: it returns inert `Action.Send(attempt,text)` data and emits zero host requests. Only the shell turns that data into protected native work. r[RA-ASYNC-02]

## Decisions

### Decision: independent async component peer is the evidence host

**Choice:** Keep the production compiler and `noble worker prepare`/`typed` transitions, but run the native shell in the independently built Wasmtime component-model-async peer under `verification/worker-send/`. Extend `worker-send.wit` with an explicit asynchronous finish/await operation returning `result<s64,string>`. Do not add async Wasmtime/Tokio features to the main `noble-cli` binary. r[RA-ASYNC-05]

**Rationale:** This is the accepted M6 trust boundary: production-generated component bytes run against an independent host that links the production kernel tables. It avoids silently changing the synchronous CLI's dependency and executor surface while still exercising real component suspension.

### Decision: ordering and observations are explicit

**Choice:** The integrated WORKER-01 order is preparation; Submit transition; authority/witness/admission/task/resource commit; native handoff; cancellation commit; kernel `Unknown` observation; post-commit Cancelled transition; cancellation acknowledgement (`Retiring`, one pin); late native success; kernel `OperationSuccess` observation and operation receipt with invocation still Cancelled; native-stop, resource completion, pin settlement, cleanup and finish (`Retired`, zero pins). No step restores a witness, input owner or deliverable result. r[RA-ASYNC-04]

The report derives every negative from tables and observations: worker host requests across both transitions, task/resource snapshots, authority counters and attempt outcome, owner returns, task deliveries, worker-event sequence and receipts. Remove constant `external_send_outcome` and `external_rollback_claimed` fields. r[H-OUTCOME-02]

### Decision: both race orders and WORKER-09 native rows

**Choice:** Add shell modes:

- `cancel-before-delivery`: the WORKER-01 path above; late completion retires and cannot deliver.
- `delivery-before-cancel`: await/finish delivers once, then cancellation is a duplicate and does not revoke the delivered owner.
- `suspend-awaiting-send`: component dispatch suspends in `finish-send`; the task remains Pending and owns its input until explicit retirement. This is WORKER-09 `permitted-suspension`.
- `deadline-with-native-outstanding`: a bounded driver deadline retires with `Failure::Deadline` while the resource/native pin remains host-owned; join and cleanup follow. This is WORKER-09 `host-deadline-native-work-outstanding`.

Wasmtime Store drop is never native-stop evidence. A host deadline is wall-clock and distinct from the typed source engine's deterministic fuel accounting. r[H-OUTCOME-01]

## Native-async-interfaces gate closure

The MW2 entry gate closes only when retained execution demonstrates all of the following: r[RA-ASYNC-05]

1. The WIT names a move-only task/future disposition and explicit await/result/error operation.
2. A failed preflight starts no native work and leaves every owner with the caller.
3. Successful admission atomically registers authority attempt, consumes the witness, transfers resource/input custody to one Pending task and records all quotas.
4. Suspension reports `suspended` with the Pending task as owner and no typed output.
5. Cancellation acknowledgement reports Retiring with the native pin retained and an externally unknown attempt.
6. Delivery-first transfers once; cancellation-first never later delivers or resurrects an owner.
7. Deadline while native work is pinned retains host retirement ownership until native stop and cleanup.
8. Late external success may create a receipt but never changes the cancelled invocation, retries, or emits a late worker event.
9. All seventy-five RA-ASYNC-03 state/event decisions remain covered by the M6 matrix, and the M6 receipt replays unchanged.

## Risks / Trade-offs

- The peer has a separate lock/vendor surface and every peer/WIT file is tool-selection pinned.
- WORKER-01 and WORKER-09 inputs/expected are check-specs pinned; code and gates must conform rather than edit cases.
- WORKER-09's case-wide 8-unit source interval applies to its typed-engine rows; the async driver has its separately declared 1,000-fuel quantum and deadline.
- `resources::Retirement` has no Deadline variant; deadline maps to `HostCleanup` and the task table retains `Failure::Deadline`.
- The datagram is an in-process synthetic native operation, not network delivery or exactly-once external semantics.
