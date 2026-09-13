# Specification decision register

Document: SPEC-0001  
Revision: 0.1.0-draft.1  
Date: 2026-09-09  
Basis: RFC 0001, preserved unchanged alongside the specification

This register distinguishes the RFC's proposed direction from elaborations made while drafting the specification. “Adopted in this draft” means written as a requirement for this working revision; it does not mean ratified by a later review or proved correct by an implementation.

## 1. RFC positions carried into the working draft

| Decision | RFC basis | Draft treatment |
|---|---|---|
| Programs are immutable, inspectable typed values | §§1, 4–5 | Core requirement; not merely function pointers |
| Left-to-right stack composition with three expression forms | §3 | Core evaluation and quotation rules |
| Programs may be passed, returned, and placed in homogeneous collections | §4.5 | First-class conformance obligation |
| Runtime typed builders do not need a source compiler | §§4.5, 6.1 | `quote`, `compose`, and `call` execution obligations |
| Arbitrary edited syntax requires explicit checking/preparation | §§5.2–5.4, 6.2 | `Syntax`/`Program` distinction and preparation contract |
| Program capture excludes live resources | §4.4, §8 | Recursive capture restrictions |
| Move-only resources require explicit normal-path accounting | §8 | Not described as unrestricted affine discard |
| Rank-1 named polymorphism, stack tails, and effect variables | §7.1 | Required type obligations; solver still to be specified |
| Host-operation summaries are not permissions | §7.3 | Type effects and host authority remain separate |
| Immutable resolved definitions are separate from names | §§3.2, 10 | Snapshot resolution and pinned references |
| Definition, build, and artifact identities differ | §10 | Separate identity concepts |
| Wasm is the production execution substrate | §6 | No second production language VM required |
| Domain APIs, streams, and distribution remain outside kernel syntax | §§2, 9 | Standard-library/profile boundary |
| The REPL owns explicit session state | §6.4 | Type errors preserve prior state; runtime effects are not rolled back |
| Exact ABI, encoding, and proof details remain work to do | §§12–13 | Named release gates, not hidden assumptions |

## 2. New selections and elaborations in this revision

### D-01 — Draft status, requirements, and conformance labels

**Status:** Adopted in this draft.  
**Location:** Specification §§1, 16.

Use revisioned requirement identifiers and separate core, programs-as-data, resource, preparation, and Wasm draft reporting labels. They are subsets for implementation reporting, not different language semantics. The whole language is not declared stable. Fixture outcomes remain expectations until executed against an implementation.

**Reason:** The RFC was exploratory. A specification must distinguish requirements from commentary and avoid implying that an unfinished ABI or checker is already standardized.

### D-02 — Expression lexical microprofile and submission boundary

**Status:** Adopted in this draft; syntax remains revisable.  
**Location:** Specification §3.

Choose UTF-8 source, explicit ASCII separators, `//` comments, decimal `I64` literals, `true`/`false`, a small set of string escapes, and bracket quotations. Specify maximal-token behavior and no interpolation or shell expansion. Keep declarations separate from expression execution. A submission is one declaration or one body; tools may compose submissions.

**Reason:** The RFC supplied only the expression skeleton. Concrete fixtures need one unambiguous lexical profile. The profile does not pretend to settle modules or annotation syntax.

### D-03 — Wrapping I64 arithmetic

**Status:** Adopted in this draft; review explicitly before stable v0.  
**Location:** Specification §4.4.

Select wrapping modulo-`2^64` behavior for `+`, `-`, and `*`, independent of build mode. Out-of-range literals remain static errors. Checked arithmetic remains a future ordinary `Result`-returning library interface.

**Reason:** This selects a candidate explicitly identified as open in RFC §7.2. Leaving overflow to the implementation would make the first arithmetic examples underspecified at their boundaries.

### D-04 — Abstract evaluation steps and trace-aware composition laws

**Status:** Adopted in this draft.  
**Location:** Specification §5.

Write stack/host/trace transition rules and distinguish normal `Result` values, divergence, and abnormal termination. Composition runs its second operand only after a normal return from the first. Pure equations are not treated as rollback or allocation-equivalence guarantees.

**Reason:** The RFC's laws need explicit failure and effect-order interpretation. These rules specify behavior; they are not a new production evaluator.

### D-05 — Built-in recursive eligibility predicates

**Status:** Adopted in this draft.  
**Location:** Specification §§4.3, 9.

Use `Data(a)` and `Capture(a)` as built-in judgments, not an extensible typeclass system. Propagate them through instantiated schemas. Program input types may mention resources without making the program itself resource-owning. Wrapped resources remain non-data.

**Reason:** Directly banning `dup` on a resource is insufficient if wrapping it in an aggregate or quotation bypasses the check. The eligibility proof and algorithm still need formalization.

### D-06 — Bootstrap data and control contracts

**Status:** Adopted in this draft; internal primitive count remains unfixed.  
**Location:** Specification §7.

Specify `unit`, products, sums, boolean branching, a recursive list schema, and payload order for eliminators. Specify `dip`'s restoration rule. These are ordinary words/data contracts, not extra expression forms. `map` and other traversal libraries remain later work.

**Reason:** Typed programs need a usable control/data basis to demonstrate more than arithmetic. An implementation may derive operations rather than implement each as a kernel primitive.

### D-07 — Generalization and instantiation obligations

**Status:** Adopted as constraints; complete solver is open.  
**Location:** Specification §8.

Independent named uses instantiate freshly. Duplicating a monomorphic program value does not create independent polymorphic instances. Recursive checking uses a declared signature; polymorphic recursion and mutual groups remain out of scope. Invalid infinite type equations are rejected.

**Reason:** These rules expose a real distinction between reusable program values and reusable generic definitions. They do not silently promise impredicative first-class programs or a proved inference algorithm.

### D-08 — Recipe normalization for composition and program capture

**Status:** Adopted semantically; binary encoding remains open.  
**Location:** Specification §11.

Normalize `compose` to ordered recipe concatenation. Normalize capturing a program to a quotation node, with interface information retained. Preserve nested quotation boundaries; do not inline definitions or rewrite recipes merely because an optimizer can simplify execution.

**Reason:** Reflection should observe the language program rather than runtime adapter chains. This is a new elaboration of RFC §5.1 and §11, not a frozen wire format.

### D-09 — Explicit program-value identity label

**Status:** Adopted as terminology.  
**Location:** Specification §12.

Use `ProgramValueId` to name the identity of portable code plus its captures. Keep it distinct from `DefinitionId`, `BuildKey`, and `ArtifactId`.

**Reason:** The RFC already distinguished closure identity from function-template identity. An explicit name prevents implementation discussions from overloading “program hash.” No hash algorithm is selected.

### D-10 — Error categories, fixture format, and release gates

**Status:** Adopted for the draft package.  
**Location:** Specification §§5.4, 16 and `conformance/cases.json`.

Classify parse, resolution, type/stack, resource eligibility, and effect-bound errors without freezing diagnostic wording. Use descriptive structured fixtures for operations whose final source syntax is not yet selected. Require explicit unsupported/not-run status.

**Reason:** A specification should make uncertainty visible rather than count missing tests as passes or invent source syntax for unfinished features.

## 3. Open decisions; not silently resolved

| Identifier | Open item | What must be produced next |
|---|---|---|
| O-01 | Full type inference and checking | Formal rules plus solver/generalization strategy, constraint handling, and proof obligations |
| O-02 | Annotation, module, import, and type-declaration grammar | Source grammar and resolution tests |
| O-03 | Recursive definition identities and type witnesses | Canonical owner/reference and generic-instantiation representation |
| O-04 | Canonical bytes and hashing | Encoding/version specification and independent test vectors |
| O-05 | Wasm program calling convention | Prototype with truly dynamic capture/composition and reflection |
| O-06 | Memory management and closure support | Allocation, lifetimes, limits, and cleanup design |
| O-07 | WIT/component ABI and version profiles | Concrete lowering and interface compatibility rules |
| O-08 | Artifact/recipe trust | Validation and provenance policy for accepting executable program values |
| O-09 | Resource failure semantics and REPL recovery | Host ownership accounting, abort policy, and resumed-stack rules |
| O-10 | Async streams and cancellation | Explicit boundedness, ordering, fairness, completion/error, and ownership semantics |
| O-11 | Portable code transport | Package/dependency loading and capability reacquisition/delegation |
| O-12 | Language naming and packaging | Public name, extension, package namespace, repository and governance decisions |

## 4. Recommended next specification increment

Write the checker specification against the kernel examples and negative cases before adding more syntax. In parallel, use a narrowly scoped Wasm backend experiment to test dynamic `quote`/`compose`/`call` with recipe retention. That experiment should validate feasibility without selecting an entire shell, scheduler, or distribution architecture.

No backend experiment or checker implementation is included in this package.
