# Developer experience and library direction

<!-- cairn:purpose:start -->
## Purpose

This accepted specification records Noble draft contracts, not completed implementation.
Original requirement IDs, explanatory prose, examples, and open decisions remain authoritative.
Scenario clauses declare designs; the conformance ledger records execution and evidence.

## Requirements

<!-- cairn:purpose:end -->


Document: SPEC-DX001  
Revision: 0.1.0-draft.5  
Status: Selected draft contracts; bounded DX-03/08/09 implementation/execution passed under source-bound DXM1 evidence; proof and broader profile open

## Scope and adoption boundary

This document selects scoped language and library contracts, including the later worker and Octet amendments. It does not expand the bootstrap type system or change ordinary program execution.

Elm, Gleam, and Idris inspire diagnostics and typed holes. Gleam, Rust, and Roc inspire opaque types and variants. Rust and Gleam inspire fallible composition. Unison inspires identity tooling. Koka and Eff inspire effect substitution.

Factor and Kitten inspire local names. Roc inspires capability-aware modules. QuickCheck and Elm inspire property testing. Austral and Rust inspire resource protocols. Rust and Racket inspire executable documentation.

These names record design inspiration, not audited upstream behavior, dependencies, compatibility claims, or copied implementations. Noble's existing safety and verification contracts govern every adaptation.

## 1. Stack diagnostics and editor holes

### Requirement: DX-DIAG-01
r[DX-DIAG-01]

**DX-DIAG-01.** The checker MUST report the failing word or join, required stack, actual stack, and relevant effect or eligibility constraint. It MUST retain available source spans and value-origin spans. If provenance is unavailable, it MUST report that fact rather than invent a location.

Successful editor analysis should expose stack states at word boundaries. Diagnostics must distinguish stack order, stack arity, type mismatch, branch mismatch, and resource duplication. Exact wording is not standardized.


<!-- cairn:scenario-links:start -->
#### Scenario: DX-01 for DX-DIAG-01

- GIVEN the `Core-Bootstrap` profile and every field of `input` in [DX-01](../../../specs/conformance/developer-experience-cases.json)
- WHEN the `static` procedure for case `DX-01` runs against those inputs
- THEN the observations match every field of `expected` in case `DX-01`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: CALC-10 for DX-DIAG-01

- GIVEN the `AI-Authoring-Design` profile and every field of `input` in [CALC-10](../../../specs/conformance/calculator-cases.json)
- WHEN the `review` procedure for case `CALC-10` runs against those inputs
- THEN the observations match every field of `expected` in case `CALC-10`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: DX-HOLE-01
r[DX-HOLE-01]

**DX-HOLE-01.** Editor holes MUST remain incomplete syntax, not executable values or trusted witnesses. Analysis MUST report known input/output stack constraints and effect constraints, including unresolved variables. Every admission path MUST reject a candidate containing a hole. Analysis MUST NOT execute the candidate body.

Hole punctuation and editor transport remain open. Structured syntax fixtures do not select new source grammar. Hole support follows M2 and does not block the first checker.


<!-- cairn:scenario-links:start -->
#### Scenario: DX-02 for DX-HOLE-01

- GIVEN the `Editor-Draft` profile and every field of `input` in [DX-02](../../../specs/conformance/developer-experience-cases.json)
- WHEN the `admission` procedure for case `DX-02` runs against those inputs
- THEN the observations match every field of `expected` in case `DX-02`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: CALC-10 for DX-HOLE-01

- GIVEN the `AI-Authoring-Design` profile and every field of `input` in [CALC-10](../../../specs/conformance/calculator-cases.json)
- WHEN the `review` procedure for case `CALC-10` runs against those inputs
- THEN the observations match every field of `expected` in case `CALC-10`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

## 2. Opaque domain types and exhaustive variants

### Requirement: DX-TYPE-01
r[DX-TYPE-01]

**DX-TYPE-01.**

Distinct declared type identities MUST NOT unify merely because their representations match. `Declared-Modules-v1` selects real guest source declarations (`opaque` and two-constructor `variant`), resolved immutable module/version/declaration identities, visibility-controlled constructors and explicit conversions. A source declaration cannot be represented only by structured checker fixtures or erased into structural `I64`/`Sum` for acceptance. Public type export does not expose private construction or representation. The module interface freezes its source version and resolved dependency identities; implementation-local identity is not a portable package hash. The Core-Bootstrap profile is unchanged.


<!-- cairn:scenario-links:start -->
#### Scenario: DX-03 for DX-TYPE-01

- GIVEN the `Declared-Modules-v1` profile and every field of `input` in [DX-03](../../../specs/conformance/developer-experience-cases.json)
- WHEN the `static` procedure for case `DX-03` runs against those inputs
- THEN the observations match every field of `expected` in case `DX-03`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

#### Scenario: DX-03 nominal source checking

- GIVEN actual `Declared-Modules-v1` source modules declaring `UserId` and `OrderId` over `I64`, plus private and public constructors
- WHEN the source passes immutable resolution, type inference and independent kernel acceptance
- THEN different nominal identities cannot unify, matching identities can, and a foreign module cannot invoke a private constructor; static checking starts zero guest calls

### Requirement: DX-TYPE-02
r[DX-TYPE-02]

**DX-TYPE-02.**

A variant match MUST cover both constructors of its resolved immutable schema. The schema owner may match private arms; an external caller MAY match only when both constructors and their payload interfaces are public/exported. Exporting the variant type alone MUST NOT expose a private arm or payload; an owner may instead export its own checked eliminator. Each admitted typed branch receives its constructor's payload, agrees on the complete ordered result stack and conservatively joins latent effects. A partial/mismatched match or foreign private-arm match MUST fail before execution. Eligibility MUST inspect every nested payload and representation of a nominal wrapper or variant, including an opaque resource-containing wrapper, and MUST reject duplication, discard, capture and serialization whenever recursive eligibility fails. Only a validated resource kind may enter the static fixture; that fixture does not assert resource-positive Wasm support.


<!-- cairn:scenario-links:start -->
#### Scenario: DX-03 for DX-TYPE-02

- GIVEN the `Declared-Modules-v1` profile and every field of `input` in [DX-03](../../../specs/conformance/developer-experience-cases.json)
- WHEN the `static` procedure for case `DX-03` runs against those inputs
- THEN the observations match every field of `expected` in case `DX-03`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

#### Scenario: DX-03 exhaustive and resource eligibility

- GIVEN actual source declarations of a two-constructor variant with a private arm, an all-public variant and an opaque wrapper containing an explicitly validated resource kind
- WHEN each complete/incomplete match and dup/drop/quotation capture is checked by the independent kernel
- THEN the owner or an external all-public caller can complete the matching-stack match; missing or mismatched branches, a foreign private-arm match and forbidden resource operations are rejected, and no guest or protected host operation starts

### Requirement: DX-TYPE-03
r[DX-TYPE-03]

**DX-TYPE-03.** Domain declarations MUST distinguish nominal identity from representation aliases. Distinct identities such as `AttemptId` and `TaskGeneration` MUST NOT unify because both contain integers. Budget and quantity contracts MUST declare their units, valid ranges, and explicit conversions. Admission budgets, execution fuel, durations, and byte counts are not interchangeable.

A zero budget can be valid data while its execution policy denies all work. The domain contract determines that distinction. Checked unit conversion does not alter core `I64` wrapping semantics.


<!-- cairn:scenario-links:start -->
#### Scenario: OCTET-06 for DX-TYPE-03

- GIVEN the `Octet-Adoption-Design` profile and every field of `input` in [OCTET-06](../../../specs/conformance/octet-adoption-cases.json)
- WHEN the `admission` procedure for case `OCTET-06` runs against those inputs
- THEN the observations match every field of `expected` in case `OCTET-06`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: DX-TYPE-04
r[DX-TYPE-04]

**DX-TYPE-04.** A declaration owner MUST enumerate every admitted construction path and the invariant each path establishes. Constructors, conversions, defaults, decoders, and package imports MUST preserve that invariant. A public field, wrapper tag, or representation alias MUST NOT bypass validation. Invalid inputs MUST remain errors rather than sentinel or default values that appear valid.

Use ordinary opaque declarations and explicit fallible library constructors. These requirements add no general refinement inference, dependent type system, or proof search. Nominal data remains subject to recursive eligibility. It does not become a capability merely because its name includes authorization.


<!-- cairn:scenario-links:start -->
#### Scenario: OCTET-03 for DX-TYPE-04

- GIVEN the `Octet-Adoption-Design` profile and every field of `input` in [OCTET-03](../../../specs/conformance/octet-adoption-cases.json)
- WHEN the `static` procedure for case `OCTET-03` runs against those inputs
- THEN the observations match every field of `expected` in case `OCTET-03`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: OCTET-06 for DX-TYPE-04

- GIVEN the `Octet-Adoption-Design` profile and every field of `input` in [OCTET-06](../../../specs/conformance/octet-adoption-cases.json)
- WHEN the `admission` procedure for case `OCTET-06` runs against those inputs
- THEN the observations match every field of `expected` in case `OCTET-06`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

## 3. Fallible composition as a library

### Requirement: DX-RESULT-01
r[DX-RESULT-01]

**DX-RESULT-01.** The Result library MUST provide success mapping, error mapping, fallible chaining, and recovery through ordinary checked programs. Result MUST remain an ordinary variant, not an exception effect or new expression form.


<!-- cairn:scenario-links:start -->
#### Scenario: DX-04 for DX-RESULT-01

- GIVEN the `Result-Library-Draft` profile and every field of `input` in [DX-04](../../../specs/conformance/developer-experience-cases.json)
- WHEN the `runtime` procedure for case `DX-04` runs against those inputs
- THEN the observations match every field of `expected` in case `DX-04`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: DX-RESULT-02
r[DX-RESULT-02]

**DX-RESULT-02.** Each combinator MUST declare its ordered stack interface and conservative latent effect bound. Execution MUST invoke only the selected branch. Both branches MUST account for every owned input without implicit duplication or discard.

Names and concrete signatures remain open until the schema/library interface gate. The initial slice uses resource-free values. Resource-bearing acceptance requires the ownership profile and explicit release behavior. No combinator may add an unchecked early-return path.


<!-- cairn:scenario-links:start -->
#### Scenario: DX-04 for DX-RESULT-02

- GIVEN the `Result-Library-Draft` profile and every field of `input` in [DX-04](../../../specs/conformance/developer-experience-cases.json)
- WHEN the `runtime` procedure for case `DX-04` runs against those inputs
- THEN the observations match every field of `expected` in case `DX-04`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

## 4. Identity-based development tools

### Requirement: DX-IDENTITY-01
r[DX-IDENTITY-01]

**DX-IDENTITY-01.** Dependency browsing and semantic diffs MUST use resolved identities and retained recipes, not mutable names or optimizer layout. Tools MUST distinguish semantic changes from build-only changes and unavailable comparisons.


<!-- cairn:scenario-links:start -->
#### Scenario: DX-05 for DX-IDENTITY-01

- GIVEN the `Identity-Tooling-Draft` profile and every field of `input` in [DX-05](../../../specs/conformance/developer-experience-cases.json)
- WHEN the `identity` procedure for case `DX-05` runs against those inputs
- THEN the observations match every field of `expected` in case `DX-05`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: DX-CACHE-01
r[DX-CACHE-01]

**DX-CACHE-01.** Test and proof caches MUST bind results to the exact subject, relevant dependencies, claim, assumptions, and tool/schema revisions. Execution caches MUST also bind backend/build and relevant environment inputs. Consumers MUST recheck evidence compatibility and current admission policy before reuse.

A cache hit does not authorize execution or prove a claim. Recorded execution is not a fresh execution. Unknown dependencies or environment inputs prevent reuse as current evidence. Proof-cache lookup does not replace evidence validation under SPEC-V002.

Stable persisted cache keys require the canonical encoding gate. Earlier tools may use explicitly versioned, implementation-local keys without portable identity claims.


<!-- cairn:scenario-links:start -->
#### Scenario: DX-05 for DX-CACHE-01

- GIVEN the `Identity-Tooling-Draft` profile and every field of `input` in [DX-05](../../../specs/conformance/developer-experience-cases.json)
- WHEN the `identity` procedure for case `DX-05` runs against those inputs
- THEN the observations match every field of `expected` in case `DX-05`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

## 5. Explicit test-host substitution

### Requirement: DX-HOST-01
r[DX-HOST-01]

**DX-HOST-01.** Test-host substitution MUST use an explicit operation-to-adapter mapping with checked interfaces. It MUST preserve declared effect accounting and authorization checks. Missing, denied, or incompatible mappings MUST fail explicitly, without fallback to a real host operation.


<!-- cairn:scenario-links:start -->
#### Scenario: DX-06 for DX-HOST-01

- GIVEN the `Test-Host-Draft` profile and every field of `input` in [DX-06](../../../specs/conformance/developer-experience-cases.json)
- WHEN the `adapter` procedure for case `DX-06` runs against those inputs
- THEN the observations match every field of `expected` in case `DX-06`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: DX-HOST-02
r[DX-HOST-02]

**DX-HOST-02.** Test reports MUST identify substituted operations, adapter versions, scripted inputs, and host requests, including denied requests. Mocked execution MUST remain distinct from real-host evidence. Unexpected requests and exhausted scripts MUST produce explicit test failures.

The deterministic core owns substitution validation and dispatch decisions. The shell invokes adapters. A fake clock or filesystem does not grant credentials or erase an effect. These contracts do not introduce resumable handlers, continuations, or a second concurrency model.


<!-- cairn:scenario-links:start -->
#### Scenario: DX-06 for DX-HOST-02

- GIVEN the `Test-Host-Draft` profile and every field of `input` in [DX-06](../../../specs/conformance/developer-experience-cases.json)
- WHEN the `adapter` procedure for case `DX-06` runs against those inputs
- THEN the observations match every field of `expected` in case `DX-06`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

## 6. Optional local names

### Requirement: DX-LOCAL-01
r[DX-LOCAL-01]

**DX-LOCAL-01.** Local bindings MUST be lexical and lower into existing checked stack operations. Lowering MUST preserve ordered inputs, outputs, effects, and resource ownership. Repeated use requires duplication eligibility; unused values require discard eligibility. A binding MUST NOT introduce mutable cells or dynamic name lookup.


<!-- cairn:scenario-links:start -->
#### Scenario: DX-07 for DX-LOCAL-01

- GIVEN the `Local-Binding-Design` profile and every field of `input` in [DX-07](../../../specs/conformance/language-workflow-cases.json)
- WHEN the `review` procedure for case `DX-07` runs against those inputs
- THEN the observations match every field of `expected` in case `DX-07`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: DX-LOCAL-02
r[DX-LOCAL-02]

**DX-LOCAL-02.** Quotation captures through local names MUST obey existing Capture eligibility and appear in retained recipes. Local names MUST NOT permit capture of live resources. The design MUST specify scope, shadowing, source diagnostics, and the relationship between binding syntax, lowered recipes, and program identity before acceptance.

Concrete syntax remains open. First compare a realistic stack-only program with a binding-based version. Acceptance requires equivalent observable results and effects, plus rejection of resource reuse, implicit resource discard, and resource capture. Local bindings remain outside Core-Bootstrap; they add surface convenience, not a new kernel mechanism.


<!-- cairn:scenario-links:start -->
#### Scenario: DX-07 for DX-LOCAL-02

- GIVEN the `Local-Binding-Design` profile and every field of `input` in [DX-07](../../../specs/conformance/language-workflow-cases.json)
- WHEN the `review` procedure for case `DX-07` runs against those inputs
- THEN the observations match every field of `expected` in case `DX-07`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

## 7. Capability-aware modules

### Requirement: DX-MODULE-01
r[DX-MODULE-01]

**DX-MODULE-01.**

`Declared-Modules-v1` source modules MUST explicitly declare exports and required operations and resolve source-versioned imports against an explicitly supplied immutable registry. Source/text bytes remain UTF-8, while declared identifiers are ASCII; a non-ASCII declaration name MUST reject rather than silently normalize. Scoped `@` version notation and generic type commas do not expand Core-Bootstrap's ordinary word grammar. Linking MUST validate exact ordered operation signatures, conservative effect bounds, visibility and resolved dependencies before publication. A module body contains declarations only: top-level execution, initializer or import-time host/guest invocation MUST be rejected, not merely deferred behind an unchecked shim. A source file is an explicitly supplied compiler input; optional bounded compiler-shell file reading is not a guest effect. Package registry, remote transport and portable encoding remain outside this profile.


<!-- cairn:scenario-links:start -->
#### Scenario: DX-08 for DX-MODULE-01

- GIVEN the `Declared-Modules-v1` profile and every field of `input` in [DX-08](../../../specs/conformance/language-workflow-cases.json)
- WHEN the `admission` procedure for case `DX-08` runs against those inputs
- THEN the observations match every field of `expected` in case `DX-08`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

#### Scenario: DX-08 zero-execution linking

- GIVEN a source module requiring `test.emit : Text -- ! {test.emit}` and a separately supplied typed adapter binding
- WHEN matching, missing, wrong-input, wrong-effect and initializer variants undergo actual module link admission
- THEN only the matching binding links, all others reject, no guest or host request executes, no protected operation occurs and no authority is acquired

### Requirement: DX-MODULE-02
r[DX-MODULE-02]

**DX-MODULE-02.**

A module requirement or supplied binding MUST NOT be a host credential. The linked checked program MUST retain immutable module/version, operation and selected adapter identities even if a display alias is rebound. Absent, incompatible and denied bindings MUST NOT fall back to an ambient host operation. At runtime the real compiled Noble/Wasm guest request MUST reach independent host authorization; denied requests remain in declared effect/request accounting but perform no protected work. Link-time checks MUST NOT themselves authorize or invoke the host adapter. Exact module transport and unavailable operation contracts MUST fail explicitly.


<!-- cairn:scenario-links:start -->
#### Scenario: DX-08 for DX-MODULE-02

- GIVEN the `Declared-Modules-v1` profile and every field of `input` in [DX-08](../../../specs/conformance/language-workflow-cases.json)
- WHEN the `admission` procedure for case `DX-08` runs against those inputs
- THEN the observations match every field of `expected` in case `DX-08`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: DX-09 for DX-MODULE-02

- GIVEN the `Declared-Modules-v1` profile and every field of `input` in [DX-09](../../../specs/conformance/language-workflow-cases.json)
- WHEN the `runtime` procedure for case `DX-09` runs against those inputs
- THEN the observations match every field of `expected` in case `DX-09`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

#### Scenario: DX-08 link without authority

- GIVEN the declared source requirement and explicit adapter contracts, with a host policy that has not granted access
- WHEN the source module is imported and linked
- THEN matching binding admission occurs without guest or host execution; missing or incompatible bindings reject without fallback or acquired authority

#### Scenario: DX-09 retained A and independent denial

- GIVEN a checked compiled program resolved to version-A module/adapter, a later display-name rebind to version B, and an independent host denial of `test.emit`
- WHEN the earlier compiled Noble/Wasm program executes once
- THEN its one version-A request is recorded and denied, `test.emit` remains in the checked effect set, zero protected operations execute and version B receives no call

### Requirement: DX-MODULE-03
r[DX-MODULE-03]

**DX-MODULE-03.** A module that exports a transition program MUST declare exact state, event, action, and program interfaces. Constructor visibility and exhaustiveness MUST follow the resolved schema identities. An adapter between schema versions MUST be explicit, typed, and subject to ordinary effect and ownership checks.

A registry can normalize programs to one interface through checked adapters and effect widening under K-PROG-09/10 and K-EFFECT-05. It cannot introduce dynamic `Any`, implicit variant joins, or unchecked calls. Package boundaries follow P-PACK-01/02. The [worker scenario](../../../specs/WORKER-CONFORMANCE.md) uses these contracts without new declaration syntax.


<!-- cairn:scenario-links:start -->
#### Scenario: WORKER-01 for DX-MODULE-03

- GIVEN the `Worker-Design` profile and every field of `input` in [WORKER-01](../../../specs/conformance/worker-cases.json)
- WHEN the `runtime` procedure for case `WORKER-01` runs against those inputs
- THEN the observations match every field of `expected` in case `WORKER-01`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: WORKER-02 for DX-MODULE-03

- GIVEN the `Worker-Design` profile and every field of `input` in [WORKER-02](../../../specs/conformance/worker-cases.json)
- WHEN the `static` procedure for case `WORKER-02` runs against those inputs
- THEN the observations match every field of `expected` in case `WORKER-02`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: DX-MODULE-04
r[DX-MODULE-04]

**DX-MODULE-04.** Module admission MUST derive effect requirements from resolved definitions, host contracts, and trusted instantiated program interfaces. Each invocation MUST have an applicable interface with a conservative effect bound. An unresolved operation or unavailable contract MUST NOT become an empty effect set or a name-based purity exemption.

A typed indirect call can use its trusted `Program<S,T,e>` bound without enumerating every possible runtime program value. Supported recursive calls use their checked signatures. Effect closure does not prove termination. Dependency traversal and constraint work remain bounded under K-CHECK-07.

A provider summary cannot erase a known direct effect or override the admitted contract for another identity. Rust source-token analysis can supply separate implementation evidence. It cannot replace Noble's resolved effect judgments or backend correspondence.


<!-- cairn:scenario-links:start -->
#### Scenario: OCTET-07 for DX-MODULE-04

- GIVEN the `Octet-Adoption-Design` profile and every field of `input` in [OCTET-07](../../../specs/conformance/octet-adoption-cases.json)
- WHEN the `admission` procedure for case `OCTET-07` runs against those inputs
- THEN the observations match every field of `expected` in case `OCTET-07`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

## 8. Property testing and shrinking

### Requirement: DX-PROPERTY-01
r[DX-PROPERTY-01]

**DX-PROPERTY-01.** Property tests MUST record the subject, property, generator and shrinker revisions, seed, bounds, environment, and outcomes. Well-typed generation MUST pass each candidate through independent acceptance. Malformed-candidate fuzzing MUST remain a separate lane. Rejection by acceptance MUST NOT silently count as successful well-typed coverage.


<!-- cairn:scenario-links:start -->
#### Scenario: DX-10 for DX-PROPERTY-01

- GIVEN the `Property-Test-Design` profile and every field of `input` in [DX-10](../../../specs/conformance/language-workflow-cases.json)
- WHEN the `review` procedure for case `DX-10` runs against those inputs
- THEN the observations match every field of `expected` in case `DX-10`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: DX-PROPERTY-02
r[DX-PROPERTY-02]

**DX-PROPERTY-02.** Shrinking MUST preserve the property's input domain and reproduce the same failure predicate before accepting a smaller counterexample. Shrinking MUST be bounded and retain the original failure if reduction fails. Reports MUST distinguish passed trials, counterexamples, discarded inputs, exhaustion, unsupported cases, and harness errors.

The first properties cover compatible program composition, retained recipe structure, and result/effect agreement with the reference model for bounded bootstrap cases. Observable disagreement requires a counterexample, not merely an exhausted budget. Seeds alone are insufficient replay records. Random testing does not establish a universal theorem, and a shared implementation bug can invalidate a differential oracle.


<!-- cairn:scenario-links:start -->
#### Scenario: DX-10 for DX-PROPERTY-02

- GIVEN the `Property-Test-Design` profile and every field of `input` in [DX-10](../../../specs/conformance/language-workflow-cases.json)
- WHEN the `review` procedure for case `DX-10` runs against those inputs
- THEN the observations match every field of `expected` in case `DX-10`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

## 9. Resource protocol types

### Requirement: DX-PROTOCOL-01
r[DX-PROTOCOL-01]

**DX-PROTOCOL-01.** A typed protocol transition MUST consume the prior owner and account for ownership on every result branch. The protocol design MUST specify states, legal operations, failure outcomes, cleanup, and correspondence to runtime handle states. Source-level state labels MUST NOT bypass runtime handle validation or host authorization.


<!-- cairn:scenario-links:start -->
#### Scenario: DX-11 for DX-PROTOCOL-01

- GIVEN the `Protocol-Type-Design` profile and every field of `input` in [DX-11](../../../specs/conformance/language-workflow-cases.json)
- WHEN the `review` procedure for case `DX-11` runs against those inputs
- THEN the observations match every field of `expected` in case `DX-11`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: DX-PROTOCOL-02
r[DX-PROTOCOL-02]

**DX-PROTOCOL-02.** A failed or interrupted host transition MUST NOT imply that the prior state remains usable. The adapter MUST report the owner disposition required by its explicit boundary contract. Ambiguous external outcomes MUST remain explicit and MUST NOT cause an automatic retry without a suitable host contract.

This is a later extension of [resource adapters](../resource-adapters/spec.md) and [program contracts](../program-contracts/spec.md), not a competing ownership system. Static typestate does not establish an external service's current state. Behavioral protocol proofs additionally require the corresponding host model; ordinary resource programs do not acquire a mandatory proof requirement. Protocol declarations, error-state representation, and runtime correspondence remain open gates.


<!-- cairn:scenario-links:start -->
#### Scenario: DX-11 for DX-PROTOCOL-02

- GIVEN the `Protocol-Type-Design` profile and every field of `input` in [DX-11](../../../specs/conformance/language-workflow-cases.json)
- WHEN the `review` procedure for case `DX-11` runs against those inputs
- THEN the observations match every field of `expected` in case `DX-11`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: DX-PROTOCOL-03
r[DX-PROTOCOL-03]

**DX-PROTOCOL-03.** A declared finite lifecycle protocol MUST account explicitly for every state/event constructor pair, including invalid transitions. Each invalid pair MUST produce its declared rejection and ownership disposition. Wildcards, default success, and guards without an explicit remaining case MUST NOT establish complete transition coverage.

Explicit groups of named alternatives can share a transition. Coverage binds the exact resolved state/event schema identities. A schema change requires renewed coverage before admission. Data-dependent conditions still need explicit outcomes and applicable invariant evidence.

This requirement applies to designated lifecycle cores, not every ordinary program or open external schema. A decoder rejects unsupported external variants before typed transition admission. Unsupported transition shapes remain unsupported, not exhaustiveness evidence. No special Noble state-machine syntax is selected.


<!-- cairn:scenario-links:start -->
#### Scenario: OCTET-05 for DX-PROTOCOL-03

- GIVEN the `Octet-Adoption-Design` profile and every field of `input` in [OCTET-05](../../../specs/conformance/octet-adoption-cases.json)
- WHEN the `review` procedure for case `OCTET-05` runs against those inputs
- THEN the observations match every field of `expected` in case `OCTET-05`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

## 10. Executable documentation

### Requirement: DX-DOC-01
r[DX-DOC-01]

**DX-DOC-01.** Executable examples MUST bind source, expected observations, specification revision, compiler/build context, and required host configuration. The harness MUST use the actual compiler and applicable backend. Static-rejection examples MUST check the expected diagnostic category and zero candidate-body host requests.


<!-- cairn:scenario-links:start -->
#### Scenario: DX-12 for DX-DOC-01

- GIVEN the `Documentation-Test-Design` profile and every field of `input` in [DX-12](../../../specs/conformance/language-workflow-cases.json)
- WHEN the `review` procedure for case `DX-12` runs against those inputs
- THEN the observations match every field of `expected` in case `DX-12`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: DX-DOC-02
r[DX-DOC-02]

**DX-DOC-02.** Published examples MUST distinguish illustrative or unexecuted text from executed evidence. Reports MUST retain failures, unsupported examples, and timeouts rather than omitting them from coverage. Documentation checks MUST NOT substitute rendered text matching for compiler execution or represent passing examples as proofs.

Expectations may cover stack shapes, values, effects, or diagnostics. A static check does not establish runtime behavior. Effectful examples require explicit test-host mappings and budgets; documentation processing must not run them with ambient credentials. Example extraction syntax remains open. The actual compiler executed the separately recorded M4 DX-12 selected examples; illustrative or unsupported examples remain unexecuted.


<!-- cairn:scenario-links:start -->
#### Scenario: DX-12 for DX-DOC-02

- GIVEN the `Documentation-Test-Design` profile and every field of `input` in [DX-12](../../../specs/conformance/language-workflow-cases.json)
- WHEN the `review` procedure for case `DX-12` runs against those inputs
- THEN the observations match every field of `expected` in case `DX-12`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

## First AI-authoring reference application

[CALCULATOR.md](../calculator/spec.md) selects the exact programmable calculator and its independent acceptance criteria. CALC-AI-01 through CALC-AI-04 add bounded change tasks, comparable authoring routes, structured compiler feedback, and rejection of stale edits.

The benchmark compares stack-only source, local names, and structured edits only after their respective gates close. It does not presume that a smaller grammar improves AI correctness. Calculator execution does not close a language proof obligation.

## Delivery and evidence

| Slice | Entry gate | Acceptance evidence |
|---|---|---|
| Diagnostics | M2 checker interfaces | Stack mismatch, branch mismatch, and resource-duplication fixtures |
| Editor holes | M2; editor syntax contract | Constraint reports and rejection through all admission paths |
| Domain types and Result library | After M4; declaration/module and library interfaces | Identity isolation, exhaustive elimination, branch behavior, and ownership negatives |
| Identity tooling | Resolved recipes; canonical encoding for portable keys | Semantic/build-only diffs and stale-cache rejection |
| Test hosts | M2 test environment; M5 for live resources | Scripted success, denial, mismatch, missing mapping, and exhaustion |

### Second-group delivery gates

| Slice | Entry gate | Acceptance evidence |
|---|---|---|
| Local names | After M4; syntax, lexical scope, and lowering design | Stack-only comparison, capture identity, and ownership negatives |
| Capability-aware modules | After M4; module interface and dependency design | Explicit linking, no import execution, missing/incompatible bindings, and denied use |
| Property testing | M2 acceptance; M4 for Wasm properties | Reproducible trials, independent acceptance, bounded shrinking, and failing oracle controls |
| Resource protocol types | M5 and declared type/module interfaces | Legal transitions, invalid reuse, error ownership, and runtime correspondence; host models for proof claims |
| Executable documentation | M2 for static examples; M4 for runtime examples | Actual compiler/backend observations and visible non-passing outcomes |

Property and documentation harnesses accompany the applicable M2/M4 acceptance work. Later surface features do not block bootstrap or require a new milestone dependency.

[Developer-experience scenarios](../../../specs/conformance/developer-experience-cases.json) and [language-workflow scenarios](../../../specs/conformance/language-workflow-cases.json) retain their exact expected inputs and case-specific evidence. DX-03, DX-08 and DX-09 pass only the selected [source-bound DXM1 receipt](../../../verification/declared-modules-v1/acceptance.json): nine static rows, twelve off/on zero-execution linker rows and two real compiled-Wasm denial rows. The accepted M4 DX-10/12 workflows retain their own evidence. Other designs and all new nominal/module refinement proofs remain open. New semantic functions inherit the Aeneas-first inventory and proof requirements.

No slice closes its gate through document validation alone. Dependent types, general effect handlers, macros, and replacement concurrency semantics remain unselected.
