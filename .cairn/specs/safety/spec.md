# Noble Safety Contract

<!-- cairn:purpose:start -->
## Purpose

This accepted specification records Noble draft contracts, not completed implementation.
Original requirement IDs, explanatory prose, examples, and open decisions remain authoritative.
Scenario clauses refer to unexecuted designs in the conformance ledger.

## Requirements
<!-- cairn:purpose:end -->


Document: SPEC-S001  
Revision: 0.1.0-draft.5  
Project: noble  
Status: Canonical working safety contract; implementation and proofs remain absent  
Depends on: SPEC-0001 and SPEC-V001 at 0.1.0-draft.5

[DECISIONS.md](../../../specs/DECISIONS.md) records the selected direction. [EVIDENCE.md](../evidence/spec.md) defines independent implementation, execution, proof, and trust status.

## 1. Purpose

This document defines what Noble means by **language safety**. It strengthens the existing stack/type, effect, resource, host-boundary, and verification requirements without adding a guest `unsafe` sublanguage.

The target is:

> **Every accepted Noble program executes within defined semantics. Guest code cannot opt out of type, stack, memory, resource, effect, or authority safety. Operations requiring machine-level unsafety live outside the guest language behind validated host boundaries.**

Safety is intentionally separated from total correctness, availability, confidentiality of intentionally released information, distributed exactly-once behavior, and application-specific correctness.

## 2. Safety claim classes

Noble SHALL distinguish four claim classes.

| Class | Stable-platform intent |
|---|---|
| **Language safety** | Mandatory: type, stack, memory/representation, and defined-failure safety |
| **Resource and authority safety** | Mandatory: ownership accounting, non-forgeable live authority, checked host boundaries |
| **Standard concurrency safety** | Mandatory for the standard concurrency profile: no guest shared-mutable-memory races, scoped participant state, checked protocol/wire boundaries |
| **Behavioral correctness** | Optional stronger proofs: postconditions, termination, liveness, business invariants, protocol progress |

### Requirement: S-CLAIM-01
r[S-CLAIM-01]

**S-CLAIM-01.** A release or report using the unqualified phrase **Noble-safe** SHALL identify the exact specification revision and MUST establish all applicable mandatory properties above for the supported subset. Unsupported features and unproved correspondence boundaries MUST remain visible.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: S-CLAIM-02
r[S-CLAIM-02]

**S-CLAIM-02.** Memory safety, type safety, and resource safety MUST NOT be used as shorthand for termination, deadlock freedom, network availability, confidentiality against authorized code, or exactly-once external effects.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

## 3. No guest unsafe escape and no language-level undefined behavior

### Requirement: S-LANG-01
r[S-LANG-01]

**S-LANG-01.** Noble source SHALL have no construct whose purpose is to disable or bypass the core stack/type checker, resource-eligibility rules, effect accounting, host authorization, or representation invariants. In particular, the standard language SHALL NOT provide a guest `unsafe` mode analogous to unchecked pointer or representation operations.


<!-- cairn:scenario-links:start -->
#### Scenario: S-CASE-14 for S-LANG-01

- GIVEN the `Core-Bootstrap` profile and every field of `input` in [S-CASE-14](../../../specs/conformance/safety-cases.json)
- WHEN the `static` procedure for case `S-CASE-14` runs against those inputs
- THEN the observations match every field of `expected` in case `S-CASE-14`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: S-LANG-02
r[S-LANG-02]

**S-LANG-02.** Raw machine addresses and unchecked native pointers SHALL NOT be ordinary Noble value types. An integer, byte sequence, text value, syntax node, artifact identifier, or program identity MUST NOT be reinterpret-able as a live resource or native address by guest code.


<!-- cairn:scenario-links:start -->
#### Scenario: S-CASE-14 for S-LANG-02

- GIVEN the `Core-Bootstrap` profile and every field of `input` in [S-CASE-14](../../../specs/conformance/safety-cases.json)
- WHEN the `static` procedure for case `S-CASE-14` runs against those inputs
- THEN the observations match every field of `expected` in case `S-CASE-14`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: S-LANG-03
r[S-LANG-03]

**S-LANG-03.** The defined semantic outcomes of accepted Noble execution MUST be limited to normal return, explicit modeled waiting/suspension where the execution profile permits it, divergence, a specified trap/abnormal termination, or a specified execution-profile failure such as quota exhaustion or cancellation. **Undefined behavior is not a Noble-language outcome.**


<!-- cairn:scenario-links:start -->
#### Scenario: S-CASE-13 for S-LANG-03

- GIVEN the `Wasm-Draft` profile and every field of `input` in [S-CASE-13](../../../specs/conformance/safety-cases.json)
- WHEN the `runtime` procedure for case `S-CASE-13` runs against those inputs
- THEN the observations match every field of `expected` in case `S-CASE-13`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: S-LANG-04
r[S-LANG-04]

**S-LANG-04.** A bounds violation, invalid variant/tag, malformed external value, stale handle, wrong handle kind/context, or equivalent representation error MUST be rejected before entering trusted Noble state or produce a specified failure. It MUST NOT authorize memory corruption or arbitrary execution.


<!-- cairn:scenario-links:start -->
#### Scenario: S-CASE-02 for S-LANG-04

- GIVEN the `Resources-Draft` profile and every field of `input` in [S-CASE-02](../../../specs/conformance/safety-cases.json)
- WHEN the `adapter` procedure for case `S-CASE-02` runs against those inputs
- THEN the observations match every field of `expected` in case `S-CASE-02`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: S-LANG-05
r[S-LANG-05]

**S-LANG-05.** Operations that require native unsafety MAY exist inside the compiler, runtime, engine integration, or host adapters. They are outside Noble guest semantics and MUST be covered by explicit implementation boundaries, validated public wrappers, and the verification/trust policy.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

## 4. Memory and representation safety

### Requirement: S-MEM-01
r[S-MEM-01]

**S-MEM-01.** A conforming host MUST NOT expose a guest operation that permits arbitrary reads or writes to host/native memory by numeric address. Guest-visible memory operations MUST be mediated by typed values, bounded byte regions, validated component memories, or opaque resources with documented contracts.


<!-- cairn:scenario-links:start -->
#### Scenario: S-CASE-02 for S-MEM-01

- GIVEN the `Resources-Draft` profile and every field of `input` in [S-CASE-02](../../../specs/conformance/safety-cases.json)
- WHEN the `adapter` procedure for case `S-CASE-02` runs against those inputs
- THEN the observations match every field of `expected` in case `S-CASE-02`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: S-MEM-02
r[S-MEM-02]

**S-MEM-02.** Use-after-retirement, double release, forged handles, cross-context handles, and wrong-kind handles MUST NOT produce guest-visible memory unsafety. They MUST be rejected or produce a specified failure at the validated resource boundary.


<!-- cairn:scenario-links:start -->
#### Scenario: S-CASE-03 for S-MEM-02

- GIVEN the `Resources-Draft` profile and every field of `input` in [S-CASE-03](../../../specs/conformance/safety-cases.json)
- WHEN the `adapter` procedure for case `S-CASE-03` runs against those inputs
- THEN the observations match every field of `expected` in case `S-CASE-03`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: S-CASE-08 for S-MEM-02

- GIVEN the `Resources-Draft` profile and every field of `input` in [S-CASE-08](../../../specs/conformance/safety-cases.json)
- WHEN the `adapter` procedure for case `S-CASE-08` runs against those inputs
- THEN the observations match every field of `expected` in case `S-CASE-08`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: S-MEM-03
r[S-MEM-03]

**S-MEM-03.** Serialization/deserialization, Preserves decoding, portable-code loading, Wasm import/export adaptation, and FFI/host callbacks are hostile-input boundaries unless a stronger trusted provenance contract is explicitly established. Successful byte decoding alone MUST NOT establish a trusted Noble type, live resource, authority, recipe, or proof claim.


<!-- cairn:scenario-links:start -->
#### Scenario: S-CASE-09 for S-MEM-03

- GIVEN the `Syndicate-planned` profile and every field of `input` in [S-CASE-09](../../../specs/conformance/safety-cases.json)
- WHEN the `adapter` procedure for case `S-CASE-09` runs against those inputs
- THEN the observations match every field of `expected` in case `S-CASE-09`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: S-MEM-04
r[S-MEM-04]

**S-MEM-04.** Compiler/runtime representation choices MAY use indexes, pointers, arenas, tables, garbage collection, reference counting, linear ownership, or another strategy internally. Those choices MUST preserve the abstract Noble invariants and MUST NOT be observable as permission for guest representation casts.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### 4.1 Byte views and semantic acceptance

### Requirement: S-DECODE-01
r[S-DECODE-01]

**S-DECODE-01.** Byte-layout checks MUST remain separate from semantic acceptance, artifact correspondence, and authority checks. A decoded representation is untrusted candidate data until the relevant semantic checks pass. Public constructors and deserializers MUST NOT manufacture accepted state from layout validity alone.

Zerocopy is an optional adapter candidate, not a required dependency. `FromBytes` and `TryFromBytes` concern Rust representation validity. They do not establish Noble schemas, effects, recipes, proofs, or live authority.


<!-- cairn:scenario-links:start -->
#### Scenario: ADAPT-04 for S-DECODE-01

- GIVEN the `Decoder-Experiment` profile and every field of `input` in [ADAPT-04](../../../specs/conformance/adaptation-cases.json)
- WHEN the `adapter` procedure for case `ADAPT-04` runs against those inputs
- THEN the observations match every field of `expected` in case `ADAPT-04`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: ADAPT-06 for S-DECODE-01

- GIVEN the `Core-Bootstrap` profile and every field of `input` in [ADAPT-06](../../../specs/conformance/adaptation-cases.json)
- WHEN the `admission` procedure for case `ADAPT-06` runs against those inputs
- THEN the observations match every field of `expected` in case `ADAPT-06`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: ADAPT-15 for S-DECODE-01

- GIVEN the `Core-Bootstrap` profile and every field of `input` in [ADAPT-15](../../../specs/conformance/adaptation-cases.json)
- WHEN the `admission` procedure for case `ADAPT-15` runs against those inputs
- THEN the observations match every field of `expected` in case `ADAPT-15`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: ADAPT-16 for S-DECODE-01

- GIVEN the `Implementation-Policy` profile and every field of `input` in [ADAPT-16](../../../specs/conformance/adaptation-cases.json)
- WHEN the `review` procedure for case `ADAPT-16` runs against those inputs
- THEN the observations match every field of `expected` in case `ADAPT-16`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: S-DECODE-02
r[S-DECODE-02]

**S-DECODE-02.** A borrowed byte view MUST retain valid, stable backing storage for its entire use. Across callbacks, suspension, or shared-memory mutation, the adapter MUST copy unless a reviewed ownership contract prevents invalidation. Bounds, offsets, arithmetic overflow, and total work MUST be checked before allocation or traversal.

A Rust `Immutable` marker does not freeze external storage. A copy also requires a safe, bounded read from its source. Copying a header does not stabilize a separately mutable payload.


<!-- cairn:scenario-links:start -->
#### Scenario: ADAPT-04 for S-DECODE-02

- GIVEN the `Decoder-Experiment` profile and every field of `input` in [ADAPT-04](../../../specs/conformance/adaptation-cases.json)
- WHEN the `adapter` procedure for case `ADAPT-04` runs against those inputs
- THEN the observations match every field of `expected` in case `ADAPT-04`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: ADAPT-05 for S-DECODE-02

- GIVEN the `Decoder-Experiment` profile and every field of `input` in [ADAPT-05](../../../specs/conformance/adaptation-cases.json)
- WHEN the `adapter` procedure for case `ADAPT-05` runs against those inputs
- THEN the observations match every field of `expected` in case `ADAPT-05`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: ADAPT-11 for S-DECODE-02

- GIVEN the `Component-Sync-Bootstrap` profile and every field of `input` in [ADAPT-11](../../../specs/conformance/adaptation-cases.json)
- WHEN the `adapter` procedure for case `ADAPT-11` runs against those inputs
- THEN the observations match every field of `expected` in case `ADAPT-11`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: S-DECODE-03
r[S-DECODE-03]

**S-DECODE-03.** A portable binary format MUST define integer encodings, byte order, tags, padding treatment, versioning, and limits explicitly. Native layouts and `IntoBytes` output MUST NOT define canonical program identity implicitly. Unsupported versions and malformed semantic fields MUST be rejected.

Preserves remains the selected protocol direction. A local binary-layout experiment does not select a new interchange format.


<!-- cairn:scenario-links:start -->
#### Scenario: ADAPT-04 for S-DECODE-03

- GIVEN the `Decoder-Experiment` profile and every field of `input` in [ADAPT-04](../../../specs/conformance/adaptation-cases.json)
- WHEN the `adapter` procedure for case `ADAPT-04` runs against those inputs
- THEN the observations match every field of `expected` in case `ADAPT-04`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: ADAPT-07 for S-DECODE-03

- GIVEN the `Implementation-Policy` profile and every field of `input` in [ADAPT-07](../../../specs/conformance/adaptation-cases.json)
- WHEN the `review` procedure for case `ADAPT-07` runs against those inputs
- THEN the observations match every field of `expected` in case `ADAPT-07`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

## 5. Type, stack, and effect safety

### Requirement: S-TYPE-01
r[S-TYPE-01]

**S-TYPE-01.** An accepted program executing with conforming host bindings MUST NOT reach a stuck state from wrong operand type, language-level stack underflow, invalid stack shape, or use of a consumed resource. These states are not permitted dynamic fallbacks.


<!-- cairn:scenario-links:start -->
#### Scenario: S-CASE-01 for S-TYPE-01

- GIVEN the `Core-Bootstrap` profile and every field of `input` in [S-CASE-01](../../../specs/conformance/safety-cases.json)
- WHEN the `static` procedure for case `S-CASE-01` runs against those inputs
- THEN the observations match every field of `expected` in case `S-CASE-01`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: S-TYPE-02
r[S-TYPE-02]

**S-TYPE-02.** `Program<S,T,e>` remains the executable interface. `run` MUST only execute a checked/prepared `Program`; `Syntax`, arbitrary bytes, untrusted artifacts, or claimed manifests MUST NOT be directly runnable.


<!-- cairn:scenario-links:start -->
#### Scenario: CORE-10 for S-TYPE-02

- GIVEN the `Core-Bootstrap` profile and every field of `input` in [CORE-10](../../../specs/conformance/cases.json)
- WHEN the `static` procedure for case `CORE-10` runs against those inputs
- THEN the observations match every field of `expected` in case `CORE-10`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: S-CASE-07 for S-TYPE-02

- GIVEN the `Wasm-Draft` profile and every field of `input` in [S-CASE-07](../../../specs/conformance/safety-cases.json)
- WHEN the `admission` procedure for case `S-CASE-07` runs against those inputs
- THEN the observations match every field of `expected` in case `S-CASE-07`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: S-EFFECT-01
r[S-EFFECT-01]

**S-EFFECT-01.** For every finite execution prefix, every guest-requested host operation MUST be included in the instantiated effect bound of the executing computation.


<!-- cairn:scenario-links:start -->
#### Scenario: S-CASE-05 for S-EFFECT-01

- GIVEN the `Core-Bootstrap` profile and every field of `input` in [S-CASE-05](../../../specs/conformance/safety-cases.json)
- WHEN the `static` procedure for case `S-CASE-05` runs against those inputs
- THEN the observations match every field of `expected` in case `S-CASE-05`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: S-CASE-16 for S-EFFECT-01

- GIVEN the `Wasm-Draft` profile and every field of `input` in [S-CASE-16](../../../specs/conformance/safety-cases.json)
- WHEN the `admission` procedure for case `S-CASE-16` runs against those inputs
- THEN the observations match every field of `expected` in case `S-CASE-16`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: S-EFFECT-02
r[S-EFFECT-02]

**S-EFFECT-02.** A program with an empty effect bound MUST NOT issue a guest host request. This does not imply termination, bounded resource use, or freedom from runtime housekeeping outside guest effects.

A denied request still counts as a request under S-EFFECT-01. Static effect rejection and forged-metadata admission rejection require zero candidate-body host requests. Runtime authority denial is a separate case whose declared effect bound already includes the requested operation. See [safety scenarios](../../../specs/conformance/safety-cases.json).


<!-- cairn:scenario-links:start -->
#### Scenario: S-CASE-05 for S-EFFECT-02

- GIVEN the `Core-Bootstrap` profile and every field of `input` in [S-CASE-05](../../../specs/conformance/safety-cases.json)
- WHEN the `static` procedure for case `S-CASE-05` runs against those inputs
- THEN the observations match every field of `expected` in case `S-CASE-05`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: S-CASE-16 for S-EFFECT-02

- GIVEN the `Wasm-Draft` profile and every field of `input` in [S-CASE-16](../../../specs/conformance/safety-cases.json)
- WHEN the `admission` procedure for case `S-CASE-16` runs against those inputs
- THEN the observations match every field of `expected` in case `S-CASE-16`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: S-EFFECT-03
r[S-EFFECT-03]

**S-EFFECT-03.** Effect information is descriptive, not authoritative. A type/effect check, `Program` possession, program identity, or successful proof MUST NOT create host authority.


<!-- cairn:scenario-links:start -->
#### Scenario: S-CASE-06 for S-EFFECT-03

- GIVEN the `Resources-Draft` profile and every field of `input` in [S-CASE-06](../../../specs/conformance/safety-cases.json)
- WHEN the `runtime` procedure for case `S-CASE-06` runs against those inputs
- THEN the observations match every field of `expected` in case `S-CASE-06`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: WORKER-10 for S-EFFECT-03

- GIVEN the `Worker-Design` profile and every field of `input` in [WORKER-10](../../../specs/conformance/worker-cases.json)
- WHEN the `runtime` procedure for case `WORKER-10` runs against those inputs
- THEN the observations match every field of `expected` in case `WORKER-10`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: WORKER-12 for S-EFFECT-03

- GIVEN the `Worker-Design` profile and every field of `input` in [WORKER-12](../../../specs/conformance/worker-cases.json)
- WHEN the `runtime` procedure for case `WORKER-12` runs against those inputs
- THEN the observations match every field of `expected` in case `WORKER-12`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

## 6. Resource and capability safety

This section extends the baseline move-only resource discipline.

### Requirement: S-RES-01
r[S-RES-01]

**S-RES-01.** A live resource/capability has an explicit ownership obligation in Noble. Generic duplication, generic discard, capture by `quote`, persistence, and serialization MUST remain unavailable for resource-bearing values unless a separately specified operation has semantics that preserve the resource contract.


<!-- cairn:scenario-links:start -->
#### Scenario: S-CASE-04 for S-RES-01

- GIVEN the `Resources-Draft` profile and every field of `input` in [S-CASE-04](../../../specs/conformance/safety-cases.json)
- WHEN the `static` procedure for case `S-CASE-04` runs against those inputs
- THEN the observations match every field of `expected` in case `S-CASE-04`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: S-RES-02
r[S-RES-02]

**S-RES-02.** Live authority MUST be non-forgeable from ordinary guest data. A resource may enter guest execution only through an authorized host operation, a checked transfer from another ownership context, or another profile-defined validated authority boundary.


<!-- cairn:scenario-links:start -->
#### Scenario: S-CASE-03 for S-RES-02

- GIVEN the `Resources-Draft` profile and every field of `input` in [S-CASE-03](../../../specs/conformance/safety-cases.json)
- WHEN the `adapter` procedure for case `S-CASE-03` runs against those inputs
- THEN the observations match every field of `expected` in case `S-CASE-03`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: S-RES-03
r[S-RES-03]

**S-RES-03.** Host resource tables MUST validate at least resource kind, owning/authorized context, liveness/generation, and operation-specific rights before acting on an incoming guest handle representation.


<!-- cairn:scenario-links:start -->
#### Scenario: S-CASE-03 for S-RES-03

- GIVEN the `Resources-Draft` profile and every field of `input` in [S-CASE-03](../../../specs/conformance/safety-cases.json)
- WHEN the `adapter` procedure for case `S-CASE-03` runs against those inputs
- THEN the observations match every field of `expected` in case `S-CASE-03`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: S-RES-04
r[S-RES-04]

**S-RES-04.** Resource-containing aggregates carry the ownership obligation recursively and MUST NOT satisfy `Data` or `Capture` merely because a particular runtime variant does not currently expose the resource.


<!-- cairn:scenario-links:start -->
#### Scenario: S-CASE-04 for S-RES-04

- GIVEN the `Resources-Draft` profile and every field of `input` in [S-CASE-04](../../../specs/conformance/safety-cases.json)
- WHEN the `static` procedure for case `S-CASE-04` runs against those inputs
- THEN the observations match every field of `expected` in case `S-CASE-04`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: S-RES-05
r[S-RES-05]

**S-RES-05.** Abnormal termination and cancellation MUST retire invocation-owned local handles according to the execution profile without requiring guest code to resume. This local cleanup guarantee MUST NOT be overstated as exactly-once cleanup of remote external effects.


<!-- cairn:scenario-links:start -->
#### Scenario: S-CASE-12 for S-RES-05

- GIVEN the `Syndicate-planned` profile and every field of `input` in [S-CASE-12](../../../specs/conformance/safety-cases.json)
- WHEN the `adapter` procedure for case `S-CASE-12` runs against those inputs
- THEN the observations match every field of `expected` in case `S-CASE-12`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

## 7. Standard concurrency safety

The standard concurrency profile is based on the Syndicated Actor Model / Syndicate/Synit direction selected for Noble, with Preserves as the de facto protocol/schema interchange layer. Concurrency remains outside the language kernel.

### Requirement: S-CONC-01
r[S-CONC-01]

**S-CONC-01.** Concurrent Noble participants in the standard profile MUST NOT obtain direct shared mutable Noble memory. Cross-participant interaction occurs through immutable data, typed dataspace assertions/interests/messages, explicit host operations, or profile-defined ownership transfer of resources.


<!-- cairn:scenario-links:start -->
#### Scenario: S-CASE-11 for S-CONC-01

- GIVEN the `Syndicate-planned` profile and every field of `input` in [S-CASE-11](../../../specs/conformance/safety-cases.json)
- WHEN the `admission` procedure for case `S-CASE-11` runs against those inputs
- THEN the observations match every field of `expected` in case `S-CASE-11`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: S-CONC-02
r[S-CONC-02]

**S-CONC-02.** The absence of guest shared mutable memory SHALL imply **Noble-visible data-race freedom** for the standard concurrency profile. This claim does not automatically prove the Rust runtime itself race-free; runtime synchronization must separately refine the abstract concurrency model.


<!-- cairn:scenario-links:start -->
#### Scenario: S-CASE-11 for S-CONC-02

- GIVEN the `Syndicate-planned` profile and every field of `input` in [S-CASE-11](../../../specs/conformance/safety-cases.json)
- WHEN the `admission` procedure for case `S-CASE-11` runs against those inputs
- THEN the observations match every field of `expected` in case `S-CASE-11`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: S-CONC-03
r[S-CONC-03]

**S-CONC-03.** Assertions, interests, reactions, and subordinate conversational activity MUST have explicit owners/scopes. Facet/actor termination SHALL retract or retire scope-owned conversational state according to the standard profile, including failure paths.


<!-- cairn:scenario-links:start -->
#### Scenario: S-CASE-12 for S-CONC-03

- GIVEN the `Syndicate-planned` profile and every field of `input` in [S-CASE-12](../../../specs/conformance/safety-cases.json)
- WHEN the `adapter` procedure for case `S-CASE-12` runs against those inputs
- THEN the observations match every field of `expected` in case `S-CASE-12`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: S-CONC-04
r[S-CONC-04]

**S-CONC-04.** Network or process-boundary protocol data MUST be decoded under bounded resource limits and validated against its Preserves schema/protocol before it becomes a typed Noble protocol value. Preserves embedded/native extension mechanisms MUST NOT by themselves create Noble resources or authority.


<!-- cairn:scenario-links:start -->
#### Scenario: S-CASE-09 for S-CONC-04

- GIVEN the `Syndicate-planned` profile and every field of `input` in [S-CASE-09](../../../specs/conformance/safety-cases.json)
- WHEN the `adapter` procedure for case `S-CASE-09` runs against those inputs
- THEN the observations match every field of `expected` in case `S-CASE-09`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: S-CASE-10 for S-CONC-04

- GIVEN the `Syndicate-planned` profile and every field of `input` in [S-CASE-10](../../../specs/conformance/safety-cases.json)
- WHEN the `adapter` procedure for case `S-CASE-10` runs against those inputs
- THEN the observations match every field of `expected` in case `S-CASE-10`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: S-CONC-05
r[S-CONC-05]

**S-CONC-05.** Typed protocol validity and runtime authority are distinct. A participant may only publish, observe, message, or transfer what both its static protocol interface and its runtime capability permit.


<!-- cairn:scenario-links:start -->
#### Scenario: S-CASE-10 for S-CONC-05

- GIVEN the `Syndicate-planned` profile and every field of `input` in [S-CASE-10](../../../specs/conformance/safety-cases.json)
- WHEN the `adapter` procedure for case `S-CASE-10` runs against those inputs
- THEN the observations match every field of `expected` in case `S-CASE-10`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: S-CONC-06
r[S-CONC-06]

**S-CONC-06.** Deadlock freedom, global progress, fairness, and service availability MUST remain separate claims. They require explicit protocol/profile assumptions or proofs and are not implied by race freedom or type safety.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

## 8. Host and artifact boundary safety

### Requirement: S-HOST-01
r[S-HOST-01]

**S-HOST-01.** Every public boundary from unverified Rust, FFI, Wasm, external wire data, connector/network input, or host callback into verified/runtime-internal state MUST validate all preconditions not already guaranteed by a proven caller relation.


<!-- cairn:scenario-links:start -->
#### Scenario: S-CASE-08 for S-HOST-01

- GIVEN the `Resources-Draft` profile and every field of `input` in [S-CASE-08](../../../specs/conformance/safety-cases.json)
- WHEN the `adapter` procedure for case `S-CASE-08` runs against those inputs
- THEN the observations match every field of `expected` in case `S-CASE-08`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: S-HOST-02
r[S-HOST-02]

**S-HOST-02.** Host adapters MUST validate their outputs before making them available as trusted Noble values. A malformed native adapter result is an implementation/binding defect and MUST NOT be accepted as if the guest type system had proved it.


<!-- cairn:scenario-links:start -->
#### Scenario: S-CASE-08 for S-HOST-02

- GIVEN the `Resources-Draft` profile and every field of `input` in [S-CASE-08](../../../specs/conformance/safety-cases.json)
- WHEN the `adapter` procedure for case `S-CASE-08` runs against those inputs
- THEN the observations match every field of `expected` in case `S-CASE-08`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: S-HOST-03
r[S-HOST-03]

**S-HOST-03.** A valid Wasm artifact, digest, signature, recipe hash, source identity, or attached proof object is not by itself authorization and is not by itself sufficient evidence of source/recipe correspondence. Loading policy MUST separately establish artifact validity, interface compatibility, provenance/correspondence policy, and concrete runtime authority.


<!-- cairn:scenario-links:start -->
#### Scenario: S-CASE-07 for S-HOST-03

- GIVEN the `Wasm-Draft` profile and every field of `input` in [S-CASE-07](../../../specs/conformance/safety-cases.json)
- WHEN the `admission` procedure for case `S-CASE-07` runs against those inputs
- THEN the observations match every field of `expected` in case `S-CASE-07`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: S-CASE-16 for S-HOST-03

- GIVEN the `Wasm-Draft` profile and every field of `input` in [S-CASE-16](../../../specs/conformance/safety-cases.json)
- WHEN the `admission` procedure for case `S-CASE-16` runs against those inputs
- THEN the observations match every field of `expected` in case `S-CASE-16`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: S-HOST-04
r[S-HOST-04]

**S-HOST-04.** Unsafe/native implementation code MUST be kept behind narrow reviewed boundaries. The project SHALL inventory such boundaries for any release that advertises the corresponding safety claim.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

## 9. Verification obligations

The reviewed Lean model remains the reference mathematical model for the safety claims. The existing distinction between language metatheory, Rust implementation refinement, optional program proofs, and source-to-Wasm correspondence remains mandatory.

The safety workstream adds these theorem/implementation obligations:

| Obligation | Required claim |
|---|---|
| **SO-01** | Accepted core executions have no forbidden stuck state and no undefined-behavior semantic outcome |
| **SO-02** | Representation/eligibility rules prevent resource or authority forgery through ordinary data |
| **SO-03** | Effect soundness covers every finite execution prefix, including denied/failed requests |
| **SO-04** | Whole-configuration resource ownership is preserved modulo declared host transitions |
| **SO-05** | Checked preparation cannot turn `Syntax`/untrusted metadata directly into an executable interface without validation |
| **SO-06** | Public host/resource wrappers establish all verified internal preconditions from hostile representations |
| **SO-07** | The selected Noble-to-Wasm lowering preserves the safety-relevant observation and failure relation for the claimed subset |
| **SO-08** | Standard concurrency semantics preserve participant isolation, scope-owned conversational state, protocol typing, and capability checks |
| **SO-09** | The concrete concurrency/runtime state machinery refines SO-08 under its documented synchronization assumptions |

### Requirement: S-VERIFY-01
r[S-VERIFY-01]

**S-VERIFY-01.** A stable-core safety claim MUST include reviewed Lean-kernel-checked evidence for the complete subset advertised as stable, together with the actual acceptance-checker correspondence required by SPEC-V001.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: S-VERIFY-02
r[S-VERIFY-02]

**S-VERIFY-02.** An end-to-end Noble-safe executable claim requires evidence for the relevant checked-source-to-artifact and loader/runtime boundaries, or those boundaries MUST be explicitly listed as trusted assumptions.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: S-VERIFY-03
r[S-VERIFY-03]

**S-VERIFY-03.** No proof result may convert an unsupported feature, timeout, solver unknown, runtime validation failure, or unverified boundary into a safety success.


<!-- cairn:scenario-links:start -->
#### Scenario: S-CASE-15 for S-VERIFY-03

- GIVEN the `release-policy` profile and every field of `input` in [S-CASE-15](../../../specs/conformance/safety-cases.json)
- WHEN the `review` procedure for case `S-CASE-15` runs against those inputs
- THEN the observations match every field of `expected` in case `S-CASE-15`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

## 10. Required safety outcomes and diagnostics

The implementation SHALL distinguish at least:

- static parse/resolution/type/stack rejection;
- resource/eligibility rejection;
- effect-bound rejection;
- authority denial;
- malformed/untrusted input rejection;
- stale/forged/wrong-context resource rejection;
- ordinary domain `Result` errors;
- specified guest trap/abnormal termination;
- execution budget/quota/cancellation failures;
- implementation/binding defect reports.

These categories MAY share user-facing presentation, but they MUST NOT be collapsed internally in a way that converts a safety failure into successful execution.

## 11. Explicit non-goals of the word “safe”

The mandatory safety claim does not by itself establish:

- termination or total correctness;
- deadlock freedom or distributed progress;
- availability of a host, network, filesystem, or remote service;
- confidentiality against code that legitimately possesses authority to disclose data;
- constant-time execution or side-channel freedom;
- exactly-once remote cleanup or business effects;
- correctness of application/business logic;
- correctness of Wasmtime, rustc, the OS, hardware, cryptography, or unproved host adapters.

Each such property requires a separately named model, contract, profile, or proof.

## 12. Stable safety release gate

### Requirement: S-GATE-01
r[S-GATE-01]

**S-GATE-01.** A stable Noble core SHALL NOT be advertised as Noble-safe until the stable subset has: (a) executed conformance tests covering the safety families, (b) reviewed Lean safety metatheory, (c) an actual acceptance checker related to that model, (d) explicit host-boundary validation policy, and (e) a published trust/assumption ledger.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: S-GATE-02
r[S-GATE-02]

**S-GATE-02.** A standard-concurrency Noble-safe claim MUST additionally include the profile's isolation/scope/protocol/capability invariants and a concrete runtime correspondence result or explicitly disclosed trusted-runtime boundary.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: S-GATE-03
r[S-GATE-03]

**S-GATE-03.** Future advanced type-system features—including dependent types, higher-rank polymorphism, principled subtyping, refinement facilities, or user-extensible inference—are not rejected by this safety contract. They MAY be added only with rules that preserve or deliberately strengthen the safety invariants and with corresponding proof obligations before joining a stable-safe subset.


<!-- cairn:scenario-links:start -->
#### Scenario: S-CASE-15 for S-GATE-03

- GIVEN the `release-policy` profile and every field of `input` in [S-CASE-15](../../../specs/conformance/safety-cases.json)
- WHEN the `review` procedure for case `S-CASE-15` runs against those inputs
- THEN the observations match every field of `expected` in case `S-CASE-15`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->
