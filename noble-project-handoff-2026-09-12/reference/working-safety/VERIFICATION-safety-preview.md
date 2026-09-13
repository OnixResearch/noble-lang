# Verification and Trust Model

Document: SPEC-V001  
Revision: 0.1.0-draft.1  
Date: 2026-09-09  
Project: noble  
Status: Working specification; requirements adopted for this verification workstream, not implemented or proved  
Baseline: SPEC-0001, 0.1.0-draft.1

## 1. Purpose and authority

Across this specification family, **MUST** and **SHALL** are mandatory, **MUST NOT** and **SHALL NOT** are prohibitions, **SHOULD** is a recommendation permitting a documented exception, and **MAY** is permission. Requirement identifiers are revisioned; normative obligation wording is not a claim that the obligation has been fulfilled.

Noble SHALL have a small, mechanized semantic core and support optional compositional verification of individual programs. Verification is a development and assurance commitment, not a fourth expression form or a requirement that every caller supply a handwritten proof.

This document specifies the claims to establish and the boundaries those claims must preserve. [VERIFICATION-TOOLCHAIN.md](VERIFICATION-TOOLCHAIN.md) selects Lean 4, Charon/Aeneas, and Verus for distinct implementation roles. [PROGRAM-CONTRACTS.md](PROGRAM-CONTRACTS.md) specifies optional behavioral contracts and evidence. [SPEC-AMENDMENT.md](SPEC-AMENDMENT.md) integrates these requirements into SPEC-0001 without rewriting unrelated syntax work.

**V-STATUS-01.** A specification requirement, a mathematical theorem statement, an implemented checker, an executed test, and an accepted proof MUST be reported as distinct things. The obligations in this package are all open; the accompanying fixtures are expectations, not executed Noble tests.

**V-AUTH-01.** The reviewed Lean 4 formalization SHALL be the reference mathematical model for verification claims. Its exact source revision and supporting definitions MUST be identified. A mismatch between that model and the language specification is a specification defect to resolve explicitly; neither a successful proof nor an informal paragraph silently overrides the other.

The existing draft already requires stack/type safety, conservative effects, resource accounting, immutable recipes, and explicit preparation. It explicitly does not establish a complete checking algorithm or soundness proof. These are the source-derived starting obligations, not completed results. [B1 §§5.4, 8–9, 11, 16; references in SOURCES.md]

## 2. Preserve the small language

**V-CORE-01.** Ordinary `Program<S,T,e>`, `quote`, `compose`, `run`, and `reflect` MUST retain their existing semantics. Ordinary execution MUST NOT require Lean, Aeneas, Verus, an SMT solver, or proof search to be installed or invoked. An admission service MAY check evidence separately under an explicit host policy.

**V-CORE-02.** This revision introduces no dependent types, general refinement-type inference, proof expressions, solver directives, implicit behavioral coercions, or termination requirement for all Noble programs. The core checker MUST NOT depend on arbitrary application contracts to establish stack, eligibility, or effect validity.

**V-CORE-03.** A new core construct or semantic change MUST identify its effect on the typing rules, operational model, ownership invariant, recipe observations, and existing proofs. Before stable-core inclusion, its relevant obligations MUST be discharged or the feature MUST remain explicitly outside that stable subset. A tool limitation is not permission to weaken the language silently.

User-facing annotations and proof-language ergonomics remain deferred. Specifications use mathematical notation and host-side metadata, not proposed Noble source syntax. No punctuation is allocated to proofs, and `#` is not repurposed away from the user's comment-syntax direction.

## 3. Three verification targets

| Target | Required correspondence | What it does not establish by itself |
|---|---|---|
| Language metatheory | Formal typing and evaluation rules imply their stated invariants | Correctness of the production Rust checker or backend |
| Implementation | Particular Rust functions refine their abstract contracts | Soundness of a different function, feature configuration, compiler, or binary |
| Noble program behavior | An exact program satisfies an exact claim under stated assumptions | Correctness of its supplied Wasm, satisfaction of its precondition, or authority to execute |

**V-CLAIM-01.** Every assurance claim MUST state its target, scope, semantic revision, assumptions, evidence class, and unresolved correspondence boundaries. “Formally verified Noble” MUST NOT be used as an unqualified substitute for those fields.

A kernel proof about an Aeneas-generated Lean function is evidence about that function. Its application to Rust also relies on the extraction path and external models. A Verus result belongs to the Verus verification chain. Neither automatically proves that a Noble-to-Wasm compiler preserves semantics. These distinctions are normative Noble policy; the selected tools' documented roles motivate them. [R1–R7]

## 4. Formal model requirements

### 4.1 Domains and explicit assumptions

**V-MODEL-01.** The Lean model MUST represent resolved bodies and references, value and stack types, instantiated first-class program interfaces, rank-1 named schemes, effect bounds, eligibility judgments, values and captures, recipe normalization, and an immutable definition environment. Recursive references MUST retain owner scope.

The initial mechanization MAY cover a smaller bootstrap subset, but MUST enumerate omissions. A result for nonrecursive arithmetic quotations MUST NOT be presented as a result for the whole language, lists, recursion, host operations, or resources.

**V-MODEL-02.** The model MUST separate language evaluation, host responses, resource ownership, and execution-profile failures. Host contracts SHALL be explicit parameters with well-formedness and preservation conditions. Their satisfaction by a concrete adapter is a separate implementation obligation.

A useful configuration refinement is:

```text
Configuration = (pending code, data stack, continuation frames,
                 host state, ownership accounting, event history)
```

This is proof notation, not a frozen runtime representation. Continuation frames include values temporarily held by `dip`. Pure programs need not carry a concrete host-state object in their runtime representation.

### 4.2 Steps, waiting, failure, and divergence

**V-MODEL-03.** The model MUST distinguish normal return, permitted abnormal termination, host waiting where supported, divergence, and forbidden language-level stuck states. Ordinary `Result` error variants are normal returns. An implementation MUST NOT conceal a wrong operand type, stack underflow, or consumed-handle use by labeling it an allowed trap.

The ideal core semantics may abstract from allocation and execution budgets. The relation to a bounded execution profile must then state how allocation failure, cancellation, and fuel exhaustion are represented. Exhausting fuel in a test evaluator is not evidence that the source program diverges.

**V-MODEL-04.** Safety and effect claims MUST cover finite prefixes of executions, including prefixes of diverging, waiting, and abnormally terminating computations. A theorem only about successful returns is insufficient for either claim.

A closed, well-typed configuration must either be at an allowed outcome, be waiting at a permitted host boundary, or admit a modeled step. This is progress relative to the stated host model, not a host-availability or termination guarantee.

### 4.3 Host effects and authority

**V-EFFECT-01.** Effect soundness SHALL establish that each guest-requested `HostOpId` in every modeled execution prefix is in the instantiated effect bound. The model MUST account for requests that are denied or fail, not only successful responses. Logical request labels need not become publicly stored audit logs.

Operation internals are governed by the host contract; an effect set is not an inventory of every OS action inside native code. Runtime housekeeping and abort cleanup MUST be classified separately from guest-requested operations rather than incorrectly added to quotation construction's latent effects.

**V-EFFECT-02.** Construction and inspection MUST issue none of the guest host requests latent in the programs they manipulate. Invocation and higher-order combinators MUST propagate their called programs' bounds without changing event order.

Effect sets summarize possible operation identities. They do not prove event counts, temporal protocols, success, authorization, or information-flow properties.

### 4.4 Ownership across the whole machine

**V-RESOURCE-01.** Ownership invariants MUST range over the entire live configuration, including aggregate payloads, hidden `dip` values, invocation/session transfers, and pending host-boundary state. The model MUST track multiplicity of ownership obligations; a set that silently collapses duplicate handles is insufficient.

A normal host transition may create, transfer, replace, or retire obligations according to its contract. Conservation therefore means preservation modulo those declared transitions, not preservation of an unchanging number of handles. Proofs MUST distinguish a guest handle's obligation from physical exclusivity of the underlying external object.

**V-RESOURCE-02.** The formal eligibility relation MUST exclude resource-bearing values recursively from `Data` and `Capture`, including all payload alternatives allowed by an instantiated schema. Program interfaces may mention resources without making the program's environment resource-owning. Captured environments MUST actually satisfy their own eligibility invariant.

**V-RESOURCE-03.** Normal-path preservation and abnormal cleanup SHALL have separate claims. A guest typing proof does not prove host cleanup. The cleanup claim MUST state invocation boundaries, locally observable retirement, repeat-retirement behavior, and prevention of stale-handle resurrection. It MUST NOT assert remote exactly-once cleanup or cleanup after total machine failure without a separately modeled protocol.

### 4.5 Arithmetic and observation

**V-NUM-01.** The model and every implementation bridge MUST use the baseline's wrapping `I64` semantics for `+`, `-`, and `*`. Mathematical integers in Lean or Verus MUST be connected to these operations by an explicit modulo-`2^64` representation relation. Out-of-range literals remain rejections, not wrapped literals. [B1 §4.4]

**V-OBS-01.** Proofs MUST distinguish execution-only equivalence from recipe equality and contextual equivalence in a language with `reflect`. Program-valued results and programs nested inside returned data retain their observable recipes. Equal scalar answers alone do not justify substituting program values in all contexts.

For example, calling `[ 1 1 + ]` and `[ 2 ]` gives the same integer in the ideal pure semantics, but reifying those program values distinguishes them. A backend may optimize executable instructions while preserving the specified recipe. An explicit source transformation may produce a different program identity. [B1 §11.2]

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

**V-CHECK-01.** Acceptance MUST validate the candidate from an untrusted representation. A proof whose checker input already has the desired typing invariant by construction does not establish validation of hostile input. Declared signatures, type witnesses, schemas, effects, and dependency identities MUST NOT be accepted solely because the producer supplies them.

**V-CHECK-02.** Inference MAY be complex or initially unverified; acceptance MUST independently validate the relevant elaboration evidence against the declarative rules. Acceptance MUST include reference scope, scheme instantiation, stack order, eligibility, conservative effects, and resource obligations. The exact witness encoding and complete inference algorithm remain separate open deliverables.

**V-CHECK-03.** The acceptance theorem SHALL have the following shape, with explicit representation and environment well-formedness premises:

```text
check(candidate, expected_interface, environment) = Accepted(checked)
    implies
WellFormed(checked) and TypingDerivation(checked, expected_interface)
```

Correct extraction of the Rust function and its correspondence to `check` are additional claims. Well-formedness of an externally supplied environment must itself be established or appear as a disclosed trusted boundary.

**V-CHECK-04.** Rejection, unsupported input, resource exhaustion, internal failure, and successful acceptance MUST be distinguishable internally and in assurance reports. A checker MUST NOT accept on timeout. Soundness does not imply completeness, and a diagnostic MUST NOT claim a proof of semantic impossibility without an applicable theorem.

**V-CHECK-05.** A usable-checker claim MUST include positive acceptance coverage and a termination argument for the selected finite checking problem under the ideal model. Rejecting every candidate cannot satisfy the advertised supported-language coverage. Cyclic type equations, recursive schema checks, and effect constraints must not be hidden sources of divergence.

**V-CHECK-06.** Validation MUST finish before the submission body runs. `prepare` performs its own documented service effects but MUST NOT execute candidate bodies, tactics, or arbitrary user macros as an undocumented checking step. Successful checking does not prove an optional behavioral contract.

### 5.2 Required theorem families

The machine-readable backlog is [verification/obligations.json](verification/obligations.json). The identifiers below are stable within this revision series; statements are theorem obligations, not Lean declarations already implemented.

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
| PO-13 | Handle-table access and retirement preserve the imperative contract | Verus |
| PO-14 | Host adapter, wrapper, and cross-tool contract correspondence | Explicit boundary evidence; mixed tools |
| PO-15 | Behavioral contract sequencing and builder-family rules are sound | Lean 4 |
| PO-16 | Evidence admission binds the intended claim, subject, and assumptions | Lean 4 / Rust refinement, after envelope design |
| PO-17 | Selected backend lowering preserves execution and recipe observations | Separate Lean/Wasm proof or validated-pass effort |
| PO-18 | Artifact loading binds executed Wasm to checked program and recipe | Separate loader/compilation trust-chain effort |

Typing rules for recursive named definitions and generalization are part of the eventual PO-01/02/09 coverage. They are not discharged by a theorem for only monomorphic, nonrecursive source. Parser correctness, canonical binary encoding, ABI details, and full schema checking remain separately scoped until their specifications exist.

## 6. Evidence classes and trust closure

**V-EVIDENCE-01.** Reports MUST label evidence as one of: `lean-kernel`, `aeneas-lean`, `verus`, `translation-validation`, `test`, `review`, or `assumption`. These are not a numerical ranking and are not interchangeable. A `translation-validation` result MUST name its validator and that validator's own assurance.

**V-EVIDENCE-02.** A claim's trust description MUST include the transitive assumptions of its supporting claims and bridges. No translation, external model, host contract, or build step becomes proved merely by being placed between two proved components.

**V-EVIDENCE-03.** “Lean-kernel checked” SHALL mean that the exact theorem and proof have been accepted under the selected Lean policy. It MUST NOT mean that Lean verified the correctness of Charon, Aeneas, Verus, rustc, LLVM, the Wasm engine, or the hardware. Additional compiler/native-evaluation trust must be disclosed if permitted.

Lean's validation guidance distinguishes proof validity from theorem meaning and recommends axiom inspection and stronger checking for untrusted submissions. This workstream adopts the concrete policies in IMPL-V001 rather than treating a green editor indicator as the complete assurance argument. [R4]

**V-EVIDENCE-04.** A proof result MUST bind exact relevant source and dependency revisions, feature flags, target architecture, semantic model, statement, and toolchain. Changed inputs invalidate the prior applicability decision until revalidated. Unchanged code may acquire new proofs without changing its language identity.

## 7. Release gates and staged delivery

These stages order work; they are not time estimates.

| Stage | Deliverable | Permitted claim after actual completion |
|---|---|---|
| V0 | This specification, boundary register, fixture expectations | Verification design specified |
| V1 | Lean model and metatheory for enumerated bootstrap quotation subset | Named theorems for that model/subset |
| V2 | Explicit acceptance checker; Aeneas extraction and refinement; compatible pinned tools | Rust-checker properties relative to disclosed extraction/build assumptions |
| V3 | Resource extension, Verus handle table, checked wrappers and bridges | Scoped mixed-tool resource assurance |
| V4 | Optional contracts, builder-family proofs, evidence admission prototype | Exact behavioral claims for accepted subjects |
| V5 | Selected Wasm lowering and loading correspondence | Only the specifically connected compilation/execution claims |

**V-GATE-01.** A stable core specification MUST have reviewed, Lean-kernel-checked metatheory for the entire subset it labels stable, covering PO-01 through PO-10 as applicable. A proof plan alone is insufficient. Exploratory drafts and implementations MAY exist earlier but MUST retain their draft and coverage labels.

**V-GATE-02.** An implementation advertised as having a verified acceptance checker MUST additionally satisfy PO-11 for the actual supported implementation and publish its translation/model assumptions and configuration. Finishing V1 alone cannot justify that label.

**V-GATE-03.** A claim of source-to-Wasm behavioral preservation or end-to-end assurance MUST identify evidence covering the backend, loading, runtime adapters, and remaining execution trust. Otherwise those boundaries MUST remain visibly unproved or assumed. Stable language semantics do not by themselves mean a verified compiler.

**V-GATE-04.** Any release that advertises verification SHALL publish a claim ledger, feature coverage, transitive assumption register, pinned toolchain, and executed validation results. Unproved obligations and unsupported features MUST remain visible. CI MUST reject unexplained growth of the trusted boundary or weakened claims.


## 7A. Safety extension obligations

SPEC-S001 defines the mandatory Noble-safe claim. The following obligations extend, rather than replace, PO-01 through PO-18.

| Obligation | Claim to establish | Initial evidence route |
|---|---|---|
| SO-01 | Accepted core executions have no forbidden stuck state and no undefined-behavior semantic outcome | Lean 4 |
| SO-02 | Representation/eligibility rules exclude resource or authority forgery through ordinary data | Lean 4 |
| SO-03 | Effect soundness holds for every finite prefix, including denied/failed host requests | Lean 4 |
| SO-04 | Whole-configuration ownership is preserved modulo declared host transitions | Lean 4 |
| SO-05 | `Syntax`, untrusted metadata, and artifacts cannot become callable without validation/preparation | Lean 4 + checker refinement |
| SO-06 | Concrete public host/resource wrappers establish all verified internal preconditions from hostile representations | Verus / checked-wrapper evidence |
| SO-07 | Selected Noble-to-Wasm lowering preserves the claimed safety-relevant behavior/failure relation | Backend proof or translation validation |
| SO-08 | Standard concurrency semantics preserve isolation, scope-owned conversational state, protocol typing, and runtime capability checks | Lean 4, after concurrency profile freeze |
| SO-09 | Concrete concurrency runtime state machinery refines SO-08 under documented synchronization assumptions | Verus/mixed implementation evidence |

**V-SAFE-01.** A stable Noble-safe core claim requires completed applicable SO obligations in addition to the existing stable-core gates. A theorem about ideal source semantics alone does not establish safety of hostile host boundaries or emitted executable artifacts.

**V-SAFE-02.** A guest-language safety proof MUST model all permitted outcomes as defined outcomes. Wrong operand types, stack underflow, consumed-handle use, forged live authority, and arbitrary representation reinterpretation are forbidden stuck states or rejected boundary inputs, not allowed traps used to hide unsoundness.

**V-SAFE-03.** Standard-concurrency race-freedom means Noble participants have no direct shared mutable Noble memory under the standard profile. It does not remove the obligation to prove or validate synchronization in the Rust runtime that implements the abstract dataspace/facet model.

**V-SAFE-04.** Future advanced type-system features may join a stable-safe subset only when their declarative rules, acceptance checks, implementation correspondence, and affected safety theorems are included in the release evidence.

## 8. Explicit non-goals and open work

This revision does not prove every application, make host availability decidable, verify Wasmtime or rustc, establish constant-time execution, supply cryptographic proof compression, or introduce a new general-purpose prover. Those properties require separate models and evidence where desired.

The full checker rules/algorithm, Lean mechanization, actual Rust components, compatible tool versions, boundary-model implementations, canonical evidence transport, and Wasm correspondence remain to be produced. Their open status is recorded in DECISIONS.md and the obligation ledger, not hidden by this document's normative wording.
