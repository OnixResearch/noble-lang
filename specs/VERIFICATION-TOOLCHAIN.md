<!-- Generated compatibility view. Edit .cairn/specs/verification-toolchain/spec.md instead. -->
# Verification Toolchain and Implementation Boundaries

Document: IMPL-V001  
Revision: 0.1.0-draft.5  
Status: Canonical working tool policy; no compatible pin set or executed proof is claimed  
Depends on: [SPEC-V001](VERIFICATION.md) at 0.1.0-draft.5

[SOURCES.md](SOURCES.md) marks inherited external citation labels as historical, not fresh compatibility evidence.

## 1. Selected roles

**VT-ROLE-01.** Noble MUST use Lean 4 for its reference semantics, metatheory, and reusable program-contract logic. Noble-owned production Rust MUST target Charon → Aeneas → Lean 4 under the scope rules below. This route is mandatory for the semantic kernel, not merely preferred for selected checker utilities. Verus is an optional exception tool, not a second default implementation language.

Aeneas is a Rust verification toolchain, not the language in which Noble source or proofs are written. Production implementation code remains Rust. Reference definitions and refinement proofs remain Lean. Build configuration and documentation retain their appropriate formats.

| Component or task | Primary route | Required evidence or boundary |
|---|---|---|
| Reference semantics, metatheory, optional program contracts | Handwritten Lean 4 | Reviewed statements and kernel-checked proofs |
| Entire semantic kernel, including checking, values, builders, recipes, effects, and ownership | Safe sequential Rust → Charon → Aeneas → Lean 4 | Actual generated functions refine the reference contracts |
| Parser, resolver, inference, IR transformations, optimizers, and Wasm emission | Same Rust-to-Lean route | Staged function contracts; separate execution/reflection preservation theorems |
| Resource-table decisions, admission, loading decisions, caches, and runtime state transitions | Same Rust-to-Lean route | Deterministic transitions with explicit state and typed effect plans |
| CLI, storage, native allocation, FFI, Wasmtime, callbacks, and synchronization | Extractable Rust logic plus narrow external adapters | Concrete effect and synchronization contracts; reviewed exceptions for unsupported bodies |

The [Aeneas projects page](https://aeneasverif.github.io/projects/) recommends the Lean backend. The [upstream README](https://github.com/AeneasVerif/aeneas#targeted-subset-and-current-limitations) identifies unsafe code and concurrency as current limitations. These live sources establish the tool's stated scope, not compatibility with an unselected Noble implementation.

### 1.1 Whole-project scope and kernel priority

**VT-SCOPE-01.** All Noble-owned production Rust MUST appear in the verification inventory, including compiler frontends, backends, runtime, adapters, and CLI logic. New code MUST target the selected Aeneas subset from its first implementation. Delivery order MUST NOT become a permanent exclusion of non-kernel code. Non-Rust tools, generated code, and external dependencies MUST have explicit scope classifications rather than disappear from the assurance report.

**VT-SCOPE-02.** The entire semantic kernel MUST use safe, sequential, extractable Rust. Kernel scope includes types, substitutions, acceptance, eligibility, effects, stack/value operations, program builders, recipes, identity rules, and ownership transitions. Semantic admission and resource-table decisions also belong in this scope, regardless of their crate location. Kernel code MUST NOT use unsafe code, ambient I/O, hidden mutable state, or concurrency. An unsupported kernel function MUST block the affected verification gate until redesign or supported extraction. Moving that function into an adapter MUST NOT exempt its semantic decision.

Local mutation of explicit owned state is permitted within the supported subset. The reference model can describe concurrency as transitions over explicit state and event inputs. This does not establish correctness of a concurrent Rust executor.

**VT-SCOPE-03.** Shells MUST obtain external facts and execute typed effect plans from the deterministic core. Plans MUST NOT count as evidence that effects occurred. Host calls, FFI, callbacks, synchronization, and dependency models require separate concrete correspondence contracts. Resource retirement decisions MUST remain separate from physical release and remote cleanup.

**VT-SCOPE-04.** An unsupported non-kernel boundary requires a reviewed exception before inclusion in a claimed verification scope. Each exception MUST name exact symbols, source/configuration identity, an owner, and the tool diagnostic or external-effect constraint. It MUST record the abstract contract, assumptions, evidence, and a removal or reassessment condition. Whole-crate exclusions and convenience-only exceptions MUST NOT replace function-level accounting. Verus requires such an exception and the section 3 controls. No exception makes its body Aeneas-verified or discharges an open correspondence obligation.

**VT-SCOPE-05.** Each release inventory MUST map the complete production source set to extracted functions, external models, reviewed exceptions, or explicit open work. It MUST include dependency closure, macro/generated bodies, feature/target configurations, linked contracts, theorem identifiers, and independent extraction/proof statuses. Required verification gates MUST reject omissions, stale mappings, unapproved exceptions, and unresolved obligations in their claimed scope. Reports MUST show total, extracted, proved, modeled, excepted, and open counts with named scopes. Successful extraction alone MUST NOT count as a refinement proof.

The first experiment covers a narrow subset. It does not satisfy whole-kernel or whole-project verification. Draft code can retain open proofs, but no stable verified subset can include unresolved required claims.

**VT-ROLE-02.** Each component MUST have one declared primary verification route. Dual verification MAY be used for selected boundaries, but MUST NOT be required by default. No tool result may silently substitute for a result in another logic.

**VT-ROLE-03.** Noble MUST NOT select Eurydice or Scylla as dependencies in this revision. Their Rust-to-C and C-to-Rust roles do not supply the selected Noble-to-Wasm backend. Charon and Aeneas are verification/build-time dependencies, not guest execution machinery. [R1]

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

**VT-AENEAS-01.** Every component assigned to the Aeneas route MUST stay within the experimentally confirmed subset of the pinned Charon/Aeneas pair. The starting policy requires safe sequential Rust, explicit owned data, simple control flow, and narrow modeled dependencies. Safe Rust alone does not establish extractability.

The inspected upstream README excludes some nested-loop control flow and generic instantiation with mutable references. Exact support requires an extraction run under compatible pins. Extraction remains a trust boundary, not an assumed proved compiler. [Current source record](SOURCES.md#aeneas-first-direction-in-draft4)

**VT-AENEAS-02.** Unsupported constructs MUST fail the relevant verification lane. Kernel functions require redesign or supported extraction under VT-SCOPE-02. Only non-kernel boundaries can use the exception process in VT-SCOPE-04. Unsupported bodies MUST NOT disappear from coverage or become unconstrained success or axioms that assert the desired result. A separately proved abstraction requires explicit concrete correspondence.

**VT-AENEAS-03.** Generated Lean files MUST be regenerated from the claimed Rust revision and kept separate from handwritten specifications and proofs. Editing generated functions to make proofs pass invalidates the Rust correspondence claim. Generation commands, source/configuration digests, generated-file digests, and extraction diagnostics MUST be retained.

**VT-AENEAS-04.** The refinement theorem MUST connect actual generated functions to the intended component contract. Merely compiling the generated Lean module, or proving properties of a separately rewritten reference function, is insufficient. The theorem MUST account for all modeled return alternatives, failure, and the relevant representation invariants.

**VT-AENEAS-05.** Every external model, opaque function, modeled standard-library operation, panic behavior, and admitted translation boundary MUST appear in the assumption register with its concrete target. A definition implemented in Lean is not automatically a proof that the corresponding Rust library behaves that way. Generated templates with missing obligations MUST NOT count as verified implementation.

**VT-AENEAS-06.** Reports using this route MUST retain rustc/MIR extraction, Charon, Aeneas, and external-model fidelity as trusted components unless separate evidence discharges the exact boundary. The Lean kernel checks the resulting theorem, not the faithfulness of the extraction process. Published research about an algorithm is not proof that every current tool binary implements it correctly.

**VT-AENEAS-07.** A theorem asserting success or termination MUST distinguish translation-level failure or divergence from Noble domain-error values. Aeneas's ability to represent partial functions MUST NOT be reported as an automatic termination proof. [R2]

The production checker remains Rust. Lean's definitions or an executable reference evaluator may be used as test oracles; they do not become a second Noble production VM.

## 3. Optional Verus exception lane

The VT-VERUS requirements apply only when a reviewed VT-SCOPE-04 exception selects Verus. No component currently has such an exception. These retained requirement IDs do not require a Verus dependency or a parallel kernel implementation.

VT-WRAP and VT-NATIVE remain mandatory at applicable boundaries, regardless of proof tool selection.

**VT-VERUS-01.** When an exception selects Verus, it MUST verify exact executable functions against explicit specification functions and invariants. Proof and ghost state MUST NOT change Noble-visible behavior or be assumed to exist at runtime. The compiled code/configuration must match the one whose verification conditions were checked.

**VT-VERUS-02.** If a resource adapter uses Verus, its specification MUST cover owner context, resource kind, liveness, transfer, retirement, and invalid-handle rejection. Invalid handles include stale, forged, wrongly typed, and cross-context handles. Pure handle-table transitions remain on the mandatory Aeneas route under VT-SCOPE-02. The representation may use generations or another reviewed stale-reference defense; this document does not freeze an ABI.

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

### 3.2 Native boundary review and regression checks

**VT-NATIVE-01.** Every Noble-owned, non-test unsafe block or unsafe implementation MUST have a safety argument for its entry conditions and lifetime obligations. The argument cites the relevant stable Rust contract and retained wording. Undocumented assumptions require an explicit owner, review record, and scope in the trust inventory.

The inventory includes native callbacks, FFI, dependencies, and derive-generated code at the claimed boundary. A comment-presence lint is not a proof. No safe Rust wrapper or derived trait can substitute for missing semantic acceptance.

**VT-NATIVE-02.** Native representation boundaries MUST have positive and negative tests for applicable alignment, validity, bounds, lifetime, and ownership conditions. The assurance plan MUST include scoped Miri runs with pinned targets and memory-model configuration. Unsupported runs MUST remain explicit and MUST NOT satisfy a required lane.

Compile-fail tests cover forbidden construction and escaping views where Rust can reject them. Runtime tests cover malformed input and state transitions. Miri does not establish Noble-to-Wasm correspondence or cover an excluded target.

**VT-NATIVE-03.** Dependency adoption MUST record the exact version, features, macros, applicable target assumptions, and verification boundary. A byte-view dependency MUST remain outside the initial owned-data acceptance core unless extraction compatibility and its concrete boundary are established.

Zerocopy remains a candidate for a concrete decoding adapter. Its upstream Miri/Kani results do not discharge Noble obligations. Kani is not a mandatory primary proof route. Dependency adoption must preserve the Aeneas-first scope and exception policy.

## 4. Cross-boundary contracts

**VT-BRIDGE-01.** Every external model, shell adapter, or approved alternate verification lane MUST publish its boundary contract with the Lean/Aeneas core. The contract MUST specify the concrete representation relation, argument/result ordering, integer widths, error alternatives, ownership transitions, and allowed effects. Both sides MUST name the same contract revision.

For a resource adapter, the shared contract separates abstract handle obligations from concrete host-object access. A transition proof does not establish the adapter's physical release or synchronization behavior.

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

The required pin set covers Lean/Lake, proof libraries, Charon, extraction rustc, Aeneas, its Lean backend, and the relevant production Rust/Wasm profile. An approved Verus exception additionally requires its Verus, rustc, specification library, and SMT solver pins. Different lanes can require different rustc versions. Compatibility requires executed evidence.

**VT-PIN-02.** The Charon revision and Lean backend configuration MUST match the selected Aeneas revision's compatibility requirements. Aeneas's README explicitly ties these dependencies together. A local symlink or manual tool override MUST NOT bypass the package's own pin checks. [R2]

**VT-PIN-03.** The tracked configuration MUST include target triple, word size, Cargo features, dependency lock, relevant `cfg` flags, panic strategy, overflow semantics, generation options, solver options, and timeout/resource limits. Configuration changes invalidate affected evidence applicability until rerun.

[verification/toolchain-lock.template.json](verification/toolchain-lock.template.json) is a newly authored, deliberately unselected template. Its null revisions are not usable pins. No compatible set is claimed.

The workspace milestone in [ROADMAP.md](ROADMAP.md) must select immutable Nix and Octet inputs. Nix creates `flake.lock`; hand-edited lock files are not accepted. Rust tests, Clippy, and the full pinned Octet catalog run as errors across the declared workspace scope.

### 6.1 Octet architecture and evidence integration

**VT-OCTET-01.** The Rust workspace MUST activate a pinned Octet project architecture policy in addition to the full deny-all lint gate. Human-authored policy MUST use reviewed Nickel source with a validated export and freshness manifest. Policy MUST declare production roles, source/package coverage, targets, features, permitted dependencies, and external providers explicitly.

A deterministic-core profile alone does not activate architecture policy. Inventory or advisory output cannot satisfy a required architecture gate. Finding allowances and warning budgets remain prohibited. A reviewed function-address identity baseline records source identity, not permission to retain findings.

**VT-OCTET-02.** Architecture acceptance MUST cover core/shell separation, application-owned ports, adapter construction, inward type boundaries, and protected executor/witness/receipt protocols. Configured nominal-domain checks MUST cover approved constructors and unit/identity distinctions. Designated Rust lifecycle cores MUST use explicit closed state/event coverage.

Compiler-derived facts MUST bind the actual source, resolved providers, toolchain, target, features, and collection limits. Missing, stale, ambiguous, or over-limit required facts MUST remain blocking unknowns. Unsupported required crate kinds or configurations MUST remain blockers until a complementary check covers the same contract. A library-only result cannot establish coverage of production binaries or tests.

Roles do not imply `no_std`, and `no_std` does not prove purity. Octet rules about recursion, function length, and source shape remain Rust implementation policy, not Noble language laws. Narrow source exceptions retain the existing owner, reason, scope, and evidence requirements. They cannot bypass mandatory kernel refinement.

**VT-OCTET-03.** Octet lint, architecture, effect-closure, artifact-integrity, and external-tool results MUST retain separate evidence roles and phase outcomes. A failed required phase MUST block the associated admission claim. Source-token closure MUST NOT replace Noble's resolved effect rules or the Charon → Aeneas → Lean route.

Reviewed provider summaries require exact source, dependency, call-site, target, and feature bindings for their declared analysis lane. An unknown direct effect cannot disappear through a summary. A passed artifact verifier establishes stored identity/linkage consistency only.

The current Octet Charon semantic rail covers a named fixture and records analysis evidence. It does not verify Noble's kernel. Reuse of a provider, bounded executor, or receipt interface requires published immutable inputs and an executed compatibility check. This amendment does not select toolchain pins or require a Verus migration.

Noble-owned runtime receipt DTOs remain Rust-owned. Their generated contracts must match the emitted schema. Octet's artifact formats stay externally owned behind a versioned adapter. [OCTET-ADOPTION.md](OCTET-ADOPTION.md) records the inspected design revision and non-claims.

## 7. Continuous verification contract

**VT-CI-01.** CI MUST maintain separate document-validation, Lean-model, Rust-extraction/refinement, boundary-review, and compiler/runtime-test lanes. CI MUST include a Verus lane only when a reviewed exception selects it. Results MUST distinguish `passed`, `failed`, `timeout`, `unsupported`, and `not-run`. A nonzero exit, omitted obligation, or solver unknown result MUST NOT count as proof success.

**VT-CI-02.** Assurance runs MUST capture full obligation counts, tool exit status, diagnostics, source/configuration identities, trusted-boundary changes, and expected-claim changes. Grep-based checks MAY supplement but MUST NOT replace semantic and transitive dependency checks.

**VT-CI-03.** Changes to core primitives, typing rules, recipe observations, or host contracts MUST identify affected obligations and invalidate dependent assurance records. A proof against a weakened or differently interpreted statement cannot satisfy the old gate without review.

**VT-CI-04.** Differential and property-based tests SHOULD exercise reference semantics, Rust implementations, and Wasm execution on the same generated cases, including boundary integers and runtime-built programs. Such tests are regression evidence, not replacements for refinement theorems.

**VT-CI-05.** CI MUST regenerate extraction and recheck affected refinement proofs whenever production source, dependencies, macros, features, targets, or semantic contracts change. The gate MUST compare compiler-derived source coverage with the VT-SCOPE-05 inventory. A hand-maintained list of successful functions alone is insufficient. Missing functions, proof holes, unexplained opaque bodies, and unsupported required configurations MUST fail the claimed verification gate. Separate experimental lanes MUST retain their incomplete status.

### 7.1 Application-contract lane

**VT-CONTRACT-01.** CI MUST keep application-contract evidence separate from Rust implementation refinement and backend correspondence. The contract lane MUST use actual accepted Noble programs and typed claims under SPEC-V002. It MUST retain contract IR, generated propositions, rule-library revisions, and exact accepted theorem references. An Aeneas result for the exporter MUST NOT automatically mark an application claim proved.

**VT-CONTRACT-02.** Proof-required builds MUST invalidate affected application evidence when subjects, captures, logical definitions, policies, or semantic dependencies change. CI MUST exercise proof failure, unsupported claims, forged companions, stale applicability, and unrelated-artifact rejection. Pinned Lean libraries and sound finite replay rules support runtime companions without a production Lean or SMT process.

## 8. Implementation entry point

The first vertical slice SHALL implement the pure explicit acceptance checker, wrapping integer operations, and runtime-independent recipe builders in ordinary Rust; extract them with a pinned Charon/Aeneas pair; and prove one concrete checker/representation refinement against the Lean model. In parallel, the Wasm experiment SHALL exercise nonconstant `quote` and `compose` without claiming backend proof.

After that slice establishes a usable proof pattern, extend extraction and refinement across the kernel and the remaining compiler/runtime logic. Resource-table transitions follow the same Aeneas route. External effects remain behind checked adapters with explicit correspondence obligations.

M1 must establish the source inventory and extraction CI entry point. M2 must demonstrate actual extraction, one nontrivial refinement, and rejection of incomplete verification coverage. MC1 adds typed application contracts and proof rules after M2. MC2 adds the first-class Wasm demonstration after MC1 and M4. Neither requires host resource protocols. The current package contains document checks only. Noble implementation, compatible pins, proof sources, and verifier execution remain future work.
