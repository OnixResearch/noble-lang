# Verification Toolchain and Implementation Boundaries

<!-- cairn:purpose:start -->
## Purpose

This accepted specification records Noble draft contracts, not completed implementation.
Original requirement IDs, explanatory prose, examples, and open decisions remain authoritative.
Scenario clauses declare designs; the conformance ledger records execution and evidence.

## Requirements

<!-- cairn:purpose:end -->


Document: IMPL-V001  
Revision: 0.1.0-draft.5  
Status: Canonical tool policy with implemented MC1 consumer workflow; executed scopes and assumptions recorded separately  
Depends on: [SPEC-V001](../verification/spec.md) at 0.1.0-draft.5

[SOURCES.md](../../../specs/SOURCES.md) marks inherited external citation labels as historical, not fresh compatibility evidence.

MC1 has a concrete isolated application-proof workflow and a separate actual-source correspondence lane; neither establishes whole-project verification.

## 1. Selected roles

### Requirement: VT-ROLE-01
r[VT-ROLE-01]

**VT-ROLE-01.** Noble MUST use Lean 4 for its reference semantics, metatheory, and reusable program-contract logic. Noble-owned production Rust MUST target Charon → Aeneas → Lean 4 under the scope rules below. This route is mandatory for the semantic kernel, not merely preferred for selected checker utilities. Verus is an optional exception tool, not a second default implementation language.

Aeneas is a Rust verification toolchain, not the language in which Noble source or proofs are written. Production implementation code remains Rust. Reference definitions and refinement proofs remain Lean. Build configuration and documentation retain their appropriate formats.

| Component or task | Primary route | Required evidence or boundary |
|---|---|---|
| Reference semantics, metatheory, optional program contracts | Handwritten Lean 4 | Reviewed statements and kernel-checked proofs |
| Entire semantic kernel, including checking, values, builders, recipes, effects, and ownership | Safe sequential Rust → Charon → Aeneas → Lean 4 | Actual generated functions refine the reference contracts |
| Parser, resolver, inference, IR transformations, optimizers, and Wasm emission | Same Rust-to-Lean route | Staged function contracts; separate execution/reflection preservation theorems |
| Resource-table decisions, admission, loading decisions, caches, and runtime state transitions | Same Rust-to-Lean route | Deterministic transitions with explicit state and typed effect plans |
| CLI, storage, native allocation, FFI, Wasmtime, callbacks, and synchronization | Extractable Rust logic plus narrow external adapters | Concrete effect and synchronization contracts; reviewed exceptions for unsupported bodies |

The [Aeneas projects page](https://aeneasverif.github.io/projects/) recommends the Lean backend. The [upstream README](https://github.com/AeneasVerif/aeneas#targeted-subset-and-current-limitations) identifies unsafe code and concurrency as current limitations. These sources establish the tool's stated scope, not compatibility or refinement for every Noble function. Exact selected pins and executed source/configuration scopes require the project records below.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### 1.1 Whole-project scope and kernel priority

### Requirement: VT-SCOPE-01
r[VT-SCOPE-01]

**VT-SCOPE-01.** All Noble-owned production Rust MUST appear in the verification inventory, including compiler frontends, backends, runtime, adapters, and CLI logic. New code MUST target the selected Aeneas subset from its first implementation. Delivery order MUST NOT become a permanent exclusion of non-kernel code. Non-Rust tools, generated code, and external dependencies MUST have explicit scope classifications rather than disappear from the assurance report.


<!-- cairn:scenario-links:start -->
#### Scenario: VERIFY-01 for VT-SCOPE-01

- GIVEN the `Implementation-Policy` profile and every field of `input` in [VERIFY-01](../../../specs/conformance/verification-cases.json)
- WHEN the `review` procedure for case `VERIFY-01` runs against those inputs
- THEN the observations match every field of `expected` in case `VERIFY-01`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: VERIFY-02 for VT-SCOPE-01

- GIVEN the `Implementation-Policy` profile and every field of `input` in [VERIFY-02](../../../specs/conformance/verification-cases.json)
- WHEN the `review` procedure for case `VERIFY-02` runs against those inputs
- THEN the observations match every field of `expected` in case `VERIFY-02`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: VT-SCOPE-02
r[VT-SCOPE-02]

**VT-SCOPE-02.** The entire semantic kernel MUST use safe, sequential, extractable Rust. Kernel scope includes types, substitutions, acceptance, eligibility, effects, stack/value operations, program builders, recipes, identity rules, and ownership transitions. Semantic admission and resource-table decisions also belong in this scope, regardless of their crate location. Kernel code MUST NOT use unsafe code, ambient I/O, hidden mutable state, or concurrency. An unsupported kernel function MUST block the affected verification gate until redesign or supported extraction. Moving that function into an adapter MUST NOT exempt its semantic decision.

Local mutation of explicit owned state is permitted within the supported subset. The reference model can describe concurrency as transitions over explicit state and event inputs. This does not establish correctness of a concurrent Rust executor.


<!-- cairn:scenario-links:start -->
#### Scenario: VERIFY-03 for VT-SCOPE-02

- GIVEN the `Implementation-Policy` profile and every field of `input` in [VERIFY-03](../../../specs/conformance/verification-cases.json)
- WHEN the `review` procedure for case `VERIFY-03` runs against those inputs
- THEN the observations match every field of `expected` in case `VERIFY-03`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: VT-SCOPE-03
r[VT-SCOPE-03]

**VT-SCOPE-03.** Shells MUST obtain external facts and execute typed effect plans from the deterministic core. Plans MUST NOT count as evidence that effects occurred. Host calls, FFI, callbacks, synchronization, and dependency models require separate concrete correspondence contracts. Resource retirement decisions MUST remain separate from physical release and remote cleanup.


<!-- cairn:scenario-links:start -->
#### Scenario: VERIFY-03 for VT-SCOPE-03

- GIVEN the `Implementation-Policy` profile and every field of `input` in [VERIFY-03](../../../specs/conformance/verification-cases.json)
- WHEN the `review` procedure for case `VERIFY-03` runs against those inputs
- THEN the observations match every field of `expected` in case `VERIFY-03`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: VERIFY-04 for VT-SCOPE-03

- GIVEN the `Implementation-Policy` profile and every field of `input` in [VERIFY-04](../../../specs/conformance/verification-cases.json)
- WHEN the `review` procedure for case `VERIFY-04` runs against those inputs
- THEN the observations match every field of `expected` in case `VERIFY-04`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: VT-SCOPE-04
r[VT-SCOPE-04]

**VT-SCOPE-04.** An unsupported non-kernel boundary requires a reviewed exception before inclusion in a claimed verification scope. Each exception MUST name exact symbols, source/configuration identity, an owner, and the tool diagnostic or external-effect constraint. It MUST record the abstract contract, assumptions, evidence, and a removal or reassessment condition. Whole-crate exclusions and convenience-only exceptions MUST NOT replace function-level accounting. Verus requires such an exception and the section 3 controls. No exception makes its body Aeneas-verified or discharges an open correspondence obligation.


<!-- cairn:scenario-links:start -->
#### Scenario: VERIFY-03 for VT-SCOPE-04

- GIVEN the `Implementation-Policy` profile and every field of `input` in [VERIFY-03](../../../specs/conformance/verification-cases.json)
- WHEN the `review` procedure for case `VERIFY-03` runs against those inputs
- THEN the observations match every field of `expected` in case `VERIFY-03`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: VERIFY-04 for VT-SCOPE-04

- GIVEN the `Implementation-Policy` profile and every field of `input` in [VERIFY-04](../../../specs/conformance/verification-cases.json)
- WHEN the `review` procedure for case `VERIFY-04` runs against those inputs
- THEN the observations match every field of `expected` in case `VERIFY-04`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: VT-SCOPE-05
r[VT-SCOPE-05]

**VT-SCOPE-05.** Each release inventory MUST map the complete production source set to extracted functions, external models, reviewed exceptions, or explicit open work. It MUST include dependency closure, macro/generated bodies, feature/target configurations, linked contracts, theorem identifiers, and independent extraction/proof statuses. Required verification gates MUST reject omissions, stale mappings, unapproved exceptions, and unresolved obligations in their claimed scope. Reports MUST show total, extracted, proved, modeled, excepted, and open counts with named scopes. Successful extraction alone MUST NOT count as a refinement proof.

Each extraction/refinement experiment covers only its recorded source and claim scope. MC1's actual-source extraction does not establish universal frontend correctness, whole-kernel correctness, or whole-project verification. Draft code can retain open proofs, but no stable verified subset can include unresolved required claims.


<!-- cairn:scenario-links:start -->
#### Scenario: VERIFY-01 for VT-SCOPE-05

- GIVEN the `Implementation-Policy` profile and every field of `input` in [VERIFY-01](../../../specs/conformance/verification-cases.json)
- WHEN the `review` procedure for case `VERIFY-01` runs against those inputs
- THEN the observations match every field of `expected` in case `VERIFY-01`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: VERIFY-02 for VT-SCOPE-05

- GIVEN the `Implementation-Policy` profile and every field of `input` in [VERIFY-02](../../../specs/conformance/verification-cases.json)
- WHEN the `review` procedure for case `VERIFY-02` runs against those inputs
- THEN the observations match every field of `expected` in case `VERIFY-02`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: VERIFY-05 for VT-SCOPE-05

- GIVEN the `Implementation-Policy` profile and every field of `input` in [VERIFY-05](../../../specs/conformance/verification-cases.json)
- WHEN the `review` procedure for case `VERIFY-05` runs against those inputs
- THEN the observations match every field of `expected` in case `VERIFY-05`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: VT-ROLE-02
r[VT-ROLE-02]

**VT-ROLE-02.** Each component MUST have one declared primary verification route. Dual verification MAY be used for selected boundaries, but MUST NOT be required by default. No tool result may silently substitute for a result in another logic.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: VT-ROLE-03
r[VT-ROLE-03]

**VT-ROLE-03.** Noble MUST NOT select Eurydice or Scylla as dependencies in this revision. Their Rust-to-C and C-to-Rust roles do not supply the selected Noble-to-Wasm backend. Charon and Aeneas are verification/build-time dependencies, not guest execution machinery. [R1]


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

## 2. The Rust-to-Lean chain

```text
exact ordinary-Rust crate, dependencies, features and target configuration
    -> rustc front end / MIR
    -> Charon extraction and LLBC
    -> Aeneas functional translation
    -> generated Lean functions + explicitly inventoried external models
    -> handwritten refinement proofs
    -> reviewed Noble reference definitions
```

### Requirement: VT-AENEAS-01
r[VT-AENEAS-01]

**VT-AENEAS-01.** Every component assigned to the Aeneas route MUST stay within the experimentally confirmed subset of the pinned Charon/Aeneas pair. The starting policy requires safe sequential Rust, explicit owned data, simple control flow, and narrow modeled dependencies. Safe Rust alone does not establish extractability.

The inspected upstream README excludes some nested-loop control flow and generic instantiation with mutable references. Exact support requires an extraction run under compatible pins. Extraction remains a trust boundary, not an assumed proved compiler. [Current source record](../../../specs/SOURCES.md#aeneas-first-direction-in-draft4)


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: VT-AENEAS-02
r[VT-AENEAS-02]

**VT-AENEAS-02.** Unsupported constructs MUST fail the relevant verification lane. Kernel functions require redesign or supported extraction under VT-SCOPE-02. Only non-kernel boundaries can use the exception process in VT-SCOPE-04. Unsupported bodies MUST NOT disappear from coverage or become unconstrained success or axioms that assert the desired result. A separately proved abstraction requires explicit concrete correspondence.


<!-- cairn:scenario-links:start -->
#### Scenario: VERIFY-03 for VT-AENEAS-02

- GIVEN the `Implementation-Policy` profile and every field of `input` in [VERIFY-03](../../../specs/conformance/verification-cases.json)
- WHEN the `review` procedure for case `VERIFY-03` runs against those inputs
- THEN the observations match every field of `expected` in case `VERIFY-03`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: VT-AENEAS-03
r[VT-AENEAS-03]

**VT-AENEAS-03.** Generated Lean files MUST be regenerated from the claimed Rust revision and kept separate from handwritten specifications and proofs. Editing generated functions to make proofs pass invalidates the Rust correspondence claim. Generation commands, source/configuration digests, generated-file digests, and extraction diagnostics MUST be retained.


<!-- cairn:scenario-links:start -->
#### Scenario: VERIFY-01 for VT-AENEAS-03

- GIVEN the `Implementation-Policy` profile and every field of `input` in [VERIFY-01](../../../specs/conformance/verification-cases.json)
- WHEN the `review` procedure for case `VERIFY-01` runs against those inputs
- THEN the observations match every field of `expected` in case `VERIFY-01`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: VT-AENEAS-04
r[VT-AENEAS-04]

**VT-AENEAS-04.** The refinement theorem MUST connect actual generated functions to the intended component contract. Merely compiling the generated Lean module, or proving properties of a separately rewritten reference function, is insufficient. The theorem MUST account for all modeled return alternatives, failure, and the relevant representation invariants.


<!-- cairn:scenario-links:start -->
#### Scenario: VERIFY-01 for VT-AENEAS-04

- GIVEN the `Implementation-Policy` profile and every field of `input` in [VERIFY-01](../../../specs/conformance/verification-cases.json)
- WHEN the `review` procedure for case `VERIFY-01` runs against those inputs
- THEN the observations match every field of `expected` in case `VERIFY-01`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: VERIFY-05 for VT-AENEAS-04

- GIVEN the `Implementation-Policy` profile and every field of `input` in [VERIFY-05](../../../specs/conformance/verification-cases.json)
- WHEN the `review` procedure for case `VERIFY-05` runs against those inputs
- THEN the observations match every field of `expected` in case `VERIFY-05`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: VT-AENEAS-05
r[VT-AENEAS-05]

**VT-AENEAS-05.** Every external model, opaque function, modeled standard-library operation, panic behavior, and admitted translation boundary MUST appear in the assumption register with its concrete target. A definition implemented in Lean is not automatically a proof that the corresponding Rust library behaves that way. Generated templates with missing obligations MUST NOT count as verified implementation.


<!-- cairn:scenario-links:start -->
#### Scenario: VERIFY-04 for VT-AENEAS-05

- GIVEN the `Implementation-Policy` profile and every field of `input` in [VERIFY-04](../../../specs/conformance/verification-cases.json)
- WHEN the `review` procedure for case `VERIFY-04` runs against those inputs
- THEN the observations match every field of `expected` in case `VERIFY-04`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: VERIFY-05 for VT-AENEAS-05

- GIVEN the `Implementation-Policy` profile and every field of `input` in [VERIFY-05](../../../specs/conformance/verification-cases.json)
- WHEN the `review` procedure for case `VERIFY-05` runs against those inputs
- THEN the observations match every field of `expected` in case `VERIFY-05`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: VT-AENEAS-06
r[VT-AENEAS-06]

**VT-AENEAS-06.** Reports using this route MUST retain rustc/MIR extraction, Charon, Aeneas, and external-model fidelity as trusted components unless separate evidence discharges the exact boundary. The Lean kernel checks the resulting theorem, not the faithfulness of the extraction process. Published research about an algorithm is not proof that every current tool binary implements it correctly.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: VT-AENEAS-07
r[VT-AENEAS-07]

**VT-AENEAS-07.** A theorem asserting success or termination MUST distinguish translation-level failure or divergence from Noble domain-error values. Aeneas's ability to represent partial functions MUST NOT be reported as an automatic termination proof. [R2]

The production checker remains Rust. Lean's definitions or an executable reference evaluator may be used as test oracles; they do not become a second Noble production VM.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

## 3. Optional Verus exception lane

The VT-VERUS requirements apply only when a reviewed VT-SCOPE-04 exception selects Verus. No component currently has such an exception. These retained requirement IDs do not require a Verus dependency or a parallel kernel implementation.

VT-WRAP and VT-NATIVE remain mandatory at applicable boundaries, regardless of proof tool selection.

### Requirement: VT-VERUS-01
r[VT-VERUS-01]

**VT-VERUS-01.** When an exception selects Verus, it MUST verify exact executable functions against explicit specification functions and invariants. Proof and ghost state MUST NOT change Noble-visible behavior or be assumed to exist at runtime. The compiled code/configuration must match the one whose verification conditions were checked.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: VT-VERUS-02
r[VT-VERUS-02]

**VT-VERUS-02.** If a resource adapter uses Verus, its specification MUST cover owner context, resource kind, liveness, transfer, retirement, and invalid-handle rejection. Invalid handles include stale, forged, wrongly typed, and cross-context handles. Pure handle-table transitions remain on the mandatory Aeneas route under VT-SCOPE-02. The representation may use generations or another reviewed stale-reference defense; this document does not freeze an ABI.

Normal successful calls and ordinary error returns must preserve each contract's disposition of ownership. Repeated retirement must not cause repeated local release or make a retired handle live again. OS effects and remote cleanup remain separate adapter contracts.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: VT-VERUS-03
r[VT-VERUS-03]

**VT-VERUS-03.** Each verified component MUST inventory `assume`, admitted proof bodies, external bodies/specifications, ignored code, unsafe implementations, and other mechanisms adding trust. Inventory MUST include relevant library and macro-expanded dependencies, not just text in the project's top-level file. Unexplained additions MUST fail assurance CI.

Verus documents several mechanisms for trusting code or specifications without checking bodies. In this profile those mechanisms are permitted only at named, reviewed boundaries; they do not discharge the boundary's obligation. [R7, R8]


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: VT-VERUS-04
r[VT-VERUS-04]

**VT-VERUS-04.** A Verus success report MUST NOT be represented as a Lean proof certificate. The trust inventory MUST include the selected Verus implementation, relevant specification library, verification-condition translation, SMT solver, and external specifications. A future independently checked proof interchange mechanism requires a separate specification and validation before changing this policy.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: VT-VERUS-05
r[VT-VERUS-05]

**VT-VERUS-05.** Termination or total-correctness claims MUST have explicit evidence covering the claimed call graph and host assumptions. A decreases annotation on an executable function alone is not sufficient. The Verus guide describes its executable termination checks as conditional on callees terminating. [R9]


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### 3.1 Public wrappers are a security boundary

### Requirement: VT-WRAP-01
r[VT-WRAP-01]

**VT-WRAP-01.** Functions called from unverified Rust, FFI, Wasm, or a host callback MUST validate all preconditions not guaranteed by an established caller invariant. Verification-only preconditions MUST NOT be used to remove checks on adversarial inputs at a public boundary. A wrapper SHALL either reject safely or establish the verified internal function's full entry contract.

This matters because Verus's safety argument can depend on verified callers meeting preconditions; ordinary Rust type-checking is not the same assurance. [R10]


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: VT-WRAP-02
r[VT-WRAP-02]

**VT-WRAP-02.** Guest handles and imported values MUST be validated on their concrete representation and context. A proof attached to a program does not make arbitrary integers valid handles or grant host access. Concurrency, callbacks, and reentrancy MUST be excluded explicitly or covered by the invariant and wrapper protocol; a sequential proof does not automatically survive reentrant mutation.


<!-- cairn:scenario-links:start -->
#### Scenario: VERIFY-04 for VT-WRAP-02

- GIVEN the `Implementation-Policy` profile and every field of `input` in [VERIFY-04](../../../specs/conformance/verification-cases.json)
- WHEN the `review` procedure for case `VERIFY-04` runs against those inputs
- THEN the observations match every field of `expected` in case `VERIFY-04`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: VT-WRAP-03
r[VT-WRAP-03]

**VT-WRAP-03.** Unverified callers MUST NOT be able to manufacture a privileged internal state by invoking an otherwise safe-looking constructor or mutator. Entry, exit, and construction boundaries are part of the verification scope.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### 3.2 Native boundary review and regression checks

### Requirement: VT-NATIVE-01
r[VT-NATIVE-01]

**VT-NATIVE-01.** Every Noble-owned, non-test unsafe block or unsafe implementation MUST have a safety argument for its entry conditions and lifetime obligations. The argument cites the relevant stable Rust contract and retained wording. Undocumented assumptions require an explicit owner, review record, and scope in the trust inventory.

The inventory includes native callbacks, FFI, dependencies, and derive-generated code at the claimed boundary. A comment-presence lint is not a proof. No safe Rust wrapper or derived trait can substitute for missing semantic acceptance.


<!-- cairn:scenario-links:start -->
#### Scenario: ADAPT-13 for VT-NATIVE-01

- GIVEN the `Implementation-Policy` profile and every field of `input` in [ADAPT-13](../../../specs/conformance/adaptation-cases.json)
- WHEN the `review` procedure for case `ADAPT-13` runs against those inputs
- THEN the observations match every field of `expected` in case `ADAPT-13`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: VT-NATIVE-02
r[VT-NATIVE-02]

**VT-NATIVE-02.** Native representation boundaries MUST have positive and negative tests for applicable alignment, validity, bounds, lifetime, and ownership conditions. The assurance plan MUST include scoped Miri runs with pinned targets and memory-model configuration. Unsupported runs MUST remain explicit and MUST NOT satisfy a required lane.

Compile-fail tests cover forbidden construction and escaping views where Rust can reject them. Runtime tests cover malformed input and state transitions. Miri does not establish Noble-to-Wasm correspondence or cover an excluded target.


<!-- cairn:scenario-links:start -->
#### Scenario: ADAPT-04 for VT-NATIVE-02

- GIVEN the `Decoder-Experiment` profile and every field of `input` in [ADAPT-04](../../../specs/conformance/adaptation-cases.json)
- WHEN the `adapter` procedure for case `ADAPT-04` runs against those inputs
- THEN the observations match every field of `expected` in case `ADAPT-04`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: ADAPT-05 for VT-NATIVE-02

- GIVEN the `Decoder-Experiment` profile and every field of `input` in [ADAPT-05](../../../specs/conformance/adaptation-cases.json)
- WHEN the `adapter` procedure for case `ADAPT-05` runs against those inputs
- THEN the observations match every field of `expected` in case `ADAPT-05`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: ADAPT-13 for VT-NATIVE-02

- GIVEN the `Implementation-Policy` profile and every field of `input` in [ADAPT-13](../../../specs/conformance/adaptation-cases.json)
- WHEN the `review` procedure for case `ADAPT-13` runs against those inputs
- THEN the observations match every field of `expected` in case `ADAPT-13`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: ADAPT-16 for VT-NATIVE-02

- GIVEN the `Implementation-Policy` profile and every field of `input` in [ADAPT-16](../../../specs/conformance/adaptation-cases.json)
- WHEN the `review` procedure for case `ADAPT-16` runs against those inputs
- THEN the observations match every field of `expected` in case `ADAPT-16`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: VT-NATIVE-03
r[VT-NATIVE-03]

**VT-NATIVE-03.** Dependency adoption MUST record the exact version, features, macros, applicable target assumptions, and verification boundary. A byte-view dependency MUST remain outside the initial owned-data acceptance core unless extraction compatibility and its concrete boundary are established.

Zerocopy remains a candidate for a concrete decoding adapter. Its upstream Miri/Kani results do not discharge Noble obligations. Kani is not a mandatory primary proof route. Dependency adoption must preserve the Aeneas-first scope and exception policy.


<!-- cairn:scenario-links:start -->
#### Scenario: ADAPT-04 for VT-NATIVE-03

- GIVEN the `Decoder-Experiment` profile and every field of `input` in [ADAPT-04](../../../specs/conformance/adaptation-cases.json)
- WHEN the `adapter` procedure for case `ADAPT-04` runs against those inputs
- THEN the observations match every field of `expected` in case `ADAPT-04`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: ADAPT-13 for VT-NATIVE-03

- GIVEN the `Implementation-Policy` profile and every field of `input` in [ADAPT-13](../../../specs/conformance/adaptation-cases.json)
- WHEN the `review` procedure for case `ADAPT-13` runs against those inputs
- THEN the observations match every field of `expected` in case `ADAPT-13`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

## 4. Cross-boundary contracts

### Requirement: VT-BRIDGE-01
r[VT-BRIDGE-01]

**VT-BRIDGE-01.** Every external model, shell adapter, or approved alternate verification lane MUST publish its boundary contract with the Lean/Aeneas core. The contract MUST specify the concrete representation relation, argument/result ordering, integer widths, error alternatives, ownership transitions, and allowed effects. Both sides MUST name the same contract revision.

For a resource adapter, the shared contract separates abstract handle obligations from concrete host-object access. A transition proof does not establish the adapter's physical release or synchronization behavior.


<!-- cairn:scenario-links:start -->
#### Scenario: VERIFY-04 for VT-BRIDGE-01

- GIVEN the `Implementation-Policy` profile and every field of `input` in [VERIFY-04](../../../specs/conformance/verification-cases.json)
- WHEN the `review` procedure for case `VERIFY-04` runs against those inputs
- THEN the observations match every field of `expected` in case `VERIFY-04`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: VT-BRIDGE-02
r[VT-BRIDGE-02]

**VT-BRIDGE-02.** Each correspondence edge SHALL have a status: `proved`, `validated`, `reviewed-assumption`, or `open`. Matching names, copied comments, matching digests, or passing examples alone do not make an edge `proved`. A report MUST identify the proof system and theorem for a proved edge and the exact validator for a validated edge.

A mixed-tool result may legitimately rely on a reviewed-assumption bridge. It must say so. A Lean theorem using a host-contract hypothesis is conditional on that hypothesis; a separate Verus result does not discharge it inside Lean without an explicit justified connection.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: VT-BRIDGE-03
r[VT-BRIDGE-03]

**VT-BRIDGE-03.** The project MUST NOT assume Verus-annotated source can pass through Charon/Aeneas unchanged. Components SHOULD meet through ordinary, narrow Rust interfaces. Any generated or annotation-erased source used in another lane requires its own source-to-source correspondence and configuration record.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: VT-BRIDGE-04
r[VT-BRIDGE-04]

**VT-BRIDGE-04.** Arithmetic bridges MUST preserve wrapping operations, signed interpretation, checked literal ranges, and architecture-dependent index widths. Mathematical `int` or `nat` values MUST NOT replace executable fixed-width semantics without a proof of the representation relation.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

## 5. Lean acceptance policy

### Requirement: VT-LEAN-01
r[VT-LEAN-01]

**VT-LEAN-01.** Release-gating core theorems MUST be kernel checked under a pinned Lean 4 toolchain, with a transitive axiom inventory. The default allowed logical foundations are Lean's standard `propext`, `Classical.choice`, and `Quot.sound`; use of a smaller set is acceptable. `sorryAx`, unfinished proofs, and axioms that simply assert Noble's desired guarantees MUST fail acceptance.

Host behavior, well-formed environments, and external state laws SHOULD be explicit theorem parameters rather than hidden global axioms. Their concrete satisfaction remains an assurance obligation.


<!-- cairn:scenario-links:start -->
#### Scenario: VERIFY-05 for VT-LEAN-01

- GIVEN the `Implementation-Policy` profile and every field of `input` in [VERIFY-05](../../../specs/conformance/verification-cases.json)
- WHEN the `review` procedure for case `VERIFY-05` runs against those inputs
- THEN the observations match every field of `expected` in case `VERIFY-05`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: VT-LEAN-02
r[VT-LEAN-02]

**VT-LEAN-02.** Native-evaluation shortcuts or other mechanisms extending the default kernel trust policy MUST NOT be used for strict core release gates. An expanded-trust experimental lane MAY use them, but MUST identify the added assumptions and MUST NOT masquerade as the strict lane. Exact detection MUST follow the pinned toolchain, not a permanently hard-coded list of one version's mechanism names.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: VT-LEAN-03
r[VT-LEAN-03]

**VT-LEAN-03.** CI MUST rebuild theorem dependencies from the selected sources and perform a fresh proof recheck, using `lean4checker --fresh` where supported by the pinned toolchain or an explicitly reviewed equivalent. Axiom inspection and rechecking supplement, not replace, review of the intended statement and its definitions. [R4]


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### 5.1 Untrusted and generated proof submissions

### Requirement: VT-SANDBOX-01
r[VT-SANDBOX-01]

**VT-SANDBOX-01.** Unreviewed proof source, tactics, macros, dependencies, and build scripts SHALL be treated as executable untrusted input. Elaboration/build MUST run in an isolated worker without ambient credentials, unrestricted network access, production code-store write access, or authority to change the expected claim. Resource budgets MUST be enforced.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: VT-SANDBOX-02
r[VT-SANDBOX-02]

**VT-SANDBOX-02.** Acceptance MUST compare the proved statement and referenced definitions against a trusted, independently selected claim. It MUST validate the evidence representation and recheck the proof outside the influence of the producer's build process. Untrusted compiled proof objects MUST NOT simply be deserialized into a privileged verifier process.

MC1 implements this boundary for application proofs through [`noble verify`](../../../crates/noble-cli/src/workflow.rs). The consumer independently prepares the accepted subject and expected claim, snapshots the selected rule-library sources, and rebuilds them without producer caches. Untrusted proof source is elaborated in isolation. A separate untrusted exporter may read its compiled artifacts, but only bounded inert `noble-mc1-proof/v1` JSON declarations cross into the [fresh consumer](../../../crates/noble-cli/src/consumer). The trusted consumer never imports or deserializes a producer `.olean`.

The consumer validates the wire shape and bounds, rejects duplicate/trusted-name replacement, missing or cyclic dependencies, and declarations outside the selected theorem's dependency closure, reconstructs safe Lean declarations, and kernel-checks them in a fresh environment. It compares the theorem type with the exact consumer-selected `MC1Obligation.claim` or its negation and inventories transitive axioms from raw declaration types/bodies. A producer's status text, weakened statement, or renamed expected definition cannot substitute for those checks. This is the MC1 implementation of the independent-checking boundary, not a claim that the CLI, OS isolation, decoder, Lean implementation, or every dependency has itself been universally verified. Lean's validation guidance motivates challenge/solution comparison and independent checking for this threat model. [R4]


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: VT-SANDBOX-03
r[VT-SANDBOX-03]

**VT-SANDBOX-03.** Proof-generation changes to source, preconditions, postconditions, definitions, trusted boundaries, or proof-checker settings MUST be independently reviewed. A stronger precondition, weaker conclusion, or circular assumption can make a valid proof useless for the requested claim.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### 5.2 MC1 consumer configuration and limits

The concrete commands and seven outcomes are specified in [SPEC-V002 section 12](../program-contracts/spec.md#12-verification-tooling); the [README](../../../README.md#using-mc1-contracts) gives build and invocation examples. The binary is `noble`, built by package `noble-cli`. `explain-proof` and verification without evidence prepare/export statements but launch no proof tools. Actual proof/refutation checking requires Linux and the following consumer-selected configuration:

| Setting | Required meaning |
|---|---|
| `NOBLE_LEAN` | Absolute path to the real release installation's `bin/lean`, with `lib/lean/Init.olean` available; not an elan shim |
| `NOBLE_CONTRACT_LIBRARY` | Absolute source directory containing `lean-toolchain`, `NobleContracts.lean`, and its `NobleContracts/` modules; default is the build-time repository's `proofs/mc1` |
| `NOBLE_BWRAP` | Absolute bubblewrap executable supporting the required namespace/mount isolation |
| `NOBLE_PRLIMIT` | Absolute `prlimit` executable for file, descriptor, core, and CPU limits |
| `NOBLE_SYSTEMD_RUN` | Absolute `systemd-run` executable; the worker runs in a user scope |
| `XDG_RUNTIME_DIR` | Active user runtime directory with a usable `bus` and user systemd manager capable of applying the scope limits |
| `NOBLE_NIX_STORE` | Absolute `nix-store` executable when Lean resides in `/nix/store`, used to select only its runtime closure |

Executable overrides can be omitted when the tools are present in the implementation's conventional system locations. Bubblewrap, `prlimit`, and `systemd-run` are discovered under `/run/current-system/sw/bin`, `/usr/bin`, or `/bin`; Lean is discovered under `/run/current-system/sw/bin`, `/usr/local/bin`, or `/usr/bin`. Nix-store discovery also considers `/nix/var/nix/profiles/default/bin`. Selecting executable/library paths is a consumer trust decision, not permission for the proof producer to choose its acceptance policy. Library source symlinks are rejected.

The library pin must be `leanprover/lean4:v4.31.0`. Before any proof source runs, a sandboxed version probe must report Lean 4.31.0, commit `68218e876d2a38b1985b8590fff244a83c321783`. Missing tools, wrong pins, invalid layouts, unavailable user scopes, or failed isolation probes fail closed with `unsupported`; there is no unsandboxed fallback. The repository development shell does not itself create a working user systemd session or enable namespace support.

Each proof worker uses bubblewrap with all namespaces unshared, no network, cleared environment, no ambient credentials, read-only source/library/claim inputs, and no writable host directories. Only preselected artifact files are writable; temporary storage is private. Nix installations expose the selected toolchain closure rather than the whole store or project. The user systemd scope applies aggregate worker/descendant memory and task limits; `prlimit` adds per-process limits. These are host-enforced checking budgets, not guest runtime or termination proofs.

| Budget | MC1 limit |
|---|---|
| Frontend source | 65,536 bytes; other preparation limits in [SPEC-V002 section 8.1](../program-contracts/spec.md#81-delivered-mc1-source-and-ir) |
| Submitted proof source | 524,288 bytes, UTF-8 |
| Selected rule library | 4,194,304 source bytes and 128 Lean modules, with bounded tree traversal |
| Proof JSON | 8,388,608 bytes; 1–4096 declarations |
| Wire expressions/universes | 262,144 nodes; nesting depth 512; bounded uint32 indices |
| Transitive declaration audit | 200,000 declarations |
| Total proof wall-clock budget | Default 120000 ms; `--timeout-ms` accepts 1–600000 ms, including library rebuild and independent recheck |
| Worker scope | 2 GiB aggregate memory, no swap, 64 tasks |
| Process output/artifacts | 128 KiB per stdout/stderr stream; 32 MiB per artifact file; 128 file descriptors; no core dumps |
| Private temporary filesystems | 64 MiB `/tmp`, 1 MiB `/dev/shm` |

CPU limits derive from the remaining wall-clock budget. Time exhaustion reports `timeout`; other budget/recheck failures report their diagnostic `error` or `unsupported`, never proof success. Proof elaboration failure is not disproof. Only a fresh accepted `MC1Proof.refutation : Not MC1Obligation.claim` produces `disproved`. The report retains the selected tool identities, library source snapshot, expected statement, accepted assumptions, and separate implementation/backend status. See [CLI acceptance records](../../../verification/mc1/acceptance.json) for executed controls; the existence of this configuration does not itself prove that a host run passed them.

## 6. Toolchain pinning and reproducibility

### Requirement: VT-PIN-01
r[VT-PIN-01]

**VT-PIN-01.** Every executed assurance run MUST identify immutable revisions and artifact digests for its tools and dependencies. Floating `main`, `latest`, unspecified rustc channels, or unrecorded solver versions MUST NOT be release evidence.

The required pin set covers Lean/Lake, proof libraries, Charon, extraction rustc, Aeneas, its Lean backend, and the relevant production Rust/Wasm profile. An approved Verus exception additionally requires its Verus, rustc, specification library, and SMT solver pins. Different lanes can require different rustc versions. Compatibility requires executed evidence.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: VT-PIN-02
r[VT-PIN-02]

**VT-PIN-02.** The Charon revision and Lean backend configuration MUST match the selected Aeneas revision's compatibility requirements. Aeneas's README explicitly ties these dependencies together. A local symlink or manual tool override MUST NOT bypass the package's own pin checks. [R2]


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: VT-PIN-03
r[VT-PIN-03]

**VT-PIN-03.** The tracked configuration MUST include target triple, word size, Cargo features, dependency lock, relevant `cfg` flags, panic strategy, overflow semantics, generation options, solver options, and timeout/resource limits. Configuration changes invalidate affected evidence applicability until rerun.

[verification/toolchain-lock.template.json](../../../specs/verification/toolchain-lock.template.json) remains a deliberately unselected template; its null revisions are not usable pins. Actual selection is recorded in [flake.lock](../../../flake.lock), [policy/tool-selection.json](../../../policy/tool-selection.json), and the proof-library toolchain/manifest files. Configuration identities are not compatibility or refinement evidence by themselves. Executed evidence must bind them to the claimed source and theorem scope, including the separate [MC1 record](../../../verification/mc1/evidence.json).

The workspace milestone in [ROADMAP.md](../../../specs/ROADMAP.md) must select immutable Nix and Octet inputs. Nix creates `flake.lock`; hand-edited lock files are not accepted. Rust tests, Clippy, and the full pinned Octet catalog run as errors across the declared workspace scope.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### 6.1 Octet architecture and evidence integration

### Requirement: VT-OCTET-01
r[VT-OCTET-01]

**VT-OCTET-01.** The Rust workspace MUST activate a pinned Octet project architecture policy in addition to the full deny-all lint gate. Human-authored policy MUST use reviewed Nickel source with a validated export and freshness manifest. Policy MUST declare production roles, source/package coverage, targets, features, permitted dependencies, and external providers explicitly.

A deterministic-core profile alone does not activate architecture policy. Inventory or advisory output cannot satisfy a required architecture gate. Finding allowances and warning budgets remain prohibited. A reviewed function-address identity baseline records source identity, not permission to retain findings.


<!-- cairn:scenario-links:start -->
#### Scenario: OCTET-08 for VT-OCTET-01

- GIVEN the `Octet-Adoption-Design` profile and every field of `input` in [OCTET-08](../../../specs/conformance/octet-adoption-cases.json)
- WHEN the `review` procedure for case `OCTET-08` runs against those inputs
- THEN the observations match every field of `expected` in case `OCTET-08`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: OCTET-10 for VT-OCTET-01

- GIVEN the `Octet-Adoption-Design` profile and every field of `input` in [OCTET-10](../../../specs/conformance/octet-adoption-cases.json)
- WHEN the `review` procedure for case `OCTET-10` runs against those inputs
- THEN the observations match every field of `expected` in case `OCTET-10`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: VT-OCTET-02
r[VT-OCTET-02]

**VT-OCTET-02.** Architecture acceptance MUST cover core/shell separation, application-owned ports, adapter construction, inward type boundaries, and protected executor/witness/receipt protocols. Configured nominal-domain checks MUST cover approved constructors and unit/identity distinctions. Designated Rust lifecycle cores MUST use explicit closed state/event coverage.

Compiler-derived facts MUST bind the actual source, resolved providers, toolchain, target, features, and collection limits. Missing, stale, ambiguous, or over-limit required facts MUST remain blocking unknowns. Unsupported required crate kinds or configurations MUST remain blockers until a complementary check covers the same contract. A library-only result cannot establish coverage of production binaries or tests.

Roles do not imply `no_std`, and `no_std` does not prove purity. Octet rules about recursion, function length, and source shape remain Rust implementation policy, not Noble language laws. Narrow source exceptions retain the existing owner, reason, scope, and evidence requirements. They cannot bypass mandatory kernel refinement.


<!-- cairn:scenario-links:start -->
#### Scenario: OCTET-05 for VT-OCTET-02

- GIVEN the `Octet-Adoption-Design` profile and every field of `input` in [OCTET-05](../../../specs/conformance/octet-adoption-cases.json)
- WHEN the `review` procedure for case `OCTET-05` runs against those inputs
- THEN the observations match every field of `expected` in case `OCTET-05`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: OCTET-06 for VT-OCTET-02

- GIVEN the `Octet-Adoption-Design` profile and every field of `input` in [OCTET-06](../../../specs/conformance/octet-adoption-cases.json)
- WHEN the `admission` procedure for case `OCTET-06` runs against those inputs
- THEN the observations match every field of `expected` in case `OCTET-06`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: OCTET-08 for VT-OCTET-02

- GIVEN the `Octet-Adoption-Design` profile and every field of `input` in [OCTET-08](../../../specs/conformance/octet-adoption-cases.json)
- WHEN the `review` procedure for case `OCTET-08` runs against those inputs
- THEN the observations match every field of `expected` in case `OCTET-08`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: VT-OCTET-03
r[VT-OCTET-03]

**VT-OCTET-03.** Octet lint, architecture, effect-closure, artifact-integrity, and external-tool results MUST retain separate evidence roles and phase outcomes. A failed required phase MUST block the associated admission claim. Source-token closure MUST NOT replace Noble's resolved effect rules or the Charon → Aeneas → Lean route.

Reviewed provider summaries require exact source, dependency, call-site, target, and feature bindings for their declared analysis lane. An unknown direct effect cannot disappear through a summary. A passed artifact verifier establishes stored identity/linkage consistency only.

The current Octet Charon semantic rail covers a named fixture and records analysis evidence. It does not verify Noble's kernel. Reuse of a provider, bounded executor, or receipt interface requires published immutable inputs and an executed compatibility check. This amendment does not select toolchain pins or require a Verus migration.

Noble-owned runtime receipt DTOs remain Rust-owned. Their generated contracts must match the emitted schema. Octet's artifact formats stay externally owned behind a versioned adapter. [OCTET-ADOPTION.md](../../../specs/OCTET-ADOPTION.md) records the inspected design revision and non-claims.


<!-- cairn:scenario-links:start -->
#### Scenario: OCTET-07 for VT-OCTET-03

- GIVEN the `Octet-Adoption-Design` profile and every field of `input` in [OCTET-07](../../../specs/conformance/octet-adoption-cases.json)
- WHEN the `admission` procedure for case `OCTET-07` runs against those inputs
- THEN the observations match every field of `expected` in case `OCTET-07`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: OCTET-08 for VT-OCTET-03

- GIVEN the `Octet-Adoption-Design` profile and every field of `input` in [OCTET-08](../../../specs/conformance/octet-adoption-cases.json)
- WHEN the `review` procedure for case `OCTET-08` runs against those inputs
- THEN the observations match every field of `expected` in case `OCTET-08`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: OCTET-09 for VT-OCTET-03

- GIVEN the `Octet-Adoption-Design` profile and every field of `input` in [OCTET-09](../../../specs/conformance/octet-adoption-cases.json)
- WHEN the `review` procedure for case `OCTET-09` runs against those inputs
- THEN the observations match every field of `expected` in case `OCTET-09`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: OCTET-10 for VT-OCTET-03

- GIVEN the `Octet-Adoption-Design` profile and every field of `input` in [OCTET-10](../../../specs/conformance/octet-adoption-cases.json)
- WHEN the `review` procedure for case `OCTET-10` runs against those inputs
- THEN the observations match every field of `expected` in case `OCTET-10`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

## 7. Continuous verification contract

### Requirement: VT-CI-01
r[VT-CI-01]

**VT-CI-01.** CI MUST maintain separate document-validation, Lean-model, Rust-extraction/refinement, boundary-review, and compiler/runtime-test lanes. CI MUST include a Verus lane only when a reviewed exception selects it. Results MUST distinguish `passed`, `failed`, `timeout`, `unsupported`, and `not-run`. A nonzero exit, omitted obligation, or solver unknown result MUST NOT count as proof success.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: VT-CI-02
r[VT-CI-02]

**VT-CI-02.** Assurance runs MUST capture full obligation counts, tool exit status, diagnostics, source/configuration identities, trusted-boundary changes, and expected-claim changes. Grep-based checks MAY supplement but MUST NOT replace semantic and transitive dependency checks.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: VT-CI-03
r[VT-CI-03]

**VT-CI-03.** Changes to core primitives, typing rules, recipe observations, or host contracts MUST identify affected obligations and invalidate dependent assurance records. A proof against a weakened or differently interpreted statement cannot satisfy the old gate without review.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: VT-CI-04
r[VT-CI-04]

**VT-CI-04.** Differential and property-based tests SHOULD exercise reference semantics, Rust implementations, and Wasm execution on the same generated cases, including boundary integers and runtime-built programs. Such tests are regression evidence, not replacements for refinement theorems.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: VT-CI-05
r[VT-CI-05]

**VT-CI-05.** CI MUST regenerate extraction and recheck affected refinement proofs whenever production source, dependencies, macros, features, targets, or semantic contracts change. The gate MUST compare compiler-derived source coverage with the VT-SCOPE-05 inventory. A hand-maintained list of successful functions alone is insufficient. Missing functions, proof holes, unexplained opaque bodies, and unsupported required configurations MUST fail the claimed verification gate. Separate experimental lanes MUST retain their incomplete status.


<!-- cairn:scenario-links:start -->
#### Scenario: VERIFY-02 for VT-CI-05

- GIVEN the `Implementation-Policy` profile and every field of `input` in [VERIFY-02](../../../specs/conformance/verification-cases.json)
- WHEN the `review` procedure for case `VERIFY-02` runs against those inputs
- THEN the observations match every field of `expected` in case `VERIFY-02`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: VERIFY-05 for VT-CI-05

- GIVEN the `Implementation-Policy` profile and every field of `input` in [VERIFY-05](../../../specs/conformance/verification-cases.json)
- WHEN the `review` procedure for case `VERIFY-05` runs against those inputs
- THEN the observations match every field of `expected` in case `VERIFY-05`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### 7.1 Application-contract lane

### Requirement: VT-CONTRACT-01
r[VT-CONTRACT-01]

**VT-CONTRACT-01.** CI MUST keep application-contract evidence separate from Rust implementation refinement and backend correspondence. The contract lane MUST use actual accepted Noble programs and typed claims under SPEC-V002. It MUST retain contract IR, generated propositions, rule-library revisions, and exact accepted theorem references. An Aeneas result for the exporter MUST NOT automatically mark an application claim proved.


<!-- cairn:scenario-links:start -->
#### Scenario: CONTRACT-01 for VT-CONTRACT-01

- GIVEN the `Contracts-Draft` profile and every field of `input` in [CONTRACT-01](../../../specs/conformance/contract-cases.json)
- WHEN the `review` procedure for case `CONTRACT-01` runs against those inputs
- THEN the observations match every field of `expected` in case `CONTRACT-01`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: CONTRACT-14 for VT-CONTRACT-01

- GIVEN the `Contracts-Draft` profile and every field of `input` in [CONTRACT-14](../../../specs/conformance/contract-cases.json)
- WHEN the `admission` procedure for case `CONTRACT-14` runs against those inputs
- THEN the observations match every field of `expected` in case `CONTRACT-14`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: VT-CONTRACT-02
r[VT-CONTRACT-02]

**VT-CONTRACT-02.** Proof-required builds MUST invalidate affected application evidence when subjects, captures, logical definitions, policies, or semantic dependencies change. CI MUST exercise proof failure, unsupported claims, forged companions, stale applicability, and unrelated-artifact rejection. Pinned Lean libraries and sound finite replay rules support runtime companions without a production Lean or SMT process.


<!-- cairn:scenario-links:start -->
#### Scenario: CONTRACT-02 for VT-CONTRACT-02

- GIVEN the `Contracts-Draft` profile and every field of `input` in [CONTRACT-02](../../../specs/conformance/contract-cases.json)
- WHEN the `admission` procedure for case `CONTRACT-02` runs against those inputs
- THEN the observations match every field of `expected` in case `CONTRACT-02`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: CONTRACT-09 for VT-CONTRACT-02

- GIVEN the `Contracts-Draft` profile and every field of `input` in [CONTRACT-09](../../../specs/conformance/contract-cases.json)
- WHEN the `review` procedure for case `CONTRACT-09` runs against those inputs
- THEN the observations match every field of `expected` in case `CONTRACT-09`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### 7.2 MC1 evidence boundaries

MC1 has three distinct subjects of assurance:

| Lane | Delivered scope | Not established |
|---|---|---|
| Contract frontend/CLI | Version-1 source and typed IR; immutable accepted `Prepared`; exact statement generation; isolated checking with seven outcomes | Universal correctness of the Rust frontend or shell; runtime proof admission |
| Strict application/rule library | Reviewed pure normal-return semantics and Lean rules for primitives, sequencing, structural cases, quotation/invocation, increment/composition, and universally quantified runtime-capture behavior | Rust implementation refinement, termination/host/resource guarantees, or Wasm/backend correspondence |
| Actual-source implementation correspondence | Whole `noble-contracts` extraction; universal node/body projection preservation; full preparation/export equations for six named source cases | Universal parser/inference/exporter correctness or strict acceptance of native-evaluated correspondence equations |

The strict application library is [`NobleContracts`](../../../proofs/mc1/NobleContracts.lean). Its imported definitions/rules and accepted application dependencies are checked against the standard allowed foundation set `propext`, `Classical.choice`, and `Quot.sound`; missing proofs, `sorryAx`, unsafe dependencies, and native-evaluation assumptions cannot satisfy that lane. Its `Exec` relation models all normal returns and its `Maps` relation does not assert existence of a result. Quotas, allocation failure, abnormal outcomes, host responses, and real runtime execution lie outside those application statements.

The implementation lane is [`NobleContractImpl`](../../../proofs/mc1/NobleContractImpl.lean). [Projection theorems](../../../proofs/mc1/NobleContractImpl/Projection.lean) quantify over actual extracted node/body operations and preserve literal bits, word identities, quotation references, and order. The [source-bound statement equations](../../../proofs/mc1/NobleContractImpl/Statement.lean) evaluate the extracted full `prepare`/`export_lean` path, including inherited acceptance, for increment, composed increments, the universal capture-family source, structural data, reflected syntax, and signed wrap.

Those six equations use native evaluation. Their extra native compiler/evaluator trust must be recorded separately from standard logical foundations under VT-LEAN-02; they do not become strict proofs by sharing a repository with strict application theorems. `NobleContractImpl` is not imported by the application rule library. The runtime-capture **application theorem** quantifies over runtime captures and logical inputs; the corresponding **source-export equation** concerns one exact contract source describing that family. Neither should be confused with universal frontend preservation.

Charon/Aeneas extraction, generated Lean compilation, external-model fidelity, theorem checking, and concrete source/configuration correspondence retain distinct statuses. Actual-source extraction is not a proof by itself, and successful CLI proof acceptance explicitly reports implementation refinement as `not-checked-by-this-command` and backend correspondence as `not-claimed`. These statuses are not changed by producer metadata or application success.

Durable [MC1 evidence](../../../verification/mc1/evidence.json) and [CLI acceptance](../../../verification/mc1/acceptance.json) records must identify exact tools, sources, theorem inventories, assumptions, generated artifacts, controls, and exclusions. The [source/proof fixtures](../../../verification/mc1) are inputs, not execution evidence. The [obligation ledger](../../../specs/verification/obligations.json) and [milestone status](../../../specs/STATUS.json) determine scoped acceptance; this policy does not close all of PO-15/16/19/20/21 or full `Contracts-Draft` conformance.

MC1 implements the future companion interface/design selection, not guest companion values, finite runtime replay, applicability checks, proof-required builds, or certified admission. MC2/Wasm and any backend/load correspondence remain outside this milestone, as do general termination, host/resource protocols, and whole-language compiler correctness.

## 8. Implementation entry point

The first vertical slice SHALL implement the pure explicit acceptance checker, wrapping integer operations, and runtime-independent recipe builders in ordinary Rust; extract them with a pinned Charon/Aeneas pair; and prove one concrete checker/representation refinement against the Lean model. In parallel, the Wasm experiment SHALL exercise nonconstant `quote` and `compose` without claiming backend proof.

After that slice establishes a usable proof pattern, extend extraction and refinement across the kernel and the remaining compiler/runtime logic. Resource-table transitions follow the same Aeneas route. External effects remain behind checked adapters with explicit correspondence obligations.

M1 establishes the source inventory and extraction CI entry point. M2 requires actual extraction, a nontrivial refinement, and rejection of incomplete verification coverage. MC1 adds the implemented typed contract frontend, source/IR revision 1, strict application rules, CLI consumer, and separately scoped implementation correspondence described above. MC2 still requires the first-class Wasm demonstration after MC1 and M4. Neither contract milestone supplies host resource protocols. The repository now contains production Rust, pinned tools, generated extraction, and proof sources; their existence does not establish full-kernel, whole-project, runtime, or backend completion. Current milestone acceptance is recorded in the status/evidence ledgers, not inferred from the historical entry-point requirements below.

### Requirement: VT-M1-01
r[VT-M1-01]

**VT-M1-01.**

M1 MUST select immutable, mutually compatible build and verification inputs through executed compatibility checks.
The inputs MUST cover Nixpkgs, production Rust, extraction rustc, Charon, Aeneas, Lean/Lake, its backend libraries, Octet, and scoped Miri tooling.
Nix MUST generate `flake.lock`. The selected Charon and Lean backend MUST satisfy the selected Aeneas revision's compatibility requirements.
The configuration MUST name required targets, features, word size, panic behavior, overflow behavior, generation arguments, and resource limits.
Ambient sibling checkouts, floating revisions, and manual pin-check overrides MUST NOT establish compatibility.
Future unselected components MUST remain explicit without supporting a release claim.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

#### Scenario: Compatible workspace toolchain

- GIVEN immutable inputs and the declared host, target, features, and generation configuration
- WHEN the compatibility check extracts the workspace subject and compiles the generated Lean module
- THEN the result records exact input identities and accepts only that executed configuration

#### Scenario: Invalid toolchain selection

- GIVEN a missing pin, mismatched Charon revision, mismatched Lean backend, or manual compatibility override
- WHEN the compatibility gate evaluates the configuration
- THEN the gate rejects it without a compatible-toolchain claim

### Requirement: VT-M1-02
r[VT-M1-02]

**VT-M1-02.**

M1 MUST establish a deterministic semantic-kernel crate and a separate CLI shell with dependencies directed toward the kernel.
The kernel MUST use safe sequential Rust with explicit owned inputs and typed outcomes.
The kernel MUST NOT access ambient I/O, environment state, clocks, randomness, or hidden mutable state.
The shell MUST own external observations, orchestration, and effect execution.
Real external capabilities MUST use application-owned contracts with adapters outside the kernel.
The initial production extraction subject MUST include positive and exhausted budget outcomes without a public language-syntax change.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

#### Scenario: Deterministic budget transition

- GIVEN a positive budget and a zero budget as separate inputs to the actual kernel transition
- WHEN the Rust tests invoke the transition with each input
- THEN the positive budget decrements once and the zero budget returns exhaustion without underflow or external effects

#### Scenario: Invalid kernel boundary

- GIVEN a kernel body with I/O, unsafe code, an inward host type, or a reverse dependency on the shell
- WHEN the required boundary checks evaluate the configured workspace
- THEN the checks reject the boundary and preserve the function's kernel ownership

### Requirement: VT-M1-03
r[VT-M1-03]

**VT-M1-03.**

M1 MUST compare compiler-derived source and function coverage with a complete verification inventory.
The inventory MUST classify production Rust, generated and macro bodies, dependencies, tests, non-Rust tools, and future components.
Each current production body MUST map to extracted work, an external model, an exact reviewed exception, or explicit open work.
The inventory MUST retain dependency closure, target and feature coverage, linked contracts, and separate extraction and refinement statuses.
Non-kernel exceptions MUST satisfy VT-SCOPE-04. Kernel bodies MUST NOT use exceptions or crate relocation to escape mandatory scope.
Required gates MUST reject omissions, stale mappings, missing facts, and unsupported required configurations.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

#### Scenario: Complete named scope

- GIVEN compiler-derived bodies for every required configuration and a matching classified inventory
- WHEN the coverage gate compares the body sets and their bindings
- THEN it reports total, extracted, proved, modeled, excepted, and open counts without a whole-project verification claim

#### Scenario: Missing or invalid coverage

- GIVEN an omitted function, macro body, target, feature, stale mapping, or kernel exception as separate mutations
- WHEN the coverage gate evaluates each mutation
- THEN it rejects every mutation without treating an unknown body as verified

### Requirement: VT-M1-04
r[VT-M1-04]

**VT-M1-04.**

M1 MUST provide a bounded Charon → Aeneas → Lean entry point for the actual production workspace subject.
The entry point MUST regenerate outputs after relevant source, dependency, macro, feature, target, or contract changes.
The gate MUST bind generated functions to the exact source and configuration inventory.
Hand-edited generated functions and unrelated fixtures MUST NOT substitute for actual extraction.
Missing functions, unexplained opaque models, unsupported required bodies, and stale artifacts MUST block the claimed scope.
Successful extraction and Lean compilation MUST NOT establish a refinement proof or Wasm correspondence.
Proof-required claims MUST reject unresolved required obligations, proof holes, and reference-only substitutes.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

#### Scenario: Actual workspace extraction

- GIVEN the named production budget transition and the compatible pinned toolchain
- WHEN the entry point regenerates extraction and compiles the generated Lean module
- THEN the result binds the actual Rust subject and reports extraction separately from open refinement work

#### Scenario: Invalid extraction artifact

- GIVEN a missing function, unsupported body, edited generated function, unexplained model, stale input, or unrelated fixture
- WHEN the extraction gate evaluates the artifact against independently expected inputs
- THEN the gate rejects the artifact for the requested scope

#### Scenario: Extraction does not close a proof

- GIVEN passed extraction with a missing proof, proof hole, timeout, or theorem about a separate handwritten implementation
- WHEN a consumer requests a refinement claim for the extracted subject
- THEN the claim remains rejected even though the smoke extraction succeeded

### Requirement: VT-M1-05
r[VT-M1-05]

**VT-M1-05.**

M1 MUST enforce the full pinned Octet lint catalog as errors across the workspace, all targets, and all compatible features.
Nix and the `octet-deny-all` pre-commit hook MUST use the same immutable Octet revision and required scope.
The workspace MUST declare its Octet metadata and core source scopes without disabled lints, warning budgets, or finding baselines.
Architecture policy MUST use reviewed Nickel source, a checked export, and a freshness manifest.
The policy MUST declare roles, packages, sources, providers, dependencies, actual ports and adapters, targets, and features.
Required compiler facts MUST cover production libraries, binaries, and tests without advisory-only or library-only substitution.
Local acceptance and CI MUST run the same Nix gate, including formatting, Rust tests, strict Clippy, document checks, and required assurance lanes.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

#### Scenario: Complete enforced quality gate

- GIVEN a fresh architecture policy and compiler facts for the complete required configuration matrix
- WHEN local acceptance and CI run the Nix gate with the pinned deny-all hook contract
- THEN all required checks pass with matching scope and separately recorded phase outcomes

#### Scenario: Incomplete or weakened gate

- GIVEN a lint finding, empty policy, stale export, missing compiler fact, advisory-only result, or unsupported required configuration
- WHEN the workspace acceptance gate evaluates that input
- THEN it rejects the input without a warning budget, finding baseline, or silent scope exclusion

### Requirement: VT-M1-06
r[VT-M1-06]

**VT-M1-06.**

M1 MUST inventory native boundaries, relevant dependencies, generated code, and Noble-owned unsafe sites.
Every applicable unsafe site MUST retain the safety argument and exact ownership required by VT-NATIVE-01.
Dependency records MUST name versions, features, macros, target assumptions, and verification boundaries.
An empty Noble-owned unsafe inventory MUST derive from complete source accounting and MUST NOT imply that dependencies contain no unsafe code.
M1 MUST include scoped positive and negative native-assurance controls with pinned Miri targets and memory-model configuration.
Unsupported required Miri runs MUST remain blockers. Byte-view dependencies MUST NOT enter the initial owned-data acceptance core without VT-NATIVE-03 evidence.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

#### Scenario: Accounted native scope

- GIVEN a complete native inventory, dependency records, safety arguments, and supported pinned Miri configuration
- WHEN the scoped positive and negative controls execute
- THEN valid boundaries pass and invalid boundaries fail within the named scope only

#### Scenario: Missing native assurance

- GIVEN an unrecorded unsafe site, absent safety argument, missing dependency binding, or unsupported required Miri configuration
- WHEN the native-assurance gate evaluates the scope
- THEN it rejects acceptance rather than reporting an empty or passed lane

### Requirement: VT-M1-07
r[VT-M1-07]

**VT-M1-07.**

M1 acceptance MUST use independently expected subjects, claims, source scopes, dependencies, policy, tools, targets, features, commands, inputs, assumptions, and results.
Every required phase MUST retain its own outcome and coverage, including open, unsupported, failed, and unexecuted work.
Lint, architecture, extraction, native tests, artifact integrity, and formal proofs MUST retain separate evidence roles.
Stale bindings, missing required phases, and evidence-role promotion MUST block acceptance.
M1 MUST NOT close through document validation, an unrelated extraction fixture, or passing checks for only part of its required configuration.
Spec sync MUST follow implementation acceptance. Archive MUST follow the completed checklist and an unblocked archive plan.
Approval of this planning package MUST NOT authorize either completion claim.
M1 completion MUST leave unrelated runtime, refinement, and later-milestone claims open unless their own evidence satisfies the applicable contracts.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

#### Scenario: Scoped workspace acceptance

- GIVEN complete source-bound evidence for all M1 tasks and every required configuration
- WHEN the consumer compares all phase outcomes with its independent acceptance contract
- THEN it accepts the workspace scope while preserving open refinement and unimplemented language/runtime scopes

#### Scenario: Stale or promoted evidence

- GIVEN a changed subject, source, policy, target, feature, missing required phase, or lint result relabeled as a proof
- WHEN the acceptance gate evaluates the record against independently expected bindings
- THEN it rejects the record even if an unrelated phase passed

#### Scenario: Planning is not implementation

- GIVEN this package passes proposal, design, tasks, and document validation with implementation tasks still open
- WHEN its readiness and lifecycle state are reported
- THEN the package remains active and M1 is neither completed, synced, nor archived
