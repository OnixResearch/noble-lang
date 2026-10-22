# WORKER-10 bounded send observation design

## Context

WORKER-10 fixes the scenario: invocation `invocation-1` of a Program declaring `test.send` makes three requests; attempt-1 is admitted and completed, attempt-2 is denied, attempt-3 is admitted with an externally unknown outcome, and cancellation is the terminal event. Its three variants differ in observation and diagnostic output bounds and share one expected record. The MW1 roadmap places WORKER-10 in MW1 and its requirements contain no native-async obligation. The selected profile therefore models attempt-3 as a synchronous hand-off whose acknowledgement is lost, not as native work in flight. A late completion racing cancellation remains WORKER-01's MW2 scenario. r[H-OUTCOME-02]

## Decisions

### Decision: Exact bound `test.send` source operation

**Choice:** Host selection supplies `BoundOperation { module_name: "Sender", module_version: 1, operation: "test.send", adapter_identity: "worker-test-send@1.0.0", adapter_slot: 5, input: [Text], output: [], effects: [TEST_SEND] }` with `TEST_SEND = EffId(7)`. The fixture is:

```noble
module Sender@1 [ require send Text -- ! test.send export plus def plus [ "w10-a" send "w10-b" send "w10-c" send 1 + ] ]
```

Kernel registration refuses any other identity, input, output, effect or a used slot; Wasm environment replay re-registers the same row, and the Wasm effect mask gains bit 7. The Program atom tag is 39 because 35 to 38 are reflection event codes. r[H-OUTCOME-02]

**Rationale:** Ids 2 to 6 are taken by `test.clock`, live effects and `test.code-export`, and a WIT world assigns its imports ids from 2. A distinct effect keeps `test.send` from aliasing an existing grant or WIT import. r[H-OUTCOME-02]

### Decision: Outcome-free guest import

**Choice:** `$op_send_bound` pops the Text, sets failure before calling `noble/test_send_bound(offset, length, slot)` and clears it only on status 0. The host returns 0 for every recorded request, whether denied, completed or left unknown, and a nonzero status only when its terminal event ends the invocation or a request is malformed. r[H-OUTCOME-02]

**Rationale:** The guest continues past a denial without learning it, so a guest string can neither create nor hide a host observation, and a returned status cannot be read as completed external work. r[H-OUTCOME-02]

### Decision: Kernel authority host record

**Choice:** Issuing the installed cell opens one `noble_kernel::authority::Authority` with owner context `{instance: Program session, invocation: 1, generation: 1}`. Each request is numbered by the host (attempt-N is the N-th request) and planned with its exact attempt number and payload. The host grant table holds exact plans for attempt-1 `w10-a` and attempt-3 `w10-c`. Authorization, admission, `start_execution`, the hand-off to an in-process synthetic peer and `observe_from_trusted_host` follow. The peer's acknowledgement script delivers attempt-1's and loses attempt-3's. While attempt-3 is outstanding the host cancels the authority invocation, observes `Unknown` and cancels the Worker task owner outside the policy lock. The primary outcome is `cancelled` only when the kernel invocation is cancelled, the task owner acknowledged the cancel and the guest returned no result. r[H-OUTCOME-02]

**Rationale:** Denial comes from the host's own grant table and admission from kernel commit, never from guest data. Completed and unknown come from kernel-retained observations, so a plan or admission cannot be counted as completion. Cancellation keeps every attempt record. r[H-OUTCOME-02]

### Decision: Bounded rows, counters and diagnostic

**Choice:** Observation rows `{sequence, invocation, attempt, observation, kernel_attempt}` are retained up to a host-selected bound: 16 for `send-observe` and `send-observe-diagnostic`, 6 for `send-observe-rows`. Later observations extend one counted, contiguous omitted tail (first sequence, first and last attempt, count). Counters, kernel-retained attempt outcomes and the unknown-outcome list stay outside the rows, so they remain exact when rows are omitted. The diagnostic is derived from the record and cut at a 512-byte bound, or 48 bytes for `send-observe-diagnostic`; omitted bytes are counted in the same scope. Payload Text is never exported. r[H-OUTCOME-02]

**Rationale:** Missing rows are named, not silent, and an omitted admission row cannot make the record report fewer admissions or a known outcome. r[H-OUTCOME-02]

## Risks / Trade-offs

- The peer, grant table, acknowledgement script and cancellation point are host constants of a test embedder; the profile claims no network delivery, native asynchronous work, late completion reconciliation or retry policy.
- A send policy records one invocation; repeated invocations need a fresh selection.
