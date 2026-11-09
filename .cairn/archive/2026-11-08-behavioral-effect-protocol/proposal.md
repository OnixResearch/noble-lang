## Why

Issue #9 asks for one small effectful contract profile that specifies allowed ordering, results and cleanup obligations beyond an effect set.

Program-contracts already require explicit host and trace assumptions (VC-LOGIC-05), separate claim kinds (VC-LOGIC-03) and order-preserving composition (VC-COMPOSE-02). The language already constrains resource movement and cleanup (K-RES-01–04).

However, the delivered MC1/MC2 behavioral fragment is resource-free, and no protocol model, trace contract or monitor exists. Auditing the existing adapters also exposes observability gaps: release kinds are conflated, trap and quota outcomes are conflated, there is no ordered trace, and there is no cancellation control. These gaps would let a finite run be misread as protocol evidence. r[EPROTO-SCOPE-01] r[EPROTO-OBS-01]

## What Changes

- Select the existing synchronous `noble-test:bounded-region/bounded@1.0.0` adapter, after an audit of its actual host code, as `RegionProtocol-v1`: host-injected owner, borrowed reads, explicit release. Specify its state machine, result schema, resource identity tuple and host assumptions A1–A6. r[EPROTO-SCOPE-01] r[EPROTO-TRACE-01]
- Separate effect permission, behavioral proof, invocation applicability, ownership, cleanup authority and host authorization. r[EPROTO-SCOPE-01]
- Specify:
  - a trace model with guest requests, host responses and committed effects;
  - normal-return postconditions for success and ordinary error;
  - prefix-safety properties P1–P6;
  - host-owned cleanup after trap, quota exhaustion or cancellation, with no total-correctness claim. r[EPROTO-TRACE-01]
- Add an order-preserving composition rule with a worked safe composition and rejected out-of-order, omitted/double release, wrong-identity and unmodeled-response cases. r[EPROTO-COMPOSE-01]
- Specify the required host report extensions, the proof plan T1–T6, the monitor responsibilities, eleven unexecuted EPROTO-CASE designs, and a first-profile recommendation. r[EPROTO-OBS-01]

## Impact

- **Files**: only `.cairn/changes/behavioral-effect-protocol/`, with ADDED deltas in program-contracts (EPROTO-SCOPE-01, EPROTO-TRACE-01, EPROTO-COMPOSE-01) and resource-adapters (EPROTO-OBS-01). There are no canonical spec, conformance, obligation, status or generated-view edits.
- **Code**: none. `bounded_region.rs`, the WIT world, the S-CASE-02 receipts and the M5 proof lane are unchanged.
- **Testing**: document validation only. No runtime, monitor or proof evidence is produced. Ownership/lifetimes (#3), allocation (#5), compiled-artifact correspondence (#10) and applicability guards (#8) are coordination dependencies.
