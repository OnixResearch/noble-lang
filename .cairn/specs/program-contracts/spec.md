# Program Contracts and Verification Evidence

<!-- cairn:purpose:start -->
## Purpose

This accepted specification records Noble draft contracts, not completed implementation.
Original requirement IDs, explanatory prose, examples, and open decisions remain authoritative.
Scenario clauses declare designs; the conformance ledger records execution and evidence.

## Requirements

<!-- cairn:purpose:end -->


Document: SPEC-V002  
Revision: 0.1.0-draft.5  
Status: MC1 frontend/rules and verification CLI plus bounded MC2-v1 pure first-class companions and proof-required builds implemented; wider profile and refinement open  
Depends on: [SPEC-V001](../verification/spec.md), [IMPL-V001](../verification-toolchain/spec.md) at 0.1.0-draft.5

[SOURCES.md](../../../specs/SOURCES.md) records inherited baseline citations.

MC1 delivers the frontend, source/IR revision 1, and proof-rule/CLI fragment described below. MC2 separately delivers bounded resource-free first-class runtime companions and proof-required admission under its retained [completion record](../../../verification/mc2/evidence.json); new owner-law release policy VC-OWNER-01 is not part of that completion.

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

#### Scenario: CONTRACT-16 for VC-SCOPE-01

- GIVEN the `Contracts-Draft` profile and every field of `input` in [CONTRACT-16](../../../specs/conformance/contract-cases.json)
- WHEN the `admission` procedure for case `CONTRACT-16` runs against those inputs
- THEN the observations match every field of `expected` in case `CONTRACT-16`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: VC-SCOPE-02
r[VC-SCOPE-02]

**VC-SCOPE-02.**

Noble MUST support typed contract declarations and first-class evidence companions through optional `Contracts-Draft`. The additional optional `Intrinsic-Proofs-Draft` selects checked proof declarations inside versioned Noble `.noble` modules, rather than only external Lean application proof files. Lean proof modules remain available as separately checked MC1 artifacts and Lean is the first independent target for compiled Noble proof terms. Ordinary `Program<S,T,e>` types and execution semantics MUST remain unchanged. Neither profile introduces dependent `Program` parameters, implicit behavioral coercions, mandatory proofs for ordinary execution, or an in-guest prover.


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

Sections 9–13 select the compiler and companion API contracts. MC1 implements the versioned declaration grammar and typed IR in section 8.1 and the verification/explanation commands in section 12. Its proof-declaration JSON is an experimental host-verification format, not a portable guest companion encoding. MC2 implements bounded first-class companion operations, representation, runtime replay, and proof-required admission in the resource-free Core-Bootstrap fragment, with execution and assurance scoped by its own evidence; this does not complete all Contracts-Draft obligations or general proof of source/backend correctness. Unless explicitly labeled MC1 source, examples in this specification remain mathematical or harness notation. The profile is a required project deliverable but optional for application use.

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

#### Scenario: CONTRACT-16 for VC-LOGIC-02

- GIVEN the `Contracts-Draft` profile and every field of `input` in [CONTRACT-16](../../../specs/conformance/contract-cases.json)
- WHEN the `admission` procedure for case `CONTRACT-16` runs against those inputs
- THEN the observations match every field of `expected` in case `CONTRACT-16`

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

#### Scenario: CONTRACT-20 for VC-ID-02

- GIVEN the `Intrinsic-Proofs-Draft` profile and every field of `input` in [CONTRACT-20](../../../specs/conformance/contract-cases.json)
- WHEN the `admission` procedure for case `CONTRACT-20` runs against those inputs
- THEN the observations match every field of `expected` in case `CONTRACT-20`

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

#### Scenario: CONTRACT-16 for VC-ADMIT-03

- GIVEN the `Contracts-Draft` profile and every field of `input` in [CONTRACT-16](../../../specs/conformance/contract-cases.json)
- WHEN the `admission` procedure for case `CONTRACT-16` runs against those inputs
- THEN the observations match every field of `expected` in case `CONTRACT-16`

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

PO-15, PO-16, and PO-19 through PO-21 govern this profile. Their scoped statuses remain in [verification/obligations.json](../../../specs/verification/obligations.json); MC1 evidence does not close their later runtime, host, or whole-language portions. [Contract scenarios](../../../specs/conformance/contract-cases.json) retain independent design/execution states. Durable MC1 records belong in [evidence.json](../../../verification/mc1/evidence.json) and [acceptance.json](../../../verification/mc1/acceptance.json), with source/tool identities and trust boundaries; neither this specification nor a fixture alone is an acceptance result.


<!-- cairn:scenario-links:start -->
#### Scenario: CONTRACT-01 for VC-BOOT-01

- GIVEN the `Contracts-Draft` profile and every field of `input` in [CONTRACT-01](../../../specs/conformance/contract-cases.json)
- WHEN the `review` procedure for case `CONTRACT-01` runs against those inputs
- THEN the observations match every field of `expected` in case `CONTRACT-01`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### 8.1 Delivered MC1 source and IR

The production [`noble-contracts`](../../../crates/noble-contracts/src/lib.rs) crate is `no_std` with explicit allocation and finite preparation budgets. It implements a pure, optional contract frontend, not a general Noble source compiler or guest evaluator. One source file contains one `(contract 1 name ...)` container. Version 1 fields are:

| Field | MC1 meaning |
|---|---|
| `(input (name Type) ...)` | Required ordered input bindings; empty is permitted |
| `(output (name Type) ...)` | Required ordered normal-return output bindings |
| `(program [ instruction ... ])` | Required executable subject |
| `(requires expression)` | Required Boolean precondition; cannot refer to final outputs, directly or through definitions |
| `(ensures expression)` | Required Boolean normal-return postcondition |
| `(params (name Type) ...)` | Optional universally quantified ghost parameters; not runtime stack entries |
| `(define name Type expression)` | Zero or more typed, total logical definitions, resolved in declaration order |
| `(kind partial-correctness)` | Optional explicit spelling of the only supported claim kind |

Fields need not follow the table's order. Each field except `define` occurs at most once; names cannot be rebound within the same namespace. References use `(in name)`, `(out name)`, `(param name)`, or `(def name)` explicitly. Initial inputs and final outputs are distinct namespaces even when their display names coincide. Forward/cyclic logical definitions, unknown names, and type mismatches are rejected.

The grammar uses parenthesized forms, bracketed program bodies, ASCII identifiers, decimal `I64` literals, `true`, `false`, `unit`, and `;` line comments. Supported types are `Unit`, `Bool`, `I64`, `Text`, `Syntax`, `(Pair A B)`, `(Sum A B)`, `(List A)`, and pure `(Program (InputTypes ...) (OutputTypes ...))`. Stack lists are bottom-first; the last entry is the top. `Text` bindings are supported, but text literals are not.

Program instructions are integer/Boolean literals, the pure bootstrap words `dup`, `drop`, `swap`, `dip`, `+`, `-`, `*`, `=`, `quote`, `compose`, `run`, `reflect`, `unit`, `pair`, `unpair`, `inl`, `inr`, `case`, `if`, `nil`, `cons`, and `list.case`, and typed quotations:

```text
(block (InputTypes ...) (OutputTypes ...) [ instruction ... ])
```

Nested bare `[ ... ]` quotations are rejected: their interfaces must be explicit. An explicit word instantiation can use `(word ID witness ...)`, with witnesses `(stack Type ...)`, `(value Type)`, `(effect)`, or `(ref ID)` in the builtin's binding order. IDs and witnesses refer to the fixed bootstrap environment, not producer-selected definitions. Inference supplies ordinary typing witnesses where possible; the inherited `noble-kernel` acceptance checker remains authoritative.

Logical expressions include Boolean `not`, `and`, `or`, `implies`; structural `eq`; signed `lt`/`le`; wrapping `add`/`sub`/`mul`; and pair/sum/list construction and observation. Constructors with a needed type annotation are `(inl OtherType expression)`, `(inr OtherType expression)`, and `(nil ItemType)`. Structural observations include `first`, `second`, `is-left`, `left`, `right`, `is-nil`, `head`, `tail`, and `length`. `cons` takes an item and a list.

`(maps program input expected)` states that **every normal result** of the pure program on the single input value equals the single expected output value. The `Program` must have exactly one input and one output of the corresponding scalar/structural types. This is the logical `Maps`/normal-return relation, not execution of the program during preparation, a decidable runtime guard, or a promise that any normal result exists. For example, the actual [capture-family fixture](../../../verification/mc1/family.noble-contract) is:

```text
(contract 1 addition-builder-family
  (input (n I64))
  (output (p (Program (I64) (I64))))
  (params (x I64))
  (program [ quote (block (I64 I64) (I64) [ + ]) compose ])
  (requires true)
  (ensures (maps (out p) (param x) (add (param x) (in n)))))
```

Here `n` is an actual runtime input captured by `quote`; `x` is a universally quantified logical argument to the returned program. A ghost reference cannot appear in the executable body to manufacture a capture. Logical terms do not run arbitrary program bodies, host calls, or tactics. Snapshot/ghost data does not duplicate resource ownership.

The logical evaluator distinguishes undefined terms from false Boolean values. `head`/`tail` of an empty list and `left`/`right` of the wrong sum alternative are undefined. Boolean operators propagate undefined operands; they are not short-circuit guards, and `not` of an undefined expression is not true. A predicate establishes an assertion only when it evaluates to true. Such partial projections are permitted in predicates, but not in total `define` bodies; MC1 does not infer path-sensitive totality.

All `I64` arithmetic uses two's-complement 64-bit wrapping, including logical `add`, `sub`, and `mul`; `lt`/`le` compare signed interpretations. Source literals must fit `I64`. List `length` returns an `I64` modulo \(2^{64}\), not an unbounded natural. The [signed-wrap fixture](../../../verification/mc1/wrap.noble-contract) covers the maximum signed value plus one; the [monotonicity refutation](../../../verification/mc1/monotonic-refutation.lean) refutes unconditional signed increase.

Unsupported corners are explicit: text literals; `eq` on `Program`, `Syntax`, or structures containing them; `maps` inputs/results containing those types; resources, host/effectful instructions and nonempty effects; invariants, effect/trace or ownership clauses; producer-supplied assumption clauses; prefix-safety and total-correctness claims. Unsupported features are not silently dropped or reinterpreted as partial correctness. Logical elaboration follows ordinary kernel acceptance, and a later logical diagnostic retains that ordinary acceptance result instead of declaring the subject ill-typed.

Successful preparation returns an immutable `Prepared`: private construction and read-only access retain the exact accepted candidate, acceptance request/result, typed binding namespaces, indexed logical definitions, expression types/spans, and pre/postcondition roots. `export_lean` accepts only this prepared object, not arbitrary unaccepted source or producer proposition text. The exported `MC1Obligation` declares `irRevision = 1`; the inherited candidate format is 1 and its semantic revision is 0 for `0.1.0-draft.5`. These revision identities are separate. Names and comments are not propositions, and this experimental representation is not a stable cross-version encoding.

The generated claim universally quantifies an unchanged bottom-stack prefix, typed declared inputs, and typed ghost parameters. If the precondition holds, every modeled normal return must preserve that prefix, have exactly the declared output types, and satisfy the postcondition with distinct initial and final observations. The ideal pure model abstracts from allocation/quota failures. It proves neither termination nor safety of abnormal outcomes, invocation applicability, host/resource behavior, or backend/runtime correspondence.

The CLI uses the frontend defaults: 65,536 source bytes, 16,384 metered nodes, nesting depth 64, and 2,000,000 work units divided between frontend work and inherited acceptance. Type size and stack height are bounded at 256. These are rejection limits, not promises that every smaller source succeeds; inference and kernel checks have their own bounded work within that budget. Preparation exhaustion reports `error`, not `proved`, `disproved`, or divergence.

The [MC1 source/proof fixtures](../../../verification/mc1) cover increment, composed increments, the universal capture family, structural data, reflected syntax, signed wrap, and a refuted arithmetic claim. Additional [frontend fixtures](../../../crates/noble-contracts/src/fixtures) demonstrate source forms. Fixture existence is distinct from the executed records linked above.

### 8.2 Source, model, and rule boundaries

The [application library](../../../proofs/mc1/NobleContracts.lean) provides the reviewed pure `Exec`, typed stack/claim definitions, wrapping arithmetic, structural rules, sequencing with explicit intermediate implications, quotation/invocation, branch rules, and the builder-family proofs. Application proofs and refutations are strict Lean-kernel results under the allowed logical foundations; this library imports no native-evaluated implementation-correspondence modules.

The separate [implementation library](../../../proofs/mc1/NobleContractImpl.lean) concerns actual production Rust extracted by Charon/Aeneas, not a handwritten substitute. Universal node/body projection theorems preserve literal bits, primitive identity, quotation edges, and order for their stated inputs. The full `prepare`/`export_lean` equations are source-bound to the increment, composed, capture-family, structural, syntax, and signed-wrap matrix and include the inherited checker linkage. These equations use native evaluation and require its separately inventoried expanded-trust assumption. They are not strict application proofs or universal correctness of parsing, inference, acceptance, all logical expressions, or the whole frontend.

Successful extraction/Lean compilation alone is not a refinement theorem; successful application checking does not establish Rust implementation correctness. Neither lane claims runtime admission, first-class guest evidence construction/replay, MC2/Wasm, whole-language preservation, general termination, host effects, or resource protocols. [IMPL-V001](../verification-toolchain/spec.md#72-mc1-evidence-boundaries) specifies the separation and evidence accounting.

### 8.3 MC2 companion implementation and evidence boundaries

MC2 extends the selected resource-free Core-Bootstrap runtime with immutable Contract, Evidence and Certified cells. These are actual compiled Wasm values: they can cross program inputs/results and homogeneous aggregates, retain inspection metadata after debug stripping, and explicitly project their underlying Program. A producer's serialized certification flag or constructor tag carries no admission authority. This experimental runtime representation is not a stable cross-host encoding.

The deterministic [`companion` core](../../../crates/noble-contracts/src/companion) decides admission from an opaque request and a complete host-check observation. The source-bound CLI runs the isolated independent Lean consumer on the exact expected statement and submitted evidence bytes. Completion checks statement, bytes, evidence class and the retained/current context before minting evidence. Arbitrary host Rust can construct an observation: its authenticity and the independent checker's soundness are explicit trusted-host assumptions, not consequences of Rust constructor privacy.

Certified composition requires the exact intermediate implication and compatible interfaces and context. Capture-family instantiation binds the value supplied after compilation to the derived subject. Derivations use the finite versioned rules and bounded, acyclic replay; unknown rules, missing premises, cycles and exhausted budgets fail explicitly. These operations do not run proof search or candidate preparation services. Retained evidence is not a capability to invoke hosts or bypass resource limits.

Invocation also requires an installed applicability guard for the exact contract. An admitted theorem does not automatically synthesize a guard. A false precondition rejects before the candidate body starts; an unsupported guard remains unsupported. Companion operations preserve the original aggregate stack tail and live identities, including restoration on refusal. Explicit proof erasure projects the same underlying Program; it does not turn conditional correctness into unconditional applicability.

`noble companions` provides streaming sessions over these operations; [`README.md`](../../../README.md#mc2-companion-sessions-and-proof-required-builds) documents executable commands, outcomes and setup. `noble build --require-proof` admits only an applicable independently checked proof and binds the exact accepted quotation to the emitted artifact. Its report separates the claim outcome from release permission. This is a trusted-build correspondence boundary, not a verified-backend theorem. Ordinary `noble run` remains available without contract evidence or proof services.

The [semantic certificate gate](../../../proofs/mc2/MC2Gate.lean), [actual-source extraction gate](../../../verification/mc2/extraction.mjs) and [source-bound implementation gate](../../../verification/mc2/implementation.mjs) have separate obligations. Strict semantic theorems, strict extracted correspondences, renderer-layout equations, closed native-evaluated source equations and runtime controls must not be conflated. Native evaluation belongs only to its disclosed implementation lane, never the strict application-proof admission policy.

The [canonical CONTRACT workload](../../../specs/conformance/contract-cases.json) and its per-case evidence record execution independently of proof status. The runtime gate must execute all fifteen cases and every declared variant, including real consumer statement mismatches and actual replay/registry exhaustion, against retained frozen executable bytes. The [status ledger](../../../specs/STATUS.json) and final source-bound receipts determine milestone acceptance. General frontend/compiler/backend refinement, host effects, live resources, total correctness and the broad PO-16/20/21 obligations remain outside this bounded fragment.

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

This section remains the selected first-class API contract. MC1 delivers its interface/design selection only: its host report and proof JSON are not guest `Contract`, `Evidence`, or `CertifiedProgram` values. Passing, returning, inspecting, capturing, storing, composing, and invoking such companions at runtime remain separate implementation and conformance obligations.

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

MC1 implements the pure Lean rule library and explicit semantic composition premises. A Lean `TypedPC` theorem is mathematical evidence, not a constructed runtime companion. The finite guest replay checker, family-instantiation API, certified-status invariant, and applicability/admission operations below remain future runtime work.

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

**VC-TOOL-01.** The toolchain MUST provide verification, proof explanation, and proof-required build operations. The selected CLI command families are `noble verify`, `noble explain-proof`, and `noble build --require-proof`. MC1 implements the first two with the argument grammar below; MC2 implements proof-required builds for its separately evidenced pure fragment. Reports MUST name source spans, the expected claim, assumptions, dependencies, scope, outstanding obligations, and artifact correspondence.

The `noble-cli` package builds the binary named `noble`:

```text
noble verify CONTRACT [--emit DIR] [--proof FILE | --refutation FILE] [--timeout-ms N]
noble explain-proof CONTRACT
```

The contract path precedes options. Each option occurs at most once; `--proof` and `--refutation` are mutually exclusive. `explain-proof` accepts no options. A valid explanation prepares the subject and emits the exact statement without launching proof tools. `verify` without evidence likewise performs no proof search and returns `unknown`; it can still emit the statement. `--timeout-ms` accepts 1–600000, default 120000, for the whole proof workflow including rule-library compilation.

Both commands print a `noble-mc1-report/v1` JSON report. It separates ordinary acceptance, the exact source and generated claim, typed IR, logical assumptions, selected library sources, application outcome, implementation refinement, and backend correspondence. Supplied UTF-8 proof source must declare `MC1Proof.proof : MC1Obligation.claim` or `MC1Proof.refutation : Not MC1Obligation.claim`; the usual import is `MC1Obligation`. Producer status text does not establish acceptance.

`--emit DIR` creates a new directory without overwriting an existing destination. It retains `report.json` plus available `contract.noble`, `MC1Obligation.lean`, submitted `MC1Proof.lean`, and accepted `MC1Proof.json` declaration data. These files are host verification artifacts, not proof-carrying guest values or portable runtime certificates.

See the [README invocation examples](../../../README.md#using-mc1-contracts) and [sandbox/tool prerequisites](../verification-toolchain/spec.md#52-mc1-consumer-configuration-and-limits). Explanation and statement generation do not require executable Lean/sandbox tools; they report the selected source-library availability without executing it. Actual evidence checking requires the pinned tools and Linux isolation, and fails closed when those prerequisites are unavailable.


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

MC1 maps the seven outcomes to process exits:

| Outcome | Exit | MC1 condition |
|---|---:|---|
| `proved` | 0 | Fresh independent consumer accepts the exact application theorem |
| `disproved` | 1 | Fresh independent consumer accepts its refutation |
| `unknown` | 3 | Valid prepared claim, no supplied proof/refutation |
| `timeout` | 124 | Total proof deadline or recognized CPU-time limit exhausted |
| `unsupported` | 4 | Unsupported source/profile or missing/incompatible required checking configuration |
| `error` | 2 | Invalid source/options, preparation exhaustion, rejected proof, I/O or other checking failure |
| `not-run` | 0 | Successful explanation with no evidence checking |

Malformed or rejected proof material is `error`, not a refutation. Resource-limit failures other than recognized time exhaustion can also report `error`; no resource failure establishes a semantic claim.


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

**VC-GATE-01.**

Before profile implementation acceptance, the project MUST select a versioned contract IR, declaration grammar, companion representation, rule encoding, and API interfaces. The design MUST specify eligibility, concrete construction boundaries, failures, budgets, and evidence decoding. Experimental encodings MUST retain their scope labels and MUST NOT claim portable cross-version interoperability.

MC1 selects experimental `contracts-draft/v1`: a bounded s-expression declaration `(contract 1 name (input ...) (output ...) (params ...) (define ...) (program [ ... ]) (requires ...) (ensures ...))`. Parameters and pure nonrecursive logical definitions are optional. Binding references MUST explicitly distinguish initial input, final output, quantified parameter and logical-definition indices; scalar and structural expression operands MUST be typed. The ordered typed expression graph and the actual kernel-accepted candidate MUST be retained as the obligation subject. Program quotation interfaces MUST be explicit when not inferable. Ghost values MUST NOT synthesize executable literals or resources. Unsupported predicates and invalid bindings MUST remain separate diagnostic outcomes.

The selected MC1 semantic model is resource-free, pure normal-return partial correctness with wrapping `I64` arithmetic. It includes scalar/structural predicates, Boolean connectives, preserved stack tails, and universally quantified addition-builder rules. Every partial logical projection MUST have a defined failure interpretation, and undefined assertions MUST NOT become accepted truths. Totality and host protocols are not inferred.

Companion records are immutable descriptions: contract IR/context, inert offered evidence, and an abstract accepted program/evidence pairing whose construction is restricted to independent acceptance or checked derivation. Experimental rule records MUST name a version, exact subjects/claims, prior premise indices, context and assumptions. Construction and import MUST NOT accept supplied certified flags; replay MUST be finite, bounded and acyclic. Composition MUST retain premises and require its intermediate implication. These interfaces specify MC2 implementation obligations; MC1 host proof reports MUST NOT claim Wasm companion, runtime guard or portable-encoding conformance.

MC1 CLI operations are `noble verify CONTRACT [--emit DIR] [--proof FILE | --refutation FILE] [--timeout-ms N]` and `noble explain-proof CONTRACT`. Verification MUST regenerate the independently selected expected claim, isolate proof source execution, independently recheck the exact declaration and transitive assumptions, and retain the seven claim outcomes. Explanation MUST NOT invoke proof code. Accepted application evidence MUST remain separate from implementation refinement and backend correspondence.


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

#### Scenario: Versioned typed increment export

- GIVEN a version 1 declaration for the actual accepted `[ 1 + ]` program and the unconditional wrapping postcondition with distinct initial/final references
- WHEN the contract frontend and Lean export execute
- THEN the typed IR and exact proposition are retained and an independently checked theorem is required before reporting proved

#### Scenario: Invalid and unsupported declarations remain distinct

- GIVEN an ordinarily valid increment subject with an unbound variable, a predicate type mismatch or an unsupported host predicate
- WHEN optional contract elaboration runs
- THEN the contract is rejected with the corresponding invalid or unsupported diagnostic without weakening its claim or reclassifying the ordinary subject as ill-typed

#### Scenario: No evidence by compiler success

- GIVEN an accepted program and an emitted contract proposition but absent or invalid application proof material
- WHEN verify or explain-proof reports the result
- THEN no application proved result, termination guarantee, Wasm correspondence or certified runtime admission is reported

### Requirement: VC-GATE-02
r[VC-GATE-02]

**VC-GATE-02.** `Contracts-Draft` conformance MUST include `[ 1 + ]`, certified increment composition, and an increment-by-runtime-capture family. The runtime demonstration MUST pass, return, inspect, and store eligible companions in aggregates. It MUST include positive applicability checks and rejection of forged evidence, changed captures, unmet preconditions, unresolved composition, and unrelated Wasm. Literal-only specialization or host-only metadata cannot satisfy this gate.

MC2 acceptance MUST execute all fifteen declared CONTRACT cases and every declared variant against actual implementations. Expected fields MUST NOT be copied into observed results. Refusals at an earlier unrelated boundary MUST NOT substitute for a named consumer, replay or applicability control. Independent-checker controls MUST run the real isolated Lean consumer; service-disabled runtime controls MUST demonstrate that no proof service is available to the tested path.

Runtime receipts MUST preserve actual inputs, commands, outcomes, complete ordered stacks, relevant live handles, reflected recipes, counters and artifact identities. Optimized aggregate transport MUST retain the original tail and companion identity. Executable bytes MUST be frozen before subprocess use and checked again before accepting a receipt; mutable build output paths alone are insufficient provenance.


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

#### Scenario: Complete runtime and hostile-evidence matrix

- GIVEN the complete canonical CONTRACT case set and every named input variant
- WHEN the actual selected runtime and independent evidence consumer execute the positive and hostile paths
- THEN retained observations satisfy every expected field, including exact subject binding, real bounded exhaustion, zero failed-path candidate execution and unchanged ordinary execution

#### Scenario: Immutable executable and live-cell evidence

- GIVEN frozen CLI and control executables and optimized first-class companion values
- WHEN argument/result/list transport, inspection, projection and checked operations execute
- THEN raw receipts bind the executed bytes and preserve the original compiled program, aggregate tail, live identity and reflected recipe without implicit evidence fetching

### Requirement: VC-GATE-03
r[VC-GATE-03]

**VC-GATE-03.** Profile reports MUST include applicable PO-15, PO-16, and PO-19 through PO-21 results and their implementation correspondence. Conformance requires accepted evidence for these obligations in the declared fragment, including actual Rust correspondence. Open obligations permit only an experimental report. Wasm demonstrations MUST separately identify their build/loading evidence under PO-17/18. A trusted-build demonstration MUST NOT claim a verified backend. Document validation and test execution MUST NOT close unproved theorem obligations.

MC2 reports MUST distinguish strict semantic certificates, strict extracted correspondences, closed native-evaluated source equations, runtime observations and full source/compiler-policy coverage. Fresh extraction MUST bind current authored sources and actual transparent generated bodies to the reviewed inventory, models, compiled declarations, dependencies and axioms; stale or substituted generated code MUST be rejected. Host-check observation authenticity and independent-checker soundness MUST remain explicit assumptions rather than claims of arbitrary-host constructor unforgeability.

The final milestone record MUST retain the complete unchanged quality policy, complete all-target source/architecture coverage and exact evidence scopes. Source/status/proof ledgers and the completed lifecycle archive MUST agree with the retained final receipts. Broad unproved obligations MUST remain open beyond each explicitly accepted fragment.


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

#### Scenario: Source-bound assurance without scope promotion

- GIVEN current Rust sources, selected tools and a reviewed extraction lock with disclosed external models and axioms
- WHEN independent extraction, compiled audits, refusal controls and full unchanged source/compiler-policy gates pass
- THEN the report accepts only the evidenced fragment, distinguishes native equations from strict proofs and leaves universal backend and other unproved claims open

### Requirement: VC-OWNER-01
r[VC-OWNER-01]

**VC-OWNER-01.**

For an explicitly proof-required release of an MC2-supported, resource-free Core-Bootstrap program, the release owner MUST freeze an immutable law record independently of and before candidate edits. The record MUST name the expected subject interface/schema and selected effect-free fragment, theorem declaration and expected statement (including initial/final observations and normal-return versus totality scope), explicit preconditions and model/host/quota assumptions, immutable owner-law revision, accepted proof-library requirements and independently selected consumer policy. It need not know the future candidate's body, recipe or artifact at authoring time. At release, a separately controlled gate MUST bind that frozen law to the exact submitted resolved program/recipe identity, generated proposition, independently checked theorem declaration and proof artifact, and final emitted artifact before admission. The candidate author or AI editor MUST NOT silently edit the law, substitute an easier theorem, forge evidence, or turn an ordinary runtime result into a proof; any owner-approved law change creates a new review subject and requires fresh proof/admission. Missing, stale, weakened, unproved or inapplicable bindings block only the proof-required release, not ordinary well-typed program execution under VC-SCOPE-01.

An adversarial challenge MUST include a candidate that preserves an example output but violates the owner law, an altered law/proof statement, an unrelated or stale artifact, a vacuous precondition, and an owner-approved law revision. The expected gate rejects the first four without granting a proof claim; the last can proceed only after independent rechecking under the new law identity. This is an additional release-policy test, not a claim that MC2's already executed CONTRACT-09/11/13 scenarios or existing VC-ADMIT-01–05 failed, nor a proof of the compiler, backend or host. Do not claim general Noble source verification, resource/host protocols, termination, or backend correspondence from the pure MC2 fragment.


<!-- cairn:scenario-links:start -->
#### Scenario: CONTRACT-16 for VC-OWNER-01

- GIVEN the `Contracts-Draft` profile and every field of `input` in [CONTRACT-16](../../../specs/conformance/contract-cases.json)
- WHEN the `admission` procedure for case `CONTRACT-16` runs against those inputs
- THEN the observations match every field of `expected` in case `CONTRACT-16`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: CONTRACT-23 for VC-OWNER-01

- GIVEN the `Intrinsic-Proofs-Draft` profile and every field of `input` in [CONTRACT-23](../../../specs/conformance/contract-cases.json)
- WHEN the `admission` procedure for case `CONTRACT-23` runs against those inputs
- THEN the observations match every field of `expected` in case `CONTRACT-23`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: DX-13 for VC-OWNER-01

- GIVEN the `Property-Test-Design` profile and every field of `input` in [DX-13](../../../specs/conformance/language-workflow-cases.json)
- WHEN the `review` procedure for case `DX-13` runs against those inputs
- THEN the observations match every field of `expected` in case `DX-13`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

#### Scenario: CONTRACT-16 independent owner-law challenge

- GIVEN an owner-approved immutable pure law, exact expected theorem statement, recorded assumptions, separately accepted proof artifact and hostile candidate revisions
- WHEN a proof-required release consumer compares the independently owned law and prepared claim to the checked declaration and emitted artifact
- THEN changes to the candidate, law, statement, assumption, proof or artifact invalidate the prior admission; ordinary proof-free execution remains governed by its existing gate

### Requirement: VC-INTRINSIC-01
r[VC-INTRINSIC-01]

**VC-INTRINSIC-01.**

The `contract 1` module member specified by K-PROOF-01 MUST elaborate its `subject` from an actual independently accepted, closed, nonrecursive, resource-free, pure Core-Bootstrap definition into the same typed MC1-v1 logical semantics and exact subject claim as an equivalent MC1 contract. Inputs/outputs are ordered and named; `requires` and `ensures` use MC1's typed total/predicate vocabulary, including explicitly distinct `(in name)` and `(out name)` references, `I64` wrapping arithmetic, and undefined-predicate refusal. Only true-valued predicates establish claims. The consumer independently derives its normal-return partial-correctness obligation with preserved bottom-stack prefix, exact resolved recipe/dependencies, complete stack/effect/schema interface, contract/module/model revisions and immutable assumptions. It MUST refuse a contract with an incompatible subject/interface, host/resource references, unsupported claim kind or producer assumptions rather than weaken or silently copy the claim. A proof `for` refers to this separately generated obligation; renaming a law cannot change its semantic binding.

For the additive `contract 2` named-call selection, the consumer MUST bind the contract to the accepted subject DEFINITION BODY and independently reconstruct its finite named-call environment from immutable versioned source, not substitute the submitted top-level invocation, an inlined proof author's copy, or merely a display name. The first selected subject is `twice [ d.step d.step ]`, where `d.step` imports the exported `step [ 1 + ]` from `Definitions@5` into `Subject@3`. Both uses retain the same original lexical definition provenance but MUST have distinct authentic occurrence identities and distinct freshly specialized rows/slots, each with the full checked instruction/type derivation, ordered source byte span and ordinal. The selected `paramTypes` is empty, including the typed empty actual parameter stack. Complete graph reachability and absence of orphan/extra rows, uniqueness of lexical call occurrences and slots, version/owner/source identities, and exact accepted interfaces MUST be checked; an `ExactClosure`-shaped graph alone does not authenticate source or occurrences. The host separately authenticates the submitted root as the selected single invocation of `twice`; this is not the definition-body root of the Lean theorem. Changed source, alias rebinding, row owner/span/ordinal/slot, incomplete instantiation or substituted call code MUST fail before proof admission. The same MC1-v1 `input`/`output`/`requires`/`ensures` fields are used, without a new parameter source form.


<!-- cairn:scenario-links:start -->
#### Scenario: CONTRACT-18 for VC-INTRINSIC-01

- GIVEN the `Intrinsic-Proofs-Draft` profile and every field of `input` in [CONTRACT-18](../../../specs/conformance/contract-cases.json)
- WHEN the `admission` procedure for case `CONTRACT-18` runs against those inputs
- THEN the observations match every field of `expected` in case `CONTRACT-18`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: CONTRACT-20 for VC-INTRINSIC-01

- GIVEN the `Intrinsic-Proofs-Draft` profile and every field of `input` in [CONTRACT-20](../../../specs/conformance/contract-cases.json)
- WHEN the `admission` procedure for case `CONTRACT-20` runs against those inputs
- THEN the observations match every field of `expected` in case `CONTRACT-20`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: CONTRACT-26 for VC-INTRINSIC-01

- GIVEN the `Intrinsic-Proofs-Draft` profile and every field of `input` in [CONTRACT-26](../../../specs/conformance/contract-cases.json)
- WHEN the `admission` procedure for case `CONTRACT-26` runs against those inputs
- THEN the observations match every field of `expected` in case `CONTRACT-26`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: VC-INTRINSIC-02
r[VC-INTRINSIC-02]

**VC-INTRINSIC-02.**

The first logical fragment MUST typecheck `Γ ⊢ A : Type0`, `Γ ⊢ P : Prop`, and bidirectional proof judgments `Γ;Δ ⊢ t ⇐ P` / `Γ;Δ ⊢ t ⇒ P`. `Γ` contains total typed immutable terms and exact reviewed proof rules, while `Δ` contains only explicitly selected premise binders; no producer-added axiom can enter either environment. `Type0` denotes independently checked `PureTyCode` with `El` decoding, not unrestricted Lean `Type`: its only constructors are pure `Unit`, `Bool`, `I64`, `Text` and recursive `Pair`, `Sum`, `List` of valid eligible codes. A recursive `tyOf`/`encode` interpretation MUST relate `El(A)` to the actual reviewed MC1 `Ty`/`Value`/`HasType` definitions with faithful typed equality, never an unchecked type/value cast. `Type0` type binders and eligible total value binders introduce proof-level dependent `(Pi (A Type0) P)` and `(Pi (x A) P)`, with `intro` and `apply` elimination and capture-avoiding type/value substitution preserving code validity. `(Eq A a b)` requires total `a,b:El(A)`; `refl` establishes it only on bounded definitional equality; `subst` eliminates it with an explicit well-formed total motive `El(A) → Prop` and checked premise. Logical `Eq` does NOT automatically establish MC1's `(eq ...)` predicate, which may be undefined; a separately checked typed `Holds`/`evaluate` bridge is required before using equality for a contract. A closed source declaration such as `proof 1 polymorphic-reflexivity : [ (Pi (A Type0) (Pi (x A) (Eq A x x))) ] [ (intro (A Type0) (intro (x A) (refl x))) ]` MUST be expressible, with positive type instantiations and rejection of Resource, Program, Syntax, forged universe, wrong motive and mismatched type witnesses. Its Lean lowering quantifies over `PureTyCode` and `El(A)` exactly, not unconstrained `Type`. The type universal is predicative (`Type0 : Type1`, with no `Type1` quantification, `Type0 : Type0`, impredicativity or computational dependent runtime). Proof-level terms have no guest execution or implicit `Program` instantiation.


<!-- cairn:scenario-links:start -->
#### Scenario: CONTRACT-17 for VC-INTRINSIC-02

- GIVEN the `Intrinsic-Proofs-Draft` profile and every field of `input` in [CONTRACT-17](../../../specs/conformance/contract-cases.json)
- WHEN the `static` procedure for case `CONTRACT-17` runs against those inputs
- THEN the observations match every field of `expected` in case `CONTRACT-17`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: CONTRACT-19 for VC-INTRINSIC-02

- GIVEN the `Intrinsic-Proofs-Draft` profile and every field of `input` in [CONTRACT-19](../../../specs/conformance/contract-cases.json)
- WHEN the `static` procedure for case `CONTRACT-19` runs against those inputs
- THEN the observations match every field of `expected` in case `CONTRACT-19`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: CONTRACT-25 for VC-INTRINSIC-02

- GIVEN the `Intrinsic-Proofs-Draft` profile and every field of `input` in [CONTRACT-25](../../../specs/conformance/contract-cases.json)
- WHEN the `static` procedure for case `CONTRACT-25` runs against those inputs
- THEN the observations match every field of `expected` in case `CONTRACT-25`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: VC-INTRINSIC-03
r[VC-INTRINSIC-03]

**VC-INTRINSIC-03.**

A proof of a Noble program claim MUST be a term checked against the exact independently derived accepted-subject obligation, not a raw Lean module, a textual `by` script or an arbitrary self-chosen `PC` proposition. For `[ 1 + ]` the bounded example is a source definition, module `contract 1` and `proof 1 ... for ... [ (export-unary-I64 (intro (tail Stack) (intro (x I64) (pc-sequence (pc-exact (exec-literal 1)) (pc-exact (exec-add)) (bridge (by-exact-append-assoc)) (join (by-exact-result))))) (mc1-true-eq-wrap)) ]`: `exec-literal` and `exec-add` recognize exact independently resolved subject slices, whole ordered stack, wrapping `I64` operands and complete normal-return effects. The fixed-initial-stack `pc_exact` is instantiated afresh under explicit logical `tail:Stack` and `x:I64` introductions; Stack is a dedicated contract-goal sort, never a Type0 code or guest value. These rules MUST expand to proof terms from the *actual reviewed* `NobleContracts.Rules.exec_lit_iff`, `exec_add_iff`, `pc_exact`, `pc_sequence` and equality/quantifier introductions, not newly postulated `pc-add` or a stamped preproved increment theorem. `pc-sequence` verifies compatible full intermediate predicates, explicit bridge and final join. With `s0=tail++[x]`, `m0=s0++[1]`, `m1=tail++[x,1]`, `t0=tail++[wrap64(x+1)]`, `by-exact-append-assoc` introduces the real bridge's `s,m` and hypotheses including exact `m=m0`, substitutes that checked equality and uses whitelisted `List.append_assoc` to prove `m=m1`; `by-exact-result` introduces the real join's `s,m,t` and hypotheses, returning only its checked exact `t=t0` premise. Neither can rewrite arbitrary predicates or prove an implication without matching exact-result premises. `by-defeq` discharges only an actually definitional reflexive goal, never symbolic append associativity. Missing bridge/join, forged/mismatched exact equality, foreign rewrites and swapped operands fail.

The PC result is not automatically the exported MC1 claim. `export-unary-I64` extracts the universally quantified normal-return equality and uses the reviewed `NobleContracts.Obligation.exportedClaim_unaryI64` with actual accepted code; `mc1-true-eq-wrap` MUST reconstruct the separate typed/pre/post consequence from `exportedClaim_consequence` and `Expression.holds_true`, `holds_eq` and checked `evaluate` equations for the *exact independently generated MC1 IR* (input, output, wrapping add, literal 1). Neither form can discharge arbitrary predicates or fabricate an output-type witness. The result is the exact MC1 universal partial-correctness claim for every `I64` input and unchanged stack tail, with `wrap64(x+1)` including overflow, NOT termination, host/resource behavior or Wasm correctness.

For `proof 2 ... for ...`, the exact expected selected claim has MC1-strength typed pre/post boundaries: for every bottom tail, `before`, and typed empty `params`, typed `before` and true `pre before [] params` imply that every normal named execution of the accepted `twice` DEFINITION BODY from `tail ++ before` ends at `tail ++ after`, with typed `after` and `post before after params`. `NamedV2.exportedNamedClaim` expresses this universal partial-correctness shape; `Holds₂` MUST interpret every existing MC1 `Term` constructor over the named value domain, including `maps` by quantifying over normal execution of the program on `[x]` with exact result `[y]`, and undefined evaluation MUST NOT count as true. The selected law has `I64 -- I64`, `requires [ true ]`, `ensures [ (eq (out y) (add (in x) 2)) ]` under BitVec64 wrapping. `NamedV1.Satisfies` is too weak to replace this typed claim. A strict Lean proof of this proposition over a modeled environment establishes neither authenticity of submitted source/graph nor source-to-Lean translation, host admission, execution termination, artifact correspondence or owner-law approval.

The selected revision-2 source term is `(export-named-unary-I64 (pc-named-call 0 (exec-literal-1) (exec-add)) (pc-named-call 1 (exec-literal-1) (exec-add)) (by-exact-tail) (named-true-eq-wrap))`. Exact I64 input/output and True/`x+2` predicate shapes MUST be checked during statement preparation, before an independently generated claim is emitted, as well as proof acceptance. The checker MUST match both ordered call indices to independently accepted uses and their exact literal/add bodies, verify the exact-tail join and typed True/`x+2` wrap, then instantiate the reviewed two-call rule with actual source-bound rows. A bare `(named-two-call-I64)` theorem-name marker, reordered/missing call slice or unchecked expansion MUST NOT be accepted. This bounded source term does not add a general named-call proof calculus or alter revision-1 proof grammar.

The host consumer MUST instantiate a reusable reviewed two-call rule with the *actual authenticated* accepted named graph, both source-checked fresh specialization rows and independently regenerated exact claim; it MUST NOT invoke the synthetic fixed-slot `NamedV2.Examples.proof` as a proof of a different source graph. A generic Lean rule alone also does not authenticate source. Admission remains unavailable until source origin, reusable rule and strict generated exact-claim check all succeed together.

Lean 4 is the **first independent checking target**. Source elaboration MUST independently typecheck untrusted Noble terms and lower them to restricted closed Lean terms under pinned reviewed model/rules. The independently selected MC1 obligation, not generated proof text, supplies the expected exact theorem, model revision, transitive dependencies and permitted axioms to an isolated strict Lean checker. A Lean success alone does not prove the Noble-source parser/elaborator/lowering translation correct or its correspondence to the actual accepted subject: those remain separate open metatheory and Rust refinement obligations. Bend2 remains an optional comparative backend study, never a proof compiler/kernel. Unknown rule identifiers, unsound proof-library dependencies, native evaluation, tactics, solver status, raw user Lean imports, substitution of another accepted proof and mismatched expected claims MUST fail closed. A candidate-authored module `contract 1` MUST NOT select or amend an independent release owner's law: under VC-OWNER-01 the proof-required consumer additionally matches the complete elaborated proposition, precondition, statement, subject, assumptions, model and revision to the immutable owner-frozen law recorded *before* candidate edits. A false precondition or weakened ensures can yield a valid theorem but MUST fail that independent release comparison; an owner-approved new law needs a fresh exact proof/admission.


<!-- cairn:scenario-links:start -->
#### Scenario: CONTRACT-18 for VC-INTRINSIC-03

- GIVEN the `Intrinsic-Proofs-Draft` profile and every field of `input` in [CONTRACT-18](../../../specs/conformance/contract-cases.json)
- WHEN the `admission` procedure for case `CONTRACT-18` runs against those inputs
- THEN the observations match every field of `expected` in case `CONTRACT-18`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: CONTRACT-20 for VC-INTRINSIC-03

- GIVEN the `Intrinsic-Proofs-Draft` profile and every field of `input` in [CONTRACT-20](../../../specs/conformance/contract-cases.json)
- WHEN the `admission` procedure for case `CONTRACT-20` runs against those inputs
- THEN the observations match every field of `expected` in case `CONTRACT-20`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: CONTRACT-23 for VC-INTRINSIC-03

- GIVEN the `Intrinsic-Proofs-Draft` profile and every field of `input` in [CONTRACT-23](../../../specs/conformance/contract-cases.json)
- WHEN the `admission` procedure for case `CONTRACT-23` runs against those inputs
- THEN the observations match every field of `expected` in case `CONTRACT-23`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: CONTRACT-24 for VC-INTRINSIC-03

- GIVEN the `Intrinsic-Proofs-Draft` profile and every field of `input` in [CONTRACT-24](../../../specs/conformance/contract-cases.json)
- WHEN the `review` procedure for case `CONTRACT-24` runs against those inputs
- THEN the observations match every field of `expected` in case `CONTRACT-24`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: CONTRACT-26 for VC-INTRINSIC-03

- GIVEN the `Intrinsic-Proofs-Draft` profile and every field of `input` in [CONTRACT-26](../../../specs/conformance/contract-cases.json)
- WHEN the `admission` procedure for case `CONTRACT-26` runs against those inputs
- THEN the observations match every field of `expected` in case `CONTRACT-26`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: VC-INTRINSIC-04
r[VC-INTRINSIC-04]

**VC-INTRINSIC-04.**

Proof declarations MUST be erased in full before guest code/Wasm emission, leaving no guest proof value, word, program code, latent effect, runnable proof term, new runtime polymorphism or changed recipe/DefinitionId for an unchanged executable definition. Importing or inspecting a proof description does not run a verifier, execute a program or acquire authority. Separately inspectable MC2 evidence companions MAY retain exact proof/claim/source metadata and previously accepted evidence under their existing versioned admission rules, without putting proof terms on the guest stack. Ordinary rank-1 preparation, `quote`/`compose`/`run`/`reflect`, proof-free invocation and resource/host authorization remain unchanged. Proof-required invocation separately rechecks exact evidence, actual-input preconditions, artifact/build correspondence, current host policy and capability checks; a proof never self-authorizes those gates.


<!-- cairn:scenario-links:start -->
#### Scenario: CONTRACT-22 for VC-INTRINSIC-04

- GIVEN the `Intrinsic-Proofs-Draft` profile and every field of `input` in [CONTRACT-22](../../../specs/conformance/contract-cases.json)
- WHEN the `runtime` procedure for case `CONTRACT-22` runs against those inputs
- THEN the observations match every field of `expected` in case `CONTRACT-22`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: CONTRACT-23 for VC-INTRINSIC-04

- GIVEN the `Intrinsic-Proofs-Draft` profile and every field of `input` in [CONTRACT-23](../../../specs/conformance/contract-cases.json)
- WHEN the `admission` procedure for case `CONTRACT-23` runs against those inputs
- THEN the observations match every field of `expected` in case `CONTRACT-23`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: CONTRACT-26 for VC-INTRINSIC-04

- GIVEN the `Intrinsic-Proofs-Draft` profile and every field of `input` in [CONTRACT-26](../../../specs/conformance/contract-cases.json)
- WHEN the `admission` procedure for case `CONTRACT-26` runs against those inputs
- THEN the observations match every field of `expected` in case `CONTRACT-26`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: VC-INTRINSIC-05
r[VC-INTRINSIC-05]

**VC-INTRINSIC-05.**

Proof checking MUST be decidable for the advertised fragment and account for input bytes, nodes, nesting, finite dependency DAG, type/stack/term size, normalization and substitution work, time/memory and diagnostic output in a shared bounded admission request. The grammar and proof core forbid general recursion, cyclic `use` dependencies, `axiom`, `admit`, `sorry`, unchecked extensionality, tactics/solver search, arbitrary Noble program evaluation, partial selectors as equality witnesses, host effects and runtime proof callbacks. `use` refers only to a previously independently accepted exact immutable proof identity with inspected transitive assumptions/context; unsupported constants, stale model/library/module/subject revisions and forged evidence are refused. Reject, unsupported, timeout/exhaustion and internal failure MUST be distinct and MUST NOT be reported as proved. A test vector, copied Lean statement, unchecked reflection of a program or an inconsistent `Type0:Type0` derivation is never authority.


<!-- cairn:scenario-links:start -->
#### Scenario: CONTRACT-19 for VC-INTRINSIC-05

- GIVEN the `Intrinsic-Proofs-Draft` profile and every field of `input` in [CONTRACT-19](../../../specs/conformance/contract-cases.json)
- WHEN the `static` procedure for case `CONTRACT-19` runs against those inputs
- THEN the observations match every field of `expected` in case `CONTRACT-19`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: CONTRACT-20 for VC-INTRINSIC-05

- GIVEN the `Intrinsic-Proofs-Draft` profile and every field of `input` in [CONTRACT-20](../../../specs/conformance/contract-cases.json)
- WHEN the `admission` procedure for case `CONTRACT-20` runs against those inputs
- THEN the observations match every field of `expected` in case `CONTRACT-20`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: CONTRACT-21 for VC-INTRINSIC-05

- GIVEN the `Intrinsic-Proofs-Draft` profile and every field of `input` in [CONTRACT-21](../../../specs/conformance/contract-cases.json)
- WHEN the `admission` procedure for case `CONTRACT-21` runs against those inputs
- THEN the observations match every field of `expected` in case `CONTRACT-21`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: CONTRACT-26 for VC-INTRINSIC-05

- GIVEN the `Intrinsic-Proofs-Draft` profile and every field of `input` in [CONTRACT-26](../../../specs/conformance/contract-cases.json)
- WHEN the `admission` procedure for case `CONTRACT-26` runs against those inputs
- THEN the observations match every field of `expected` in case `CONTRACT-26`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->
