# WORKER-12 exact declared source host adapter

## MODIFIED Requirements

### Requirement: H-HOST-02
r[H-HOST-02]

**H-HOST-02.** Runtime authorization MUST be enforced independently of guest type/effect claims. Forged, stale, cross-context, or wrongly typed resource handles MUST NOT grant access to a host object.

For the selected WORKER-12 denial profile, `test.code-export` MUST be a genuine declared and invoked source operation, not an effect string attached after checking. An independently host-selected `worker-test-code-export@1.0.0` adapter MUST have exactly `Text Text -- ! test.code-export`, zero results, a module-owned immutable slot and its own reserved effect identity; it MUST independently survive source checking, kernel acceptance, Wasm environment replay and typed emission. An unbound or forged signature/identity/effect/owner MUST NOT emit or execute the import. The two Text inputs are the capture and destination, not authority.

The emitted Core import MUST be `noble/test_code_export_bound: (adapter_slot: i32) -> i32`. It MUST be called only by an invoked source-bound operation, with no capture/destination bytes, pointers or lengths passed. Its request MUST be denied before any capture serialization or publication; even a zero status or host throw MUST NOT make the generated operation succeed. Neither a declared effect, checked Program, import status, CLI, reflected capturable value nor guest-selected destination may mint authorization. Positive code export is outside this profile.

#### Scenario: WORKER-12 source-bound denial adapter

- GIVEN the existing unexecuted [WORKER-12](../../../../../specs/conformance/worker-cases.json) design, the exact checked `ledger@1.capture` source fixture and separately host-selected bound adapter identity/slot described in this change's design
- WHEN an independently checked `I64 -- I64 ! test.code-export` Program invokes its bound operation in a real generated Core guest linked to the deny-only import
- THEN exactly one real host request is counted, zero protected operations and capture publications occur, the generated execution remains denied even if the host reports success, and forged or unregistered bindings are rejected before guest execution; WORKER-12's absent/not-run/open/unassessed state does not change without its separately retained component scenario evidence
