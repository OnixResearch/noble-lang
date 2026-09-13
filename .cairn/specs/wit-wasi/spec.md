# WIT, Component Model, and WASI Integration Profile

<!-- cairn:purpose:start -->
## Purpose

This accepted specification records Noble draft contracts, not completed implementation.
Original requirement IDs, explanatory prose, examples, and open decisions remain authoritative.
Scenario clauses refer to unexecuted designs in the conformance ledger.

## Requirements
<!-- cairn:purpose:end -->


Document: SPEC-W001  
Revision: 0.1.0-draft.5  
Project: noble  
Status: Canonical working profile; implementation and conformance remain open  
Depends on: SPEC-0001, SPEC-S001, SPEC-V001 and SPEC-R001 at 0.1.0-draft.5

The first delivery is a named synchronous subset, not full `Component-Draft` conformance. See [RESOURCE-ADAPTERS.md](../resource-adapters/spec.md) and [ROADMAP.md](../../../specs/ROADMAP.md).

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

The standard component profile targets the stable WASI 0.3 family. An implementation MAY support WASI 0.2 through an explicit compatibility profile. Exact package versions and runtime/toolchain versions are build inputs and MUST be pinned for reproducible artifacts.

## 2. Architectural position

### Requirement: WI-ARCH-01
r[WI-ARCH-01]

**WI-ARCH-01.** A conforming `Component-Draft` implementation SHALL treat WebAssembly components, rather than unadorned core modules, as the primary portable deployment/interoperability artifact. Internal compiler stages MAY use core Wasm modules, and an explicitly named lower-level profile MAY expose them, but that profile is not the standard Noble component contract.


<!-- cairn:scenario-links:start -->
#### Scenario: WI-17 for WI-ARCH-01

- GIVEN the `release-policy` profile and every field of `input` in [WI-17](../../../specs/conformance/wit-wasi-cases.json)
- WHEN the `review` procedure for case `WI-17` runs against those inputs
- THEN the observations match every field of `expected` in case `WI-17`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: WI-ARCH-02
r[WI-ARCH-02]

**WI-ARCH-02.** WIT SHALL be the standard language for describing Noble component imports, exports, resource interfaces, and host-facing component contracts. Noble MUST NOT invent a parallel general-purpose component IDL where WIT can express the required contract.


<!-- cairn:scenario-links:start -->
#### Scenario: WI-17 for WI-ARCH-02

- GIVEN the `release-policy` profile and every field of `input` in [WI-17](../../../specs/conformance/wit-wasi-cases.json)
- WHEN the `review` procedure for case `WI-17` runs against those inputs
- THEN the observations match every field of `expected` in case `WI-17`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: WI-ARCH-03
r[WI-ARCH-03]

**WI-ARCH-03.** WIT is not Noble's internal type system. It MUST NOT constrain the semantics of `Program<S,T,e>`, stack-tail polymorphism, effect bounds, recipes, syntax reflection, definition identity, proof evidence, or other Noble-only concepts merely because those concepts lack direct WIT equivalents.


<!-- cairn:scenario-links:start -->
#### Scenario: WI-17 for WI-ARCH-03

- GIVEN the `release-policy` profile and every field of `input` in [WI-17](../../../specs/conformance/wit-wasi-cases.json)
- WHEN the `review` procedure for case `WI-17` runs against those inputs
- THEN the observations match every field of `expected` in case `WI-17`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: WI-ARCH-04
r[WI-ARCH-04]

**WI-ARCH-04.** Noble MUST interpret a WIT world as describing a component boundary: the functionality a component requires and provides. It does not specify the component's internal behavior, purity, Noble effect bound, proof status, or concrete runtime authority.


<!-- cairn:scenario-links:start -->
#### Scenario: WI-17 for WI-ARCH-04

- GIVEN the `release-policy` profile and every field of `input` in [WI-17](../../../specs/conformance/wit-wasi-cases.json)
- WHEN the `review` procedure for case `WI-17` runs against those inputs
- THEN the observations match every field of `expected` in case `WI-17`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

## 3. WIT package ingestion and generated bindings

### Requirement: WI-WIT-01
r[WI-WIT-01]

**WI-WIT-01.** The Noble compiler SHALL accept versioned WIT packages/worlds as compiler inputs and generate typed Noble bindings without requiring the user to hand-write equivalent host declarations.


<!-- cairn:scenario-links:start -->
#### Scenario: WI-01 for WI-WIT-01

- GIVEN the `Component-Sync-Bootstrap` profile and every field of `input` in [WI-01](../../../specs/conformance/wit-wasi-cases.json)
- WHEN the `adapter` procedure for case `WI-01` runs against those inputs
- THEN the observations match every field of `expected` in case `WI-01`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: WI-WIT-02
r[WI-WIT-02]

**WI-WIT-02.** Generated bindings MUST preserve the complete externally visible WIT type distinction. Where the Noble kernel has no identical scalar or aggregate type, the binding layer MUST introduce an exact declared/boundary type or checked conversion; it MUST NOT silently narrow, wrap, reinterpret, or otherwise lose information.


<!-- cairn:scenario-links:start -->
#### Scenario: WI-03 for WI-WIT-02

- GIVEN the `Component-Draft` profile and every field of `input` in [WI-03](../../../specs/conformance/wit-wasi-cases.json)
- WHEN the `adapter` procedure for case `WI-03` runs against those inputs
- THEN the observations match every field of `expected` in case `WI-03`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: WI-WIT-03
r[WI-WIT-03]

**WI-WIT-03.** For a WIT function with ordered parameters `A1 ... An` and ordered results `B1 ... Bm`, a generated Noble import word MUST have the schematic stack interface:

```text
S A1 ... An -- S B1 ... Bm ! {WitOpId}
```

Here `WitOpId` identifies the exact versioned WIT package, interface, and function. These resolved identities participate in semantic identity, not only build identity.

This schematic signature applies to directly lowered parameters and results. Synchronous borrowed imports use the owner-threading adapter in SPEC-R001. Generated bindings publish the adapted signature explicitly.

Every guest-requested import MUST retain its `WitOpId` in the effect bound, including imports with reviewed deterministic or side-effect-free implementations. Such review does not remove the boundary request. Host-operation internals remain separate from request accounting under V-EFFECT-01. This profile defines no pure foreign-call exemption.


<!-- cairn:scenario-links:start -->
#### Scenario: WI-01 for WI-WIT-03

- GIVEN the `Component-Sync-Bootstrap` profile and every field of `input` in [WI-01](../../../specs/conformance/wit-wasi-cases.json)
- WHEN the `adapter` procedure for case `WI-01` runs against those inputs
- THEN the observations match every field of `expected` in case `WI-01`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: WI-07 for WI-WIT-03

- GIVEN the `Component-Sync-Bootstrap` profile and every field of `input` in [WI-07](../../../specs/conformance/wit-wasi-cases.json)
- WHEN the `static` procedure for case `WI-07` runs against those inputs
- THEN the observations match every field of `expected` in case `WI-07`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: WI-WIT-04
r[WI-WIT-04]

**WI-WIT-04.** A WIT import MUST contribute its operation identity to the effect bound. A signature or reviewed purity claim MUST NOT erase that requirement. A WIT signature alone MUST NOT establish purity, determinism, termination, authorization, or absence of external interaction.


<!-- cairn:scenario-links:start -->
#### Scenario: WI-07 for WI-WIT-04

- GIVEN the `Component-Sync-Bootstrap` profile and every field of `input` in [WI-07](../../../specs/conformance/wit-wasi-cases.json)
- WHEN the `static` procedure for case `WI-07` runs against those inputs
- THEN the observations match every field of `expected` in case `WI-07`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: WI-WIT-05
r[WI-WIT-05]

**WI-WIT-05.** A Noble export implementing a WIT function MUST have a closed, WIT-lowerable external interface. The component adapter SHALL invoke that program against an isolated adapter stack containing only the declared parameters and SHALL validate that its normal results match the declared WIT result contract. An ambient Noble stack tail MUST NOT cross a component boundary implicitly.


<!-- cairn:scenario-links:start -->
#### Scenario: WI-02 for WI-WIT-05

- GIVEN the `Component-Sync-Bootstrap` profile and every field of `input` in [WI-02](../../../specs/conformance/wit-wasi-cases.json)
- WHEN the `static` procedure for case `WI-02` runs against those inputs
- THEN the observations match every field of `expected` in case `WI-02`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: WI-WIT-06
r[WI-WIT-06]

**WI-WIT-06.** Values crossing the component boundary MUST be validated and lowered/lifted according to the selected Component Model/Canonical ABI contract. An implementation MUST NOT expose internal addresses, closure pointers, stack locations, or unspecified language representations as WIT values.


<!-- cairn:scenario-links:start -->
#### Scenario: WI-03 for WI-WIT-06

- GIVEN the `Component-Draft` profile and every field of `input` in [WI-03](../../../specs/conformance/wit-wasi-cases.json)
- WHEN the `adapter` procedure for case `WI-03` runs against those inputs
- THEN the observations match every field of `expected` in case `WI-03`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: WI-WIT-07
r[WI-WIT-07]

**WI-WIT-07.** The adapter contract MUST account for allocation, copying, and buffer ownership during lifting/lowering and cleanup. A failed conversion MUST NOT publish a partially initialized trusted value. Internal GC support or a byte-view library MUST NOT imply zero-copy component transfer.

M5 checks normal and failed string/list conversions against an independent peer. The M3 component probe in [SPEC-BE001](../backend-experiments/spec.md) is not that conformance evidence.


<!-- cairn:scenario-links:start -->
#### Scenario: ADAPT-11 for WI-WIT-07

- GIVEN the `Component-Sync-Bootstrap` profile and every field of `input` in [ADAPT-11](../../../specs/conformance/adaptation-cases.json)
- WHEN the `adapter` procedure for case `ADAPT-11` runs against those inputs
- THEN the observations match every field of `expected` in case `ADAPT-11`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

## 4. WIT resources and Noble ownership

### Requirement: WI-RES-01
r[WI-RES-01]

**WI-RES-01.** An owned WIT resource handle SHALL map to a Noble move-only resource obligation. It MUST NOT satisfy `Data` or `Capture` merely because the WIT representation is handle-shaped.


<!-- cairn:scenario-links:start -->
#### Scenario: WI-04 for WI-RES-01

- GIVEN the `Component-Sync-Bootstrap` profile and every field of `input` in [WI-04](../../../specs/conformance/wit-wasi-cases.json)
- WHEN the `static` procedure for case `WI-04` runs against those inputs
- THEN the observations match every field of `expected` in case `WI-04`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: WI-RES-02
r[WI-RES-02]

**WI-RES-02.** A borrowed WIT resource SHALL be represented as a scoped non-owning boundary value whose lifetime cannot escape the adapter call or declared borrow scope. It MUST NOT be captured, persisted, serialized, stored beyond its scope, or released as though it were owned. This requirement does not introduce a general Rust-style borrow checker into ordinary Noble code.


<!-- cairn:scenario-links:start -->
#### Scenario: WI-05 for WI-RES-02

- GIVEN the `Component-Sync-Bootstrap` profile and every field of `input` in [WI-05](../../../specs/conformance/wit-wasi-cases.json)
- WHEN the `static` procedure for case `WI-05` runs against those inputs
- THEN the observations match every field of `expected` in case `WI-05`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: WI-RES-03
r[WI-RES-03]

**WI-RES-03.** WIT resource type identity, ownership mode, component instance/context, and host validation MUST participate in resource-handle validation. Integers, bytes, external IDs, or stale handles MUST NOT be accepted as live resources solely because they have a compatible machine representation.


<!-- cairn:scenario-links:start -->
#### Scenario: WI-06 for WI-RES-03

- GIVEN the `Component-Sync-Bootstrap` profile and every field of `input` in [WI-06](../../../specs/conformance/wit-wasi-cases.json)
- WHEN the `adapter` procedure for case `WI-06` runs against those inputs
- THEN the observations match every field of `expected` in case `WI-06`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: WI-RES-04
r[WI-RES-04]

**WI-RES-04.** Resource lowering MUST preserve the existing Noble normal-path and abnormal-cleanup obligations. A Component Model adapter MUST NOT weaken SPEC-0001's move-only ownership rules.


<!-- cairn:scenario-links:start -->
#### Scenario: WI-12 for WI-RES-04

- GIVEN the `Component-Async-planned` profile and every field of `input` in [WI-12](../../../specs/conformance/wit-wasi-cases.json)
- WHEN the `adapter` procedure for case `WI-12` runs against those inputs
- THEN the observations match every field of `expected` in case `WI-12`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: WI-RES-05
r[WI-RES-05]

**WI-RES-05.** The first synchronous subset MUST obey SPEC-R001. It rejects borrowed exports, escaping borrows, reentrant access to busy owners, and suspension with an active borrow. This restriction does not reject those features permanently. Their later admission requires a separate lifetime contract and evidence.


<!-- cairn:scenario-links:start -->
#### Scenario: WI-05 for WI-RES-05

- GIVEN the `Component-Sync-Bootstrap` profile and every field of `input` in [WI-05](../../../specs/conformance/wit-wasi-cases.json)
- WHEN the `static` procedure for case `WI-05` runs against those inputs
- THEN the observations match every field of `expected` in case `WI-05`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

## 5. Worlds, effects, and authority

### Requirement: WI-WORLD-01
r[WI-WORLD-01]

**WI-WORLD-01.** The selected WIT world and exact imported/exported interface versions SHALL be explicit compilation inputs and SHALL participate in the build key. Replacing an interface version or world is not an invisible linker detail.


<!-- cairn:scenario-links:start -->
#### Scenario: WI-09 for WI-WORLD-01

- GIVEN the `Component-Sync-Bootstrap` profile and every field of `input` in [WI-09](../../../specs/conformance/wit-wasi-cases.json)
- WHEN the `identity` procedure for case `WI-09` runs against those inputs
- THEN the observations match every field of `expected` in case `WI-09`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: WI-WORLD-02
r[WI-WORLD-02]

**WI-WORLD-02.** The compiler SHALL reject an export whose Noble interface cannot be soundly lowered to the selected WIT contract. It MUST NOT satisfy an incompatible WIT export through `Any`, unchecked casts, implicit resource laundering, or runtime operand guessing.


<!-- cairn:scenario-links:start -->
#### Scenario: WI-02 for WI-WORLD-02

- GIVEN the `Component-Sync-Bootstrap` profile and every field of `input` in [WI-02](../../../specs/conformance/wit-wasi-cases.json)
- WHEN the `static` procedure for case `WI-02` runs against those inputs
- THEN the observations match every field of `expected` in case `WI-02`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: WI-AUTH-01
r[WI-AUTH-01]

**WI-AUTH-01.** WIT import availability and a Noble effect bound are not, by themselves, authority grants. Runtime authority SHALL remain controlled by the linked host/component implementation, resource/capability values, and host policy. Some imported functions may themselves convey ambient authority; such authority MUST be documented by the host profile and is not created by the Noble type checker.


<!-- cairn:scenario-links:start -->
#### Scenario: WI-08 for WI-AUTH-01

- GIVEN the `Component-Sync-Bootstrap` profile and every field of `input` in [WI-08](../../../specs/conformance/wit-wasi-cases.json)
- WHEN the `runtime` procedure for case `WI-08` runs against those inputs
- THEN the observations match every field of `expected` in case `WI-08`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: WI-AUTH-02
r[WI-AUTH-02]

**WI-AUTH-02.** Standard Noble profiles SHOULD prefer capability-scoped/resource-scoped WIT and WASI interfaces over ambient global authority where the ecosystem interface permits it.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: WI-AUTH-03
r[WI-AUTH-03]

**WI-AUTH-03.** WIT package/interface/function identity MAY supply stable names for Noble host-operation/effect identities. Such identities describe the operation contract, not the caller's permission to perform it.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

## 6. WASI as the standard host-interface family

### Requirement: WI-WASI-01
r[WI-WASI-01]

**WI-WASI-01.** The standard Noble host profile SHALL use stable WASI interfaces where a stable WASI interface adequately represents the desired host capability. A Noble-specific duplicate filesystem, clock, random, socket, HTTP, CLI, or similar interface SHOULD NOT be standardized without a documented semantic reason.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: WI-WASI-02
r[WI-WASI-02]

**WI-WASI-02.** The preferred standard host profile SHALL target the stable WASI 0.3 family. Exact WIT/WASI package patch versions MUST be pinned in a reproducible build/profile and MUST participate in compatibility checks and build identity.


<!-- cairn:scenario-links:start -->
#### Scenario: WI-17 for WI-WASI-02

- GIVEN the `release-policy` profile and every field of `input` in [WI-17](../../../specs/conformance/wit-wasi-cases.json)
- WHEN the `review` procedure for case `WI-17` runs against those inputs
- THEN the observations match every field of `expected` in case `WI-17`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: WI-WASI-03
r[WI-WASI-03]

**WI-WASI-03.** WASI 0.2 support MAY be provided as an explicitly named compatibility profile or adapter. A compiler/runtime MUST NOT silently reinterpret a 0.2 component as 0.3, or vice versa, without a documented compatibility layer.


<!-- cairn:scenario-links:start -->
#### Scenario: WI-13 for WI-WASI-03

- GIVEN the `Component-Draft` profile and every field of `input` in [WI-13](../../../specs/conformance/wit-wasi-cases.json)
- WHEN the `admission` procedure for case `WI-13` runs against those inputs
- THEN the observations match every field of `expected` in case `WI-13`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: WI-WASI-04
r[WI-WASI-04]

**WI-WASI-04.** Noble-specific platform facilities that cross a component boundary SHOULD themselves be expressed as versioned WIT packages so Rust, Noble, and other Component Model languages can participate without language-specific ABI agreements.


<!-- cairn:scenario-links:start -->
#### Scenario: WI-15 for WI-WASI-04

- GIVEN the `Component-Sync-Bootstrap` profile and every field of `input` in [WI-15](../../../specs/conformance/wit-wasi-cases.json)
- WHEN the `runtime` procedure for case `WI-15` runs against those inputs
- THEN the observations match every field of `expected` in case `WI-15`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

## 7. Native async, streams, and futures

WASI 0.3 and the Component Model provide native cross-component `async func`, `stream<T>`, and `future<T>` primitives. Noble adopts these as its standard *component-boundary* async ABI. This choice does not add `async` or `await` expression forms to Noble.

### Requirement: WI-ASYNC-01
r[WI-ASYNC-01]

**WI-ASYNC-01.** A WIT `async func` imported into Noble SHALL be callable through direct-style Noble code. The runtime MAY suspend and resume the invocation according to the Component Model async ABI; source-level sequential evaluation order MUST remain preserved.


<!-- cairn:scenario-links:start -->
#### Scenario: WI-11 for WI-ASYNC-01

- GIVEN the `Component-Async-planned` profile and every field of `input` in [WI-11](../../../specs/conformance/wit-wasi-cases.json)
- WHEN the `runtime` procedure for case `WI-11` runs against those inputs
- THEN the observations match every field of `expected` in case `WI-11`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: WI-ASYNC-02
r[WI-ASYNC-02]

**WI-ASYNC-02.** Suspension of an async import MUST NOT imply concurrency of subsequent sequential Noble words, speculative execution, retry, transactionality, or reordering.


<!-- cairn:scenario-links:start -->
#### Scenario: WI-11 for WI-ASYNC-02

- GIVEN the `Component-Async-planned` profile and every field of `input` in [WI-11](../../../specs/conformance/wit-wasi-cases.json)
- WHEN the `runtime` procedure for case `WI-11` runs against those inputs
- THEN the observations match every field of `expected` in case `WI-11`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: WI-ASYNC-03
r[WI-ASYNC-03]

**WI-ASYNC-03.** WIT `stream<T>` and `future<T>` SHALL be represented by typed profile-level Noble values with explicit completion/cancellation/error semantics. They are not additional expression forms.


<!-- cairn:scenario-links:start -->
#### Scenario: WI-12 for WI-ASYNC-03

- GIVEN the `Component-Async-planned` profile and every field of `input` in [WI-12](../../../specs/conformance/wit-wasi-cases.json)
- WHEN the `adapter` procedure for case `WI-12` runs against those inputs
- THEN the observations match every field of `expected` in case `WI-12`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: WI-ASYNC-04
r[WI-ASYNC-04]

**WI-ASYNC-04.** Until duplication/capture semantics are proven and standardized for a particular async value, `stream<T>` and `future<T>` boundary values SHALL conservatively be treated as non-`Data` and non-`Capture` when they carry live runtime state.


<!-- cairn:scenario-links:start -->
#### Scenario: WI-16 for WI-ASYNC-04

- GIVEN the `Component-Async-planned` profile and every field of `input` in [WI-16](../../../specs/conformance/wit-wasi-cases.json)
- WHEN the `static` procedure for case `WI-16` runs against those inputs
- THEN the observations match every field of `expected` in case `WI-16`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: WI-ASYNC-05
r[WI-ASYNC-05]

**WI-ASYNC-05.** The standard async profile MUST specify cancellation propagation, outstanding resource ownership, completion errors, quotas/backpressure where applicable, and cleanup after abnormal termination. Native Component Model async is an ABI mechanism, not a complete concurrency policy.

SPEC-R001 RA-ASYNC-02 through RA-ASYNC-05 define required local ownership transitions for this later extension. Direct-style imports can use host-owned task records. They do not require new guest `await` syntax or a task constructor. The concrete ABI mapping and task/stream interfaces remain open.


<!-- cairn:scenario-links:start -->
#### Scenario: WI-12 for WI-ASYNC-05

- GIVEN the `Component-Async-planned` profile and every field of `input` in [WI-12](../../../specs/conformance/wit-wasi-cases.json)
- WHEN the `adapter` procedure for case `WI-12` runs against those inputs
- THEN the observations match every field of `expected` in case `WI-12`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

## 8. `Program` values and portable code

### Requirement: WI-PROG-01
r[WI-PROG-01]

**WI-PROG-01.** Noble `Program<S,T,e>` is not automatically a WIT function value, resource, or closure. A component boundary MUST NOT export an internal program as a raw address or unspecified handle.

P-ROUND-01/02 and P-PACK-01/02 in SPEC-0001 define round-trip and dependency-closure obligations. They do not select a portable byte format or make a digest sufficient for admission.


<!-- cairn:scenario-links:start -->
#### Scenario: WI-10 for WI-PROG-01

- GIVEN the `Component-Sync-Bootstrap` profile and every field of `input` in [WI-10](../../../specs/conformance/wit-wasi-cases.json)
- WHEN the `static` procedure for case `WI-10` runs against those inputs
- THEN the observations match every field of `expected` in case `WI-10`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: WI-PROG-02
r[WI-PROG-02]

**WI-PROG-02.** Transporting a Noble program across a component, process, or network boundary remains an explicit portable-code/package problem. Such transport MUST preserve recipe/interface/dependency identity, validation/preparation requirements, capture eligibility, and destination-side authorization.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: WI-PROG-03
r[WI-PROG-03]

**WI-PROG-03.** A concrete WIT resource interface for prepared Noble programs MAY be standardized later, but it MUST specify ownership, interface specialization, recipe/provenance binding, lifecycle, and authorization independently of the internal `Program<S,T,e>` representation.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

## 9. Syndicate, Preserves, and component boundaries

### Requirement: WI-SYN-01
r[WI-SYN-01]

**WI-SYN-01.** Syndicate/Synit remains the standard Noble concurrency/service model. Where the Syndicate layer crosses Component Model boundaries, its host/component operations SHOULD be exposed through versioned WIT packages rather than bespoke native ABIs.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: WI-SYN-02
r[WI-SYN-02]

**WI-SYN-02.** Preserves and WIT are complementary. Preserves is the default protocol/schema representation for Syndicate conversational data; WIT defines component interfaces, resources, and ABI-visible operations. A Preserves value MUST NOT become a WIT resource/capability without an explicit validated adapter.


<!-- cairn:scenario-links:start -->
#### Scenario: WI-14 for WI-SYN-02

- GIVEN the `Syndicate-planned` profile and every field of `input` in [WI-14](../../../specs/conformance/wit-wasi-cases.json)
- WHEN the `adapter` procedure for case `WI-14` runs against those inputs
- THEN the observations match every field of `expected` in case `WI-14`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: WI-SYN-03
r[WI-SYN-03]

**WI-SYN-03.** A componentized Syndicate implementation MUST preserve the standard concurrency profile's scope, assertion-lifetime, isolation, capability, and hostile-input safety requirements across the WIT boundary.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

## 10. Versioning, identity, and reproducibility

### Requirement: WI-ID-01
r[WI-ID-01]

**WI-ID-01.** Exact WIT package/world/interface versions, Component Model feature profile, WASI profile, adapters, and relevant runtime/compiler configuration SHALL participate in the `BuildKey` when they can affect generated artifacts or observable boundary semantics.


<!-- cairn:scenario-links:start -->
#### Scenario: WI-09 for WI-ID-01

- GIVEN the `Component-Sync-Bootstrap` profile and every field of `input` in [WI-09](../../../specs/conformance/wit-wasi-cases.json)
- WHEN the `identity` procedure for case `WI-09` runs against those inputs
- THEN the observations match every field of `expected` in case `WI-09`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: ID-02 for WI-ID-01

- GIVEN the `ProgramsData-Draft` profile and every field of `input` in [ID-02](../../../specs/conformance/identity-cases.json)
- WHEN the `identity` procedure for case `ID-02` runs against those inputs
- THEN the observations match every field of `expected` in case `ID-02`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: ID-06 for WI-ID-01

- GIVEN the `Component-Draft` profile and every field of `input` in [ID-06](../../../specs/conformance/identity-cases.json)
- WHEN the `identity` procedure for case `ID-06` runs against those inputs
- THEN the observations match every field of `expected` in case `ID-06`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: WI-ID-02
r[WI-ID-02]

**WI-ID-02.** WIT or WASI package identity MUST NOT replace `DefinitionId` or `ProgramValueId`. Component interface identity and Noble program identity answer different questions.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: WI-ID-03
r[WI-ID-03]

**WI-ID-03.** Upgrading a WIT/WASI dependency MUST NOT silently mutate a previously prepared Noble program or running component. Rebuilding against a changed boundary creates a new build context and, where emitted bytes differ, a new artifact identity.


<!-- cairn:scenario-links:start -->
#### Scenario: ID-06 for WI-ID-03

- GIVEN the `Component-Draft` profile and every field of `input` in [ID-06](../../../specs/conformance/identity-cases.json)
- WHEN the `identity` procedure for case `ID-06` runs against those inputs
- THEN the observations match every field of `expected` in case `ID-06`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: WI-ID-04
r[WI-ID-04]

**WI-ID-04.** A changed resolved `WitOpId`, semantic schema, or dependency MUST change the containing definition identity. Dependent program-value identities MUST include that changed dependency. Unrelated definitions do not change merely because another world import changes.


<!-- cairn:scenario-links:start -->
#### Scenario: ID-03 for WI-ID-04

- GIVEN the `Component-Draft` profile and every field of `input` in [ID-03](../../../specs/conformance/identity-cases.json)
- WHEN the `identity` procedure for case `ID-03` runs against those inputs
- THEN the observations match every field of `expected` in case `ID-03`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: ID-06 for WI-ID-04

- GIVEN the `Component-Draft` profile and every field of `input` in [ID-06](../../../specs/conformance/identity-cases.json)
- WHEN the `identity` procedure for case `ID-06` runs against those inputs
- THEN the observations match every field of `expected` in case `ID-06`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: WI-ID-05
r[WI-ID-05]

**WI-ID-05.** An adapter-only change can preserve semantic identity only when the resolved Noble operation and schema contracts remain identical. The build key MUST change when the adapter can affect emitted artifacts or boundary behavior. Matching WIT function shapes alone does not establish semantic equivalence.

The [identity scenarios](../../../specs/conformance/identity-cases.json) distinguish semantic changes from build-only changes. They specify relations, not stable digest bytes. Canonical Noble encoding remains open.


<!-- cairn:scenario-links:start -->
#### Scenario: ID-04 for WI-ID-05

- GIVEN the `Component-Draft` profile and every field of `input` in [ID-04](../../../specs/conformance/identity-cases.json)
- WHEN the `identity` procedure for case `ID-04` runs against those inputs
- THEN the observations match every field of `expected` in case `ID-04`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

## 11. Safety and trust boundary

### Requirement: WI-SAFE-01
r[WI-SAFE-01]

**WI-SAFE-01.** WIT/component decoding, lifting/lowering, resource-table access, host callbacks, and native adapter entry points are hostile or semi-trusted boundaries under SPEC-S001. Public wrappers MUST validate every precondition not already established by an independently verified caller.


<!-- cairn:scenario-links:start -->
#### Scenario: WI-06 for WI-SAFE-01

- GIVEN the `Component-Sync-Bootstrap` profile and every field of `input` in [WI-06](../../../specs/conformance/wit-wasi-cases.json)
- WHEN the `adapter` procedure for case `WI-06` runs against those inputs
- THEN the observations match every field of `expected` in case `WI-06`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: WI-SAFE-02
r[WI-SAFE-02]

**WI-SAFE-02.** A valid Wasm component or valid WIT world does not prove that Noble recipe metadata, effect metadata, proof evidence, resource authority, or semantic provenance is truthful. Loading/admission MUST preserve SPEC-0001's separate validation and trust checks.


<!-- cairn:scenario-links:start -->
#### Scenario: WI-14 for WI-SAFE-02

- GIVEN the `Syndicate-planned` profile and every field of `input` in [WI-14](../../../specs/conformance/wit-wasi-cases.json)
- WHEN the `adapter` procedure for case `WI-14` runs against those inputs
- THEN the observations match every field of `expected` in case `WI-14`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: WI-SAFE-03
r[WI-SAFE-03]

**WI-SAFE-03.** The standard Noble-safe claim for the Component profile MUST include tests and assurance covering WIT type mapping, resource ownership/borrows, malformed component values, wrong-context handles, import/export mismatch, async cancellation/cleanup, and the selected loader/linker boundary.


<!-- cairn:scenario-links:start -->
#### Scenario: WI-17 for WI-SAFE-03

- GIVEN the `release-policy` profile and every field of `input` in [WI-17](../../../specs/conformance/wit-wasi-cases.json)
- WHEN the `review` procedure for case `WI-17` runs against those inputs
- THEN the observations match every field of `expected` in case `WI-17`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

## 12. Conformance scenarios

The accompanying `conformance/wit-wasi-cases.json` contains expected scenarios. All scenarios in this revision are `not-run` until executed against an implementation.

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

| ID | Deliverable |
|---|---|
| OW-01 | Complete lossless WIT-to-Noble type mapping, including exact integer/floating widths and aggregate/schema identities |
| OW-02 | Implement SPEC-R001 and prove its bounded ownership rules; design borrowed exports and broader lifetime rules separately |
| OW-03 | Concrete WIT world import/export compiler pipeline and generated-binding tests |
| OW-04 | Component Model/Canonical ABI lowering and loader/linker implementation |
| OW-05 | WASI 0.3 profile package/version matrix with pinned Wasmtime/tooling compatibility |
| OW-06 | Full native async/stream/future semantics remain open; SPEC-R001 specifies only rejection and pending-retirement constraints for the first subset |
| OW-07 | Syndicate/Synit WIT package boundary and Preserves conversion profile |
| OW-08 | Executed cross-language interoperability suite with at least one independently implemented Component Model language/toolchain |
| OW-09 | Safety/trust correspondence for public wrappers, resource tables, lifting/lowering, and component loading |
| OW-10 | Source/recipe-to-component correspondence evidence appropriate to advertised verification claims |

## 14. External reference status

The architectural facts used by this revision were checked against first-party documentation on 2026-09-12:

- WIT worlds describe a component's imports and exports and define its external contract: <https://component-model.bytecodealliance.org/design/wit.html> and <https://component-model.bytecodealliance.org/design/worlds.html>
- WASI 0.3 is the current stable WASI family; 0.3 adds Component Model native async primitives `async func`, `stream<T>`, and `future<T>`: <https://wasi.dev/releases> and <https://wasi.dev/releases/wasi-p3>
- Component Model native async is documented at <https://component-model.bytecodealliance.org/design/async.html>

These references establish ecosystem mechanisms and current release status. Noble's type mapping, effect treatment, ownership rules, standard-profile policy, and safety requirements above are Noble-specific design decisions.
