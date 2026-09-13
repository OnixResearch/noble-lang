# RFC 0001 — Minimal typed programs-as-data core

Status: **Proposed design draft**, not an implemented language or an accepted specification  
Date: 2026-09-09  
Working language name: unassigned

## 1. Purpose

Define the smallest practical semantic foundation for a composable language in which programs are first-class, inspectable data. The implementation is intended to be written in Rust, with WebAssembly as the execution substrate and the Component Model as the interoperability boundary.

The central proposal is:

> A program is an immutable, typed composition that can be constructed, passed, inspected, persisted when eligible, and executed. Executing it does not erase the distinction between code and authority.

Small means few independent mechanisms and clear composition laws, not a contest to minimize keywords. A tiny parser with an undocumented evaluator, ownership model, or compiler service would not satisfy this goal.

All syntax and APIs below are provisional. Stack signatures and APIs are design notation, not claims about existing Rust crates or WIT syntax.

## 2. Scope boundary

The kernel should know about values, stack transformations, quotation values, data construction/elimination, definitions, and host operations. It should not know about terminals, clusters, deployments, networking protocols, actors, or job schedulers.

| Layer | Proposed responsibility |
|---|---|
| Language kernel | Composition, quotations, calls, stack typing, basic data, resource-use restrictions, effect summaries |
| Code model | Immutable resolved definitions, program inspection, canonical representation, identities |
| Compiler/tooling services | Parsing, checking edited syntax, compilation, namespace updates, artifact storage |
| Standard library | Collection combinators, syntax transformations, stream combinators, convenience words |
| Execution/host profile | Wasm ABI, resources, quotas, async integration, authorized imports |
| Applications | Filesystems, terminals, databases, cluster APIs, remote execution policy |

A library may depend on a runtime/host facility without becoming language syntax. For example, a stream library requires an explicitly specified async host profile, but does not require a pipeline operator or a special stream AST node.

## 3. Evaluation and surface forms

### 3.1 Evaluation

Evaluation is left-to-right. The stack top is written on the right. A word consumes the stack suffix described by its signature and leaves the unmentioned prefix alone.

```text
10 20 +
```

Illustrative stack evolution:

```text
[] -> [10] -> [10, 20] -> [30]
```

### 3.2 Expression forms

The initial expression grammar needs three forms:

```text
expression := literal | word-reference | "[" expression* "]"
program    := expression*
```

This is an expression grammar, not the entire file grammar. Definitions, imports, and type declarations belong to the module/declaration layer and must be specified separately.

A literal pushes a value. A word reference invokes its resolved definition. A quotation constructs a program value without executing its body.

```text
[ 1 + ]                    // Construct a program; do not add yet.
41 [ 1 + ] call             // 42.
```

Source names resolve against an explicit namespace snapshot. A checked program contains resolved identities, not a mutable name lookup to perform later.

### 3.3 Definitions

Proposed declaration syntax:

```text
def square [ dup * ]
def twice  [ dup compose ]
```

`def` binds a name to a checked definition. It does not run the body. Declarations are not ordinary mutable dictionary writes available from arbitrary guest code.

For the first implementation, require explicit signatures for recursive definitions. Initially support a single definition's self-reference; defer mutually recursive groups until their typing and canonical identity rules are specified.

## 4. Program values

### 4.1 Program type

Use this explanatory notation:

```text
Program<InputStack, OutputStack, Effects>
```

For example:

```text
[ 1 + ] : Program<(I64), (I64), {}>
```

Stack signatures in examples omit the untouched stack prefix. The formal type system must include a stack-tail variable so the same word works above unrelated values.

Effects in a program's type are **latent**: they describe execution of the program, not construction of its quotation.

### 4.2 Initial program-building operations

The foundational operations are:

| Operation | Meaning |
|---|---|
| `[ ... ]` | Construct a checked program literal |
| `call` | Execute a program against the current stack |
| `compose` | Construct a program that executes the first program and then the second |
| `quote` | Construct a program that pushes an eligible immutable value |

Schematic signatures:

```text
call:
    S Program<S, T, e> -- T ! e

compose:
    Program<A, B, e> Program<B, C, f>
    -- Program<A, C, union(e, f)> ! {}

quote:
    a -- Program<S, S a, {}> ! {}
    where a is eligible for capture
```

Here `S`, `T`, `A`, `B`, and `C` stand for stack shapes, not necessarily individual values. Ambient prefixes of the builder operations themselves are omitted.

Construction must not execute a quoted host operation. `compose` preserves execution order; the effect set is not an execution schedule.

### 4.3 Examples

```text
20 [ 1 + ] [ 2 * ] compose call
// 42: execute (+1), then (*2).
```

```text
40 2 quote [ + ] compose call
// 42: construct a program that pushes 2 and adds it.
```

```text
def twice [ dup compose ]

20 [ 1 + ] twice call
// 22: twice constructs a program that adds 1 twice.
```

The inferred input of `twice` is a reusable program whose input and output stack shapes match. It is not a combinator over every arbitrary program signature.

### 4.4 Explicit capture

Quotation literals do not implicitly capture the current data stack. Stack inputs belong to the later invocation.

In v0, `quote` captures only immutable, duplicable data with a defined persistence representation. Programs can accept resource handles as invocation inputs but cannot capture live resource handles.

This restriction keeps program values reusable and avoids introducing one-shot closures, borrowed environments, and lifetime-polymorphic quotations in the first version.

Capturing data is not automatically harmless: a text value may contain a secret. Persistence/export remains an explicit operation; quoting a value never uploads it or publishes it.

### 4.5 First-class guarantee

Program values must be passable to words, returnable from words, and storable in ordinary homogeneous collections subject to normal type rules. They must not be limited to quotation literals recognized syntactically at individual call sites.

Runtime construction using typed builders must work without invoking a source compiler. The first backend must demonstrate this, rather than only constant-folding every quotation example.

## 5. What programs-as-data means

### 5.1 Semantic representation

A checked program has a stable, source-oriented structural representation. An initial conceptual model is:

```text
ProgramBody = sequence<Atom>

Atom = Literal(PortableValue)
     | Invoke(DefinitionId)
     | Quotation(ProgramBody)
```

Builtin operations use stable builtin identities. Types and schemas accompany values where necessary. This schema is conceptual, not a frozen wire encoding or a complete Rust type definition.

Composition concatenates program sequences. Implementations may share structure internally. Reflective inspection must not expose optimization-dependent machine-code layout, closure memory, or incidental allocation identities.

### 5.2 Two views, not two languages

The design distinguishes:

```text
Syntax                  editable data; possibly unresolved or ill-typed
Program<S, T, e>         checked, callable program with known interface
```

The two may share the same underlying persistent tree. The difference is the invariants the type system permits the caller to rely on.

A program is not an arbitrary heterogeneous list that `call` attempts to execute. Conversely, a program must not be only an opaque function pointer with no inspectable composition.

### 5.3 Inspection and transformations

A proposed pure reflection operation returns the program's resolved recipe:

```text
reify : Program<S, T, e> -- Syntax
```

For example:

```text
[ 1 + ] reify
// sequence(Literal(1), Invoke(<stable identity of integer addition>))
```

Inspection of a reference need not expand its definition. Retrieving a referenced body from an external code store is a separate authorized operation. Rust host implementations remain opaque; reflection exposes their declared import identity and contract, not their native implementation.

A syntax transformation is an ordinary function:

```text
Syntax -- Result<Syntax, TransformError>
```

Editing syntax creates new data. It cannot mutate an already checked program, rewrite another running invocation, or confer executable status on unchecked nodes.

### 5.4 Rechecking edited code

Arbitrary syntax transformations can invalidate stack types, resource-use rules, name resolution, or effects. Such transformations need explicit checking before the result becomes callable.

For a statically typed caller, the expected interface must be known. Schematic API:

```text
prepare<ExpectedInput, ExpectedOutput, AllowedEffects>:
    CompilerCapability Syntax
    -- CompilerCapability
       Result<Program<ExpectedInput, ExpectedOutput, AllowedEffects>, Diagnostic>
    ! {code.prepare}
```

The service checks that the inferred result matches the requested interface and that actual effects are within the allowed set, then prepares executable Wasm. It does not execute the program body. This is a proposed language adapter API, not a generic WIT function signature.

A tool may infer and display an initially unknown signature. That does not allow an ordinary statically typed caller to execute an unknown stack transformation through `Any`.

An artifact's claimed type metadata is not a trusted proof. Loading externally supplied syntax/code requires validation appropriate to the artifact's trust level.

## 6. Wasm-only execution and staging

### 6.1 Ordinary quotation execution

Checked source quotation literals compile to Wasm. Runtime `quote` and `compose` can produce compiler-generated closures/adapters using already compiled operations and captured immutable environments. `call` invokes that executable representation.

This requires closure/environment and data-representation support. It does not require a second bytecode interpreter or an AST-walking evaluator.

The initial backend must prototype its internal calling convention and demonstrate dynamically constructed compositions. The specification should not assume arbitrary Wasm function signatures magically share one calling convention.

### 6.2 Arbitrary new syntax

Arbitrary new syntax is not made executable merely by existing inside a Wasm component. The proposed full development profile exposes an explicit compiler service:

```text
edited syntax -> resolve/check -> compile -> Wasm execution
```

A minimal execution-only host need not include that service. It still supports ordinary first-class quotations and type-preserving builders over the compiled vocabulary.

`call` must not hide an implicit request for a compiler, network fetch, or installation of new host capabilities.

### 6.3 Artifacts and components

A source definition and a Wasm component are different units. Many definitions may compile into one component. A function call must not inherently become a cross-component call.

An internal `Program<S,T,e>` is not automatically a WIT function value. Cross-component transport requires an explicit, separately specified representation, such as an artifact package or a concrete resource interface. Generic language types are specialized or otherwise lowered before crossing the WIT boundary.

### 6.4 REPL

The REPL compiles submitted expressions rather than requiring a separate evaluator. Its persistent stack is session-owned state, not a suspended Wasm operand stack.

A submitted expression must be checked before its body performs user-visible host effects. A type error leaves the prior session stack unchanged. This is not a promise to roll back effects of a successfully checked expression that later fails during execution.

Resource recovery after traps/cancellation must be specified by the execution/session profile.

## 7. Initial typing and data scope

### 7.1 Static core

Start with rank-1 polymorphism for named definitions, stack-tail inference, algebraic data types, and effect variables for higher-order combinators. Require annotations where recursion or exported interfaces need them.

Quotation values have known, instantiated interfaces at their call sites. Do not initially add impredicative polymorphism, general subtyping, dependent types, user-defined inference plugins, or dynamic `Any`.

The formal inference algorithm remains a required deliverable; its completeness and soundness are not established by these examples.

### 7.2 Data

Begin with a small, explicit numeric and data model. Proposed scalar starting point: signed 64-bit integers, booleans, byte sequences, and text. Add one algebraic-data mechanism for products, variants, and recursive collections rather than separate mechanisms for every domain entity.

Specify integer overflow rather than inheriting build-mode-dependent behavior. A candidate is wrapping `I64` arithmetic with explicitly named checked operations returning `Result`. Numeric semantics are an open decision until tests and canonical encodings are fixed.

Keep `Option` and `Result` ordinary variants. Parsing external data returns failure explicitly; successful schema validation does not prove that a remote machine remains available.

### 7.3 Effects

The first effect system tracks conservative host-operation requirements and their propagation through program calls. Program construction is effect-free with respect to its quoted operations; execution has the program's latent effects.

Effect summaries are not credentials. The host still controls resource scope and actual authority.

The empty effect set means no tracked external host effects. It is not a totality proof or a guarantee against resource exhaustion, traps, timing differences, or side channels.

Do not initially implement general algebraic handlers, continuation capture, or multi-shot resumptions. Those features would substantially enlarge the ownership and Wasm-lowering problem. Host substitutions and explicit data inputs provide the first testing seams.

## 8. Resource boundary

Begin with immutable reusable data and opaque move-only host resources. Avoid a full Rust-like borrow checker.

Proposed restrictions for the first resource profile:

- `dup` and generic `drop` apply only to ordinary duplicable/discardable data.
- Resource handles cannot be quoted, structurally inspected into their internals, or serialized.
- A resource must be threaded onward, consumed by an operation, or explicitly released on a normal execution path.
- Branches must agree on resource disposition.
- Abort cleanup is a host obligation and must not depend on ordinary guest code continuing to run.

These are stronger normal-path restrictions than simply allowing unrestricted affine discard. The final terminology and formal rules must match the actual release semantics.

An illustrative API avoids guest-visible borrowing:

```text
fs.read:
    Directory Text
    -- Directory Result<Bytes, FsError>
    ! {fs.read}
```

Thus a quotation may be reusable without capturing a directory:

```text
[ "main.rs" fs.read ]
```

The directory is an invocation input. The adapter can perform a scoped native borrow internally while returning the guest's ownership exactly once according to the result contract.

Move-only guest handles do not establish exclusive ownership of the corresponding external world resource across all other processes. Remote leases, revocation, authorization, and failure still require runtime protocols.

## 9. Small control basis; ordinary libraries

The kernel needs stack rearrangement, program construction/execution, data construction/elimination, and an explicit recursion mechanism. Candidate foundational words include `dup`, `drop`, `swap`, `dip`, `call`, `quote`, and `compose`, together with data eliminators and primitive operations.

Do not freeze an advertised exact primitive count before testing whether the basis is usable and whether its typing rules are tractable.

Boolean branching can be syntax/library convenience over a sum eliminator. Repetition and collection combinators should derive from the control/data basis where practical. A small kernel does not require users to hand-write stack shuffles: the prelude should provide well-tested combinators such as `map`, `fold`, `filter`, `keep`, and `cleave`.

Streams are a standard-library and execution-profile concern. A future stream specification must define bounded buffering, cancellation, ordering, completion errors, and fair merging. Sequential composition does not imply parallel execution, and a quoted stream expression is not automatically a durable workflow graph.

Do not initially add custom reader macros, arbitrary compile-time I/O, shell-compatible parsing, infix precedence, or a second macro language. Ordinary program builders and syntax-to-syntax functions should be tested before new metaprogramming mechanisms are admitted.

## 10. Unison-inspired identity

Names are metadata; resolved definition identities determine meaning. A definition body references exact dependencies. Changing a name mapping does not mutate old programs or running code.

Distinguish:

```text
DefinitionId: canonical resolved language definition and relevant semantics
BuildKey:     definition/dependencies + compiler/profile/options
ArtifactId:   exact artifact bytes
```

A portable captured program additionally includes its captured data and schema information in the identity appropriate to that value. Do not confuse a function's definition identity with the identity of a closure carrying particular captured values.

Canonical identity must not depend on formatting, source spans, incidental namespace aliases, allocation addresses, or optimizer output. It must account for meaningful type identities, builtin semantics, and resolved dependencies.

This is structural identity, not an algorithm for deciding mathematical equivalence of arbitrary programs. Programs with the same outputs can have different identities. Effectful result caching is not justified by a code hash alone.

A canonical representation and encoding version must be specified before claiming cross-implementation stable hashes. Self-recursion needs a distinguished local reference. Mutual-recursion group canonicalization is deferred.

A code hash is not a provenance certificate or authorization token. Artifact loading, signatures, trust policy, and execution authority are separate concerns.

A first implementation can use a local content-addressed directory and a namespace manifest. It need not begin with a distributed database, hosted registry, or UCM-sized application.

## 11. Proposed semantic laws and acceptance examples

Laws concern specified program behavior and resolved recipes, not elapsed time or incidental allocation. Optimizations must preserve the relevant behavior and reflective contract.

```text
run(empty, s) = s
run(compose(p, q), s) = run(q, run(p, s))
run(quote(v), s) = push(s, v)
```

Composition is associative in execution order. Empty composition is an identity. Compatible effect summaries combine without reordering effects. Canonical sequence representation should normalize composition grouping; arbitrary semantic equivalence is not required to imply equal identities.

| Test | Required observation |
|---|---|
| `41 [ 1 + ] call` | Produces 42 |
| `20 [ 1 + ] [ 2 * ] compose call` | Produces 42 in left-to-right order |
| `40 2 quote [ + ] compose call` | Produces 42 without dynamic source compilation |
| `def twice [ dup compose ]`; `20 [ 1 + ] twice call` | Produces 22 |
| A quotation containing a host write is constructed but not called | No write occurs |
| Two typed builders are composed at runtime | Remain callable with a known interface; no literal-only restriction |
| Incompatible program endpoints are composed | Static error before expression effects |
| Resource handle is passed to `dup` or `quote` | Static error |
| A syntax rewrite inserts an incompatible operation | `prepare` returns a diagnostic; no body execution |
| A program is reified | Resolved structural recipe is inspectable |
| Referenced word is renamed | Resolved program meaning and definition identity stay unchanged |
| A word body/dependency is changed | A new definition identity; existing references stay pinned |
| A host lacks compiler-service authority | Ordinary quotations still work; dynamic preparation is unavailable |
| A host lacks authority required by a valid import | Link/request denial, not fabricated authority from types |

These are acceptance targets, not reports of tests already executed. Randomly generated well-typed compositions, malformed serialized code, effect propagation, and resource-use cases should supplement the named examples.

## 12. Specification work sequence

### Document A — Kernel semantics and types

Expression grammar, data model, evaluation order, stack-tail rules, quotation typing, generalization, control eliminators, recursion, resource restrictions, and observable error behavior. Deliverable: small-step or equivalent operational semantics plus a checker design and conformance cases.

### Document B — Programs as data and identity

Resolved node schema, editable syntax, inspection, type-preserving builders, explicit capture, checked preparation, canonicalization, dependency resolution, definition IDs, and schema/version rules.

### Document C — Wasm execution and component ABI

Calling convention, closure/environment representation, memory management, resource tables, monomorphization/lowering policy, imports/exports, errors/traps, compiler-service boundary, REPL session ownership, quotas, and portability profile.

### Document D — Small standard library and host profiles

Data combinators first, then one constrained filesystem interface. Add async streams only after their runtime semantics are documented. Remote code transport comes after a local portable-code round trip; live resource migration is out of scope.

The first vertical prototype should compile and run the pure quotation acceptance examples, including runtime-built composition, while retaining reifiable recipe metadata. A custom reference evaluator may be useful strictly as a test oracle; it would not be a production execution engine.

## 13. Decisions to resolve before freezing v0

| Question | Recommended starting position |
|---|---|
| Are quotation values only opaque closures? | No: retain stable inspectable recipes |
| Can arbitrary syntax be passed directly to `call`? | No: an explicit checked preparation boundary |
| Must every deployment include a compiler? | No: full development profile vs execution-only profile |
| Can quotations capture live resources? | Not in v0; pass resources as invocation inputs |
| General algebraic effect handlers now? | Defer; begin with compositional effect summaries |
| Is every definition a Wasm component? | No: identity granularity and execution granularity differ |
| Is there a custom production bytecode VM? | No; support code and closures compile to Wasm |
| Do we need a distributed code database initially? | No; local content-addressed storage suffices to test identity |
| Are exact primitive count and punctuation fixed? | No; validate semantics and expressiveness first |
| How are allocation, cleanup, recursive identities, and resource failures encoded? | Required follow-up specification work, not implementation accidents |

## 14. References and their intended role

The references supply mechanisms and precedents, not an assertion that any existing project already implements this proposal. Project descriptions below were checked against their primary documentation on 2026-09-09.

[R1] Factor — quotations and combinators. Precedent for program values and higher-order composition. Its documentation also describes limitations of its stack checker for nonliteral/dynamically built quotations; this proposal must not inherit a literal-only first-classness limitation by accident.

- `https://docs.factorcode.org/content/article-quotations.html`
- `https://docs.factorcode.org/content/article-combinators.html`
- `https://docs.factorcode.org/content/article-inference-combinators.html`

[R2] Mirth — strongly typed concatenative language. A reference for static concatenative design, not a commitment to its syntax or implementation.

- `https://github.com/mirth-lang/mirth`

[R3] Unison — hashes and abilities. References for names-independent definition identity and explicit effects. General handlers are deliberately not included in the first core.

- `https://www.unison-lang.org/docs/language-reference/hashes/`
- `https://www.unison-lang.org/docs/language-reference/abilities-and-ability-handlers/`

[R4] wasm-tools — low-level Wasm libraries, including generation, parsing, WIT processing, and component packaging. These implement backend mechanisms, not the language type system.

- `https://github.com/bytecodealliance/wasm-tools`

[R5] wit-bindgen — guest-language bindings for WIT; its documentation distinguishes guest binding generation from Wasmtime's host-facing component bindings. Our hypothetical language requires its own guest lowering/generator.

- `https://github.com/bytecodealliance/wit-bindgen`

[R6] Wasmtime — execution/compilation engine and native Rust embedding. Use as the execution substrate, not as a source-language evaluator.

- `https://github.com/bytecodealliance/wasmtime`
- `https://docs.wasmtime.dev/api/wasmtime/component/struct.Component.html`

[R7] WIT reference — cross-component interface data and owned/borrowed resource handles. It is not the complete internal type system for quotations, effect rows, or stack-polymorphic words.

- `https://component-model.bytecodealliance.org/design/wit.html`

[R8] wstd — an async Rust library for components; its README currently describes WASI 0.2. Treat as a guest-library/API reference, not a mandatory dependency or a definition of our language semantics.

- `https://github.com/bytecodealliance/wstd`

[R9] cap-std — capability-oriented Rust APIs for host adapters. Its documentation explicitly notes that it is not, by itself, a sandbox for arbitrary untrusted Rust.

- `https://github.com/bytecodealliance/cap-std`
