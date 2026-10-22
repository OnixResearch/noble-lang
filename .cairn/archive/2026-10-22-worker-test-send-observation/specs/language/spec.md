# WORKER-10 bounded send observation

## MODIFIED Requirements

### Requirement: H-OUTCOME-02
r[H-OUTCOME-02]

**H-OUTCOME-02.** The host MUST distinguish requested, denied, admitted, completed, and externally unknown operation outcomes in its bounded observation record. Operation observations MUST retain invocation and operation-attempt correlation. A request or effect plan MUST NOT count as completed external work. Cancellation MUST NOT erase earlier observations or imply rollback of earlier effects.

If an observation limit prevents complete retention, the record MUST mark the omitted scope explicitly. Missing observations MUST NOT imply that no external action occurred. The profile MUST retain the primary invocation outcome and whether operation outcomes remain unknown. Observation export requires explicit authority and a declared policy for sensitive data.

The ownership transitions in [SPEC-R001](../../../../specs/resource-adapters/spec.md) govern cancellation/completion races for the selected async extension. These host records add no catch syntax, general handler, persistence guarantee, or automatic retry rule.

For the selected WORKER-10 profile, `test.send` MUST be a genuine declared and invoked source operation with its own reserved effect identity. An independently host-selected `worker-test-send@1.0.0` adapter MUST have exactly `Text -- ! test.send`, zero results and a module-owned immutable slot, and it MUST independently survive source checking, kernel acceptance, Wasm environment replay and typed emission. The emitted Core import MUST be `noble/test_send_bound: (offset: i32, length: i32, adapter_slot: i32) -> i32`. A zero status MUST mean only that the host recorded the request; the guest MUST NOT learn whether it was denied, completed or left unknown. Any other status MUST end the invocation.

The host MUST decide each request against its own exact-plan grants through the kernel authority table and MUST derive admitted, completed and externally unknown observations from kernel admission and kernel-retained observations, never from guest data or the plan. Each observation row MUST carry the invocation and the host-assigned attempt. Retained rows, the diagnostic and their bounds MUST be host-selected; observations or diagnostic bytes beyond a bound MUST be counted in an explicit omitted scope, while counters, kernel-retained attempt outcomes and the list of unknown outcomes stay exact. Payload Text MUST NOT be exported. The primary outcome MUST be cancelled only when the host cancelled both the authority invocation and the Worker task and the guest returned no result.

#### Scenario: WORKER-10 bounded send observation profile

- GIVEN the [WORKER-10](../../../../../specs/conformance/worker-cases.json) case, the exact `Sender@1.plus` fixture and the host-selected `test.send` binding, grant table, synthetic peer and output bounds described in this change's design
- WHEN the production `noble worker program test-host send-observe`, `send-observe-rows` and `send-observe-diagnostic` modes run the fixture once each
- THEN every record keeps invocation and attempt correlation, three requests, one denial, two kernel admissions, one completion and attempt-3's unknown outcome after the cancellation; the rows record marks its omitted tail and still reports attempt-3 admitted and unknown, the diagnostic record marks its omitted bytes, the complete record marks nothing, and no payload Text is exported
