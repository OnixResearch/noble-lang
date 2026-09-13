# Verification and Trust Model

<!-- cairn:purpose:start -->
## Purpose

This accepted specification records Noble draft contracts, not completed implementation.
Original requirement IDs, explanatory prose, examples, and open decisions remain authoritative.
Scenario clauses refer to unexecuted designs in the conformance ledger.

## Requirements
<!-- cairn:purpose:end -->


Document: SPEC-V001  
Revision: 0.1.0-draft.5  
Project: noble  
Status: Canonical working verification contract; no implementation or completed proof  
Depends on: SPEC-0001 and SPEC-S001 at 0.1.0-draft.5

## 1. Purpose and authority

Across this specification family, **MUST** and **SHALL** are mandatory, **MUST NOT** and **SHALL NOT** are prohibitions, **SHOULD** is a recommendation permitting a documented exception, and **MAY** is permission. Requirement identifiers are revisioned; normative obligation wording is not a claim that the obligation has been fulfilled.

Noble SHALL have a small, mechanized semantic core and support optional compositional verification of individual programs. Verification is a development and assurance commitment, not a fourth expression form or a requirement that every caller supply a handwritten proof.

This document specifies the claims to establish and the boundaries those claims must preserve. [VERIFICATION-TOOLCHAIN.md](../verification-toolchain/spec.md) requires an Aeneas-first Rust implementation, mandatory across the semantic kernel, with Lean 4 reference definitions and refinement proofs. Verus is available only for reviewed non-kernel exceptions. [PROGRAM-CONTRACTS.md](../program-contracts/spec.md) specifies typed contracts, first-class evidence companions, and optional proof-required admission. [SPEC-0001.md](../language/spec.md) integrates these requirements. [SOURCES.md](../../../specs/SOURCES.md) records the inherited text and unavailable historical citation records.

### Requirement: V-STATUS-01
r[V-STATUS-01]

**V-STATUS-01.** A specification requirement, a mathematical theorem statement, an implemented checker, an executed test, and an accepted proof MUST be reported as distinct things. The obligations in this package are all open; the accompanying fixtures are expectations, not executed Noble tests.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: V-AUTH-01
r[V-AUTH-01]

**V-AUTH-01.** The reviewed Lean 4 formalization SHALL be the reference mathematical model for verification claims. Its exact source revision and supporting definitions MUST be identified. A mismatch between that model and the language specification is a specification defect to resolve explicitly; neither a successful proof nor an informal paragraph silently overrides the other.

The existing draft already requires stack/type safety, conservative effects, resource accounting, immutable recipes, and explicit preparation. It explicitly does not establish a complete checking algorithm or soundness proof. These are the source-derived starting obligations, not completed results. [B1 §§5.4, 8–9, 11, 16; references in SOURCES.md]


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

## 2. Preserve the small language

### Requirement: V-CORE-01
r[V-CORE-01]

**V-CORE-01.** Ordinary `Program<S,T,e>`, `quote`, `compose`, `run`, and `reflect` MUST retain their existing semantics. Ordinary execution MUST NOT require Lean, Aeneas, Verus, an SMT solver, or proof search to be installed or invoked. An admission service MAY check evidence separately under an explicit host policy.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: V-CORE-02
r[V-CORE-02]

**V-CORE-02.** The optional contract profile MUST NOT introduce dependent `Program` parameters, general refinement inference, implicit behavioral coercions, or mandatory termination for ordinary programs. Typed contract declarations and companion APIs are separate from the core expression grammar. Ordinary preparation and execution MUST NOT evaluate Lean proof terms or solver directives. Explicit finite rule replay follows SPEC-V002 without a general theorem prover. The core checker MUST NOT depend on arbitrary application contracts to establish stack, eligibility, or effect validity.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: V-CORE-03
r[V-CORE-03]

**V-CORE-03.** A new core construct or semantic change MUST identify its effect on the typing rules, operational model, ownership invariant, recipe observations, and existing proofs. Before stable-core inclusion, its relevant obligations MUST be discharged or the feature MUST remain explicitly outside that stable subset. A tool limitation is not permission to weaken the language silently.

Typed contract declarations and first-class companion APIs are selected deliverables under SPEC-V002. Their concrete grammar and representation remain explicit entry gates, not permanent deferrals. Current examples are mathematical or harness notation. No proof punctuation is frozen, and `#` remains the comment prefix.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

## 3. Three verification targets

| Target | Required correspondence | What it does not establish by itself |
|---|---|---|
| Language metatheory | Formal typing and evaluation rules imply their stated invariants | Correctness of the production Rust checker or backend |
| Implementation | Particular Rust functions refine their abstract contracts | Soundness of a different function, feature configuration, compiler, or binary |
| Noble program behavior | An exact program satisfies an exact claim under stated assumptions | Correctness of its supplied Wasm, satisfaction of its precondition, or authority to execute |

### Requirement: V-CLAIM-01
r[V-CLAIM-01]

**V-CLAIM-01.** Every assurance claim MUST state its target, scope, semantic revision, assumptions, evidence class, and unresolved correspondence boundaries. “Formally verified Noble” MUST NOT be used as an unqualified substitute for those fields.

A kernel proof about an Aeneas-generated Lean function is evidence about that function. Its application to Rust also relies on the extraction path and external models. A Verus result belongs to the Verus verification chain. Neither automatically proves that a Noble-to-Wasm compiler preserves semantics. These distinctions are normative Noble policy; the selected tools' documented roles motivate them. [R1–R7]


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

## 4. Formal model requirements

### 4.1 Domains and explicit assumptions

### Requirement: V-MODEL-01
r[V-MODEL-01]

**V-MODEL-01.** The Lean model MUST represent resolved bodies and references, value and stack types, instantiated first-class program interfaces, rank-1 named schemes, effect bounds, eligibility judgments, values and captures, recipe normalization, and an immutable definition environment. Recursive references MUST retain owner scope.

The initial mechanization MAY cover a smaller bootstrap subset, but MUST enumerate omissions. A result for nonrecursive arithmetic quotations MUST NOT be presented as a result for the whole language, lists, recursion, host operations, or resources.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: V-MODEL-02
r[V-MODEL-02]

**V-MODEL-02.** The model MUST separate language evaluation, host responses, resource ownership, and execution-profile failures. Host contracts SHALL be explicit parameters with well-formedness and preservation conditions. Their satisfaction by a concrete adapter is a separate implementation obligation.

A useful configuration refinement is:

```text
Configuration = (pending code, data stack, continuation frames,
                 host state, ownership accounting, event history)
```

This is proof notation, not a frozen runtime representation. Continuation frames include values temporarily held by `dip`. Pure programs need not carry a concrete host-state object in their runtime representation.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### 4.2 Steps, waiting, failure, and divergence

### Requirement: V-MODEL-03
r[V-MODEL-03]

**V-MODEL-03.** The model MUST distinguish normal return, permitted abnormal termination, host waiting where supported, divergence, and forbidden language-level stuck states. Ordinary `Result` error variants are normal returns. An implementation MUST NOT conceal a wrong operand type, stack underflow, or consumed-handle use by labeling it an allowed trap.

The ideal core semantics may abstract from allocation and execution budgets. The relation to a bounded execution profile must then state how allocation failure, cancellation, and fuel exhaustion are represented. Exhausting fuel in a test evaluator is not evidence that the source program diverges.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: V-MODEL-04
r[V-MODEL-04]

**V-MODEL-04.** Safety and effect claims MUST cover finite prefixes of executions, including prefixes of diverging, waiting, and abnormally terminating computations. A theorem only about successful returns is insufficient for either claim.

A closed, well-typed configuration must either be at an allowed outcome, be waiting at a permitted host boundary, or admit a modeled step. This is progress relative to the stated host model, not a host-availability or termination guarantee.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### 4.3 Host effects and authority

### Requirement: V-EFFECT-01
r[V-EFFECT-01]

**V-EFFECT-01.** Effect soundness SHALL establish that each guest-requested `HostOpId` in every modeled execution prefix is in the instantiated effect bound. The model MUST account for requests that are denied or fail, not only successful responses. Logical request labels need not become publicly stored audit logs.

Operation internals are governed by the host contract; an effect set is not an inventory of every OS action inside native code. Runtime housekeeping and abort cleanup MUST be classified separately from guest-requested operations rather than incorrectly added to quotation construction's latent effects.


<!-- cairn:scenario-links:start -->
#### Scenario: CORE-17 for V-EFFECT-01

- GIVEN the `Core-Bootstrap` profile and every field of `input` in [CORE-17](../../../specs/conformance/cases.json)
- WHEN the `runtime` procedure for case `CORE-17` runs against those inputs
- THEN the observations match every field of `expected` in case `CORE-17`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: V-EFFECT-02
r[V-EFFECT-02]

**V-EFFECT-02.** Construction and inspection MUST issue none of the guest host requests latent in the programs they manipulate. Invocation and higher-order combinators MUST propagate their called programs' bounds without changing event order.

Effect sets summarize possible operation identities. They do not prove event counts, temporal protocols, success, authorization, or information-flow properties.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### 4.4 Ownership across the whole machine

### Requirement: V-RESOURCE-01
r[V-RESOURCE-01]

**V-RESOURCE-01.** Ownership invariants MUST range over the entire live configuration, including aggregate payloads, hidden `dip` values, invocation/session transfers, and pending host-boundary state. The model MUST track multiplicity of ownership obligations; a set that silently collapses duplicate handles is insufficient.

A normal host transition may create, transfer, replace, or retire obligations according to its contract. Conservation therefore means preservation modulo those declared transitions, not preservation of an unchanging number of handles. Proofs MUST distinguish a guest handle's obligation from physical exclusivity of the underlying external object.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: V-RESOURCE-02
r[V-RESOURCE-02]

**V-RESOURCE-02.** The formal eligibility relation MUST exclude resource-bearing values recursively from `Data` and `Capture`, including all payload alternatives allowed by an instantiated schema. Program interfaces may mention resources without making the program's environment resource-owning. Captured environments MUST actually satisfy their own eligibility invariant.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: V-RESOURCE-03
r[V-RESOURCE-03]

**V-RESOURCE-03.** Normal-path preservation and abnormal cleanup SHALL have separate claims. A guest typing proof does not prove host cleanup. The cleanup claim MUST state invocation boundaries, locally observable retirement, repeat-retirement behavior, and prevention of stale-handle resurrection. It MUST NOT assert remote exactly-once cleanup or cleanup after total machine failure without a separately modeled protocol.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### 4.5 Arithmetic and observation

### Requirement: V-NUM-01
r[V-NUM-01]

**V-NUM-01.** The model and every implementation bridge MUST use the baseline's wrapping `I64` semantics for `+`, `-`, and `*`. Mathematical integers in Lean or Verus MUST be connected to these operations by an explicit modulo-`2^64` representation relation. Out-of-range literals remain rejections, not wrapped literals. [B1 §4.4]


<!-- cairn:scenario-links:start -->
#### Scenario: CORE-14 for V-NUM-01

- GIVEN the `Core-Bootstrap` profile and every field of `input` in [CORE-14](../../../specs/conformance/cases.json)
- WHEN the `runtime` procedure for case `CORE-14` runs against those inputs
- THEN the observations match every field of `expected` in case `CORE-14`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: V-OBS-01
r[V-OBS-01]

**V-OBS-01.** Proofs MUST distinguish execution-only equivalence from recipe equality and contextual equivalence in a language with `reflect`. Program-valued results and programs nested inside returned data retain their observable recipes. Equal scalar answers alone do not justify substituting program values in all contexts.

For example, calling `[ 1 1 + ]` and `[ 2 ]` gives the same integer in the ideal pure semantics, but reflecting those program values distinguishes them. A backend may optimize executable instructions while preserving the specified recipe. An explicit source transformation may produce a different program identity. [B1 §11.2]


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

## 5. Checking architecture and proof obligations

### 5.1 Separate elaboration from acceptance

The selected architecture is:

```text
untrusted source / syntax / imported description
    -> parsing, resolution, inference, elaboration
    -> explicit resolved candidate and witnesses
    -> small acceptance checker
    -> checked representation
    -> backend and loader
```

### Requirement: V-CHECK-01
r[V-CHECK-01]

**V-CHECK-01.** Acceptance MUST validate the candidate from an untrusted representation. A proof whose checker input already has the desired typing invariant by construction does not establish validation of hostile input. Declared signatures, type witnesses, schemas, effects, and dependency identities MUST NOT be accepted solely because the producer supplies them.


<!-- cairn:scenario-links:start -->
#### Scenario: CORE-16 for V-CHECK-01

- GIVEN the `Core-Bootstrap` profile and every field of `input` in [CORE-16](../../../specs/conformance/cases.json)
- WHEN the `static` procedure for case `CORE-16` runs against those inputs
- THEN the observations match every field of `expected` in case `CORE-16`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: V-CHECK-02
r[V-CHECK-02]

**V-CHECK-02.** Inference can be complex and initially unproved during staged development. It remains an Aeneas target under VT-SCOPE-01, not a permanent verification exclusion. Acceptance MUST independently validate the relevant elaboration evidence against the declarative rules. Acceptance MUST include reference scope, scheme instantiation, stack order, eligibility, conservative effects, and resource obligations. The exact witness encoding and complete inference algorithm remain separate open deliverables.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: V-CHECK-03
r[V-CHECK-03]

**V-CHECK-03.** The acceptance theorem SHALL have the following shape, with explicit representation and environment well-formedness premises:

```text
check(candidate, expected_interface, environment) = Accepted(checked)
    implies
WellFormed(checked) and TypingDerivation(checked, expected_interface)
```

Correct extraction of the Rust function and its correspondence to `check` are additional claims. Well-formedness of an externally supplied environment must itself be established or appear as a disclosed trusted boundary.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: V-CHECK-04
r[V-CHECK-04]

**V-CHECK-04.** Rejection, unsupported input, resource exhaustion, internal failure, and successful acceptance MUST be distinguishable internally and in assurance reports. A checker MUST NOT accept on timeout. Soundness does not imply completeness, and a diagnostic MUST NOT claim a proof of semantic impossibility without an applicable theorem.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: V-CHECK-05
r[V-CHECK-05]

**V-CHECK-05.** A usable-checker claim MUST include positive acceptance coverage and a termination argument for the selected finite checking problem under the ideal model. Rejecting every candidate cannot satisfy the advertised supported-language coverage. Cyclic type equations, recursive schema checks, and effect constraints must not be hidden sources of divergence.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: V-CHECK-06
r[V-CHECK-06]

**V-CHECK-06.** Validation MUST finish before the submission body runs. `prepare` performs its own documented service effects but MUST NOT execute candidate bodies, tactics, or arbitrary user macros as an undocumented checking step. Successful checking does not prove an optional behavioral contract.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### 5.2 Required theorem families

The machine-readable backlog is [verification/obligations.json](../../../specs/verification/obligations.json). This ledger is newly transcribed from the PO and SO tables, not recovered from an older package. Each obligation remains open. Identifiers name theorem obligations, not implemented Lean declarations.

| Obligation | Claim to establish | Initial evidence route |
|---|---|---|
| PO-01 | Well-formed syntax, references, types, substitutions, and eligibility relations | Lean 4 |
| PO-02 | Preservation of stack and value typing across modeled steps | Lean 4 |
| PO-03 | Progress relative to host contracts; exclusion of forbidden stuck states | Lean 4 |
| PO-04 | Effect soundness for every finite execution prefix | Lean 4 |
| PO-05 | Whole-configuration ownership preservation on normal transitions | Lean 4 |
| PO-06 | Recursive `Data`/`Capture` soundness and resource-free program environments | Lean 4 |
| PO-07 | `quote`/`compose`/`run` execution laws and construction-effect separation | Lean 4 |
| PO-08 | Recipe normalization, quotation boundaries, and reflection consistency | Lean 4 |
| PO-09 | Acceptance-checker soundness against the declarative judgments | Lean 4 |
| PO-10 | Checker termination and supported-fragment acceptance coverage | Lean 4 plus executed fixtures |
| PO-11 | Actual Rust acceptance checker refines the Lean checker contract | Charon/Aeneas to Lean 4 |
| PO-12 | Rust arithmetic and recipe representation refine their logical models | Charon/Aeneas to Lean 4 |
| PO-13 | Handle-table access and retirement preserve the deterministic state contract | Charon/Aeneas to Lean 4 |
| PO-14 | Host adapter, wrapper, and cross-boundary contract correspondence | Aeneas-refined decisions plus explicit external-boundary evidence |
| PO-15 | Behavioral contract sequencing and builder-family rules are sound | Lean 4 |
| PO-16 | Evidence admission binds the intended claim, subject, and assumptions | Lean 4 / Rust refinement, after envelope design |
| PO-17 | Selected backend lowering preserves execution and recipe observations | Aeneas-extracted lowering plus Lean/Wasm preservation proof or translation validation |
| PO-18 | Artifact loading binds executed Wasm to checked program and recipe | Aeneas-refined loading decisions plus concrete loader/build correspondence |
| PO-19 | Contract IR export and obligation generation preserve the intended Noble subject and proposition | Lean semantics plus Aeneas refinement of the exporter/checker |
| PO-20 | Companion construction, finite proof-rule replay, and proof erasure preserve certified claims and ordinary program observations | Lean rules plus Aeneas refinement of companion operations |
| PO-21 | Applicability checks establish the exact precondition before certified invocation | Lean guard correspondence plus Aeneas refinement and boundary tests |

Typing rules for recursive named definitions and generalization are part of the eventual PO-01/02/09 coverage. They are not discharged by a theorem for only monomorphic, nonrecursive source. Parser correctness, canonical binary encoding, ABI details, and full schema checking remain separately scoped until their specifications exist.

### Worker-contract coverage

The worker amendment extends existing obligations rather than declaring a verified substrate:

| Contract | Required obligation coverage |
|---|---|
| Typed dispatch and effect widening | PO-01/02/04/07/09 and actual checker refinement under PO-11 |
| Round trips and package closure | PO-08/09/12/17/18, including encoding and artifact correspondence |
| Bounded candidate admission | PO-09/10/11 and hostile-boundary checks under SO-05/06 |
| Async ownership and cancellation races | PO-05/13/14 and synchronization correspondence under SO-09 |
| Bounded execution and outcome records | PO-03/04/14/17 and safety-relevant failures under SO-07 |

[Worker conformance](../../../specs/WORKER-CONFORMANCE.md) supplies test designs, not proofs of these claims. Scalar bootstrap evidence cannot close schema, package, async, or complete execution-profile coverage. An ideal semantic theorem does not establish a concrete interruption bound or eventual native retirement.

### Octet-adoption coverage

H-AUTH-01–04 extend PO-05/06/13/14 and SO-02/04/06 for witness ownership, constructor opacity, and protected admission. H-RECEIPT-01/02 extend PO-14/16/18 for observed-outcome and artifact/claim correspondence. A proved policy function alone does not establish current credentials, revocation facts, or external success.

DX-TYPE-03/04, DX-PROTOCOL-03, and DX-MODULE-04 extend PO-01/02/04/09 and the corresponding Rust refinement obligations. Explicit transition coverage does not prove liveness. Resolved effect bounds for higher-order programs do not require enumeration of every runtime program value.

VT-OCTET-01–03 and EV-BIND-01/EV-TIER-01 add separate policy and evidence-admission obligations. Their results cannot close these semantic proof obligations by themselves. No obligation is accepted through the [adoption test designs](../../../specs/conformance/octet-adoption-cases.json).

## 6. Evidence classes and trust closure

### Requirement: V-EVIDENCE-01
r[V-EVIDENCE-01]

**V-EVIDENCE-01.** Reports MUST label evidence as one of: `lean-kernel`, `aeneas-lean`, `verus`, `translation-validation`, `test`, `review`, or `assumption`. These are not a numerical ranking and are not interchangeable. A `translation-validation` result MUST name its validator and that validator's own assurance.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: V-EVIDENCE-02
r[V-EVIDENCE-02]

**V-EVIDENCE-02.** A claim's trust description MUST include the transitive assumptions of its supporting claims and bridges. No translation, external model, host contract, or build step becomes proved merely by being placed between two proved components.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: V-EVIDENCE-03
r[V-EVIDENCE-03]

**V-EVIDENCE-03.** “Lean-kernel checked” SHALL mean that the exact theorem and proof have been accepted under the selected Lean policy. It MUST NOT mean that Lean verified the correctness of Charon, Aeneas, Verus, rustc, LLVM, the Wasm engine, or the hardware. Additional compiler/native-evaluation trust must be disclosed if permitted.

Lean's validation guidance distinguishes proof validity from theorem meaning and recommends axiom inspection and stronger checking for untrusted submissions. This workstream adopts the concrete policies in IMPL-V001 rather than treating a green editor indicator as the complete assurance argument. [R4]


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: V-EVIDENCE-04
r[V-EVIDENCE-04]

**V-EVIDENCE-04.** A proof result MUST bind exact relevant source and dependency revisions, feature flags, target architecture, semantic model, statement, and toolchain. Changed inputs invalidate the prior applicability decision until revalidated. Unchanged code may acquire new proofs without changing its language identity.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

## 7. Release gates and staged delivery

These stages group assurance deliverables, not time estimates or mandatory serial dependencies. Pure V4 contract work can follow V2 without the V3 resource extension. [ROADMAP.md](../../../specs/ROADMAP.md) defines the MC1/MC2 dependency edges.

| Stage | Deliverable | Permitted claim after actual completion |
|---|---|---|
| V0 | This specification, boundary register, fixture expectations | Verification design specified |
| V1 | Lean model and metatheory for enumerated bootstrap quotation subset | Named theorems for that model/subset |
| V2 | Explicit acceptance checker; Aeneas extraction and refinement; compatible pinned tools | Rust-checker properties relative to disclosed extraction/build assumptions |
| V3 | Resource extension, Aeneas-refined handle transitions, checked wrappers and external-boundary evidence | Scoped resource assurance with explicit host assumptions |
| V4 | Typed contracts, first-class companions, builder-family proofs, admission and applicability checks | Exact behavioral claims for accepted subjects under the named policy |
| V5 | Selected Wasm lowering and loading correspondence | Only the specifically connected compilation/execution claims |

### Requirement: V-GATE-01
r[V-GATE-01]

**V-GATE-01.** A stable core specification MUST have reviewed, Lean-kernel-checked metatheory for the entire subset it labels stable, covering PO-01 through PO-10 as applicable. A proof plan alone is insufficient. Exploratory drafts and implementations MAY exist earlier but MUST retain their draft and coverage labels.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: V-GATE-02
r[V-GATE-02]

**V-GATE-02.** An implementation advertised as having a verified acceptance checker MUST additionally satisfy PO-11 for the actual supported implementation and publish its translation/model assumptions and configuration. Finishing V1 alone cannot justify that label.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: V-GATE-03
r[V-GATE-03]

**V-GATE-03.** A claim of source-to-Wasm behavioral preservation or end-to-end assurance MUST identify evidence covering the backend, loading, runtime adapters, and remaining execution trust. Otherwise those boundaries MUST remain visibly unproved or assumed. Stable language semantics do not by themselves mean a verified compiler.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: V-GATE-04
r[V-GATE-04]

**V-GATE-04.** Any release that advertises verification SHALL publish a claim ledger, feature coverage, transitive assumption register, pinned toolchain, and executed validation results. Unproved obligations and unsupported features MUST remain visible. CI MUST reject unexplained growth of the trusted boundary or weakened claims.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: V-GATE-05
r[V-GATE-05]

**V-GATE-05.** A verified-kernel claim MUST cover every applicable kernel function under VT-SCOPE-02/05, not only the acceptance checker. A whole-project claim additionally requires explicit coverage of all Noble-owned production components and their correspondence boundaries. Extraction success, test success, and an approved exception MUST NOT substitute for a required refinement proof. Narrow experimental results MUST retain their exact subset labels.



<!-- cairn:scenario-links:start -->
#### Scenario: VERIFY-01 for V-GATE-05

- GIVEN the `Implementation-Policy` profile and every field of `input` in [VERIFY-01](../../../specs/conformance/verification-cases.json)
- WHEN the `review` procedure for case `VERIFY-01` runs against those inputs
- THEN the observations match every field of `expected` in case `VERIFY-01`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: VERIFY-05 for V-GATE-05

- GIVEN the `Implementation-Policy` profile and every field of `input` in [VERIFY-05](../../../specs/conformance/verification-cases.json)
- WHEN the `review` procedure for case `VERIFY-05` runs against those inputs
- THEN the observations match every field of `expected` in case `VERIFY-05`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

## 7A. Safety extension obligations

SPEC-S001 defines the mandatory Noble-safe claim. The following obligations extend, rather than replace, PO-01 through PO-21.

| Obligation | Claim to establish | Initial evidence route |
|---|---|---|
| SO-01 | Accepted core executions have no forbidden stuck state and no undefined-behavior semantic outcome | Lean 4 |
| SO-02 | Representation/eligibility rules exclude resource or authority forgery through ordinary data | Lean 4 |
| SO-03 | Effect soundness holds for every finite prefix, including denied/failed host requests | Lean 4 |
| SO-04 | Whole-configuration ownership is preserved modulo declared host transitions | Lean 4 |
| SO-05 | `Syntax`, untrusted metadata, and artifacts cannot become callable without validation/preparation | Lean 4 + checker refinement |
| SO-06 | Concrete public host/resource wrappers establish all verified internal preconditions from hostile representations | Aeneas-refined validation plus concrete wrapper evidence |
| SO-07 | Selected Noble-to-Wasm lowering preserves the claimed safety-relevant behavior/failure relation | Backend proof or translation validation |
| SO-08 | Standard concurrency semantics preserve isolation, scope-owned conversational state, protocol typing, and runtime capability checks | Lean 4, after concurrency profile freeze |
| SO-09 | Concrete concurrency runtime state machinery refines SO-08 under documented synchronization assumptions | Aeneas-refined sequential transitions plus separate synchronization evidence |

### Requirement: V-SAFE-01
r[V-SAFE-01]

**V-SAFE-01.** A stable Noble-safe core claim MUST include completed applicable SO obligations in addition to the existing stable-core gates. A theorem about ideal source semantics alone does not establish safety of hostile host boundaries or emitted executable artifacts.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: V-SAFE-02
r[V-SAFE-02]

**V-SAFE-02.** A guest-language safety proof MUST model all permitted outcomes as defined outcomes. Wrong operand types, stack underflow, consumed-handle use, forged live authority, and arbitrary representation reinterpretation are forbidden stuck states or rejected boundary inputs, not allowed traps used to hide unsoundness.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: V-SAFE-03
r[V-SAFE-03]

**V-SAFE-03.** Under the standard profile, Noble participants MUST have no direct shared mutable Noble memory. This is the standard-concurrency race-freedom invariant. It does not remove the obligation to prove or validate synchronization in the Rust runtime that implements the abstract dataspace/facet model.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: V-SAFE-04
r[V-SAFE-04]

**V-SAFE-04.** Future advanced type-system features may join a stable-safe subset only when their declarative rules, acceptance checks, implementation correspondence, and affected safety theorems are included in the release evidence.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

## 8. Explicit non-goals and open work

This revision does not prove every application, make host availability decidable, verify Wasmtime or rustc, establish constant-time execution, supply cryptographic proof compression, or introduce a new general-purpose prover. Those properties require separate models and evidence where desired.

The full checker algorithm, Lean mechanization, Rust components, compatible tools, boundary models, evidence transport, and Wasm correspondence remain open.

[CORE-BOOTSTRAP.md](../core-bootstrap/spec.md) scopes the first finite checker. [ROADMAP.md](../../../specs/ROADMAP.md) places extraction feasibility beside the checker and Wasm experiments. [EVIDENCE.md](../evidence/spec.md) prevents document validation from changing runtime or proof status.
