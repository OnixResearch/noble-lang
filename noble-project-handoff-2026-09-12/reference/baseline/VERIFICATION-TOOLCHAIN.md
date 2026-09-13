# Verification Toolchain and Implementation Boundaries

Document: IMPL-V001  
Revision: 0.1.0-draft.1  
Date: 2026-09-09  
Status: Selected implementation architecture; no toolchain compatibility or executed proofs claimed  
Depends on: [SPEC-V001](VERIFICATION.md)

## 1. Selected roles

**VT-ROLE-01.** Noble SHALL use Lean 4 for its reference semantics, metatheory, and reusable program-contract logic. It SHALL use Charon plus Aeneas's Lean backend as the preferred route for relating the safe, sequential Rust checking core to that model. It SHALL use Verus for selected imperative Rust components, beginning with local handle-table invariants and retirement transitions.

This is a division of responsibility, not three independent implementations of the entire language.

| Component or task | Primary route | Required evidence or boundary |
|---|---|---|
| Semantic domains, typing, evaluation, resource logic | Handwritten Lean 4 definitions and theorems | Kernel checking; reviewed correspondence to SPEC-0001 |
| Explicit acceptance checker and pure validation utilities | Ordinary Rust → Charon → Aeneas → Lean 4 | Proof that generated functions refine the reference contract |
| Integer/recipe representation utilities | Same Rust-to-Lean route where supported | Explicit bit-width, normalization, and representation relations |
| Handle table and imperative local lifecycle state | Verus-annotated Rust | Invariants, checked public wrappers, and abstract-contract correspondence |
| Cache/arena/loader internals | Verus where beneficial after a separate scoped design | No inherited assurance merely from tool selection |
| Parser, resolution UI, CLI, filesystem, engine integration | Ordinary Rust initially | Tests and named trust boundaries; not labeled proved |
| Noble-to-Wasm lowering | Separate proof or translation-validation workstream | Execution and reflection correspondence, not supplied by Aeneas |
| Optional Noble program contracts | Lean 4 over Noble semantics | Exact subject, claim, and assumption binding |

The Aeneas projects page describes Charon's MIR-to-LLBC extraction and Aeneas's functional translation, and recommends its Lean backend. Verus documents a static Rust verification approach with specification, proof, and executable modes. These are external capabilities; the component assignment above is a new Noble design decision. [R1, R5, R6]

**VT-ROLE-02.** Each component MUST have one declared primary verification route. Dual verification MAY be used for selected boundaries, but MUST NOT be required by default. No tool result may silently substitute for a result in another logic.

**VT-ROLE-03.** Eurydice and Scylla are not Noble dependencies in this revision. Their Rust-to-C and C-to-Rust roles do not supply the selected Noble-to-Wasm backend. Charon and Aeneas are verification/build-time dependencies, not guest execution machinery. [R1]

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

**VT-AENEAS-01.** The first acceptance-checker crate MUST stay within an experimentally confirmed supported subset of the pinned Charon/Aeneas pair. The starting implementation policy is safe, sequential Rust, explicit owned data, simple control flow, and narrow modeled dependencies. This is a Noble coding policy, not a claim of support for every such program.

Aeneas's current README lists limitations, including some nested-loop control flow and generic instantiation with mutable references. Charon warns about incomplete coverage and incorrect translation in some edge cases. Therefore extraction is a real trust boundary, not a proved compiler assumed by the architecture. [R2, R3]

**VT-AENEAS-02.** Unsupported constructs MUST fail the relevant verification lane. They MAY be isolated behind a reviewed boundary or redesigned; they MUST NOT be silently dropped, translated to unconstrained success, or replaced by an axiom claiming the desired result. A separately proved abstraction may be used only with its concrete correspondence accounted for.

**VT-AENEAS-03.** Generated Lean files MUST be regenerated from the claimed Rust revision and kept separate from handwritten specifications and proofs. Editing generated functions to make proofs pass invalidates the Rust correspondence claim. Generation commands, source/configuration digests, generated-file digests, and extraction diagnostics MUST be retained.

**VT-AENEAS-04.** The refinement theorem MUST connect actual generated functions to the intended checker or utility contract. Merely compiling the generated Lean module, or proving properties of a separately rewritten reference function, is insufficient. The theorem MUST account for all modeled return alternatives, failure, and the relevant representation invariants.

**VT-AENEAS-05.** Every external model, opaque function, modeled standard-library operation, panic behavior, and admitted translation boundary MUST appear in the assumption register with its concrete target. A definition implemented in Lean is not automatically a proof that the corresponding Rust library behaves that way. Generated templates with missing obligations MUST NOT count as verified implementation.

**VT-AENEAS-06.** Reports using this route MUST retain rustc/MIR extraction, Charon, Aeneas, and external-model fidelity as trusted components unless separate evidence discharges the exact boundary. The Lean kernel checks the resulting theorem, not the faithfulness of the extraction process. Published research about an algorithm is not proof that every current tool binary implements it correctly.

**VT-AENEAS-07.** A theorem asserting success or termination MUST distinguish translation-level failure or divergence from Noble domain-error values. Aeneas's ability to represent partial functions MUST NOT be reported as an automatic termination proof. [R2]

The production checker remains Rust. Lean's definitions or an executable reference evaluator may be used as test oracles; they do not become a second Noble production VM.

## 3. The Verus lane

**VT-VERUS-01.** Verus SHALL verify exact executable functions against explicit specification functions and invariants. Proof and ghost state MUST NOT change Noble-visible behavior or be assumed to exist at runtime. The compiled code/configuration must match the one whose verification conditions were checked.

**VT-VERUS-02.** The first component SHALL model a local resource handle table. Its specification MUST cover owner context, resource kind, liveness, transfer, retirement, and rejection of stale, forged, wrongly typed, or cross-context handles. The representation may use generations or another reviewed stale-reference defense; this document does not freeze an ABI.

Normal successful calls and ordinary error returns must preserve each contract's disposition of ownership. Repeated retirement must not cause repeated local release or make a retired handle live again. OS effects and remote cleanup remain separate adapter contracts.

**VT-VERUS-03.** Each verified component MUST inventory `assume`, admitted proof bodies, external bodies/specifications, ignored code, unsafe implementations, and other mechanisms adding trust. Inventory MUST include relevant library and macro-expanded dependencies, not just text in the project's top-level file. Unexplained additions MUST fail assurance CI.

Verus documents several mechanisms for trusting code or specifications without checking bodies. In this profile those mechanisms are permitted only at named, reviewed boundaries; they do not discharge the boundary's obligation. [R7, R8]

**VT-VERUS-04.** A Verus success report MUST NOT be represented as a Lean proof certificate. The trust inventory MUST include the selected Verus implementation, relevant specification library, verification-condition translation, SMT solver, and external specifications. A future independently checked proof interchange mechanism requires a separate specification and validation before changing this policy.

**VT-VERUS-05.** Termination or total-correctness claims MUST have explicit evidence covering the claimed call graph and host assumptions. A decreases annotation on an executable function alone is not sufficient. The Verus guide describes its executable termination checks as conditional on callees terminating. [R9]

### 3.1 Public wrappers are a security boundary

**VT-WRAP-01.** Functions called from unverified Rust, FFI, Wasm, or a host callback MUST validate all preconditions not guaranteed by an established caller invariant. Verification-only preconditions MUST NOT be used to remove checks on adversarial inputs at a public boundary. A wrapper SHALL either reject safely or establish the verified internal function's full entry contract.

This matters because Verus's safety argument can depend on verified callers meeting preconditions; ordinary Rust type-checking is not the same assurance. [R10]

**VT-WRAP-02.** Guest handles and imported values MUST be validated on their concrete representation and context. A proof attached to a program does not make arbitrary integers valid handles or grant host access. Concurrency, callbacks, and reentrancy MUST be excluded explicitly or covered by the invariant and wrapper protocol; a sequential proof does not automatically survive reentrant mutation.

**VT-WRAP-03.** Unverified callers MUST NOT be able to manufacture a privileged internal state by invoking an otherwise safe-looking constructor or mutator. Entry, exit, and construction boundaries are part of the verification scope.

## 4. Cross-tool contracts

**VT-BRIDGE-01.** A boundary between the Lean/Aeneas lane and the Verus lane MUST publish an abstract contract, concrete representation relation, argument/result ordering, integer widths, error alternatives, ownership transitions, and allowed effects. Both sides MUST name the same contract revision.

For the resource table, the shared contract describes abstract handle obligations and authorized transitions. It does not assert that Lean values and Verus ghost values have identical runtime representations.

**VT-BRIDGE-02.** Each correspondence edge SHALL have a status: `proved`, `validated`, `reviewed-assumption`, or `open`. Matching names, copied comments, matching digests, or passing examples alone do not make an edge `proved`. A report MUST identify the proof system and theorem for a proved edge and the exact validator for a validated edge.

A mixed-tool result may legitimately rely on a reviewed-assumption bridge. It must say so. A Lean theorem using a host-contract hypothesis is conditional on that hypothesis; a separate Verus result does not discharge it inside Lean without an explicit justified connection.

**VT-BRIDGE-03.** The project MUST NOT assume Verus-annotated source can pass through Charon/Aeneas unchanged. Components SHOULD meet through ordinary, narrow Rust interfaces. Any generated or annotation-erased source used in another lane requires its own source-to-source correspondence and configuration record.

**VT-BRIDGE-04.** Arithmetic bridges MUST preserve wrapping operations, signed interpretation, checked literal ranges, and architecture-dependent index widths. Mathematical `int` or `nat` values MUST NOT replace executable fixed-width semantics without a proof of the representation relation.

## 5. Lean acceptance policy

**VT-LEAN-01.** Release-gating core theorems MUST be kernel checked under a pinned Lean 4 toolchain, with a transitive axiom inventory. The default allowed logical foundations are Lean's standard `propext`, `Classical.choice`, and `Quot.sound`; use of a smaller set is acceptable. `sorryAx`, unfinished proofs, and axioms that simply assert Noble's desired guarantees MUST fail acceptance.

Host behavior, well-formed environments, and external state laws SHOULD be explicit theorem parameters rather than hidden global axioms. Their concrete satisfaction remains an assurance obligation.

**VT-LEAN-02.** Native-evaluation shortcuts or other mechanisms extending the default kernel trust policy MUST NOT be used for strict core release gates. An expanded-trust experimental lane MAY use them, but MUST identify the added assumptions and MUST NOT masquerade as the strict lane. Exact detection MUST follow the pinned toolchain, not a permanently hard-coded list of one version's mechanism names.

**VT-LEAN-03.** CI MUST rebuild theorem dependencies from the selected sources and perform a fresh proof recheck, using `lean4checker --fresh` where supported by the pinned toolchain or an explicitly reviewed equivalent. Axiom inspection and rechecking supplement, not replace, review of the intended statement and its definitions. [R4]

### 5.1 Untrusted and generated proof submissions

**VT-SANDBOX-01.** Unreviewed proof source, tactics, macros, dependencies, and build scripts SHALL be treated as executable untrusted input. Elaboration/build MUST run in an isolated worker without ambient credentials, unrestricted network access, production code-store write access, or authority to change the expected claim. Resource budgets MUST be enforced.

**VT-SANDBOX-02.** Acceptance MUST compare the proved statement and referenced definitions against a trusted, independently selected claim. It MUST validate the evidence representation and recheck the proof outside the influence of the producer's build process. Untrusted compiled proof objects MUST NOT simply be deserialized into a privileged verifier process.

This is the security goal; a concrete export format and hardened checker pipeline are open deliverables. Lean's validation guidance describes stronger challenge/solution comparison and external-checker workflows for this threat model. This draft does not claim such a workflow has been integrated. [R4]

**VT-SANDBOX-03.** Proof-generation changes to source, preconditions, postconditions, definitions, trusted boundaries, or proof-checker settings MUST be independently reviewed. A stronger precondition, weaker conclusion, or circular assumption can make a valid proof useless for the requested claim.

## 6. Toolchain pinning and reproducibility

**VT-PIN-01.** Every executed assurance run MUST identify immutable revisions and artifact digests for its tools and dependencies. Floating `main`, `latest`, unspecified rustc channels, or unrecorded solver versions MUST NOT be release evidence.

The pin set must cover Lean/Lake and proof libraries; Charon, its rustc/MIR configuration, Aeneas and its Lean backend; Verus, its rustc, specification library and SMT solver; and the production Rust/Wasm build profile when the claim reaches an executable artifact. Different lanes may require different rustc versions. Their compatibility must be demonstrated rather than assumed.

**VT-PIN-02.** The Charon revision and Lean backend configuration MUST match the selected Aeneas revision's compatibility requirements. Aeneas's README explicitly ties these dependencies together. A local symlink or manual tool override MUST NOT bypass the package's own pin checks. [R2]

**VT-PIN-03.** The tracked configuration MUST include target triple, word size, Cargo features, dependency lock, relevant `cfg` flags, panic strategy, overflow semantics, generation options, solver options, and timeout/resource limits. Configuration changes invalidate affected evidence applicability until rerun.

[verification/toolchain-lock.template.json](verification/toolchain-lock.template.json) is deliberately unselected. Its null revisions are not a usable lock file. No compatible set is claimed by publishing this package.

## 7. Continuous verification contract

**VT-CI-01.** CI SHALL maintain separate document-validation, Lean-model, Rust-extraction/refinement, Verus-component, boundary-review, and compiler/runtime-test lanes. Results MUST distinguish `passed`, `failed`, `timeout`, `unsupported`, and `not-run`. A nonzero exit, omitted obligation, or solver unknown result MUST NOT count as proof success.

**VT-CI-02.** Assurance runs MUST capture full obligation counts, tool exit status, diagnostics, source/configuration identities, trusted-boundary changes, and expected-claim changes. Grep-based checks MAY supplement but MUST NOT replace semantic and transitive dependency checks.

**VT-CI-03.** Changes to core primitives, typing rules, recipe observations, or host contracts MUST identify affected obligations and invalidate dependent assurance records. A proof against a weakened or differently interpreted statement cannot satisfy the old gate without review.

**VT-CI-04.** Differential and property-based tests SHOULD exercise reference semantics, Rust implementations, and Wasm execution on the same generated cases, including boundary integers and runtime-built programs. Such tests are regression evidence, not replacements for refinement theorems.

## 8. Implementation entry point

The first vertical slice SHALL implement the pure explicit acceptance checker, wrapping integer operations, and runtime-independent recipe builders in ordinary Rust; extract them with a pinned Charon/Aeneas pair; and prove one concrete checker/representation refinement against the Lean model. In parallel, the Wasm experiment SHALL exercise nonconstant `quote` and `compose` without claiming backend proof.

After that slice establishes a usable toolchain and proof pattern, extend the model to the complete stable core and introduce the Verus resource table behind checked wrappers. No source code, proof implementation, tool installation, or verifier execution is included in this specification package.
