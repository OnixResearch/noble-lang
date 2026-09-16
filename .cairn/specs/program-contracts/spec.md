# Program Contracts and Verification Evidence

<!-- cairn:purpose:start -->
## Purpose

This accepted specification records Noble draft contracts, not completed implementation.
Original requirement IDs, explanatory prose, examples, and open decisions remain authoritative.
Scenario clauses refer to unexecuted designs in the conformance ledger.

## Requirements

<!-- cairn:purpose:end -->


Document: SPEC-V002  
Revision: 0.1.0-draft.5  
Status: First-class contract profile selected; implementation, concrete syntax, and wire format remain open  
Depends on: [SPEC-V001](../verification/spec.md), [IMPL-V001](../verification-toolchain/spec.md) at 0.1.0-draft.5

[SOURCES.md](../../../specs/SOURCES.md) records inherited baseline citations.

## 1. Scope

A program's ordinary type describes its stack interface and latent host-operation bound. A behavioral contract states a stronger property of an exact program under explicit assumptions. These are different claims.

### Requirement: VC-SCOPE-01
r[VC-SCOPE-01]

**VC-SCOPE-01.** Behavioral verification SHALL be optional. An ordinary well-typed program MUST remain callable without an attached behavioral proof unless an explicit host admission policy requires additional evidence. Such a policy MUST NOT be confused with the core typing relation.


<!-- cairn:scenario-links:start -->
#### Scenario: CONTRACT-11 for VC-SCOPE-01

- GIVEN the `Contracts-Draft` profile and every field of `input` in [CONTRACT-11](../../../specs/conformance/contract-cases.json)
- WHEN the `runtime` procedure for case `CONTRACT-11` runs against those inputs
- THEN the observations match every field of `expected` in case `CONTRACT-11`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: VC-SCOPE-02
r[VC-SCOPE-02]

**VC-SCOPE-02.** Noble MUST support typed contract declarations and first-class evidence companions through the optional `Contracts-Draft` profile. Lean proof modules remain separate from production execution. Ordinary `Program<S,T,e>` types and execution semantics MUST remain unchanged. This profile introduces no dependent `Program` parameters, implicit behavioral coercions, or general in-guest prover.


<!-- cairn:scenario-links:start -->
#### Scenario: CONTRACT-11 for VC-SCOPE-02

- GIVEN the `Contracts-Draft` profile and every field of `input` in [CONTRACT-11](../../../specs/conformance/contract-cases.json)
- WHEN the `runtime` procedure for case `CONTRACT-11` runs against those inputs
- THEN the observations match every field of `expected` in case `CONTRACT-11`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: VC-SCOPE-03
r[VC-SCOPE-03]

**VC-SCOPE-03.** A `Contracts-Draft` implementation MUST expose contracts and evidence companions as program inputs, outputs, and inspectable values, not only compiler metadata. Eligible companions MUST support aggregate storage and runtime-selected composition. Host-only proof reports do not satisfy this first-class requirement.

Sections 9–13 select the compiler and companion API contracts. Concrete declaration grammar, library word spellings, representation, and portable encoding remain implementation-entry gates. Examples in this document are mathematical or harness notation, not frozen Noble syntax. The profile is a required project deliverable but optional for application use.

The selected proof logic is Lean 4 over the reviewed Noble model. Aeneas connects Rust implementation functions to Lean contracts. Verus is optional for reviewed non-kernel exceptions, not an automatically available prover for arbitrary Noble source.


<!-- cairn:scenario-links:start -->
#### Scenario: CONTRACT-10 for VC-SCOPE-03

- GIVEN the `Contracts-Draft` profile and every field of `input` in [CONTRACT-10](../../../specs/conformance/contract-cases.json)
- WHEN the `runtime` procedure for case `CONTRACT-10` runs against those inputs
- THEN the observations match every field of `expected` in case `CONTRACT-10`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

## 2. Contract meanings

### 2.1 Preconditions and normal-return postconditions

Let a logical configuration contain the current typed stack, modeled host state, ownership obligations, and accumulated event history. Immutable ghost parameters can retain initial inputs and state snapshots for relational specifications.

Define the intended partial-correctness judgment:

```text
PC(p, P, Q) :=
  for every well-formed initial configuration c satisfying P,
  every modeled normal execution of p from c ends in a configuration satisfying Q.
```

### Requirement: VC-LOGIC-01
r[VC-LOGIC-01]

**VC-LOGIC-01.** A postcondition under `PC` is conditional on normal return. It MUST NOT be described as proving termination, absence of allowed abnormal termination, or host availability. Ordinary `Result` errors are normal returns, so their alternatives must be included in `Q` where the program can return them.

For example, a filesystem result contract must not state properties only of the successful byte payload while ignoring an error alternative that also returns the directory handle.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: VC-LOGIC-02
r[VC-LOGIC-02]

**VC-LOGIC-02.** Preconditions MUST remain visible to callers and admission policy. The evidence validator proves the implication; it does not establish that a particular invocation's inputs satisfy the precondition. Applicability requires a separate proof, a checked decidable guard, or a trusted environmental premise explicitly accepted by policy.

A false precondition makes a partial-correctness claim vacuous. The system must not advertise such a claim as useful correctness for unconstrained inputs.


<!-- cairn:scenario-links:start -->
#### Scenario: CONTRACT-07 for VC-LOGIC-02

- GIVEN the `Contracts-Draft` profile and every field of `input` in [CONTRACT-07](../../../specs/conformance/contract-cases.json)
- WHEN the `admission` procedure for case `CONTRACT-07` runs against those inputs
- THEN the observations match every field of `expected` in case `CONTRACT-07`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: VC-LOGIC-06
r[VC-LOGIC-06]

**VC-LOGIC-06.** A relational postcondition MUST distinguish initial-state snapshots from final-state observations. The claim MUST name its arithmetic semantics and applicability conditions. Two references to the same final value MUST NOT be presented as an initial/final relation.

For a bounded integer deposit, the intended relation is `balance_after = balance_before + amount`. Preconditions must establish agreement with the selected `I64` arithmetic and the domain's balance rules. At `balance_before = 100` and `amount = 10`, the result is 110, not a claim that `110 = 110 + 10`.

This is contract notation, not a new Noble record declaration or assertion form. Ghost snapshots do not duplicate a live resource. They describe its modeled state under explicit assumptions.


<!-- cairn:scenario-links:start -->
#### Scenario: ADAPT-14 for VC-LOGIC-06

- GIVEN the `Contract-Design` profile and every field of `input` in [ADAPT-14](../../../specs/conformance/adaptation-cases.json)
- WHEN the `review` procedure for case `ADAPT-14` runs against those inputs
- THEN the observations match every field of `expected` in case `ADAPT-14`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### 2.2 Safety and total correctness

A prefix-safety claim constrains every finite reachable execution prefix, including those that do not return normally. A total-correctness claim establishes normal termination and its postcondition under stated environment and execution assumptions.

### Requirement: VC-LOGIC-03
r[VC-LOGIC-03]

**VC-LOGIC-03.** Reports MUST distinguish `partial-correctness`, `prefix-safety`, and `total-correctness`. A termination argument MUST include the relevant recursion, loops, called programs, and host-response assumptions. A pure effect set is not that argument.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: VC-LOGIC-04
r[VC-LOGIC-04]

**VC-LOGIC-04.** Claims abstracting from quotas or allocation failure MUST say so. A totality result in the ideal semantics does not imply successful completion under arbitrary finite memory, cancellation, execution budgets, or unresponsive hosts.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: VC-LOGIC-05
r[VC-LOGIC-05]

**VC-LOGIC-05.** Host-state and trace contracts MUST identify the modeled host operations and the assumptions on their responses. No contract may infer real-world availability, authorization, confidentiality, or exactly-once remote effects solely from a `Program` type or effect set.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

## 3. Compositional rules

### 3.1 Sequencing

For compatible stack interfaces and the same semantic/host model, the contract logic SHALL establish:

```text
PC(p, P, Q)    PC(q, Q, R)
--------------------------------
PC(compose(p, q), P, R)
```

Predicates are over the full logical configurations described above. Shared ghost constants and world assumptions must remain in scope; resource obligations passed between programs must match. The definition of normal sequencing ensures that `q` does not start after `p` aborts.

### Requirement: VC-COMPOSE-01
r[VC-COMPOSE-01]

**VC-COMPOSE-01.** Contract composition MUST require a proved intermediate implication when the first postcondition and second precondition differ. Matching stack types alone does not discharge that implication. A proof of partial correctness MUST NOT silently acquire the strength of prefix safety or total correctness during composition.


<!-- cairn:scenario-links:start -->
#### Scenario: CONTRACT-03 for VC-COMPOSE-01

- GIVEN the `Contracts-Draft` profile and every field of `input` in [CONTRACT-03](../../../specs/conformance/contract-cases.json)
- WHEN the `runtime` procedure for case `CONTRACT-03` runs against those inputs
- THEN the observations match every field of `expected` in case `CONTRACT-03`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: CONTRACT-04 for VC-COMPOSE-01

- GIVEN the `Contracts-Draft` profile and every field of `input` in [CONTRACT-04](../../../specs/conformance/contract-cases.json)
- WHEN the `admission` procedure for case `CONTRACT-04` runs against those inputs
- THEN the observations match every field of `expected` in case `CONTRACT-04`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: VC-COMPOSE-02
r[VC-COMPOSE-02]

**VC-COMPOSE-02.** Proofs MUST preserve event ordering and explicit ownership transitions. A mathematical set union of effects is not a proof that the actual event traces commute. No generic host-state frame rule is assumed; use requires the appropriate locality/preservation hypotheses for that state model.


<!-- cairn:scenario-links:start -->
#### Scenario: CONTRACT-03 for VC-COMPOSE-02

- GIVEN the `Contracts-Draft` profile and every field of `input` in [CONTRACT-03](../../../specs/conformance/contract-cases.json)
- WHEN the `runtime` procedure for case `CONTRACT-03` runs against those inputs
- THEN the observations match every field of `expected` in case `CONTRACT-03`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### 3.2 Quoting, invocation, and runtime builders

For capturable `v`, executing `quote(v)` pushes that same semantic value and issues no guest host operation. When `v` is a program, it is pushed as a program value, not executed. Finite recipe and interface witnesses remain available as required by SPEC-0001.

### Requirement: VC-BUILD-01
r[VC-BUILD-01]

**VC-BUILD-01.** Reusable builder theorems MUST quantify over eligible runtime values and compatible runtime program operands. A proof about a literal specialization MUST NOT be reported as covering all values supplied after compilation.

For the builder body:

```text
quote [ + ] compose
```

an intended theorem can state that, for every `I64` capture `n`, the returned program maps a later `I64` input `x` to `wrap64(x + n)` in the ideal pure semantics. Captured data, interface instantiation, and the builder's recipe rules are part of the theorem's subject.


<!-- cairn:scenario-links:start -->
#### Scenario: CONTRACT-05 for VC-BUILD-01

- GIVEN the `Contracts-Draft` profile and every field of `input` in [CONTRACT-05](../../../specs/conformance/contract-cases.json)
- WHEN the `runtime` procedure for case `CONTRACT-05` runs against those inputs
- THEN the observations match every field of `expected` in case `CONTRACT-05`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: VC-BUILD-02
r[VC-BUILD-02]

**VC-BUILD-02.** Ordinary `quote`, `compose`, and `run` MUST NOT run a prover. The optional companion API MUST support checked family instantiation and composition under section 11. Those operations use previously accepted rules, not runtime proof search. Tracking or transporting per-instance certificates is not a requirement of ordinary execution.


<!-- cairn:scenario-links:start -->
#### Scenario: CONTRACT-05 for VC-BUILD-02

- GIVEN the `Contracts-Draft` profile and every field of `input` in [CONTRACT-05](../../../specs/conformance/contract-cases.json)
- WHEN the `runtime` procedure for case `CONTRACT-05` runs against those inputs
- THEN the observations match every field of `expected` in case `CONTRACT-05`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: CONTRACT-11 for VC-BUILD-02

- GIVEN the `Contracts-Draft` profile and every field of `input` in [CONTRACT-11](../../../specs/conformance/contract-cases.json)
- WHEN the `runtime` procedure for case `CONTRACT-11` runs against those inputs
- THEN the observations match every field of `expected` in case `CONTRACT-11`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: VC-BUILD-03
r[VC-BUILD-03]

**VC-BUILD-03.** A higher-order contract MUST constrain the behavioral properties of program arguments it relies on. `Program<S,T,e>` alone does not imply that an argument increments, sorts, terminates, or preserves a business invariant. Duplicating a program value does not grant independent polymorphic instantiations or missing behavioral assumptions.


<!-- cairn:scenario-links:start -->
#### Scenario: CONTRACT-04 for VC-BUILD-03

- GIVEN the `Contracts-Draft` profile and every field of `input` in [CONTRACT-04](../../../specs/conformance/contract-cases.json)
- WHEN the `admission` procedure for case `CONTRACT-04` runs against those inputs
- THEN the observations match every field of `expected` in case `CONTRACT-04`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

## 4. Arithmetic example

For `[ 1 + ]`, the unconditional normal-result claim is:

```text
output = wrap64(input + 1)
```

The stronger inequality `output > input` requires `input < 9223372036854775807` under the baseline's signed `I64` interpretation. At the maximum input, the output is `-9223372036854775808`.

### Requirement: VC-NUM-01
r[VC-NUM-01]

**VC-NUM-01.** Program contracts MUST use Noble's selected arithmetic semantics or prove the preconditions under which mathematical arithmetic agrees with them. A proof over unbounded integers does not automatically apply to `I64`. [B1 §4.4]

An implementation may later provide ordinary checked-arithmetic words returning `Result`; their contracts require those words' own specifications and do not change the meaning of the existing operators.

The [exact calculator](../calculator/spec.md) uses a separate library model of arbitrary-precision integers and normalized rationals. CALC-EVIDENCE-02 requires claims for its actual implementation, including zero-divisor errors. The bootstrap `I64` examples do not establish those claims. This application does not expand VC-BOOT-01 automatically.


<!-- cairn:scenario-links:start -->
#### Scenario: CONTRACT-01 for VC-NUM-01

- GIVEN the `Contracts-Draft` profile and every field of `input` in [CONTRACT-01](../../../specs/conformance/contract-cases.json)
- WHEN the `review` procedure for case `CONTRACT-01` runs against those inputs
- THEN the observations match every field of `expected` in case `CONTRACT-01`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: CONTRACT-07 for VC-NUM-01

- GIVEN the `Contracts-Draft` profile and every field of `input` in [CONTRACT-07](../../../specs/conformance/contract-cases.json)
- WHEN the `admission` procedure for case `CONTRACT-07` runs against those inputs
- THEN the observations match every field of `expected` in case `CONTRACT-07`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

## 5. Reflection, rewrites, and identity

### Requirement: VC-REWRITE-01
r[VC-REWRITE-01]

**VC-REWRITE-01.** Rechecking transformed syntax establishes only the ordinary properties checked by `prepare`. It MUST NOT automatically transfer the original program's behavioral evidence. Evidence for the transformed program requires a fresh proof or an applicable proved transformation rule.

For instance, replacing addition by subtraction can leave the stack signature and empty effect bound unchanged while invalidating an increment contract.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: VC-REWRITE-02
r[VC-REWRITE-02]

**VC-REWRITE-02.** A transformation claim MUST name its equivalence or preservation relation. Execution-only equivalence, normal-result equivalence, trace preservation, resource behavior, recipe equality, and full contextual equivalence are distinct claims. A backend optimization that retains recipes has a different obligation from a source rewrite that changes program identity.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: VC-ID-01
r[VC-ID-01]

**VC-ID-01.** Proof evidence and optional behavioral claims MUST be associated separately from `DefinitionId` and `ProgramValueId`. Adding or replacing a proof of unchanged behavior MUST NOT change the program's canonical recipe or identity. Mandatory stack/effect interfaces and semantic dependencies remain part of ordinary identity as already specified.


<!-- cairn:scenario-links:start -->
#### Scenario: CONTRACT-10 for VC-ID-01

- GIVEN the `Contracts-Draft` profile and every field of `input` in [CONTRACT-10](../../../specs/conformance/contract-cases.json)
- WHEN the `runtime` procedure for case `CONTRACT-10` runs against those inputs
- THEN the observations match every field of `expected` in case `CONTRACT-10`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: VC-ID-02
r[VC-ID-02]

**VC-ID-02.** Evidence about a closed program MUST bind its captured values, instantiated interfaces, resolved dependencies, builtin semantics, and relevant host contracts. A theorem about a universally quantified family MUST record its quantifiers and any checked instantiation; a function-template identity alone does not identify a particular captured program value.


<!-- cairn:scenario-links:start -->
#### Scenario: CONTRACT-02 for VC-ID-02

- GIVEN the `Contracts-Draft` profile and every field of `input` in [CONTRACT-02](../../../specs/conformance/contract-cases.json)
- WHEN the `admission` procedure for case `CONTRACT-02` runs against those inputs
- THEN the observations match every field of `expected` in case `CONTRACT-02`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: CONTRACT-05 for VC-ID-02

- GIVEN the `Contracts-Draft` profile and every field of `input` in [CONTRACT-05](../../../specs/conformance/contract-cases.json)
- WHEN the `runtime` procedure for case `CONTRACT-05` runs against those inputs
- THEN the observations match every field of `expected` in case `CONTRACT-05`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: VC-ID-03
r[VC-ID-03]

**VC-ID-03.** A digest is an index or integrity check under the chosen encoding, not proof validity, provenance, or authority. Until Noble's canonical encodings are standardized, all such bindings MUST be versioned experimental representations and must be checked against the actual subject. This specification chooses no stable language hash algorithm.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

## 6. Evidence envelope: semantic requirements

The following fields define the semantic envelope for evidence companions, not a frozen transport format or primitive Noble type:

| Field | Meaning |
|---|---|
| Subject | Exact program/definition and instantiation, or explicitly quantified builder family |
| Semantic context | Language/formal-model revisions, schemas, builtin and host-contract identities |
| Claim | Exact elaborated proposition plus referenced logical definitions |
| Applicability | Preconditions, quantifiers, invocation guards, and environment assumptions |
| Evidence | Proof module/declaration or another explicitly identified evidence object |
| Evidence class | Lean, Aeneas/Lean, Verus, validation, test, review, or assumption |
| Toolchain | Immutable tool/library/configuration identities required to check the evidence |
| Dependencies | Supporting claims, proof assumptions, external models, and bridge status |
| Coverage | What is established and what is explicitly excluded |

### Requirement: VC-EVIDENCE-01
r[VC-EVIDENCE-01]

**VC-EVIDENCE-01.** An evidence object MUST NOT self-authorize its acceptance policy. The consumer selects the expected claim, allowed assumptions, semantic context, evidence classes, and resource limits independently of untrusted supplied metadata.


<!-- cairn:scenario-links:start -->
#### Scenario: CONTRACT-02 for VC-EVIDENCE-01

- GIVEN the `Contracts-Draft` profile and every field of `input` in [CONTRACT-02](../../../specs/conformance/contract-cases.json)
- WHEN the `admission` procedure for case `CONTRACT-02` runs against those inputs
- THEN the observations match every field of `expected` in case `CONTRACT-02`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: CONTRACT-14 for VC-EVIDENCE-01

- GIVEN the `Contracts-Draft` profile and every field of `input` in [CONTRACT-14](../../../specs/conformance/contract-cases.json)
- WHEN the `admission` procedure for case `CONTRACT-14` runs against those inputs
- THEN the observations match every field of `expected` in case `CONTRACT-14`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: VC-EVIDENCE-02
r[VC-EVIDENCE-02]

**VC-EVIDENCE-02.** Proof verification MUST establish that the theorem matches the intended claim, not only that some theorem is valid. Assumptions and all logically relevant definitions MUST be checked transitively. Renaming a different predicate to the expected display name does not satisfy this requirement.


<!-- cairn:scenario-links:start -->
#### Scenario: CONTRACT-02 for VC-EVIDENCE-02

- GIVEN the `Contracts-Draft` profile and every field of `input` in [CONTRACT-02](../../../specs/conformance/contract-cases.json)
- WHEN the `admission` procedure for case `CONTRACT-02` runs against those inputs
- THEN the observations match every field of `expected` in case `CONTRACT-02`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: CONTRACT-14 for VC-EVIDENCE-02

- GIVEN the `Contracts-Draft` profile and every field of `input` in [CONTRACT-14](../../../specs/conformance/contract-cases.json)
- WHEN the `admission` procedure for case `CONTRACT-14` runs against those inputs
- THEN the observations match every field of `expected` in case `CONTRACT-14`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: VC-EVIDENCE-03
r[VC-EVIDENCE-03]

**VC-EVIDENCE-03.** Evidence dependency records MUST not use circular attestations as justification. Recursive program proofs belong inside the proof logic with appropriate induction/coinduction principles; they are not cycles in an external trust manifest claiming that each component is trusted because the other is.


<!-- cairn:scenario-links:start -->
#### Scenario: CONTRACT-06 for VC-EVIDENCE-03

- GIVEN the `Contracts-Draft` profile and every field of `input` in [CONTRACT-06](../../../specs/conformance/contract-cases.json)
- WHEN the `admission` procedure for case `CONTRACT-06` runs against those inputs
- THEN the observations match every field of `expected` in case `CONTRACT-06`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: VC-EVIDENCE-04
r[VC-EVIDENCE-04]

**VC-EVIDENCE-04.** Proof verification services MUST enforce the isolated-build and independent-recheck requirements of IMPL-V001. Verification of user proof source is not an undocumented step of ordinary Noble `prepare`.

A later evidence-transport specification must select canonical statement/subject encodings, safe export formats, validated decoding, and a secure rejection protocol. This revision defines the binding obligations without claiming those engineering tasks complete.


<!-- cairn:scenario-links:start -->
#### Scenario: CONTRACT-09 for VC-EVIDENCE-04

- GIVEN the `Contracts-Draft` profile and every field of `input` in [CONTRACT-09](../../../specs/conformance/contract-cases.json)
- WHEN the `review` procedure for case `CONTRACT-09` runs against those inputs
- THEN the observations match every field of `expected` in case `CONTRACT-09`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

## 7. Admission and execution are distinct

### Requirement: VC-ADMIT-01
r[VC-ADMIT-01]

**VC-ADMIT-01.** Admission SHALL separately evaluate ordinary program validity, optional proof evidence, interface compatibility, compilation/artifact provenance or correspondence, runtime policy, and resource authorization. Satisfying one condition does not waive another.

An intended sequence is:

```text
validate subject and expected interface
  -> validate required evidence against the consumer's claim and assumptions
  -> establish source/recipe-to-executable trust under the selected policy
  -> enforce host policy and authorize concrete resources
  -> invoke the already prepared program
```

Services may reorder independent non-executing checks, but all required conditions must hold before guest execution. Evidence should not cause captured secrets to be published or execution authority to be acquired implicitly.


<!-- cairn:scenario-links:start -->
#### Scenario: CONTRACT-08 for VC-ADMIT-01

- GIVEN the `Contracts-Draft` profile and every field of `input` in [CONTRACT-08](../../../specs/conformance/contract-cases.json)
- WHEN the `runtime` procedure for case `CONTRACT-08` runs against those inputs
- THEN the observations match every field of `expected` in case `CONTRACT-08`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: CONTRACT-13 for VC-ADMIT-01

- GIVEN the `Contracts-Draft` profile and every field of `input` in [CONTRACT-13](../../../specs/conformance/contract-cases.json)
- WHEN the `admission` procedure for case `CONTRACT-13` runs against those inputs
- THEN the observations match every field of `expected` in case `CONTRACT-13`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: VC-ADMIT-02
r[VC-ADMIT-02]

**VC-ADMIT-02.** Missing evidence, invalid evidence, disallowed assumptions, mismatched subjects, unsupported proof formats, timeout, and internal verifier failure MUST remain distinguishable. Required evidence that is absent or inconclusive MUST fail closed. A permissive host may run ordinary checked programs without optional proofs, but MUST label that choice rather than fabricate a verification result.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: VC-ADMIT-03
r[VC-ADMIT-03]

**VC-ADMIT-03.** A proof about source or a recipe MUST NOT be used to justify unrelated supplied Wasm. An accepted artifact MUST be tied to that subject through an explicitly trusted build, a verified compilation chain, or appropriate translation validation, each with its real trust boundary reported.


<!-- cairn:scenario-links:start -->
#### Scenario: CONTRACT-13 for VC-ADMIT-03

- GIVEN the `Contracts-Draft` profile and every field of `input` in [CONTRACT-13](../../../specs/conformance/contract-cases.json)
- WHEN the `admission` procedure for case `CONTRACT-13` runs against those inputs
- THEN the observations match every field of `expected` in case `CONTRACT-13`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: VC-ADMIT-04
r[VC-ADMIT-04]

**VC-ADMIT-04.** Cached acceptance results MUST bind the complete applicable subject, claim, assumptions, semantic/toolchain context, and current acceptance-policy revision. A change that affects applicability MUST trigger revalidation. Immutable old evidence may remain valid for its old subject without being acceptable under the current policy.


<!-- cairn:scenario-links:start -->
#### Scenario: CONTRACT-02 for VC-ADMIT-04

- GIVEN the `Contracts-Draft` profile and every field of `input` in [CONTRACT-02](../../../specs/conformance/contract-cases.json)
- WHEN the `admission` procedure for case `CONTRACT-02` runs against those inputs
- THEN the observations match every field of `expected` in case `CONTRACT-02`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: VC-ADMIT-05
r[VC-ADMIT-05]

**VC-ADMIT-05.** Build mode or optional contract policy MUST NOT disable required host/admission checks. Only an established caller relation can discharge a boundary precondition. A failed runtime guard MUST produce a defined rejection or failure rather than undefined behavior.

This revision adds no general runtime assertion syntax. VC-USE-01 selects explicit applicability checks for certified invocation. Guard code is executable behavior, not erasable proof metadata.


<!-- cairn:scenario-links:start -->
#### Scenario: ADAPT-15 for VC-ADMIT-05

- GIVEN the `Core-Bootstrap` profile and every field of `input` in [ADAPT-15](../../../specs/conformance/adaptation-cases.json)
- WHEN the `admission` procedure for case `ADAPT-15` runs against those inputs
- THEN the observations match every field of `expected` in case `ADAPT-15`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: CONTRACT-07 for VC-ADMIT-05

- GIVEN the `Contracts-Draft` profile and every field of `input` in [CONTRACT-07](../../../specs/conformance/contract-cases.json)
- WHEN the `admission` procedure for case `CONTRACT-07` runs against those inputs
- THEN the observations match every field of `expected` in case `CONTRACT-07`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

## 8. First supported contract fragment

### Requirement: VC-BOOT-01
r[VC-BOOT-01]

**VC-BOOT-01.** The first implementation MUST cover pure scalar/structural-data postconditions, explicit wrapping arithmetic, and quotation/builder-family composition. Its proof library MUST support `[ 1 + ]`, two composed increments, and an increment-by-runtime-capture family. Host trace/resource protocols and general termination proofs remain later increments.

The logic is not claimed to be decidable, complete, or fully automated. Proof search failure does not establish falsity. General refinement inference, compressed certificates, proof markets, and a general in-guest proof-term kernel remain outside this revision. The finite rule-replay checker in section 11 is a separate, bounded facility.

PO-15, PO-16, and PO-19 through PO-21 govern this profile. All remain open in [verification/obligations.json](../../../specs/verification/obligations.json). [Contract scenarios](../../../specs/conformance/contract-cases.json) are newly authored expectations, not executed evidence.


<!-- cairn:scenario-links:start -->
#### Scenario: CONTRACT-01 for VC-BOOT-01

- GIVEN the `Contracts-Draft` profile and every field of `input` in [CONTRACT-01](../../../specs/conformance/contract-cases.json)
- WHEN the `review` procedure for case `CONTRACT-01` runs against those inputs
- THEN the observations match every field of `expected` in case `CONTRACT-01`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

## 9. Typed contract inputs

### Requirement: VC-INPUT-01
r[VC-INPUT-01]

**VC-INPUT-01.** The compiler MUST resolve contract declarations into a typed, versioned contract IR with exact program and logical-definition references. The IR MUST bind ordered input/output stacks, initial snapshots, quantifiers, assumptions, and the claim kind. It MUST distinguish preconditions, postconditions, invariants, effect/trace constraints, and ownership predicates. Display names, comments, and unresolved strings MUST NOT serve as accepted propositions.


<!-- cairn:scenario-links:start -->
#### Scenario: CONTRACT-01 for VC-INPUT-01

- GIVEN the `Contracts-Draft` profile and every field of `input` in [CONTRACT-01](../../../specs/conformance/contract-cases.json)
- WHEN the `review` procedure for case `CONTRACT-01` runs against those inputs
- THEN the observations match every field of `expected` in case `CONTRACT-01`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: CONTRACT-14 for VC-INPUT-01

- GIVEN the `Contracts-Draft` profile and every field of `input` in [CONTRACT-14](../../../specs/conformance/contract-cases.json)
- WHEN the `admission` procedure for case `CONTRACT-14` runs against those inputs
- THEN the observations match every field of `expected` in case `CONTRACT-14`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: CONTRACT-15 for VC-INPUT-01

- GIVEN the `Contracts-Draft` profile and every field of `input` in [CONTRACT-15](../../../specs/conformance/contract-cases.json)
- WHEN the `static` procedure for case `CONTRACT-15` runs against those inputs
- THEN the observations match every field of `expected` in case `CONTRACT-15`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: VC-INPUT-02
r[VC-INPUT-02]

**VC-INPUT-02.** The first contract fragment MUST support typed scalar predicates, structural-data predicates, Boolean connectives, and explicit quantified builder parameters. Logical definitions MUST have defined pure semantics, with totality or partiality accounted for. Unbound variables, type mismatches, and unsupported predicates MUST produce explicit diagnostics. An unsupported contract MUST NOT silently weaken its claim or make its otherwise valid subject program ill-typed. Required-proof admission still fails closed.

Recursion invariants and termination measures describe additional obligations rather than implicit termination guarantees. Later host/protocol predicates require the matching formal host model. A source-level declaration can reside beside a program or in an explicitly linked contract module.


<!-- cairn:scenario-links:start -->
#### Scenario: CONTRACT-15 for VC-INPUT-02

- GIVEN the `Contracts-Draft` profile and every field of `input` in [CONTRACT-15](../../../specs/conformance/contract-cases.json)
- WHEN the `static` procedure for case `CONTRACT-15` runs against those inputs
- THEN the observations match every field of `expected` in case `CONTRACT-15`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: VC-INPUT-03
r[VC-INPUT-03]

**VC-INPUT-03.** Proof obligations MUST derive from the actual accepted program representation and typed contract IR. Their translation to Lean MUST preserve subject, statement, arithmetic, captures, effects, and outcome semantics. PO-19 MUST connect that translation to the reviewed Noble model. A proof of a separately rewritten example MUST NOT close an obligation for the production subject.

The two verification paths remain separate:

```text
Noble implementation Rust -> Charon/Aeneas -> Lean refinement
Accepted Noble program + contract IR -> Noble formal semantics -> Lean behavioral proof
```

The second path does not require a Noble-to-Rust compiler. Aeneas can establish implementation properties of the contract frontend and exporter. It does not automatically prove that each exported application claim is true.


<!-- cairn:scenario-links:start -->
#### Scenario: CONTRACT-01 for VC-INPUT-03

- GIVEN the `Contracts-Draft` profile and every field of `input` in [CONTRACT-01](../../../specs/conformance/contract-cases.json)
- WHEN the `review` procedure for case `CONTRACT-01` runs against those inputs
- THEN the observations match every field of `expected` in case `CONTRACT-01`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: CONTRACT-14 for VC-INPUT-03

- GIVEN the `Contracts-Draft` profile and every field of `input` in [CONTRACT-14](../../../specs/conformance/contract-cases.json)
- WHEN the `admission` procedure for case `CONTRACT-14` runs against those inputs
- THEN the observations match every field of `expected` in case `CONTRACT-14`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: VC-INPUT-04
r[VC-INPUT-04]

**VC-INPUT-04.** Ghost values and initial snapshots MUST remain logical data with no influence on executable control flow or host effects. They MUST NOT duplicate resource ownership or make a resource capturable. Logical predicates MUST NOT execute arbitrary Noble bodies, host operations, or tactics during ordinary preparation. A required runtime value cannot exist only as erased ghost data.

Illustrative contract notation:

```text
subject:  [ 1 + ]
input:    tail followed by x:I64
kind:     partial-correctness
requires: true
ensures:  unchanged tail followed by wrap64(x + 1)
effects:  {}
```

This contract concerns normal return. It does not establish total correctness under arbitrary execution budgets.


<!-- cairn:scenario-links:start -->
#### Scenario: CONTRACT-12 for VC-INPUT-04

- GIVEN the `Contracts-Draft` profile and every field of `input` in [CONTRACT-12](../../../specs/conformance/contract-cases.json)
- WHEN the `static` procedure for case `CONTRACT-12` runs against those inputs
- THEN the observations match every field of `expected` in case `CONTRACT-12`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

## 10. First-class companions

| Conceptual entity | Meaning | What possession does not establish |
|---|---|---|
| `Contract` | Typed proposition and its semantic context | Truth of the proposition |
| `Evidence` | Offered proof material or references bound to a subject and claim | Acceptance under the consumer's policy |
| `CertifiedProgram` | An ordinary program paired with evidence accepted for an exact claim and context | Current preconditions, host authority, or Wasm correspondence |

These names identify API roles, not new dependent types or frozen source declarations.

### Requirement: VC-VALUE-01
r[VC-VALUE-01]

**VC-VALUE-01.** The companion API MUST provide explicit construction, inspection, program projection, composition, and family-instantiation operations. It MUST preserve the ordinary program's stack interface, effects, and recipe. A companion MUST NOT become callable through an implicit coercion. Explicit projection returns the underlying ordinary program without a behavioral admission guarantee.


<!-- cairn:scenario-links:start -->
#### Scenario: CONTRACT-03 for VC-VALUE-01

- GIVEN the `Contracts-Draft` profile and every field of `input` in [CONTRACT-03](../../../specs/conformance/contract-cases.json)
- WHEN the `runtime` procedure for case `CONTRACT-03` runs against those inputs
- THEN the observations match every field of `expected` in case `CONTRACT-03`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: CONTRACT-10 for VC-VALUE-01

- GIVEN the `Contracts-Draft` profile and every field of `input` in [CONTRACT-10](../../../specs/conformance/contract-cases.json)
- WHEN the `runtime` procedure for case `CONTRACT-10` runs against those inputs
- THEN the observations match every field of `expected` in case `CONTRACT-10`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: VC-VALUE-02
r[VC-VALUE-02]

**VC-VALUE-02.** Only an evidence-acceptance operation or a checked derivation from accepted evidence can establish certified status. Record constructors, mutable status fields, deserializers, and producer-supplied policy MUST NOT establish that status. The check MUST bind the actual subject, complete proposition, assumptions, instantiation, and acceptance context. Every public construction path must preserve this invariant.


<!-- cairn:scenario-links:start -->
#### Scenario: CONTRACT-02 for VC-VALUE-02

- GIVEN the `Contracts-Draft` profile and every field of `input` in [CONTRACT-02](../../../specs/conformance/contract-cases.json)
- WHEN the `admission` procedure for case `CONTRACT-02` runs against those inputs
- THEN the observations match every field of `expected` in case `CONTRACT-02`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: CONTRACT-06 for VC-VALUE-02

- GIVEN the `Contracts-Draft` profile and every field of `input` in [CONTRACT-06](../../../specs/conformance/contract-cases.json)
- WHEN the `admission` procedure for case `CONTRACT-06` runs against those inputs
- THEN the observations match every field of `expected` in case `CONTRACT-06`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: VC-VALUE-03
r[VC-VALUE-03]

**VC-VALUE-03.** Serialization MUST export inert descriptions, not transferable unchecked acceptance flags. Import MUST reestablish program validity and evidence applicability under the receiving context. Local reuse requires an established invariant for the exact immutable context. A changed policy, semantic dependency, or relevant environment fact MUST invalidate the applicability decision until revalidation.


<!-- cairn:scenario-links:start -->
#### Scenario: CONTRACT-02 for VC-VALUE-03

- GIVEN the `Contracts-Draft` profile and every field of `input` in [CONTRACT-02](../../../specs/conformance/contract-cases.json)
- WHEN the `admission` procedure for case `CONTRACT-02` runs against those inputs
- THEN the observations match every field of `expected` in case `CONTRACT-02`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: CONTRACT-06 for VC-VALUE-03

- GIVEN the `Contracts-Draft` profile and every field of `input` in [CONTRACT-06](../../../specs/conformance/contract-cases.json)
- WHEN the `admission` procedure for case `CONTRACT-06` runs against those inputs
- THEN the observations match every field of `expected` in case `CONTRACT-06`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: VC-VALUE-04
r[VC-VALUE-04]

**VC-VALUE-04.** Companion duplication, discard, aggregate storage, and capture MUST obey recursive `Data` and `Capture` eligibility for their actual payloads. Immutable resource-free descriptions and content references do not grant access to a proof service. Live service capabilities, live resource references, and session handles MUST NOT hide inside supposedly pure evidence. Logical snapshots describe state without owning or duplicating the resource. Export or remote proof work MUST NOT publish captured secrets without explicit authority.


<!-- cairn:scenario-links:start -->
#### Scenario: CONTRACT-10 for VC-VALUE-04

- GIVEN the `Contracts-Draft` profile and every field of `input` in [CONTRACT-10](../../../specs/conformance/contract-cases.json)
- WHEN the `runtime` procedure for case `CONTRACT-10` runs against those inputs
- THEN the observations match every field of `expected` in case `CONTRACT-10`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: CONTRACT-12 for VC-VALUE-04

- GIVEN the `Contracts-Draft` profile and every field of `input` in [CONTRACT-12](../../../specs/conformance/contract-cases.json)
- WHEN the `static` procedure for case `CONTRACT-12` runs against those inputs
- THEN the observations match every field of `expected` in case `CONTRACT-12`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: VC-VALUE-05
r[VC-VALUE-05]

**VC-VALUE-05.** Proof attachment, replacement, and erasure MUST NOT change the underlying program's identity or reflected recipe. Companion metadata can have its own identity and inspection contract. Evidence inspection MUST NOT fetch proof files, start a prover, or acquire authority implicitly. A retained evidence reference alone MUST NOT establish proof validity.


<!-- cairn:scenario-links:start -->
#### Scenario: CONTRACT-10 for VC-VALUE-05

- GIVEN the `Contracts-Draft` profile and every field of `input` in [CONTRACT-10](../../../specs/conformance/contract-cases.json)
- WHEN the `runtime` procedure for case `CONTRACT-10` runs against those inputs
- THEN the observations match every field of `expected` in case `CONTRACT-10`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### 10.1 Certified invocation

### Requirement: VC-USE-01
r[VC-USE-01]

**VC-USE-01.** Certified invocation MUST establish the contract precondition for the actual input before the candidate body executes. The first profile supports established caller proofs and explicit, total, pure scalar/structural applicability guards. Each guard requires a proved link to the precondition and specified cost, limits, and failure behavior. False guards, exhausted budgets, and missing applicability evidence MUST fail closed without candidate-body requests. Arbitrary logical predicates MUST NOT become executable guards by assumption.

A theorem that says `P implies Q` does not establish `P`. Invocation guards preserve the underlying program identity but form part of the wrapper's executable semantics and applicable build identity. Host-state predicates additionally require a protocol that prevents stale observations between admission and use. Those protocols are outside the first resource-free profile.

Ordinary explicit projection and `run` remain available under a permissive host policy. They do not retain a claim of certified invocation. A host that requires a contract must enforce admission independently of the caller's choice of API.


<!-- cairn:scenario-links:start -->
#### Scenario: CONTRACT-07 for VC-USE-01

- GIVEN the `Contracts-Draft` profile and every field of `input` in [CONTRACT-07](../../../specs/conformance/contract-cases.json)
- WHEN the `admission` procedure for case `CONTRACT-07` runs against those inputs
- THEN the observations match every field of `expected` in case `CONTRACT-07`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: CONTRACT-08 for VC-USE-01

- GIVEN the `Contracts-Draft` profile and every field of `input` in [CONTRACT-08](../../../specs/conformance/contract-cases.json)
- WHEN the `runtime` procedure for case `CONTRACT-08` runs against those inputs
- THEN the observations match every field of `expected` in case `CONTRACT-08`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

## 11. Compositional proof library

### Requirement: VC-LIB-01
r[VC-LIB-01]

**VC-LIB-01.** The profile MUST provide Lean-checked rules for supported primitives, sequencing, quotation, invocation, structural eliminators, and branch joins. Higher-order rules MUST state the behavioral assumptions on program operands. Recursion requires explicit invariants or induction principles, with additional measures for termination claims. Unsupported rule families remain visible rather than silently trusted.


<!-- cairn:scenario-links:start -->
#### Scenario: CONTRACT-01 for VC-LIB-01

- GIVEN the `Contracts-Draft` profile and every field of `input` in [CONTRACT-01](../../../specs/conformance/contract-cases.json)
- WHEN the `review` procedure for case `CONTRACT-01` runs against those inputs
- THEN the observations match every field of `expected` in case `CONTRACT-01`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: VC-LIB-02
r[VC-LIB-02]

**VC-LIB-02.** Certified composition MUST check interfaces, semantic contexts, evidence dependencies, and the intermediate implication required by VC-COMPOSE-01. The result MUST bind the actual composed program and the resulting claim. Its assumption set MUST retain all relevant premises. Unresolved implications MUST produce an obligation or explicit failure, never a certified result. Ordinary program composition remains available when its ordinary interfaces match.


<!-- cairn:scenario-links:start -->
#### Scenario: CONTRACT-03 for VC-LIB-02

- GIVEN the `Contracts-Draft` profile and every field of `input` in [CONTRACT-03](../../../specs/conformance/contract-cases.json)
- WHEN the `runtime` procedure for case `CONTRACT-03` runs against those inputs
- THEN the observations match every field of `expected` in case `CONTRACT-03`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: CONTRACT-04 for VC-LIB-02

- GIVEN the `Contracts-Draft` profile and every field of `input` in [CONTRACT-04](../../../specs/conformance/contract-cases.json)
- WHEN the `admission` procedure for case `CONTRACT-04` runs against those inputs
- THEN the observations match every field of `expected` in case `CONTRACT-04`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: VC-LIB-03
r[VC-LIB-03]

**VC-LIB-03.** Family instantiation MUST bind all runtime captures and program operands to the quantified theorem and resulting program identity. The checker MUST establish eligibility, behavioral premises, and instantiation constraints. A theorem for one literal or capture MUST NOT certify another instance. Runtime instances MUST use compiled operations, with preparation and proof search disabled in the first demonstration.


<!-- cairn:scenario-links:start -->
#### Scenario: CONTRACT-05 for VC-LIB-03

- GIVEN the `Contracts-Draft` profile and every field of `input` in [CONTRACT-05](../../../specs/conformance/contract-cases.json)
- WHEN the `runtime` procedure for case `CONTRACT-05` runs against those inputs
- THEN the observations match every field of `expected` in case `CONTRACT-05`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: VC-LIB-04
r[VC-LIB-04]

**VC-LIB-04.** Runtime derivation checks MUST use a finite, versioned rule set whose soundness follows from previously accepted Lean theorems. Derivation records MUST be bounded and acyclic, with checked premises and explicit failures. The checker MUST NOT run tactics, SMT search, or arbitrary proof code. Its Rust implementation follows the mandatory Aeneas route. PO-20 must cover rule replay and the certified-status invariant.

External proof generation and evidence retrieval belong to explicit shell services with capability checks and resource budgets. The deterministic core receives complete observations and returns admission decisions or effect plans. An effect plan is not proof that an external check occurred.


<!-- cairn:scenario-links:start -->
#### Scenario: CONTRACT-03 for VC-LIB-04

- GIVEN the `Contracts-Draft` profile and every field of `input` in [CONTRACT-03](../../../specs/conformance/contract-cases.json)
- WHEN the `runtime` procedure for case `CONTRACT-03` runs against those inputs
- THEN the observations match every field of `expected` in case `CONTRACT-03`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: CONTRACT-05 for VC-LIB-04

- GIVEN the `Contracts-Draft` profile and every field of `input` in [CONTRACT-05](../../../specs/conformance/contract-cases.json)
- WHEN the `runtime` procedure for case `CONTRACT-05` runs against those inputs
- THEN the observations match every field of `expected` in case `CONTRACT-05`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: CONTRACT-06 for VC-LIB-04

- GIVEN the `Contracts-Draft` profile and every field of `input` in [CONTRACT-06](../../../specs/conformance/contract-cases.json)
- WHEN the `admission` procedure for case `CONTRACT-06` runs against those inputs
- THEN the observations match every field of `expected` in case `CONTRACT-06`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

## 12. Verification tooling

### Requirement: VC-TOOL-01
r[VC-TOOL-01]

**VC-TOOL-01.** The toolchain MUST provide verification, proof explanation, and proof-required build operations. The selected CLI command families are `noble verify`, `noble explain-proof`, and `noble build --require-proof`. Exact argument grammar remains open. Reports MUST name source spans, the expected claim, assumptions, dependencies, scope, outstanding obligations, and artifact correspondence.

These commands are specification targets. No Noble executable exists in this repository.


<!-- cairn:scenario-links:start -->
#### Scenario: CONTRACT-09 for VC-TOOL-01

- GIVEN the `Contracts-Draft` profile and every field of `input` in [CONTRACT-09](../../../specs/conformance/contract-cases.json)
- WHEN the `review` procedure for case `CONTRACT-09` runs against those inputs
- THEN the observations match every field of `expected` in case `CONTRACT-09`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: CONTRACT-15 for VC-TOOL-01

- GIVEN the `Contracts-Draft` profile and every field of `input` in [CONTRACT-15](../../../specs/conformance/contract-cases.json)
- WHEN the `static` procedure for case `CONTRACT-15` runs against those inputs
- THEN the observations match every field of `expected` in case `CONTRACT-15`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: VC-TOOL-02
r[VC-TOOL-02]

**VC-TOOL-02.** Claim outcomes MUST distinguish `proved`, `disproved`, `unknown`, `timeout`, `unsupported`, `error`, and `not-run`. The `proved` outcome requires accepted evidence for the exact claim. The `disproved` outcome requires an accepted refutation or a checked counterexample that refutes that claim under Noble semantics. Failed proof checking is not disproof. Exhausted execution fuel is not proof of divergence.

These claim outcomes do not replace SPEC-EV001's independent implementation, execution, proof, and trust fields. A failed proof attempt can accompany a true proposition. A successful test is not a universal proof.


<!-- cairn:scenario-links:start -->
#### Scenario: CONTRACT-09 for VC-TOOL-02

- GIVEN the `Contracts-Draft` profile and every field of `input` in [CONTRACT-09](../../../specs/conformance/contract-cases.json)
- WHEN the `review` procedure for case `CONTRACT-09` runs against those inputs
- THEN the observations match every field of `expected` in case `CONTRACT-09`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: VC-TOOL-03
r[VC-TOOL-03]

**VC-TOOL-03.** Tactics, solvers, and AI-generated proofs MUST remain untrusted producers. Strict proof acceptance requires the selected Lean policy, transitive assumption inspection, and isolated independent rechecking. External solver success requires an accepted proof reconstruction or another explicitly permitted evidence class. A producer MUST NOT weaken the expected claim or strengthen its precondition without independent approval.


<!-- cairn:scenario-links:start -->
#### Scenario: CONTRACT-09 for VC-TOOL-03

- GIVEN the `Contracts-Draft` profile and every field of `input` in [CONTRACT-09](../../../specs/conformance/contract-cases.json)
- WHEN the `review` procedure for case `CONTRACT-09` runs against those inputs
- THEN the observations match every field of `expected` in case `CONTRACT-09`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: CONTRACT-14 for VC-TOOL-03

- GIVEN the `Contracts-Draft` profile and every field of `input` in [CONTRACT-14](../../../specs/conformance/contract-cases.json)
- WHEN the `admission` procedure for case `CONTRACT-14` runs against those inputs
- THEN the observations match every field of `expected` in case `CONTRACT-14`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: VC-TOOL-04
r[VC-TOOL-04]

**VC-TOOL-04.** A proof-required build MUST use a consumer-selected policy and require applicable accepted evidence for every selected obligation. Missing, stale, inconclusive, or disallowed evidence MUST block the claimed artifact release. Tests and review records MUST NOT satisfy a required proof. Proof terms can remain outside Wasm, but selected inspection metadata and executable applicability checks MUST survive erasure. Erasure MUST NOT remove host authorization or admission checks.

Pure proof metadata remains separate from underlying program identity. If a change adds executable assertions or guards, that change affects the wrapper or program whose behavior changes. It is not a proof-only edit.


<!-- cairn:scenario-links:start -->
#### Scenario: CONTRACT-08 for VC-TOOL-04

- GIVEN the `Contracts-Draft` profile and every field of `input` in [CONTRACT-08](../../../specs/conformance/contract-cases.json)
- WHEN the `runtime` procedure for case `CONTRACT-08` runs against those inputs
- THEN the observations match every field of `expected` in case `CONTRACT-08`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: CONTRACT-09 for VC-TOOL-04

- GIVEN the `Contracts-Draft` profile and every field of `input` in [CONTRACT-09](../../../specs/conformance/contract-cases.json)
- WHEN the `review` procedure for case `CONTRACT-09` runs against those inputs
- THEN the observations match every field of `expected` in case `CONTRACT-09`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: CONTRACT-10 for VC-TOOL-04

- GIVEN the `Contracts-Draft` profile and every field of `input` in [CONTRACT-10](../../../specs/conformance/contract-cases.json)
- WHEN the `runtime` procedure for case `CONTRACT-10` runs against those inputs
- THEN the observations match every field of `expected` in case `CONTRACT-10`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: CONTRACT-12 for VC-TOOL-04

- GIVEN the `Contracts-Draft` profile and every field of `input` in [CONTRACT-12](../../../specs/conformance/contract-cases.json)
- WHEN the `static` procedure for case `CONTRACT-12` runs against those inputs
- THEN the observations match every field of `expected` in case `CONTRACT-12`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

## 13. Delivery and conformance gates

### Requirement: VC-GATE-01
r[VC-GATE-01]

**VC-GATE-01.** Before profile implementation acceptance, the project MUST select a versioned contract IR, declaration grammar, companion representation, rule encoding, and API interfaces. The design MUST specify eligibility, concrete construction boundaries, failures, budgets, and evidence decoding. Experimental encodings MUST retain their scope labels and MUST NOT claim portable cross-version interoperability.


<!-- cairn:scenario-links:start -->
#### Scenario: CONTRACT-01 for VC-GATE-01

- GIVEN the `Contracts-Draft` profile and every field of `input` in [CONTRACT-01](../../../specs/conformance/contract-cases.json)
- WHEN the `review` procedure for case `CONTRACT-01` runs against those inputs
- THEN the observations match every field of `expected` in case `CONTRACT-01`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: CONTRACT-15 for VC-GATE-01

- GIVEN the `Contracts-Draft` profile and every field of `input` in [CONTRACT-15](../../../specs/conformance/contract-cases.json)
- WHEN the `static` procedure for case `CONTRACT-15` runs against those inputs
- THEN the observations match every field of `expected` in case `CONTRACT-15`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: VC-GATE-02
r[VC-GATE-02]

**VC-GATE-02.** `Contracts-Draft` conformance MUST include `[ 1 + ]`, certified increment composition, and an increment-by-runtime-capture family. The runtime demonstration MUST pass, return, inspect, and store eligible companions in aggregates. It MUST include positive applicability checks and rejection of forged evidence, changed captures, unmet preconditions, unresolved composition, and unrelated Wasm. Literal-only specialization or host-only metadata cannot satisfy this gate.


<!-- cairn:scenario-links:start -->
#### Scenario: CONTRACT-03 for VC-GATE-02

- GIVEN the `Contracts-Draft` profile and every field of `input` in [CONTRACT-03](../../../specs/conformance/contract-cases.json)
- WHEN the `runtime` procedure for case `CONTRACT-03` runs against those inputs
- THEN the observations match every field of `expected` in case `CONTRACT-03`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: CONTRACT-05 for VC-GATE-02

- GIVEN the `Contracts-Draft` profile and every field of `input` in [CONTRACT-05](../../../specs/conformance/contract-cases.json)
- WHEN the `runtime` procedure for case `CONTRACT-05` runs against those inputs
- THEN the observations match every field of `expected` in case `CONTRACT-05`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: CONTRACT-06 for VC-GATE-02

- GIVEN the `Contracts-Draft` profile and every field of `input` in [CONTRACT-06](../../../specs/conformance/contract-cases.json)
- WHEN the `admission` procedure for case `CONTRACT-06` runs against those inputs
- THEN the observations match every field of `expected` in case `CONTRACT-06`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: CONTRACT-10 for VC-GATE-02

- GIVEN the `Contracts-Draft` profile and every field of `input` in [CONTRACT-10](../../../specs/conformance/contract-cases.json)
- WHEN the `runtime` procedure for case `CONTRACT-10` runs against those inputs
- THEN the observations match every field of `expected` in case `CONTRACT-10`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: VC-GATE-03
r[VC-GATE-03]

**VC-GATE-03.** Profile reports MUST include applicable PO-15, PO-16, and PO-19 through PO-21 results and their implementation correspondence. Conformance requires accepted evidence for these obligations in the declared fragment, including actual Rust correspondence. Open obligations permit only an experimental report. Wasm demonstrations MUST separately identify their build/loading evidence under PO-17/18. A trusted-build demonstration MUST NOT claim a verified backend. Document validation and test execution MUST NOT close unproved theorem obligations.

[ROADMAP.md](../../../specs/ROADMAP.md) schedules MC1 after checker/proof feasibility and MC2 after MC1 and the Wasm core. Resource and concurrency contracts remain separate later increments. Ordinary core delivery does not depend on optional application proofs.


<!-- cairn:scenario-links:start -->
#### Scenario: CONTRACT-08 for VC-GATE-03

- GIVEN the `Contracts-Draft` profile and every field of `input` in [CONTRACT-08](../../../specs/conformance/contract-cases.json)
- WHEN the `runtime` procedure for case `CONTRACT-08` runs against those inputs
- THEN the observations match every field of `expected` in case `CONTRACT-08`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: CONTRACT-13 for VC-GATE-03

- GIVEN the `Contracts-Draft` profile and every field of `input` in [CONTRACT-13](../../../specs/conformance/contract-cases.json)
- WHEN the `admission` procedure for case `CONTRACT-13` runs against those inputs
- THEN the observations match every field of `expected` in case `CONTRACT-13`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->
