# WORKER-12 source-bound denial adapter design

## Context

WORKER-12 already requires one denied request, zero protected operations and zero capture publication. Existing declared `test.emit`/`test.clock` are distinct host effects and cannot be relabeled as `test.code-export`. The selected Worker Program v2 design remains unpromoted. r[H-HOST-02]

## Decisions

### Decision: Exact independently selected source binding

**Choice:** Host selection supplies a single `BoundOperation { module_name: "ledger", module_version: 1, operation: "test.code-export", adapter_identity: "worker-test-code-export@1.0.0", adapter_slot: <host-selected u32>, input: [Text, Text], output: [], effects: [TEST_CODE_EXPORT] }`. The reserved kernel identity is `EffId(6)`; the effect is not an authority token. The source fixture is:

```noble
module ledger@1 [ require code_export Text Text -- ! test.code-export export capture def capture [ "private-capture" "local-destination" code_export ] ]
```

The host selects and commits that module, then checks `ledger@1.capture` with an `I64` input and exact `I64 -- I64 ! test.code-export` instantiated Program witness. Source alone cannot register a binding. Registration refuses other effect sets, Text counts, results, identities, duplicate slots or foreign module owners; independently replayed kernel/wasm acceptance checks the same row. r[H-HOST-02]

**Rationale:** The capture and destination are source-typed ordinary Text values, but neither reflection nor their presence grants release. Existing rank-1 stack-tail polymorphism preserves an unrelated `I64` caller input without exposing Text at the host import. r[H-HOST-02]

### Decision: Slot-only, unconditional denial import

**Choice:** Source emission of the invoked bound operation includes exactly `(import "noble" "test_code_export_bound" (func $host_code_export_bound (param i32) (result i32)))`. The sole import argument is the immutable host-selected adapter slot, not capture/destination pointer, length, contents, recipe or capability. The generated operation checks prior failure, sets terminal failure before calling the import, discards its status and never clears that failure, never pops either Text and never publishes package bytes. A host that throws or incorrectly returns zero cannot turn the operation into an export. The host may count one request and independently deny; no positive grant/exporter exists in this slice. r[H-HOST-02]

**Rationale:** It is a real Wasm call associated with a source-checked operation, not an unused manifest or fake effect. Always-deny runtime semantics prevent a future host linker or return-code mistake from silently creating an issuer. The WIT `request-code-export(owner: own<program>, destination: string) -> bool` guest-owner path is a separate component adaptation: its owned Program must match the checked installed capture, and the host must still deny before any capture serialization. r[H-HOST-02]

## Boundaries and evidence

The WORKER-12 canonical input/expected/state/evidence in `specs/conformance/worker-cases.json` remain unchanged: `implementation: absent`, `execution: not-run`, `proof: open`, `trust: unassessed` until independent real-guest validation. CLI commands do not mint destination/capture authorization. This delta specifies the exact source/host adapter, not positive export, portable package issuance, proof acceptance, or completion of MW1. r[H-HOST-02]
