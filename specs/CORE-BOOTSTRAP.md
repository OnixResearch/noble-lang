<!-- Generated compatibility view. Edit .cairn/specs/core-bootstrap/spec.md instead. -->
# First core implementation subset

Document: SPEC-B001  
Revision: 0.1.0-draft.5  
Depends on: SPEC-0001, SPEC-S001 and SPEC-V001 at 0.1.0-draft.5  
Status: Bootstrap contract selected; checker, proof, and backend implementations absent

## Scope

This document bounds the first feasibility work. It does not claim to complete general inference or the full language specification.

The optional `Contracts-Draft` profile in [PROGRAM-CONTRACTS.md](PROGRAM-CONTRACTS.md) builds on this subset through MC1 and MC2. Ordinary bootstrap programs require no behavioral proofs. Proof companions do not silently add dependent types or new expression forms to the bootstrap checker.

**B-SCOPE-01.** The implementation MUST report this subset as `Core-Bootstrap`, not full `Core-Draft`, `Resources-Draft`, or `Component-Draft` conformance.

The first target includes:

1. `I64`, `Bool`, `Text`, `Unit`, homogeneous `List`, `Pair`, `Sum`, `Program`, and inert `Syntax`.
2. Literal, resolved invocation, and quotation nodes with finite stack-tail constraints.
3. Nonrecursive rank-1 named definitions and monomorphic first-class program interfaces.
4. The bootstrap stack/data/control words, wrapping arithmetic, `quote`, `compose`, `run`, and `reflect`.
5. Concrete effect identities from a supplied test environment, including a resource-free `test.emit` operation.

User-defined recursive schemas, self-recursive definitions, module imports, full preparation services, live host resources, and advanced inference are outside this first implementation subset.

The [exact calculator](CALCULATOR.md) follows the core and separate library gates. Its arbitrary-precision integers, rationals, decimal input, and expression parser do not expand this subset or change wrapping `I64` arithmetic.

The checker representation must still express resource-bearing types for negative eligibility fixtures. Those fixtures do not claim a live resource runtime. Positive resource-bearing branch scenarios belong to the later `Resources-Draft` profile.

**B-SCOPE-02.** Unsupported syntax, types, profiles, and witnesses MUST produce an explicit unsupported or rejection result. They MUST NOT fall back to `Any` or unchecked execution.

## Candidate boundary

**B-CHECK-01.** Inference produces an untrusted, finite candidate. Acceptance MUST independently check it against an explicit expected interface and environment.

The candidate describes a resolved node sequence, node interfaces, generic instantiations, schema references, and claimed effect bounds. A witness is a claim, not trusted typing evidence.

**B-CHECK-02.** The acceptance environment MUST bind exact builtin/host/schema/definition identities to checked contracts. External environment data requires separate validation before use. Recursive definition dependencies and user-declared recursive schemas are unsupported. The fixed bootstrap `List` schema is supported.

**B-CHECK-03.** Every node MUST have a declarative derivation. Sequence joins match complete ordered stacks. Quotation construction separates its own effects from latent body effects. Each named invocation instantiates its scheme freshly.

Duplicated first-class program values share one instantiated interface. A witness cannot turn duplication into independent polymorphism.

**B-CHECK-04.** Acceptance MUST check recursive eligibility and effect inclusion from the node derivations. An advertised empty effect bound cannot hide `test.emit`. Host denial does not repair an unsound effect bound.

**B-CHECK-05.** Candidate graphs and substitutions MUST be finite and well founded. The checker rejects cyclic type equations and malformed node references. Each checking pass needs a decreasing measure or an explicit resource limit that fails closed.

**B-CHECK-06.** Branch-local type information MUST come from checked eliminator derivations. Branch joins MUST preserve the complete common output stack and conservative effect bound. Arbitrary predicates and advertised witnesses MUST NOT introduce trusted refinements or implicit union joins.

Eliminating `Sum<Resource<R>,I64>` exposes the selected payload at its own type. The whole sum remains non-`Data` and noncapturable. The resource branch must account for its owner. No general flow-typing or lifetime system is added.

**B-CHECK-07.** The bootstrap acceptance request MUST bind an independently supplied expected interface, exact environment, supported subset, and finite limits. Candidate records MUST identify their format revision and bind each witness to its node and premises. Candidate metadata MUST NOT change the expected interface or grant access to another environment.

The implementation MUST account for input bytes, decoded nodes, nesting, dependency traversal, type/stack size, checking work, memory, and diagnostic output. Each limit requires a boundary case and an exhausted case. A stage MUST reject excess work before allocation or traversal exceeds its limit. Diagnostics MUST preserve the primary outcome under their own output limit.

K-CHECK-06/07 apply to this supported subset. Unsupported recursion, declarations, and preparation services remain outside bootstrap. These limits do not authorize fallback to a partial checker.

**B-DIAG-01.** Branch and composition diagnostics MUST identify the failing join and expected-versus-actual stack shapes. Available source locations and ownership/eligibility constraints belong in the diagnostic. Exact presentation remains open. DX-DIAG-01 in [DEVELOPER-EXPERIENCE.md](DEVELOPER-EXPERIENCE.md) adds word-level failures, stack constraints, and available value origins. Editor holes and guest declarations remain outside this first subset.

The complete witness byte format and solver algorithm are M2 deliverables. This draft selects the boundary and obligations, not a serialization accident.

## Observable outcomes

**B-RESULT-01.** The checker MUST distinguish acceptance, invalid input, unsupported input, resource exhaustion, and internal failure. Only acceptance permits candidate execution.

**B-RESULT-02.** A rejected submission MUST leave the existing session stack and namespace unchanged. It MUST issue zero candidate-body host requests. Explicit compiler-service effects belong to a separate trace.

## Implementation boundaries

**B-IMPL-01.** Safe sequential Rust components own syntax data, types, candidate validation, recipe operations, identity rules, and runtime semantic transitions. The entire bootstrap kernel MUST follow the mandatory Charon → Aeneas → Lean route under VT-SCOPE-02. Filesystems, Wasmtime, clocks, and authorization effects remain in the shell or adapters. Deterministic authorization decisions remain in the core.

The semantic core targets `no_std + alloc` where the selected extraction toolchain supports it. A compatibility exception to `no_std` does not permit ambient I/O or exempt kernel code from extraction and refinement.

**B-IMPL-02.** The first extraction experiment MUST use the actual acceptance representation and at least one nontrivial validation function. A proof about a separate handwritten checker does not establish Rust correspondence. This experiment MUST start the whole-kernel function inventory and extraction gate under VT-SCOPE-05 and VT-CI-05. One proved validation function does not close that inventory.

**B-IMPL-03.** Rust interfaces MUST distinguish semantic identities, build identities, owner contexts, untrusted candidates, and accepted programs with domain-specific types. Public constructors and deserializers MUST NOT bypass acceptance or authority checks. Identical storage layouts do not make these types interchangeable.

The first checker uses ordinary owned semantic data. Byte-layout views stay in decoding adapters. A byte representation trait cannot grant Noble `Data`, `Capture`, or program validity.

**B-WASM-01.** The Wasm experiment MUST support values supplied after compilation. Runtime `quote`, `compose`, and `run` use compiled operations, not a source compiler or recipe interpreter.

**B-WASM-02.** The experiment MUST define its internal calling convention and bounds for dynamic compositions. It must retain the same observable recipe with optimization enabled and disabled.

The experiment must report allocation and composition-depth limits. A quota failure is not successful execution. ABI and backend proofs remain separate obligations.

[BACKEND-EXPERIMENTS.md](BACKEND-EXPERIMENTS.md) requires the M3 WasmGC/managed-linear-memory comparison. It does not select either representation or require two production backends.

## Acceptance

[conformance/cases.json](conformance/cases.json) supplies newly authored source and structured harness cases. [EVIDENCE.md](EVIDENCE.md) defines their status and observation rules.

**B-GATE-01.** The first end-to-end milestone requires positive acceptance and Wasm execution, negative rejection, runtime-selected builders, and recipe observations. Document validation alone MUST NOT close it.
