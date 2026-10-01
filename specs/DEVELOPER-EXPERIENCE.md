<!-- Generated compatibility view. Edit .cairn/specs/developer-experience/spec.md instead. -->
# Developer experience and library direction

Document: SPEC-DX001  
Revision: 0.1.0-draft.5  
Status: Selected draft contracts; bounded DX-03/08/09 implementation/execution passed under source-bound DXM1 evidence; proof and broader profile open

## Scope and adoption boundary

This document selects scoped language and library contracts, including the later worker and Octet amendments. It does not expand the bootstrap type system or change ordinary program execution.

Elm, Gleam, and Idris inspire diagnostics and typed holes. Gleam, Rust, and Roc inspire opaque types and variants. Rust and Gleam inspire fallible composition. Unison inspires identity tooling. Koka and Eff inspire effect substitution.

Factor and Kitten inspire local names. Roc inspires capability-aware modules. QuickCheck and Elm inspire property testing. Austral and Rust inspire resource protocols. Rust and Racket inspire executable documentation.

These names record design inspiration, not audited upstream behavior, dependencies, compatibility claims, or copied implementations. Noble's existing safety and verification contracts govern every adaptation.

## 1. Stack diagnostics and editor holes

**DX-DIAG-01.** The checker MUST report the failing word or join, required stack, actual stack, and relevant effect or eligibility constraint. It MUST retain available source spans and value-origin spans. If provenance is unavailable, it MUST report that fact rather than invent a location.

Successful editor analysis should expose stack states at word boundaries. Diagnostics must distinguish stack order, stack arity, type mismatch, branch mismatch, and resource duplication. Exact wording is not standardized.

**DX-HOLE-01.**

Editor holes MUST remain incomplete syntax, not executable values or trusted witnesses. Analysis MUST report known input/output stack constraints and effect constraints, including unresolved variables. Every admission path MUST reject a candidate containing a hole. Analysis MUST NOT execute the candidate body.

The selected `Editor-Draft` profile transports incomplete syntax only as versioned `noble-editor/v1` structured nodes for signed I64 integer, Boolean, single source word, nested quotation and hole. Its JSON envelope and typed AST are editor-only; hole punctuation is NOT selected for executable `.noble` source, and a hole MUST NOT be represented as a kernel node or executable candidate. Transport MUST reject unsupported versions, missing/unknown/duplicate object fields, incorrect JSON types, invalid word encoding, trailing data and exceeded byte/node/depth/work budgets before admission. Rendering a hole-free AST MUST produce single lexically valid source tokens for ordinary parse, source preparation, independent kernel acceptance and compiled managed-Wasm execution, rather than use a second trusted backend.

Editor analysis MUST use the same resolved source namespace and operator scheme constraints as ordinary source inference. At each hole it MUST preserve the incoming stack, the constrained yet unresolved outgoing stack and the unknown possible effects, not default the hole to a pure/empty operation. The finite `1, hole, +` witness MUST report the `I64 I64` required outgoing suffix induced by the actual `+` scheme, and the minimal additional `I64` suffix conditional on retaining the known incoming `I64`, while retaining unresolved stack/effect variables. This conditional suffix MUST NOT assert that the hole preserves or does not consume its input. Direct, nested-quotation and serialized hole-bearing admission MUST reject before an accepted program, guest/host requests or protected operations. The complete editor tree `1, 2, +` MUST remain admissible to the ordinary compiled path and produce `I64(3)`.

These selected finite observations neither execute a hole nor establish universal frontend, backend, host or proof refinement; all nonselected editor syntax and application authoring tools remain open.

#### Scenario: DX-02 editor analysis and admission

- GIVEN DX-02's exact structured nodes, four variants, bounded versioned transport, a hole-free positive control and hostile malformed/collision/oversized controls
- WHEN editor analysis, direct and nested admission, serialized JSON ingress and ordinary source/kernel/Wasm admission run against their exact inputs
- THEN source-derived hole stack and unresolved effect constraints are reported without execution, every hole-bearing admission rejects before accepted candidate or guest/host requests, and the hole-free positive control executes to `I64(3)`; neither transcript nor status substitutes for source-bound observation

**DX-TYPE-01.**

Distinct declared type identities MUST NOT unify merely because their representations match. `Declared-Modules-v1` selects real guest source declarations (`opaque` and two-constructor `variant`), resolved immutable module/version/declaration identities, visibility-controlled constructors and explicit conversions. A source declaration cannot be represented only by structured checker fixtures or erased into structural `I64`/`Sum` for acceptance. Public type export does not expose private construction or representation. The module interface freezes its source version and resolved dependency identities; implementation-local identity is not a portable package hash. The Core-Bootstrap profile is unchanged.

#### Scenario: DX-03 nominal source checking

- GIVEN actual `Declared-Modules-v1` source modules declaring `UserId` and `OrderId` over `I64`, plus private and public constructors
- WHEN the source passes immutable resolution, type inference and independent kernel acceptance
- THEN different nominal identities cannot unify, matching identities can, and a foreign module cannot invoke a private constructor; static checking starts zero guest calls

**DX-TYPE-02.**

A variant match MUST cover both constructors of its resolved immutable schema. The schema owner may match private arms; an external caller MAY match only when both constructors and their payload interfaces are public/exported. Exporting the variant type alone MUST NOT expose a private arm or payload; an owner may instead export its own checked eliminator. Each admitted typed branch receives its constructor's payload, agrees on the complete ordered result stack and conservatively joins latent effects. A partial/mismatched match or foreign private-arm match MUST fail before execution. Eligibility MUST inspect every nested payload and representation of a nominal wrapper or variant, including an opaque resource-containing wrapper, and MUST reject duplication, discard, capture and serialization whenever recursive eligibility fails. Only a validated resource kind may enter the static fixture; that fixture does not assert resource-positive Wasm support.

#### Scenario: DX-03 exhaustive and resource eligibility

- GIVEN actual source declarations of a two-constructor variant with a private arm, an all-public variant and an opaque wrapper containing an explicitly validated resource kind
- WHEN each complete/incomplete match and dup/drop/quotation capture is checked by the independent kernel
- THEN the owner or an external all-public caller can complete the matching-stack match; missing or mismatched branches, a foreign private-arm match and forbidden resource operations are rejected, and no guest or protected host operation starts

**DX-TYPE-03.** Domain declarations MUST distinguish nominal identity from representation aliases. Distinct identities such as `AttemptId` and `TaskGeneration` MUST NOT unify because both contain integers. Budget and quantity contracts MUST declare their units, valid ranges, and explicit conversions. Admission budgets, execution fuel, durations, and byte counts are not interchangeable.

A zero budget can be valid data while its execution policy denies all work. The domain contract determines that distinction. Checked unit conversion does not alter core `I64` wrapping semantics.

**DX-TYPE-04.**

A declaration owner MUST enumerate every admitted construction path and the invariant each path establishes. Constructors, conversions, defaults, decoders, and package imports MUST preserve that invariant. A public field, wrapper tag, or representation alias MUST NOT bypass validation. Invalid inputs MUST remain errors rather than sentinel or default values that appear valid.

Use ordinary opaque declarations and explicit fallible library constructors. These requirements add no general refinement inference, computational dependent program type system, or runtime proof search. The separate optional `Intrinsic-Proofs-Draft` selects predicative dependent *logical* proof declarations without changing this ordinary domain-data invariant. Nominal data remains subject to recursive eligibility. It does not become a capability merely because its name includes authorization.

**DX-RESULT-01.**

The Result library MUST provide success mapping, error mapping, fallible chaining, and recovery through ordinary checked programs. Result MUST remain an ordinary two-arm variant, not an exception effect, an IO monad, or a new expression form. The selected design is four operations on `Result<A,E> = Ok(A) | Err(E)`: `map-ok` transforms `Ok(A)` by a checked `A -> B` callback and carries `Err(E)` unchanged; `map-error` transforms `Err(E)` by an `E -> F` callback and carries `Ok(A)` unchanged; `and-then` invokes an `A -> Result<B,E>` callback only on `Ok(A)`; and `or-else` invokes an `E -> Result<A,F>` callback only on `Err(E)`. These are schematic stack-interface descriptions, not accepted generic variant declaration syntax or a newly implemented module. Concrete exported names, module/version identity, source signatures, and generic schema instantiation MUST pass the existing declaration/library interface gate before acceptance; DXM1's finite monomorphic two-constructor variants alone do not establish that gate.

**DX-RESULT-02.**

Each combinator MUST declare its complete ordered stack interface and conservative latent effect bound, including the callback's possible effects even if this invocation bypasses it. It MUST invoke the callback exactly once for the selected arm and never for the other arm; `and-then` and `or-else` MUST preserve an already selected `Err` or `Ok` without unwrapping it into an implicit early return. Both branches MUST account for every owned input and every callback output without implicit duplication or discard, including on ordinary error results and abnormal outcomes. Neither unselected-callback evaluation nor speculative host requests are permitted. The resource-free first slice uses ordinary `Data` payloads; admitting a resource-bearing payload requires the separate ownership/retirement profile, an explicit complete owner transition for each arm, and real boundary evidence. No new propagation syntax, IO monad, hidden exception path, or claim of implementation follows from this design.

**DX-IDENTITY-01.** Dependency browsing and semantic diffs MUST use resolved identities and retained recipes, not mutable names or optimizer layout. Tools MUST distinguish semantic changes from build-only changes and unavailable comparisons.

**DX-CACHE-01.** Test and proof caches MUST bind results to the exact subject, relevant dependencies, claim, assumptions, and tool/schema revisions. Execution caches MUST also bind backend/build and relevant environment inputs. Consumers MUST recheck evidence compatibility and current admission policy before reuse.

A cache hit does not authorize execution or prove a claim. Recorded execution is not a fresh execution. Unknown dependencies or environment inputs prevent reuse as current evidence. Proof-cache lookup does not replace evidence validation under SPEC-V002.

Stable persisted cache keys require the canonical encoding gate. Earlier tools may use explicitly versioned, implementation-local keys without portable identity claims.

## 5. Explicit test-host substitution

**DX-HOST-01.** Test-host substitution MUST use an explicit operation-to-adapter mapping with checked interfaces. It MUST preserve declared effect accounting and authorization checks. Missing, denied, or incompatible mappings MUST fail explicitly, without fallback to a real host operation.

**DX-HOST-02.** Test reports MUST identify substituted operations, adapter versions, scripted inputs, and host requests, including denied requests. Mocked execution MUST remain distinct from real-host evidence. Unexpected requests and exhausted scripts MUST produce explicit test failures.

The deterministic core owns substitution validation and dispatch decisions. The shell invokes adapters. A fake clock or filesystem does not grant credentials or erase an effect. These contracts do not introduce resumable handlers, continuations, or a second concurrency model.

## 6. Optional local names

**DX-LOCAL-01.** Local bindings MUST be lexical and lower into existing checked stack operations. Lowering MUST preserve ordered inputs, outputs, effects, and resource ownership. Repeated use requires duplication eligibility; unused values require discard eligibility. A binding MUST NOT introduce mutable cells or dynamic name lookup.

**DX-LOCAL-02.** Quotation captures through local names MUST obey existing Capture eligibility and appear in retained recipes. Local names MUST NOT permit capture of live resources. The design MUST specify scope, shadowing, source diagnostics, and the relationship between binding syntax, lowered recipes, and program identity before acceptance.

Concrete syntax remains open. First compare a realistic stack-only program with a binding-based version. Acceptance requires equivalent observable results and effects, plus rejection of resource reuse, implicit resource discard, and resource capture. Local bindings remain outside Core-Bootstrap; they add surface convenience, not a new kernel mechanism.

## 7. Capability-aware modules

**DX-MODULE-01.**

`Declared-Modules-v1` source modules MUST explicitly declare exports and required operations and resolve source-versioned imports against an explicitly supplied immutable registry. Source/text bytes remain UTF-8, while declared identifiers are ASCII; a non-ASCII declaration name MUST reject rather than silently normalize. Scoped `@` version notation and generic type commas do not expand Core-Bootstrap's ordinary word grammar. Linking MUST validate exact ordered operation signatures, conservative effect bounds, visibility and resolved dependencies before publication. A module body contains declarations only: top-level execution, initializer or import-time host/guest invocation MUST be rejected, not merely deferred behind an unchecked shim. A source file is an explicitly supplied compiler input; optional bounded compiler-shell file reading is not a guest effect. Package registry, remote transport and portable encoding remain outside this profile.

#### Scenario: DX-08 zero-execution linking

- GIVEN a source module requiring `test.emit : Text -- ! {test.emit}` and a separately supplied typed adapter binding
- WHEN matching, missing, wrong-input, wrong-effect and initializer variants undergo actual module link admission
- THEN only the matching binding links, all others reject, no guest or host request executes, no protected operation occurs and no authority is acquired

**DX-MODULE-02.**

A module requirement or supplied binding MUST NOT be a host credential. The linked checked program MUST retain immutable module/version, operation and selected adapter identities even if a display alias is rebound. Absent, incompatible and denied bindings MUST NOT fall back to an ambient host operation. At runtime the real compiled Noble/Wasm guest request MUST reach independent host authorization; denied requests remain in declared effect/request accounting but perform no protected work. Link-time checks MUST NOT themselves authorize or invoke the host adapter. Exact module transport and unavailable operation contracts MUST fail explicitly.

#### Scenario: DX-08 link without authority

- GIVEN the declared source requirement and explicit adapter contracts, with a host policy that has not granted access
- WHEN the source module is imported and linked
- THEN matching binding admission occurs without guest or host execution; missing or incompatible bindings reject without fallback or acquired authority

#### Scenario: DX-09 retained A and independent denial

- GIVEN a checked compiled program resolved to version-A module/adapter, a later display-name rebind to version B, and an independent host denial of `test.emit`
- WHEN the earlier compiled Noble/Wasm program executes once
- THEN its one version-A request is recorded and denied, `test.emit` remains in the checked effect set, zero protected operations execute and version B receives no call

**DX-MODULE-03.** A module that exports a transition program MUST declare exact state, event, action, and program interfaces. Constructor visibility and exhaustiveness MUST follow the resolved schema identities. An adapter between schema versions MUST be explicit, typed, and subject to ordinary effect and ownership checks.

A registry can normalize programs to one interface through checked adapters and effect widening under K-PROG-09/10 and K-EFFECT-05. It cannot introduce dynamic `Any`, implicit variant joins, or unchecked calls. Package boundaries follow P-PACK-01/02. The [worker scenario](WORKER-CONFORMANCE.md) uses these contracts without new declaration syntax.

**DX-MODULE-04.** Module admission MUST derive effect requirements from resolved definitions, host contracts, and trusted instantiated program interfaces. Each invocation MUST have an applicable interface with a conservative effect bound. An unresolved operation or unavailable contract MUST NOT become an empty effect set or a name-based purity exemption.

A typed indirect call can use its trusted `Program<S,T,e>` bound without enumerating every possible runtime program value. Supported recursive calls use their checked signatures. Effect closure does not prove termination. Dependency traversal and constraint work remain bounded under K-CHECK-07.

A provider summary cannot erase a known direct effect or override the admitted contract for another identity. Rust source-token analysis can supply separate implementation evidence. It cannot replace Noble's resolved effect judgments or backend correspondence.

## 8. Property testing and shrinking

**DX-PROPERTY-01.** Property tests MUST record the subject, property, generator and shrinker revisions, seed, bounds, environment, and outcomes. Well-typed generation MUST pass each candidate through independent acceptance. Malformed-candidate fuzzing MUST remain a separate lane. Rejection by acceptance MUST NOT silently count as successful well-typed coverage.

**DX-PROPERTY-02.**

Shrinking MUST preserve the property's input domain and reproduce the same failure predicate before accepting a smaller counterexample. Shrinking MUST be bounded and retain the original failure if reduction fails. Reports MUST distinguish passed trials, counterexamples, discarded inputs, exhaustion, unsupported cases, and harness errors.

The first properties cover compatible program composition, retained recipe structure, and result/effect agreement with the reference model for bounded bootstrap cases. Observable disagreement requires a counterexample, not merely an exhausted budget. Seeds alone are insufficient replay records. Random testing does not establish a universal theorem, and a shared implementation bug can invalidate a differential oracle.

An optional later MC2 pure-fragment law experiment MAY add a finite semantic oracle independently implemented from Noble's checker, prover and selected Wasm lowerer. Its declared finite input vocabulary and bounds MUST be recorded, including exact normal-return predicate and preconditions, independent expected values/recipes and the empty host-effect trace, hostile false-law and false-premise controls, a reproducible seed or saved minimized counterexample, and allowed source/recipe-preservation transformations that actually preserve resolved identity. Tests compare this model with actual accepted resource-free programs and separately with externally checked proof/admission outcomes; model agreement MUST NOT become an accepted theorem or release authorization. Preserve the immutable owner law and distinct release gate of VC-OWNER-01. This design takes only finite-model and metamorphic testing methods from pinned qcue; it does not import CUE's open live refinement, higher-rank or impredicative types, proof-time native execution, a second kernel, or its Go dependency. qcue itself reports that raw source export/reimport leaves one separately attached live-interface conformance proof pending. Existing DX-10/M4 property execution remains its historical bounded scope, not evidence for this optional experiment.

**DX-PROTOCOL-01.** A typed protocol transition MUST consume the prior owner and account for ownership on every result branch. The protocol design MUST specify states, legal operations, failure outcomes, cleanup, and correspondence to runtime handle states. Source-level state labels MUST NOT bypass runtime handle validation or host authorization.

**DX-PROTOCOL-02.** A failed or interrupted host transition MUST NOT imply that the prior state remains usable. The adapter MUST report the owner disposition required by its explicit boundary contract. Ambiguous external outcomes MUST remain explicit and MUST NOT cause an automatic retry without a suitable host contract.

This is a later extension of [resource adapters](RESOURCE-ADAPTERS.md) and [program contracts](PROGRAM-CONTRACTS.md), not a competing ownership system. Static typestate does not establish an external service's current state. Behavioral protocol proofs additionally require the corresponding host model; ordinary resource programs do not acquire a mandatory proof requirement. Protocol declarations, error-state representation, and runtime correspondence remain open gates.

**DX-PROTOCOL-03.** A declared finite lifecycle protocol MUST account explicitly for every state/event constructor pair, including invalid transitions. Each invalid pair MUST produce its declared rejection and ownership disposition. Wildcards, default success, and guards without an explicit remaining case MUST NOT establish complete transition coverage.

Explicit groups of named alternatives can share a transition. Coverage binds the exact resolved state/event schema identities. A schema change requires renewed coverage before admission. Data-dependent conditions still need explicit outcomes and applicable invariant evidence.

This requirement applies to designated lifecycle cores, not every ordinary program or open external schema. A decoder rejects unsupported external variants before typed transition admission. Unsupported transition shapes remain unsupported, not exhaustiveness evidence. No special Noble state-machine syntax is selected.

## 10. Executable documentation

**DX-DOC-01.** Executable examples MUST bind source, expected observations, specification revision, compiler/build context, and required host configuration. The harness MUST use the actual compiler and applicable backend. Static-rejection examples MUST check the expected diagnostic category and zero candidate-body host requests.

**DX-DOC-02.** Published examples MUST distinguish illustrative or unexecuted text from executed evidence. Reports MUST retain failures, unsupported examples, and timeouts rather than omitting them from coverage. Documentation checks MUST NOT substitute rendered text matching for compiler execution or represent passing examples as proofs.

Expectations may cover stack shapes, values, effects, or diagnostics. A static check does not establish runtime behavior. Effectful examples require explicit test-host mappings and budgets; documentation processing must not run them with ambient credentials. Example extraction syntax remains open. The actual compiler executed the separately recorded M4 DX-12 selected examples; illustrative or unsupported examples remain unexecuted.

## First AI-authoring reference application

[CALCULATOR.md](CALCULATOR.md) selects the exact programmable calculator and its independent acceptance criteria. CALC-AI-01 through CALC-AI-04 add bounded change tasks, comparable authoring routes, structured compiler feedback, and rejection of stale edits.

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

[Developer-experience scenarios](conformance/developer-experience-cases.json) and [language-workflow scenarios](conformance/language-workflow-cases.json) retain their exact expected inputs and case-specific evidence. DX-03, DX-08 and DX-09 pass only the selected [source-bound DXM1 receipt](../verification/declared-modules-v1/acceptance.json): nine static rows, twelve off/on zero-execution linker rows and two real compiled-Wasm denial rows. The accepted M4 DX-10/12 workflows retain their own evidence. Other designs and all new nominal/module refinement proofs remain open. New semantic functions inherit the Aeneas-first inventory and proof requirements.

No slice closes its gate through document validation alone. General computational dependent types for ordinary runtime programs, general effect handlers, macros, and replacement concurrency semantics remain unselected. The separate optional `Intrinsic-Proofs-Draft` selects proof-level dependent Pi and Eq in nonexecuting Noble module declarations; ordinary expressions remain the existing three forms.

**DX-PURE-PAR-01.**

A future independent fork/join study MAY consider only finite checked subprograms with an explicit admissibility decision before execution: both branches must be resource-free, host-effect-free, independent under their actual ordered stack/capture interfaces, and have no live task, authority, borrow or native-pin obligation. All branch work and retained results MUST be bounded and precharged against an explicit quota; unknown cost or insufficient reservation rejects before either branch starts. Joining MUST be deterministic in declared left-then-right result order. An ordinary `Result` error is normal data and returns in that branch's ordered slot; for abnormal trap or metering exhaustion, wait for both bounded branches to settle, report the left abnormal outcome if present, otherwise the right, never publish a partial successful join, and retain all work charges until settlement. A precharge refusal precedes both branches. Any admissible execution MUST preserve the sequential reference's successful values, ordered recipe/identity observations and empty host-effect trace, with distinct recorded quota/trap outcomes rather than an unconditional equivalence claim. The study starts only after the language and schema/interface gates needed to express both branches; it adds no general guest spawn, channels, GPU backend, effectful parallel calls or M6 async executor behavior. It MUST NOT treat copyable channel endpoints, unsafe Array aliasing, `IO.within` without cancellation, fail-stop OOM, unreviewed foreign C/JS or process termination as a substitute for Noble's owner/retirement and bounded-admission contracts.

#### Scenario: ADAPT-18 deferred pure join admission

- GIVEN finite checked independent resource-free pure branch pairs, effectful and owner-bearing controls, and precharged bounded work
- WHEN an optional study compares sequential reference and permitted fork/join schedules, errors, traps, quotas, results and retained recipes
- THEN only admissible pairs may run in parallel, their ordered successful observations agree, and rejected or exhausted pairs acquire no task or authority

**DX-PROOF-01.**

`Intrinsic-Proofs-Draft` tooling MUST retain source spans and exact resolved identities across Noble `.noble` module proof/contract parse, binder scope, Type0 code eligibility, proposition elaboration, equality-motive typing, reviewed-rule premises, MC1 claim export, restricted Lean translation, independent proof checking and optional admission. A diagnostic MUST distinguish lexical/mode or delimiter failure, unsupported logical form/type, unsatisfied goal, unproved bridge/join, changed subject/law, disallowed assumption, exhausted budget, checker/translator internal failure and unavailable independent recheck. It MUST state that an accepted source proof is not a proof of termination, runtime Wasm, backend or host authorization. Editor holes may expose constraints but MUST NOT become proof terms, axioms or accepted evidence; diagnostics and explanation MUST NOT execute a guest body, host operation, proof macro or tactic. Proof erasure MUST preserve inspectable separate evidence metadata without creating executable proof values.

For `contract 2` / `proof 2 ... for`, diagnostics MUST distinguish revision mismatch/unsupported colon form, changed imported version or lexical occurrence, wrong owner/span/ordinal, reused or missing fresh specialization slot, incomplete typing/instantiation, orphan or unreachable graph row, failed typed pre/post or exact named claim, and unavailable host source authentication. A checked synthetic Lean fixture MUST be labeled separately from a reusable reviewed rule instantiated with authenticated source, admitted Noble source, owner law and runtime artifact; accepted revision-1 reports and receipts remain unchanged.
