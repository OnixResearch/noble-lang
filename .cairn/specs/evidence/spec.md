# Conformance and evidence records

<!-- cairn:purpose:start -->
## Purpose

This accepted specification records Noble draft contracts, not completed implementation.
Original requirement IDs, explanatory prose, examples, and open decisions remain authoritative.
Scenario clauses declare designs; the conformance ledger records execution and evidence.

## Requirements

<!-- cairn:purpose:end -->


Document: SPEC-EV001  
Revision: 0.1.0-draft.5  
Status: Record schema and document checks only

## Independent dimensions

### Requirement: EV-STATE-01
r[EV-STATE-01]

**EV-STATE-01.** Every scenario MUST carry independent implementation, execution, proof, and trust fields. One successful dimension MUST NOT imply success in another.

| Field | Allowed values |
|---|---|
| `implementation` | `absent`, `partial`, `implemented`, `unsupported` |
| `execution` | `not-run`, `passed`, `failed`, `timeout`, `unsupported` |
| `proof` | `open`, `accepted`, `failed`, `not-applicable` |
| `trust` | `unassessed`, `explicit` |


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: EV-STATE-02
r[EV-STATE-02]

**EV-STATE-02.** Execution or proof results other than unexecuted/open states MUST identify their evidence. Evidence binds a subject, revision, configuration, claim, assumptions, and result. `implemented` alone is not an executed result.

A failed test and an open proof can coexist with implemented code. A document-validator pass belongs to the document check lane, not to scenario execution.

Each evidence record MUST include a nonempty `claim` string that states the observed or proved claim. Non-review scenario results and component execution results MUST cite `test` evidence with the same execution outcome. Only scenarios whose `kind` is `review` can use review evidence for their execution field. Other evidence can accompany a result but cannot replace its required evidence class. Document validation checks these fields, not the truth of the claim.

The proof-obligation ledger MUST use the same evidence rules as scenario and component records. Each `accepted` or `failed` obligation MUST cite proof evidence with its own obligation ID and the same result. Review or test evidence alone MUST NOT establish a proof result. All supplied records MUST retain the required bindings, including records attached to open obligations.

SPEC-V002 adds claim outcomes: `proved`, `disproved`, `unknown`, `timeout`, `unsupported`, `error`, and `not-run`. These describe propositions and proof attempts, not the independent status fields in this document. A rejected proof does not establish a false proposition. Fixture expectations that name `proved` are not accepted-proof evidence.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: EV-STATE-03
r[EV-STATE-03]

**EV-STATE-03.** A greenfield snapshot MUST NOT report Noble runtime execution or accepted proofs without concrete implementation and evidence records. The current snapshot records no such results.


<!-- cairn:scenario-links:start -->
#### Scenario: WORKER-11 for EV-STATE-03

- GIVEN the `Worker-Design` profile and every field of `input` in [WORKER-11](../../../specs/conformance/worker-cases.json)
- WHEN the `review` procedure for case `WORKER-11` runs against those inputs
- THEN the observations match every field of `expected` in case `WORKER-11`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

## Scenario shape

Each file names `schema_version`, the specification `revision`, its origin, and a `cases` array. Each case has:

1. A stable `id`, `profile`, and `kind`.
2. Nonempty requirement references from the current family.
3. An `input` object containing source or a named structured harness with parameters.
4. An `expected` object naming the observation stage, outcome, and discriminating observations.
5. A `state` object and an `evidence` array.

### Requirement: EV-CASE-01
r[EV-CASE-01]

**EV-CASE-01.** Requirement references MUST resolve against the current revision. Unsupported or incomplete harness designs remain visible and unexecuted.

Kinds are `static`, `runtime`, `admission`, `adapter`, `identity`, and `review`. A release-policy review is not a runtime language test.

[Contract scenarios](../../../specs/conformance/contract-cases.json) use `Contracts-Draft` for the selected optional profile. Their logical predicates, symbolic artifact identities, and companion operations are harness inputs, not frozen Noble source syntax. Proof, runtime, and admission observations require their own executed evidence.

`Backend-Experiment`, `Decoder-Experiment`, `Implementation-Policy`, and `Contract-Design` label test-design lanes, not new language conformance profiles. A compile-fail design in a review record is not an executed Rust test.

`I64` values in JSON use decimal strings inside typed records. This prevents a JavaScript JSON parser from losing integer precision. Expected stacks are bottom to top.

The [calculator scenarios](../../../specs/conformance/calculator-cases.json) use `Calculator-Design` and `AI-Authoring-Design` as application/test lanes. Their expression strings are calculator input, not Noble source. Exact numerators, denominators, and conversion values use decimal strings. These records do not select portable Noble encodings. Calculator validation errors occur inside an accepted application and are not Noble static-rejection results.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: EV-CASE-02
r[EV-CASE-02]

**EV-CASE-02.** A static or admission rejection MUST observe zero candidate-body host requests and zero protected operations. Compiler-service effects, where applicable, are recorded separately.


<!-- cairn:scenario-links:start -->
#### Scenario: S-CASE-05 for EV-CASE-02

- GIVEN the `Core-Bootstrap` profile and every field of `input` in [S-CASE-05](../../../specs/conformance/safety-cases.json)
- WHEN the `static` procedure for case `S-CASE-05` runs against those inputs
- THEN the observations match every field of `expected` in case `S-CASE-05`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: EV-CASE-03
r[EV-CASE-03]

**EV-CASE-03.** Authority denial requires a declared effect that includes the request. A denied request counts toward the effect trace. Denial MUST NOT substitute for static effect rejection.


<!-- cairn:scenario-links:start -->
#### Scenario: S-CASE-06 for EV-CASE-03

- GIVEN the `Resources-Draft` profile and every field of `input` in [S-CASE-06](../../../specs/conformance/safety-cases.json)
- WHEN the `runtime` procedure for case `S-CASE-06` runs against those inputs
- THEN the observations match every field of `expected` in case `S-CASE-06`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: WI-08 for EV-CASE-03

- GIVEN the `Component-Sync-Bootstrap` profile and every field of `input` in [WI-08](../../../specs/conformance/wit-wasi-cases.json)
- WHEN the `runtime` procedure for case `WI-08` runs against those inputs
- THEN the observations match every field of `expected` in case `WI-08`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: EV-CASE-04
r[EV-CASE-04]

**EV-CASE-04.** Runtime builder cases MUST obtain operands after compilation and disable the Noble preparation service during execution. Optimized literal-only examples do not satisfy them.


<!-- cairn:scenario-links:start -->
#### Scenario: CORE-03 for EV-CASE-04

- GIVEN the `Core-Bootstrap` profile and every field of `input` in [CORE-03](../../../specs/conformance/cases.json)
- WHEN the `runtime` procedure for case `CORE-03` runs against those inputs
- THEN the observations match every field of `expected` in case `CORE-03`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: EV-CASE-05
r[EV-CASE-05]

**EV-CASE-05.** A matrix case MUST report every input combination and its expected outcome. Missing or unsupported elements MUST NOT count as passed coverage.

The [language-workflow scenarios](../../../specs/conformance/language-workflow-cases.json) use design-lane labels, not new conformance profiles. Property-runner and documentation-harness tests retain their own subject and outcomes. A successful harness control does not mark its embedded Noble example as executed or proved.

The [worker scenarios](../../../specs/conformance/worker-cases.json) use `Worker-Design`, not a new language or concurrency profile. [WORKER-CONFORMANCE.md](../../../specs/WORKER-CONFORMANCE.md) separates pure worker requests, compiler-service effects, shell actions, and native retirement. Cancellation acknowledgement does not establish external completion. Round-trip expectations retain the canonical encoding gate.

Harness names and symbolic operation IDs describe future test drivers. They do not freeze source annotation syntax, canonical identifier bytes, or an executable test implementation. The validator checks record structure, not language behavior.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

## Requirement index

[requirements.json](../../../specs/requirements.json) is a derived index of every numbered requirement in the registered normative documents. It records document ownership, planned evidence routes, scenario links, and open test-design gaps.

### Requirement: EV-TRACE-01
r[EV-TRACE-01]

**EV-TRACE-01.** Every numbered requirement MUST have a declared evidence route. A requirement with no concrete scenario remains `test-design-open`, not covered or passed.

Native requirement headings and matching `r[ID]` markers own the index. Legacy labels support historical preservation checks only. Fenced examples MUST NOT create requirements, scenario targets, or preservation obligations.

The index uses these planned routes:

| Families | Planned evidence |
|---|---|
| `K`, `P`, `B` | semantic tests, reference-model proof, review |
| `BE` | experiment tests, correspondence evidence, review |
| `W`, `H`, `S`, `WI`, `RA` | boundary tests, correspondence evidence, review |
| `C`, `EV` | validation/release-policy tests and review |
| `DX` | semantic tests, policy tests, and review |
| `CALC` | application tests, benchmark-policy tests, and review |
| `V`, `VT`, `VC` | proof or scoped assumption review, with implementation tests where relevant |

Routes do not prove completion. Exact theorem links and execution observations belong in evidence records as implementation proceeds.

The Aeneas-first scope rules in VT-SCOPE-01 through VT-SCOPE-05 additionally require a production-source inventory. The requirement index does not replace function-level extraction and refinement coverage. Parser, backend, resource, runtime, and CLI work must remain visible even before implementation.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: EV-TRACE-02
r[EV-TRACE-02]

**EV-TRACE-02.** Regenerating the index MUST NOT manufacture test results or overwrite observed evidence. The index contains only derived planning metadata. Scenario states and the obligation ledger remain separate files.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

## Bound evidence and imported tool results

### Requirement: EV-BIND-01
r[EV-BIND-01]

**EV-BIND-01.** A reusable evidence record MUST bind its subject, claim, source scope, semantic dependencies, policy, toolchain, target/features, command, inputs, assumptions, and result. Digests and versioned references can identify those inputs. Missing relevant bindings or unknown coverage MUST block reuse for a required claim.

Consumers select the expected subject, claim, policy, and trust context independently of producer-supplied evidence. A changed binding requires renewed applicability validation. Old evidence can remain valid for its historical subject without satisfying the current gate. A producer cannot establish trust by replacing both an artifact and its unkeyed digest.


<!-- cairn:scenario-links:start -->
#### Scenario: OCTET-09 for EV-BIND-01

- GIVEN the `Octet-Adoption-Design` profile and every field of `input` in [OCTET-09](../../../specs/conformance/octet-adoption-cases.json)
- WHEN the `review` procedure for case `OCTET-09` runs against those inputs
- THEN the observations match every field of `expected` in case `OCTET-09`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: EV-TIER-01
r[EV-TIER-01]

**EV-TIER-01.** Imported Octet results MUST preserve their lint, policy, transcript, integrity, or formal-proof role without promotion. Formal-proof claims require the existing verifier-backed evidence and exact subject correspondence. Fixture-scoped extraction, source-shape conformance, and artifact verification MUST NOT establish whole-kernel correctness or release eligibility.

The record MUST retain each required phase outcome and each incomplete scope. An omitted or failed phase cannot become an overall acceptance through a successful sibling phase. A type-valid configuration is not executed policy enforcement. These rules extend existing evidence bindings without making every receipt a proof or changing ordinary program admission.

[Octet adoption cases](../../../specs/conformance/octet-adoption-cases.json) contain explicit stale-binding, tier-promotion, and incomplete-coverage controls. These are unexecuted designs. The document validator does not invoke Octet or a proof tool.


<!-- cairn:scenario-links:start -->
#### Scenario: OCTET-09 for EV-TIER-01

- GIVEN the `Octet-Adoption-Design` profile and every field of `input` in [OCTET-09](../../../specs/conformance/octet-adoption-cases.json)
- WHEN the `review` procedure for case `OCTET-09` runs against those inputs
- THEN the observations match every field of `expected` in case `OCTET-09`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: OCTET-10 for EV-TIER-01

- GIVEN the `Octet-Adoption-Design` profile and every field of `input` in [OCTET-10](../../../specs/conformance/octet-adoption-cases.json)
- WHEN the `review` procedure for case `OCTET-10` runs against those inputs
- THEN the observations match every field of `expected` in case `OCTET-10`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

## Release claims

### Requirement: EV-GATE-01
r[EV-GATE-01]

**EV-GATE-01.** A release claim MUST identify its supported subset and all applicable gates. Passing document validation cannot satisfy compiler, resource, component, or safety conformance.

Proof obligations in [verification/obligations.json](../../../specs/verification/obligations.json) remain open. They name expected claims, not completed Lean declarations.


<!-- cairn:scenario-links:start -->
#### Scenario: ADAPT-12 for EV-GATE-01

- GIVEN the `Implementation-Policy` profile and every field of `input` in [ADAPT-12](../../../specs/conformance/adaptation-cases.json)
- WHEN the `review` procedure for case `ADAPT-12` runs against those inputs
- THEN the observations match every field of `expected` in case `ADAPT-12`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: WORKER-11 for EV-GATE-01

- GIVEN the `Worker-Design` profile and every field of `input` in [WORKER-11](../../../specs/conformance/worker-cases.json)
- WHEN the `review` procedure for case `WORKER-11` runs against those inputs
- THEN the observations match every field of `expected` in case `WORKER-11`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: EV-GATE-02
r[EV-GATE-02]

**EV-GATE-02.** A proof accepted under a narrower subset or different configuration MUST NOT silently close a broader obligation. Trust assumptions and cross-tool boundaries remain explicit.


<!-- cairn:scenario-links:start -->
#### Scenario: ADAPT-13 for EV-GATE-02

- GIVEN the `Implementation-Policy` profile and every field of `input` in [ADAPT-13](../../../specs/conformance/adaptation-cases.json)
- WHEN the `review` procedure for case `ADAPT-13` runs against those inputs
- THEN the observations match every field of `expected` in case `ADAPT-13`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

## Document checks

```sh
bun tools/check-specs.mjs
bun tools/check-specs.mjs --self-test
```

After intentional requirement or scenario changes:

```sh
bun tools/check-specs.mjs --refresh-ledger
bun tools/check-specs.mjs
```

The refresh command changes only derived requirement metadata. It does not change scenario states or proof status. Selected oracle checks reject known specification regressions; they do not implement the referenced harnesses.
