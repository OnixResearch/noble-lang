# M1 workspace acceptance delta

## ADDED Requirements

### Requirement: VT-M1-01
r[VT-M1-01]

M1 MUST select immutable, mutually compatible build and verification inputs through executed compatibility checks.
The inputs MUST cover Nixpkgs, production Rust, extraction rustc, Charon, Aeneas, Lean/Lake, its backend libraries, Octet, and scoped Miri tooling.
Nix MUST generate `flake.lock`. The selected Charon and Lean backend MUST satisfy the selected Aeneas revision's compatibility requirements.
The configuration MUST name required targets, features, word size, panic behavior, overflow behavior, generation arguments, and resource limits.
Ambient sibling checkouts, floating revisions, and manual pin-check overrides MUST NOT establish compatibility.
Future unselected components MUST remain explicit without supporting a release claim.

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

M1 MUST establish a deterministic semantic-kernel crate and a separate CLI shell with dependencies directed toward the kernel.
The kernel MUST use safe sequential Rust with explicit owned inputs and typed outcomes.
The kernel MUST NOT access ambient I/O, environment state, clocks, randomness, or hidden mutable state.
The shell MUST own external observations, orchestration, and effect execution.
Real external capabilities MUST use application-owned contracts with adapters outside the kernel.
The initial production extraction subject MUST include positive and exhausted budget outcomes without a public language-syntax change.

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

M1 MUST compare compiler-derived source and function coverage with a complete verification inventory.
The inventory MUST classify production Rust, generated and macro bodies, dependencies, tests, non-Rust tools, and future components.
Each current production body MUST map to extracted work, an external model, an exact reviewed exception, or explicit open work.
The inventory MUST retain dependency closure, target and feature coverage, linked contracts, and separate extraction and refinement statuses.
Non-kernel exceptions MUST satisfy VT-SCOPE-04. Kernel bodies MUST NOT use exceptions or crate relocation to escape mandatory scope.
Required gates MUST reject omissions, stale mappings, missing facts, and unsupported required configurations.

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

M1 MUST provide a bounded Charon → Aeneas → Lean entry point for the actual production workspace subject.
The entry point MUST regenerate outputs after relevant source, dependency, macro, feature, target, or contract changes.
The gate MUST bind generated functions to the exact source and configuration inventory.
Hand-edited generated functions and unrelated fixtures MUST NOT substitute for actual extraction.
Missing functions, unexplained opaque models, unsupported required bodies, and stale artifacts MUST block the claimed scope.
Successful extraction and Lean compilation MUST NOT establish a refinement proof or Wasm correspondence.
Proof-required claims MUST reject unresolved required obligations, proof holes, and reference-only substitutes.

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

M1 MUST enforce the full pinned Octet lint catalog as errors across the workspace, all targets, and all compatible features.
Nix and the `octet-deny-all` pre-commit hook MUST use the same immutable Octet revision and required scope.
The workspace MUST declare its Octet metadata and core source scopes without disabled lints, warning budgets, or finding baselines.
Architecture policy MUST use reviewed Nickel source, a checked export, and a freshness manifest.
The policy MUST declare roles, packages, sources, providers, dependencies, actual ports and adapters, targets, and features.
Required compiler facts MUST cover production libraries, binaries, and tests without advisory-only or library-only substitution.
Local acceptance and CI MUST run the same Nix gate, including formatting, Rust tests, strict Clippy, document checks, and required assurance lanes.

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

M1 MUST inventory native boundaries, relevant dependencies, generated code, and Noble-owned unsafe sites.
Every applicable unsafe site MUST retain the safety argument and exact ownership required by VT-NATIVE-01.
Dependency records MUST name versions, features, macros, target assumptions, and verification boundaries.
An empty Noble-owned unsafe inventory MUST derive from complete source accounting and MUST NOT imply that dependencies contain no unsafe code.
M1 MUST include scoped positive and negative native-assurance controls with pinned Miri targets and memory-model configuration.
Unsupported required Miri runs MUST remain blockers. Byte-view dependencies MUST NOT enter the initial owned-data acceptance core without VT-NATIVE-03 evidence.

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

M1 acceptance MUST use independently expected subjects, claims, source scopes, dependencies, policy, tools, targets, features, commands, inputs, assumptions, and results.
Every required phase MUST retain its own outcome and coverage, including open, unsupported, failed, and unexecuted work.
Lint, architecture, extraction, native tests, artifact integrity, and formal proofs MUST retain separate evidence roles.
Stale bindings, missing required phases, and evidence-role promotion MUST block acceptance.
M1 MUST NOT close through document validation, an unrelated extraction fixture, or passing checks for only part of its required configuration.
Spec sync MUST follow implementation acceptance. Archive MUST follow the completed checklist and an unblocked archive plan.
Approval of this planning package MUST NOT authorize either completion claim.
M1 completion MUST leave unrelated runtime, refinement, and later-milestone claims open unless their own evidence satisfies the applicable contracts.

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
