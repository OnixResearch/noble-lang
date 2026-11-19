<!-- Generated compatibility view. Edit .cairn/specs/developer-experience/spec.md instead. -->
# Developer experience and library direction

Document: SPEC-DX001  
Revision: 0.1.0-draft.5  
Status: Selected draft contracts; bounded DX-03/08/09 implementation/execution passed under source-bound DXM1 evidence; proof and broader profile open

## Scope and adoption boundary

This document selects scoped language and library contracts, including the later worker and Octet amendments. It does not expand the bootstrap type system or change ordinary program execution.

**DX-LIVE-01.** `noble live repl [--source FILE] [--module FILE ...] [--bindings HOST_FILE] [--engine v8|interpreter]` and `noble live watch SOURCE [--module FILE ...] [--bindings HOST_FILE] [--engine v8|interpreter]` are separately opted-in design entry points, not implemented commands. The first profile uses resource-free Core-Bootstrap expressions/definitions and explicit Declared-Modules-v1 declarations/bindings. REPL complete submissions execute only after independent acceptance and in-process Wasm bytecode admission; `:reload FILE` and watch-on-save replace the selected namespace snapshot after a complete atomic transaction without executing module initializers. A `:generation` inspection reports the active source/import generation, engine choice and stable checked stack types; every submission/reload reports accepted/rejected stage and bounded source diagnostics, including failing word, import, signature or limit. `watch` observes only explicitly selected source/module files with scoped host read authority, not arbitrary discovered dependencies; a guest cannot directly request file reload, compiler access, host permissions or ambient import resolution. The sole selected exception is the narrowly host-granted, queued `self.propose` of K-LIVE-02/H-LIVE-04: its guest callback neither compiles nor publishes and a later host admission decision confers no guest file or compiler authority. No flag on ordinary `noble run` or `noble session` silently selects live semantics. The `--engine interpreter` request MUST fail closed until a pinned compatible actual Wasm interpreter is separately implemented and accepted; `v8` runs Wasm bytecode but MUST NOT be labeled strictly interpreted.
Elm, Gleam, and Idris inspire diagnostics and typed holes. Gleam, Rust, and Roc inspire opaque types and variants. Rust and Gleam inspire fallible composition. Unison inspires identity tooling. Koka and Eff inspire effect substitution.

Factor and Kitten inspire local names. Roc inspires capability-aware modules. QuickCheck and Elm inspire property testing. Austral and Rust inspire resource protocols. Rust and Racket inspire executable documentation.

These names record design inspiration, not audited upstream behavior, dependencies, compatibility claims, or copied implementations. Noble's existing safety and verification contracts govern every adaptation.

## 1. Stack diagnostics and editor holes

**DX-DIAG-01.** The checker MUST report the failing word or join, required stack, actual stack, and relevant effect or eligibility constraint. It MUST retain available source spans and value-origin spans. If provenance is unavailable, it MUST report that fact rather than invent a location.

Successful editor analysis should expose stack states at word boundaries. Diagnostics must distinguish stack order, stack arity, type mismatch, branch mismatch, and resource duplication. Exact wording is not standardized.

For invalid UTF-8 source bytes, the parse diagnostic span identifies the first malformed byte sequence, not the entire submission: from the valid prefix length through the malformed sequence, or through the source end for an incomplete trailing sequence. These are byte offsets in the submitted source; rejected input leaves the prior stack and declared namespace unchanged.

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

**DX-LSLOT-01.**

Reflecting a checked generic live caller MUST expose its inert typed `slot.invoke` recipe/interface/effect, not resolve a target, expose a session slot ID/credential, execute a dispatch or claim selected-target behavior. Runtime traces MUST separately record the pinned root registry epoch, slot incarnation/generation, exact selected ProgramValueId/DefinitionId, captures and admitted artifact, nested dispatch selections and actual authorized/denied effects. Portable replay MUST require an explicitly frozen, independently admitted registry map and matching context/policy inputs; a bare LiveRef or slot ID MUST NOT identify portable behavior. Diagnostics MUST distinguish interface, evidence, authorization, stale CAS, unsupported profile and retention-budget refusals without treating the global registry epoch as `Live-Wasm-Draft` namespace/source generation.

Replay admission MUST reject a frozen map whose global epoch, slot incarnation/generation, exact selected artifact or captures differ from the recorded invocation, and MUST reject a changed semantic context or any missing, substituted, reordered or differently accounted host request/response. All selected versions and artifact correspondence MUST be independently admitted in the frozen context. An eligible deterministic replay is not a behavioral proof and MUST NOT replay protected real-world effects without independent current host authorization.

#### Scenario: LSLOT-08 inert reflection and frozen replay

- GIVEN pinned old roots and saved code, exhausted quota and replay variants in LSLOT-08
- WHEN publication, generic reflection and replay are attempted
- THEN publication refuses while pins remain, succeeds after last-pin retirement and fresh authorization, reflection stays inert, and hostile replay bindings refuse

**NGRAPH-CLI-01.**

`Numeric-Graph-v1` MUST expose exactly the `noble ngraph` subcommands `check`, `extract`, `eval`, `train`, `verify` and `specialize`.

**Effects.**

- Each subcommand MUST read only the files named by its arguments, each under a stated byte bound checked before parsing, and MUST write only its explicitly named output file.
- It MUST NOT spawn a process, execute Wasm or open a network connection.
- The `noble-ngraph` library MUST NOT execute any filesystem, process, environment, clock, thread or output effect, and MUST NOT depend on a third-party package.
- Identity digests MUST be computed through a hashing scheme that the composition root supplies.

**Reports.**

- Every outcome other than a usage error MUST be reported as a JSON line with the schema `noble-ngraph-report/v1`. A usage error MUST print its usage on standard error and nothing on standard output.
- A refusal MUST carry one stable code and a stable site. The codes are the D6 codes of `NumericGraph-Design`, `E-FORMAT`, `E-SNAPSHOT-INTEGRITY`, `E-SNAPSHOT-ID` and `E-SNAPSHOT-REPLAY`. Run-time observations that have no site (host-fuel exhaustion and cancellation) and host conditions MAY report a null site.
- A refusal that belongs to admission, snapshot binding or input validation MUST occur before any graph node is evaluated.

**Exit status.** The exit status MUST follow the outcome class:

- 0 for `normal`;
- 1 for `trap` (a guard failure or a non-finite update);
- 2 for `reject` (malformed or inconsistent inputs, snapshot binding refusals, usage errors, host I/O failures, exceeded input bounds and an existing output path);
- 4 for `unsupported` (effect, resource, dynamic, control, aggregate, reflection, Program-output, recursion and default-refused derivative classes);
- 5 for `exhausted` (every `E-BUDGET-<kind>`).

For `eval`, the exit status MUST be that of the first non-normal vector in file order.

**Budgets.** Every D5 budget MUST be settable by an explicit flag whose default is the `/0` value. A flag MAY only lower a budget; a raised value MUST be refused with `E-PROFILE`.

**Output.**

- For identical arguments and input bytes, standard output MUST be byte-identical across runs.
- An `F32` value MUST be rendered as its exact binary32 encoding, except that every NaN encoding MUST be rendered as the single NaN class.
- The compiled evaluator MUST be the default, and the reference interpreter MUST be selectable. Both MUST produce identical output except for the evaluator-specific point of host-fuel exhaustion.

#### Scenario: NGRAPH-CASE-03 graph admission report

- GIVEN the `Numeric-Graph-v1` profile and the graphs specified by `NGRAPH-CASE-03`
- WHEN `noble ngraph check` admits each graph file
- THEN the report MUST match every recorded `expected` field, including the recorded `GraphLogicalId` and exit 0

#### Scenario: NGRAPH-CASE-04 check refusal classes

- GIVEN every malformed, inexact, unchecked-fact, budget, oversized, missing and usage variant specified by `NGRAPH-CASE-04`
- WHEN `noble ngraph check` runs each variant
- THEN each MUST report its recorded code and exit status with zero evaluated nodes

#### Scenario: NGRAPH-CASE-07 bitwise evaluation in both evaluators

- GIVEN the graphs and vectors specified by `NGRAPH-CASE-07`
- WHEN `noble ngraph eval` runs them with the default evaluator and with `--reference`
- THEN every output MUST match its recorded encoding and both runs MUST produce byte-identical standard output

#### Scenario: NGRAPH-CASE-08 evaluation outcome classes

- GIVEN the guard, input-fact, interface, missing-snapshot, fuel and mixed-file inputs specified by `NGRAPH-CASE-08`
- WHEN `noble ngraph eval` runs each input
- THEN each vector line, code and process exit status MUST match the recorded `expected` fields

#### Scenario: NGRAPH-CASE-02 effect-free library boundary

- GIVEN the package checks specified by `NGRAPH-CASE-02`
- WHEN the `noble-ngraph` crate attributes, dependencies and collected capability facts are inspected
- THEN the library MUST be `no_std`, depend only on `noble-kernel` and `noble-contracts`, use no forbidden capability and take identity hashing as an injected scheme

#### Scenario: NGRAPH-CASE-15 claims, determinism and documented exits

- GIVEN the artifacts and repeated runs specified by `NGRAPH-CASE-15`
- WHEN the user documentation, help text and reports are reviewed and every command is run twice
- THEN no performance, GPU, backend-selection or proof claim MAY appear, the exit-code table MUST be documented and repeated runs MUST be byte-identical

**NGRAPH-PE-01.**

`Numeric-Graph-v1` MUST expose bounded static-input specialization of a **checked** graph
through its graph library entry point and the `noble ngraph specialize` subcommand.
The same subcommand's explicit `--source` mode MUST select the separate checked-Program
entry point specified by `NGRAPH-PE-08`. No source syntax, kernel word, type or effect MAY
change, and a specialization request MUST NOT modify, re-publish or reinterpret the
original graph or Program.

**Admitted subject slice.** The entry point MUST accept a checked `ngraph-i64/0` graph whose
interface is flat scalar slots, with no param slot, no interface shape tree, no fact record
and no `Guard` or `F32Index` node. A subject outside that slice MUST be refused with
`E-PROFILE` (profile, param, fact, guard), `E-AGGREGATE` (shape tree) or `E-INTERFACE`
(static-map arity), before any residual node is created.

**Effects.** The specializer MUST perform zero host requests, zero filesystem, process,
environment, clock, thread or output effects, and zero graph-node evaluations. It MUST be
reachable only through the pure library and the existing `noble ngraph` composition root.

#### Scenario: NGRAPH-PE-CASE-01 admitted slice

- GIVEN flat-interface `ngraph-i64/0` graphs extracted from ordinary Core roots and the
  authored flat-interface control graphs, each with a static map of input-leaf arity
- WHEN the entry point specializes each graph
- THEN every request MUST produce exactly one independently checked residual graph, and the
  report MUST name the accounting revision, both graph identities, the accepted static map
  and the node counts

#### Scenario: NGRAPH-PE-CASE-02 refused slice

- GIVEN a non-`ngraph-i64/0` graph, a graph with a param slot, a graph with an interface
  shape tree, a graph with a fact record, a graph with a guard node, and a static map of the
  wrong arity
- WHEN the entry point runs on each
- THEN each MUST report its recorded code with zero residual nodes created and no output

**NGRAPH-PE-02.**

The static map MUST be one optional `I64` per ordered, flattened input leaf (the graph's
`input` slots in slot order), and the residual MUST renumber the remaining inputs by the
number of dynamic input slots strictly below each original slot. The library MUST also expose
the merge function that reconstructs a full original input vector from the static map and a
vector of dynamic inputs, so that a caller can compare the two sides.

**Diagnostics.** A map whose length differs from the input-leaf count MUST be refused with
`E-INTERFACE` and MUST NOT be partially applied.

#### Scenario: NGRAPH-PE-CASE-03 non-prefix static slot

- GIVEN a graph whose only static input leaf is not below every dynamic leaf
- WHEN it is specialized and the merge function reconstructs full input vectors
- THEN the residual MUST index its inputs by the dynamic rank, and treating the static leaf
  as a prefix MUST be reported as a disagreement by the negative control

#### Scenario: NGRAPH-PE-CASE-04 arity

- GIVEN a static map one leaf too short and one leaf too long
- WHEN each is presented
- THEN both MUST be refused with `E-INTERFACE` and no residual

**NGRAPH-PE-03.**

Specialization MUST substitute a static input leaf with an `I64` constant and MUST fold a
node to a constant only when every one of its operands is statically known, and only for
wrapping `I64` addition, subtraction, multiplication and `I64` equality. It MUST NOT
reassociate, commute, apply identity or absorption rewrites, eliminate common subexpressions,
propagate a `Bool` result into an `I64` operand, or reorder nodes. Every node that is not
folded MUST remain residual with its original operation and operand positions.

**Equality extension.** `specialize_sound` alone does not cover `I64Eq`;
`SpecializeResidual.foldEqualities_sound` proves its exact equality fold in the
extended model. Rust correspondence is still an open obligation.

#### Scenario: NGRAPH-PE-CASE-05 folding whitelist

- GIVEN a graph with fully static add, subtract, multiply and equality nodes, mixed nodes and
  a node whose operands are statically known only through a static input
- WHEN it is specialized
- THEN exactly the fully static arithmetic and equality nodes MUST become constants, and no
  other node MUST change operation, order or operand position

#### Scenario: NGRAPH-PE-CASE-06 wraparound

- GIVEN the model's `2^64 - 1 + 1` boundary and a `MIN × -1` multiply
- WHEN the specializer folds them
- THEN the residual constant MUST be the same wrapping value the reference evaluator
  computes for the original graph

**NGRAPH-PE-04.**

A residual MUST be a new graph with its own independently recomputed identity and MUST NOT be
presented as the original. The recorded claim MUST be, for every vector of dynamic inputs,
that the residual's observation and the original's observation under the merged input vector
are equivalent under the profile's observation relation. The implementation MUST NOT assert
that relation as Rust-verified; differential tests are finite evidence, not correspondence.
The separately selected checked-Program operation has the additional `NGRAPH-PE-08` contract.

**Pruning.** A node that reaches no output MAY be removed and the residual MAY be renumbered
densely, but only because the admitted slice contains no failing node; the derivation record
MUST report how many nodes were folded and pruned.

#### Scenario: NGRAPH-PE-CASE-07 observation agreement

- GIVEN the fixture graphs, seeded Core programs, static maps and input vectors of the
  differential protocol
- WHEN the original is evaluated under the merged vectors and the residual, and the graph
  re-admitted from the residual's source, are evaluated under the dynamic vectors by both
  evaluators
- THEN every pair MUST agree under the observation relation, and the recorded counts MUST
  name the vectors and graphs compared

#### Scenario: NGRAPH-PE-CASE-08 distinct identity

- GIVEN one original graph and any static map
- WHEN the residual is produced
- THEN its subject identity and logical graph identity MUST differ from the original's, its
  closure MUST be the original's, and the derivation record MUST name both identities

**NGRAPH-PE-05.**

Specialization MUST be bounded under the named accounting revision `ngraph-pe/0`, with two
budgets: the number of residual nodes created before pruning and the number of residual
variants per request, each defaulting to the `/0` value and each lowering-only through the
existing policy rule. The residual MUST additionally be charged the ordinary checking budgets
when it is re-checked. The profile MUST NOT claim that the original and the residual consume
equal budget, fuel or work: the two sides have different interfaces and visit different
nodes, and the measurement MUST report both sides separately.

#### Scenario: NGRAPH-PE-CASE-09 budget ladder

- GIVEN a fixed graph and static map
- WHEN the residual-node cap is one below the created-node count, exactly equal to it, and
  one above it, and the variant cap is zero, exactly one, and one above the `/0` value
- THEN the first MUST be refused with `E-BUDGET-RESIDUAL-NODES`, the second and third MUST be
  accepted, the variant cap of zero MUST be refused with `E-BUDGET-VARIANTS`, its exact `/0`
  value MUST be accepted, and every raised cap MUST be refused with `E-PROFILE`

#### Scenario: NGRAPH-PE-CASE-10 separate accounting

- GIVEN one measurement record of an original and its residual
- WHEN the record is read
- THEN it MUST carry the two node counts, the folded and pruned counts, per-side evaluator
  node visits, retained bytes, cold first-evaluation time and warmed batch time separately,
  and it MUST NOT carry a normalized or equal-fuel figure

**NGRAPH-PE-06.**

The residual MUST be emitted as ordinary Noble source and MUST be independently re-accepted
by the ordinary frontend and kernel before it is published. Emission MUST produce one
`let`-binding per residual node in canonical order, bind the dynamic inputs first, read
outputs in output order, and render `I64` constants as exact decimal text and `Bool`
constants as `true` or `false`. A re-preparation or re-admission refusal MUST be reported as
that refusal, and then no residual file, graph file or source file may be created.

The specialization path MUST perform zero host requests: the library MUST execute no host
effect, and the command MUST spawn no process, execute no Wasm and open no network connection.
Every `noble ngraph` report MUST expose `guest_requests: 0`, including an extraction refusal,
so the existing host-request accounting makes the no-request result observable.
Effectful, resource, recursive, aggregate and control subjects MUST be refused by the
profile's extraction stage at or before admission, with no graph, no residual and no published
output. Such a subject MUST NOT reach the graph specialization entry point; `--source`
MUST independently check and refuse it before Program residualization.

#### Scenario: NGRAPH-PE-CASE-11 residual source acceptance

- GIVEN a specialized residual and its emitted source
- WHEN the source is prepared with the ordinary declared-text frontend under the ordinary
  limits and its submission is re-admitted and re-extracted
- THEN the re-admitted graph MUST be checked and MUST agree with the residual under the
  observation relation on the protocol's vectors

#### Scenario: NGRAPH-PE-CASE-12 zero host requests

- GIVEN an effectful subject, a resource-interface subject and a recursive subject
- WHEN the profile's extraction stage runs on each
- THEN extraction MUST refuse each with `E-EFFECT`, `E-RESOURCE` or `E-RECURSION` before any
  graph exists, the profile MUST create no graph file, residual or output, and specialization
  MUST be unreachable because its only entry takes an admitted graph; any CLI report MUST
  record `guest_requests: 0`

**NGRAPH-PE-07.**

Every residual request MUST return a derivation record naming the accounting revision, the
original graph identity, the residual graph identity, the accepted static map, the residual
interface types, the node, folded, pruned and residual counts, and the variant count; the
command MUST additionally name the BLAKE3 digest of the residual source (`source_blake3`) and
the re-admitted graph identity.
A consumer MUST be able to recompute every identity in the record from the files it reads,
and MUST NOT rely on the record's self-description as evidence of the relation between the
two graphs.

#### Scenario: NGRAPH-PE-CASE-13 derivation record

- GIVEN one specialized graph and its published residual source and graph file
- WHEN the record is compared with freshly recomputed identities
- THEN every named identity MUST match its recomputation, and a record whose static map,
  accounting revision or original identity is changed MUST NOT reproduce the residual
  identity

#### Scenario: NGRAPH-PE-CASE-14 cache-key inputs

- GIVEN two requests that differ only in the static map, and two that differ only in the
  original graph
- WHEN their derivation records are compared
- THEN both pairs MUST differ in the residual identity and in the binding fields, so that a
  later cache cannot admit one for the other

**NGRAPH-PE-08.**

The explicit `noble ngraph specialize --source` path MUST accept an exact checked Core
Program and its complete source-resolved dependency closure before specializing the active
suffix of I64/Bool leaves and bounded nested Pair/Unit shapes. It MUST preserve an ordered,
resource-free stack prefix of `N` opaque values without reading, moving or replacing any
value in that prefix. The original input and output prefix types MUST be identical; a tail
that an operation touches MUST refuse with `E-INTERFACE`. A static binding in the prefix
MUST refuse, not silently bind a tail value. The active map MUST count scalar leaves
bottom-first by stack item and left-to-right inside each pair; Unit contributes no map
position and Bool leaves MUST remain dynamic. The active suffix retains the graph's pure
operation whitelist and I64-only static values; directly authored graph-mode shape trees
remain outside the admitted graph slice.

**Residual Program.** The new source MUST be independently prepared as an ordinary
Declared-Text-v1 expression under the full `tail ++ dynamic_scalar_leaves` stack and
independently kernel-accepted. Its output MUST have the original Program's full ordered
type interface, including the preserved prefix and reconstructed nested Pair/Unit outputs.
Its reached dependency closure MUST be recomputed exactly
from the emitted submission and MUST be empty for the let-only emitter. Its new subject
identity MUST bind the full original subject/closure, the static map and tail count and the
independently checked residual source subject. The original's exact closure MUST be
reported separately from the residual's, not copied onto the residual as if its calls
still existed. A retained original identity pin that no longer matches MUST refuse
before publication. A missing dependency closure MUST refuse with `E-CLOSURE`.

**Observation and selection.** On successful requests the resource-free typed-stack
observation to compare is `run(residual, tail ++ dynamic_scalar_leaves)` versus
`run(original, tail ++ reconstruct_active(static, dynamic_scalar_leaves))`, where
reconstruction preserves the original checked stack-item Pair/Unit shapes.
Comparison is without equality of session-local
handles, recipe/identity reflection, timing or low-fuel results. The specialization
command MUST execute no guest code. Ordinary Core `compile` and `session` MUST remain
unspecialized and selectable; residual source MUST run through the ordinary
Declared-Text-v1 path. The graph model theorem alone does not prove Rust/Program
or backend correspondence, which remain explicit obligations.

#### Scenario: NGRAPH-PE-CASE-15 full stack tail

- GIVEN checked pure Programs over Text/Bool/Unit/pair stack tails and a bounded
  active `Pair<I64,Pair<Unit,I64>> I64` suffix, with no, all, mixed and
  non-prefix scalar-leaf static maps
- WHEN the source mode specializes them and both original Core and residual
  Declared-Text Programs execute on reconstructed inputs
- THEN their ordered type/value stacks MUST agree, the tail positions MUST
  remain unchanged, and both ordinary paths MUST remain selectable

#### Scenario: NGRAPH-PE-CASE-16 closure and fresh identity

- GIVEN a checked root with reached named definitions, including a saved old
  definition after a same-name rebinding, and an externally retained subject id
- WHEN source specialization and source re-admission succeed
- THEN the report MUST retain the exact original closure, record the empty
  residual closure and fresh identity, and a changed definition identity or
  stale retained subject id MUST refuse before publication

#### Scenario: NGRAPH-PE-CASE-17 unsupported tail and effect

- GIVEN an operation touching an opaque tail, a static binding within the
  tail, an oversized tail, or an effectful checked root
- WHEN each is presented to source specialization
- THEN it MUST report `E-INTERFACE` or `E-EFFECT` as applicable, report zero
  guest requests and publish neither a residual source nor graph

#### Scenario: NGRAPH-PE-CASE-18 exact closure refusal

- GIVEN a checked root that invokes a named definition but a request whose
  claimed closure omits its identity
- WHEN source specialization checks the original
- THEN it MUST refuse with `E-CLOSURE` before extraction or publication

**NGRAPH-FORMAT-01.**

Graph, vector and training-data files MUST each begin with a schema identifier line: `ngraph-ir/0 <sub-profile>` for graphs, `noble-ngraph-vectors/0` for evaluation vectors and `noble-ngraph-data/0` for training data. A missing or unknown identifier MUST be refused before any further parsing: a graph file without the `ngraph-ir/0` schema line with `E-GRAPH-MALFORMED`, a graph file naming an unknown sub-profile with `E-PROFILE`, and any other file with `E-FORMAT`.

**Graph files.**

- A graph file MUST record the sub-profile, the ordered slots, the nodes in canonical order, the facts with their provenance, the outputs, the input and output shapes, and the root subject kind, identity and closure. `GraphLogicalId` MUST then be recomputed from the parsed records alone.
- Comments, whitespace, file names and paths MUST NOT be identity-bearing. Producer origins are not identity-bearing and MAY be omitted.
- Printing MUST be canonical and deterministic, and every encoding MUST be symmetric between printing and parsing, including `evidence` provenance. Parsing a printed graph MUST reproduce the same graph apart from origins, with the same `GraphLogicalId`, fact records and checker outcome.
- Parsing MUST only build producer data; admission MUST still be decided by the independent graph checker. `F32` constants MUST obey the exact-literal rules of NGRAPH-F32-01, or the explicitly selected v2 rule below.
- An interface shape tree MUST nest at most 127 pairs, the deepest root interface item that source extraction can produce; a deeper tree MUST be refused with `E-GRAPH-MALFORMED` before it is walked.

**Numbers in every file.** Integers MUST be written as ASCII digits, with a leading `-` only on signed values, and bit patterns as `0x` plus hex digits; a leading `+` MUST be refused like any other malformed token. Deciding whether a decimal is exact MUST take time linear in its length.

**Vector and data files.**

- Values MUST be typed by a declared type line. It MUST equal the graph's ordered input slot types and, for data, the loss graph's target slot types; otherwise the file MUST be refused with `E-INTERFACE` before evaluation.
- An `F32` value MUST be given either as its exact binary32 encoding, which MAY be any encoding including NaN and infinities, or as an exactly representable finite decimal. An inexact unmarked decimal MUST be refused with `E-LITERAL-INEXACT`.
- Graph, vector and data files MAY select `noble-graph-literal-modes/v2` with the single line `literal-modes noble-graph-literal-modes/v2` immediately after the schema line. Without that declaration, existing exact-only behavior and diagnostics MUST remain unchanged. In v2, `rne:<decimal>` MUST parse the existing finite decimal grammar as an exact base-ten rational, round that rational exactly once to binary32 using round-to-nearest, ties-to-even, and store the resulting bits. The marker MUST NOT authorize implicit rounding of any unmarked decimal. If rounding produces infinity, the literal MUST be refused with `E-LITERAL-OVERFLOW`; an RNE decimal longer than 4096 bytes MUST be refused with `E-BUDGET-LITERAL-BYTES`; an `rne:` token without the v2 declaration MUST be refused with `E-PROFILE`. NaN and infinity words remain refused; their encodings MAY be supplied with exact bits. Canonical printing MUST emit exact bits rather than the original decimal spelling.
- The scalar boundary API MUST expose only explicit conversions with these signatures and results: `i64_to_f32_exact(i64) -> Result<u32, ConversionError>` refuses discarded integer bits with `Inexact`; `i64_to_f32_rne(i64) -> u32` rounds once to nearest, ties-to-even; `f32_to_i64_exact(u32) -> Result<i64, ConversionError>` requires a finite integral value in `[-2^63, 2^63)`; and `f32_to_i64_trunc(u32) -> Result<i64, ConversionError>` truncates toward zero before checking that range. Both F32-to-I64 functions MUST return `NotFinite` for NaN or infinity and `Overflow` when the resulting integer is outside the range; the exact conversion MUST return `Inexact` for a finite fractional value. Both signed zero encodings MUST convert to integer zero. These operations MUST NOT use saturating or wrapping casts and MUST NOT introduce implicit coercions.
- Parameters MUST NOT be supplied through vector or data files.
- Training-data order MUST be preserved, because the training-data digest binds it.
#### Scenario: NGRAPH-CASE-06 identity-preserving graph round trip

- GIVEN every authored, workload, random and extracted graph and text variant specified by `NGRAPH-CASE-06`
- WHEN each graph is printed, parsed and checked again
- THEN the parsed graph MUST equal the original apart from origins, with the same `GraphLogicalId`, fact-binding digest and checker outcome

#### Scenario: NGRAPH-CASE-09 vector and data value encodings

- GIVEN every schema, value and row variant specified by `NGRAPH-CASE-09`
- WHEN the vector and data parsers read each file
- THEN each MUST be accepted as the recorded encoding or refused with the recorded code

**DX-MODULE-05.**

The `Declared-Modules-v1` compiler/session MUST bound its *aggregate persistent source retention* separately from each input's K-CHECK-07 preparation budget. Without an explicit host-selected retained-source flag, the aggregate maximum MUST equal the selected per-input source-byte limit (65,536 bytes by default); an explicitly opted-in declared `compile` or `session` MAY set an aggregate `--retained-source-bytes N` only for `1 <= N <= 262144`. A zero per-input `--source-bytes` limit with no opt-in remains a valid closed quota, not a panic or an implicit increase. Invalid/out-of-range explicit settings MUST refuse before source admission. Each individual source input MUST still obey `Limits.bytes <= 65536` and its unchanged nodes, depth and work limits; aggregate opt-in MUST NOT raise those limits or change ordinary sessions, standalone cursor, live profiles, guest execution budgets, memory policy, or host authority.

The aggregate MUST count the exact bytes of every retained original whole-module source plus outside-definition history, including the four-byte length header for **each** outside declaration. A module-local definition already contained in its original whole-module source MUST NOT be charged twice. Every conversion and prospective sum MUST be checked before publication; overflow or excess MUST refuse without truncation, eviction, alias-dependent reclamation, hash-only substitution or a partial namespace change. Import/rebind replay MUST reparse the original retained bytes, independently account for its checking work, and charge each newly appended exact dependent-history copy and its four-byte header while preserving earlier original history. Source-derived definitions, diagnostics, proof subjects and compiled identities MUST preserve original bytes, digests, spans and immutable module/definition owners across replay; a reconstructed display name, normalized text or digest alone MUST NOT replace source provenance. Prepared publication MUST compare the retained source profile and complete preexisting resolved definition graph, not only generation, module bytes, aliases and framed-history bytes: two same-profile namespaces can share all those bytes yet have different historical captured owners. A refused submission or foreign prepared snapshot MUST leave prior stack, namespace, aliases, source history and checked programs unchanged and execute no candidate body, consistent with H-REPL-02. A declared framed session MUST drain and refuse an above-per-input frame no longer than 262,144 bytes using fixed bounded storage, preserve a monotonic submission count and read a subsequent valid frame from the same session. A count beyond the safe drain ceiling, malformed framing, or a truncated body MUST fail terminally without candidate execution, and ordinary framing behavior MUST remain unchanged. The retained-source bound is independent of runtime fuel, guest memory, host-operation quota and proof acceptance.

#### Scenario: DX-14 retained source limits and replay

- GIVEN the unexecuted DX-14 design in `specs/conformance/language-workflow-cases.json` with exact original UTF-8 module/outside-definition byte lengths, four-byte history headers, default and explicit capacities, per-input lower limits, immutable versioned aliases, a current outside dependent and hostile changed-source/proof-span controls
- WHEN the actual declared compiler/session attempts each module, declaration, alias rebind and subsequent valid submission against those exact inputs
- THEN admission at the independently counted 65,536/262,144 boundaries and refusal one byte over, separate per-input refusal, original-source reparse identity and atomic recovery MUST match every `expected` field; the case remains absent/not-run with no guest-execution or proof claim until source-bound evidence is retained

**DX-TEXT-01.** `Text-Byte-Cursor-v1` MUST be an explicitly selected, pure, independently checked Noble source/managed-Wasm profile, not an addition to ordinary `Core-Bootstrap`, accepted `Declared-Modules-v1`, or LIVE. A combined `Declared-Text-v1` profile MAY be selected only when the caller explicitly selects both `--declared-modules` and `--text-byte-cursor`; it MUST keep the declared-module export/import discipline and this byte-cursor scheme together, MUST report itself as `Declared-Text-v1`, and MUST NOT be reachable by selecting either flag alone. Only opt-in `session` and `compile` MAY resolve `text.byte` as the globally disjoint Definition(26); reserved identities 23–25 and test.emit 22 MUST NOT be invocable in this profile, including by a forged direct submission. Ordinary, declared-only and LIVE behavior, wrapping I64 arithmetic and existing accepted evidence MUST remain unchanged.

The exact scheme MUST be `forall<S:stack> [ S Text I64 -- S Text I64 ! {} ]`. It MUST preserve the original immutable Text value with no substring copy or guest heap allocation and use no host request. For signed byte offset `i`, return unsigned UTF-8 byte 0..255 when `0 <= i < len(Text)`, -1 only when `i == len(Text)` (including empty Text), -2 when `i < 0`, and -3 when `i > len(Text)`. It MUST validate memory bounds and signed offset before narrowing/loading; a consumer MUST increment the offset only after a nonnegative byte. This byte-at facility does not establish iteration, arbitrary precision, deterministic calculator budget accounting, MA1 or any CALC case acceptance.

#### Scenario: TEXT-01 compiled cursor boundary

- GIVEN the unpromoted `TEXT-01` design in [`text-byte-cursor-cases.json`](conformance/text-byte-cursor-cases.json), with its UTF-8, empty, negative, large-positive, ordinary-profile and forged-private-definition inputs
- WHEN both optimization modes compile and run genuine managed-Wasm children and the independent admission boundary checks the forged candidate
- THEN the observed byte/status, original Text, zero host requests, no native trap and negative controls must match every `expected` field; the case remains absent/not-run until selected conformance assessment, not MA1/CALC acceptance

**NGRAPH-NICKEL-01.**

The opt-in `noble-ngraph-nickel/v1` authoring route MUST lower to the existing `ngraph-ir/0` representation and MUST be independently admitted by Noble's native checker under explicit construction and graph budgets. Its distinct `authored-nickel` producer identity MUST NOT be labeled as source-extracted. The first supported route MUST admit only finite acyclic `ngraph-i64/0` graphs with ordered concrete I64 inputs, exact signed-I64 constants, wrapping Add/Sub/Mul, ordered outputs including repetition, and explicit sharing. Unsupported operations, profiles, facts, guards, dynamic/recursive constructs, and resources MUST be rejected from submitted constructor data before simplification.

The `noble-ngraph-nickel/v1` schema MUST keep graph handles distinct from Nickel numbers, assign deterministic instance-scoped identities, preserve intentional shared references, and refuse duplicate or cross-scope handles. Exact constants MUST cross the JSON boundary as canonical decimal strings; construction parameters MAY determine graph shape or exact constants, runtime numeric arguments MUST become `Input` slots, and Noble's F32-only trainable `Param` MUST NOT stand for either.

The evaluator MUST be the repository-pinned Nickel 1.17.0 and MUST remain outside the pure no-std graph core. The evaluator lane MUST bind an explicitly declared, observed, ordered import closure and MUST force complete demanded constructor export before lowering, so import-order ambiguity or delayed contract failure cannot yield an admitted graph. A successful export MUST still undergo native `check_with` admission; a Nickel contract alone is not graph admission.

The retained provenance record MUST bind exact source and import bytes, construction parameters, constructor library and contract versions, evaluator identity, declared and observed import closure, exported bytes, lowerer revision, selected profile, graph bytes, and checked graph identity. A mismatch in any bound field MUST refuse stale receipt or consumer-record reuse. Hashes and receipts MUST NOT be described as proofs, equivalence relations, or permission grants.

Composition observations MUST name the observer as ordered outputs under exact wrapping-I64 evaluation and retain independent expected graphs and values. They MUST distinguish real evaluator execution, native admission, finite differential results, model-level theorems, and any implementation/artifact correspondence. They MUST NOT imply effect independence, resource permission, or arbitrary reordering.

**NGRAPH-NET-LIMITS-01.**

The opt-in `noble ngraph eval GRAPH VECTORS --net` route MUST enforce three explicit whole-request ceilings with bounded defaults. A ceiling MUST NOT be raised above its default, and a ceiling flag MUST NOT be accepted without `--net`. Each such misuse, and a malformed value, MUST be a usage error.

- `--max-input-bytes` MUST charge the graph file and then the vector file before each is read. No read may extend past the bytes the request has left.
- `--max-retained-bytes` MUST bound the accounted bytes of every buffer that the lowering, the independent validation rebuild, the reduction arena, the output boundary, the active-pair worklist and the read-back hold at the same time. Each buffer MUST be charged at its element size times its requested capacity before it is allocated or grown. Accounting MUST use non-wrapping arithmetic, and any capacity the allocator grants beyond the request MUST be charged before use continues.
- `--max-output-bytes` MUST bound the encoded report. Each line, newline included, MUST be charged at its exact length before its input-proportional values are constructed.

Exceeding a ceiling MUST produce exactly one `exhausted` report line, exit 5, with the code `E-NET-BUDGET-input-bytes`, `E-NET-BUDGET-retained-bytes` or `E-NET-BUDGET-output-bytes`. The line MUST carry the `limit`, the `cap` and the `observed` amount, and MUST replace every header and vector line. The route MUST NOT write a truncated success or a partial output row. A request at a ceiling MUST succeed exactly as it does above the ceiling. These accounted ceilings MUST NOT be reported as process RSS bounds or sandbox limits.

**NGRAPH-NET-RECEIPT-01.**

A complete net reduction MUST be reported as normal only when the reducer's interaction counter equals the validated net's census `2·#ops + #dups + #eras`. A mismatch MUST be an `internal-invalid` refusal of class `interaction-census` with no outputs. Each normal `--net` vector line MUST report `interactions`, `expected_interactions` and `count_basis: "census-checked"`, plus its accounted `retained_bytes_high_water`. This check is an executed per-run equality. It MUST NOT be described as a refinement or correspondence proof of the lowering, the reducer or the read-back.

The `--net` header line MUST bind:

- the producer package and version, the source revision the build declared or an explicit `unavailable` status, and a null executable digest;
- the contract (`ngraph-net-i64/3`), the lowering revision, the rule-table and plan, checker and executor revisions, the schedule, the storage policy and the identity scheme;
- every effective limit and the input bytes read;
- a `semantic_count` object naming the semantic unit, the counter, the observation boundary, the census, the count basis, the fully qualified Lean theorem declarations with their instantiated system, and the pinned proof sources and retained foundation receipt identities.

The `semantic_count` object MUST state its assurance as model-only, with an explicit boundary statement. A test MUST re-measure the pinned proof sources and receipt and MUST fail when they drift.
