# Noble Language Specification

<!-- cairn:purpose:start -->
## Purpose

This accepted specification records Noble draft contracts, not completed implementation.
Original requirement IDs, explanatory prose, examples, and open decisions remain authoritative.
Scenario clauses refer to unexecuted designs in the conformance ledger.

## Requirements
<!-- cairn:purpose:end -->

## Version 0.1.0-draft.5 — Canonical working draft

| Field | Value |
|---|---|
| Document identifier | SPEC-0001 |
| Revision | 0.1.0-draft.5 |
| Project | noble |
| Language name | Noble as the working name; package and extension naming remain open |
| Status | Canonical working draft for a new implementation; no compiler or completed proof exists |
| Basis | [Preserved source chain](../../../specs/SOURCES.md) |
| Scope of this revision | Typed program contracts, first-class evidence companions, proof composition, and verification tooling |
| Verification | SPEC-V001, SPEC-V002 and IMPL-V001 at 0.1.0-draft.5 |
| Safety | SPEC-S001 at 0.1.0-draft.5 |
| Component interface | SPEC-W001 and SPEC-R001 at 0.1.0-draft.5; full profile conformance remains open |
| Not yet frozen | Complete inference algorithm, module/type-declaration syntax, canonical byte encoding, internal Wasm program ABI, exact WIT type mapping/resource adapters, host/runtime version matrix |

This document is the canonical working core specification. Its requirements apply to the declared draft subsets, not to an existing implementation.

[DEVELOPER-EXPERIENCE.md](../developer-experience/spec.md) adds scoped tooling and library contracts without changing the bootstrap expression grammar or ordinary execution.

[DECISIONS.md](../../../specs/DECISIONS.md) records the selected decisions. [CORE-BOOTSTRAP.md](../core-bootstrap/spec.md) limits the first implementation subset. [ROADMAP.md](../../../specs/ROADMAP.md) defines delivery order.

The user confirmed that Noble will be implemented from scratch. No repository recovery or historical runtime evidence is a prerequisite. Publication does not establish conformance or a soundness proof.

The source RFC remains unchanged. Appendix B maps this document to it. Details newly selected in this revision are recorded in `DECISIONS.md`; those decisions are not represented as already settled by the RFC. External projects named in the RFC remain architectural references, not normative dependencies on their latest behavior.

> **A program is an immutable, typed value with an inspectable composition. Construction of that value and execution of its effects are distinct operations.**

---

## Contents

1. Status, terminology, and conformance
2. Scope and semantic commitments
3. Expression syntax and source units
4. Values, stacks, and type notation
5. Evaluation and observable behavior
6. Program construction and execution
7. Stack operations, data, and control
8. Static checking and effect requirements
9. Resource discipline
10. Definitions and resolution
11. Reflection and checked preparation
12. Structural identity
13. Wasm execution requirements
14. Embedding, host boundaries, and sessions
15. Libraries and deferred features
16. Conformance requirements and release gates

Appendix A. Worked examples  
Appendix B. Source traceability  
Appendix C. Open decisions

---

# Part A — Kernel semantics and types

## 1. Status, terminology, and conformance

### 1.1 Requirement language

**MUST** and **SHALL** state mandatory requirements for the applicable draft subset. **MUST NOT** and **SHALL NOT** state prohibitions. **SHOULD** states a recommendation for which a documented reason may justify a different choice. **MAY** identifies a permitted choice. Explanatory paragraphs marked **Rationale** or **Example** are informative and do not add requirements.

A requirement identifier such as `K-EVAL-01` is stable within this revision series. A future revision may change its text; conformance reports MUST name the revision as well as the identifier.

A section explicitly described as open specifies no final representation, algorithm, or syntax beyond its stated constraints. An implementation MUST NOT advertise an implementation-specific resolution of an open item as a standardized language feature.

### 1.2 Terms

| Term | Meaning in this specification |
|---|---|
| Word | A named or resolved stack-transforming operation. |
| Definition | An immutable checked body with a type scheme and resolved dependencies. |
| Program | A checked, executable value with an input stack, output stack, latent effect bound, and inspectable recipe. |
| Quotation literal | Source syntax `[ ... ]` that constructs a program without running the enclosed body. |
| Recipe | The resolved language-level composition represented by a program, independent of optimizer layout. |
| Syntax | Editable, inert data representing source-oriented structure; it may be unresolved or invalid. |
| Data | An immutable value whose type permits duplication and discard. |
| Resource | An opaque, move-only host handle, or a value containing such a handle. |
| Host operation | An explicitly declared operation implemented outside the language's pure computation. |
| Effect bound | A conservative description of host operations a computation may request. It is not a credential. |
| Preparation | Explicit resolution, checking, and production of executable code from syntax. |
| Submission | The source unit a tool checks before starting that unit's execution. |

A program is not a process, a running task, a persistent workflow, or a Wasm component instance. Those may be represented by host-defined resources in later libraries.

### 1.3 Draft subsets

`Core-Draft` covers the expression microprofile, stack semantics, program builders, basic data/control contracts, and static-checking obligations. `ProgramsData-Draft` adds reflection, immutable resolution, and structural identity invariants. `Resources-Draft` adds the ownership obligations in section 9. `Preparation-Draft` adds the explicit compiler-service contract. `Wasm-Draft` adds the execution requirements in section 13. `Component-Draft` adds the standard WIT/Component Model/WASI boundary defined by section 13.5 and SPEC-W001.

`Contracts-Draft` adds the optional typed contract and first-class companion profile in SPEC-V002. It does not make behavioral proofs mandatory for ordinary programs.

These are development and reporting labels, not different source languages. A report MUST list the subsets it implements. This revision does not permit an unqualified claim of full language or cross-implementation binary conformance: several required release gates remain open.

### Requirement: K-STATUS-01
r[K-STATUS-01]

**K-STATUS-01.** A test report MUST distinguish executed results from expected outcomes and unsupported cases. The fixture file accompanying this draft defines expected behavior; it does not report execution of a language implementation.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

## 2. Scope and semantic commitments

### Requirement: K-SCOPE-01
r[K-SCOPE-01]

**K-SCOPE-01.** The kernel consists of values, ordered stack transformations, program construction and execution, data construction and elimination, definition references, and effect/resource-use rules. Domain objects such as terminals, clusters, deployments, and agents MUST NOT be required kernel syntax or semantics.


<!-- cairn:scenario-links:start -->
#### Scenario: WORKER-11 for K-SCOPE-01

- GIVEN the `Worker-Design` profile and every field of `input` in [WORKER-11](../../../specs/conformance/worker-cases.json)
- WHEN the `review` procedure for case `WORKER-11` runs against those inputs
- THEN the observations match every field of `expected` in case `WORKER-11`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: OCTET-10 for K-SCOPE-01

- GIVEN the `Octet-Adoption-Design` profile and every field of `input` in [OCTET-10](../../../specs/conformance/octet-adoption-cases.json)
- WHEN the `review` procedure for case `OCTET-10` runs against those inputs
- THEN the observations match every field of `expected` in case `OCTET-10`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: K-SCOPE-02
r[K-SCOPE-02]

**K-SCOPE-02.** Pure data and program values MUST be immutable. A transform creates a new value; it cannot mutate an existing checked program or a definition that an invocation already references.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: K-SCOPE-03
r[K-SCOPE-03]

**K-SCOPE-03.** Programs MUST be first-class: they may be arguments, results, and elements of homogeneous data collections when their types permit it. Correct execution MUST NOT depend on a caller recognizing a quotation literal syntactically.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: K-SCOPE-04
r[K-SCOPE-04]

**K-SCOPE-04.** Sequential composition MUST NOT imply concurrency, distribution, transactionality, retry, or persistence. Those behaviors require explicitly specified operations and profiles.


<!-- cairn:scenario-links:start -->
#### Scenario: WORKER-11 for K-SCOPE-04

- GIVEN the `Worker-Design` profile and every field of `input` in [WORKER-11](../../../specs/conformance/worker-cases.json)
- WHEN the `review` procedure for case `WORKER-11` runs against those inputs
- THEN the observations match every field of `expected` in case `WORKER-11`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: K-SCOPE-05
r[K-SCOPE-05]

**K-SCOPE-05.** Constructing or inspecting a program MUST NOT execute its latent host operations, acquire authority, or publish its captured values.

**Rationale.** Smallness means a small number of independent mechanisms. Library vocabulary is not counted as additional syntax merely because it requires an explicit host facility.


<!-- cairn:scenario-links:start -->
#### Scenario: CORE-06 for K-SCOPE-05

- GIVEN the `Core-Bootstrap` profile and every field of `input` in [CORE-06](../../../specs/conformance/cases.json)
- WHEN the `runtime` procedure for case `CORE-06` runs against those inputs
- THEN the observations match every field of `expected` in case `CORE-06`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: WORKER-12 for K-SCOPE-05

- GIVEN the `Worker-Design` profile and every field of `input` in [WORKER-12](../../../specs/conformance/worker-cases.json)
- WHEN the `runtime` procedure for case `WORKER-12` runs against those inputs
- THEN the observations match every field of `expected` in case `WORKER-12`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### 2.1 Verification commitment

### Requirement: K-VERIFY-01
r[K-VERIFY-01]

**K-VERIFY-01.** Formal verification SHALL be a core development commitment under [SPEC-V001](../verification/spec.md). Ordinary programs MUST retain the existing source forms and `Program<S,T,e>` interface. Handwritten behavioral proofs, a runtime prover, dependent types, and new proof punctuation are not required by this commitment.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: K-VERIFY-02
r[K-VERIFY-02]

**K-VERIFY-02.** Reports MUST distinguish mechanized language metatheory, verification of actual Rust implementation functions, optional behavioral proofs about Noble programs, and correspondence to executed Wasm. A proof for one target MUST NOT be reported as establishing the others.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: K-VERIFY-03
r[K-VERIFY-03]

**K-VERIFY-03.** Noble MUST support the optional `Contracts-Draft` profile with typed declarations, inspectable evidence companions, and compositional proof operations under SPEC-V002. The profile MUST preserve ordinary program identity, recursive resource eligibility, and explicit host authority. Its concrete grammar and representation require the VC-GATE-01 design before implementation acceptance.

Lean 4 supplies the reference semantics and metatheory. Noble-owned production Rust targets Charon → Aeneas → Lean refinement. This route is mandatory for the entire semantic kernel, including resource-state decisions. Unsupported external effects require explicit boundaries, not silent verification exclusions. Verus is optional for reviewed non-kernel exceptions only. These tools operate at proof/build time, not as a second production Noble evaluator. See [IMPL-V001](../verification-toolchain/spec.md).



<!-- cairn:scenario-links:start -->
#### Scenario: CONTRACT-10 for K-VERIFY-03

- GIVEN the `Contracts-Draft` profile and every field of `input` in [CONTRACT-10](../../../specs/conformance/contract-cases.json)
- WHEN the `runtime` procedure for case `CONTRACT-10` runs against those inputs
- THEN the observations match every field of `expected` in case `CONTRACT-10`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### 2.2 Safety commitment

### Requirement: K-SAFE-01
r[K-SAFE-01]

**K-SAFE-01.** Noble guest source MUST NOT contain a construct whose purpose is to bypass the core stack/type checker, resource eligibility and ownership rules, effect accounting, host authorization, or checked representation boundary. Machine-level unsafety may exist only behind implementation/host boundaries governed by SPEC-S001.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: K-SAFE-02
r[K-SAFE-02]

**K-SAFE-02.** Undefined behavior is not a Noble-language execution outcome. An accepted program may normally return, wait where the profile permits it, diverge, or encounter a specified trap/profile failure. Representation, handle, bounds, and boundary failures MUST be rejected or become specified failures rather than arbitrary language semantics.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: K-SAFE-03
r[K-SAFE-03]

**K-SAFE-03.** Raw native addresses and unchecked pointer/representation casts are not ordinary Noble guest values or operations. An integer, bytes value, program/artifact identity, syntax value, or serialized payload MUST NOT be reinterpreted by guest code as live authority or a native pointer.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: K-SAFE-04
r[K-SAFE-04]

**K-SAFE-04.** A stable Noble-safe claim SHALL include the applicable language, resource/authority, host-boundary, and verification obligations in SPEC-S001. Standard-concurrency safety additionally requires the selected concurrency profile's isolation, scope, protocol, and capability guarantees.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: K-SAFE-05
r[K-SAFE-05]

**K-SAFE-05.** These safety commitments do not prohibit future dependent types, higher-rank polymorphism, principled subtyping, refinements, or extensible inference. Such features must preserve or strengthen the applicable safety invariants before joining a stable-safe subset.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

## 3. Expression syntax and source units

### 3.1 Expression microprofile

This revision adopts the following expression grammar. Its concrete lexical rules are new draft decisions, not a claim that the RFC had already frozen punctuation.

```ebnf
body        = { expression } ;
expression  = integer | boolean | text | word | quotation ;
quotation   = "[", body, "]" ;
boolean     = "true" | "false" ;
integer     = [ "-" ], ( "0" | nonzero-digit, { digit } ) ;
```

Whitespace and comments may occur between tokens. Square brackets and double quotes are delimiters. A quotation is one expression; it may contain nested quotations. An empty body is valid.

A source unit is UTF-8. Outside text literals, ASCII space, tab, carriage return, and line feed separate tokens. Non-ASCII whitespace is not silently normalized.

### Requirement: K-SYN-04
r[K-SYN-04]

**K-SYN-04.** The lexer MUST apply the following comment rules. Outside text literals, `#` begins a comment through the next carriage return, line feed, or end of input. It terminates a preceding token even without whitespace. Inside a text literal, `#` is ordinary text. This revision does not support block comments.

A word token begins with an ASCII letter, underscore, or one of `+ - * / = < > ? !`. Its remaining characters can additionally include ASCII digits and `.`. The maximal token is scanned before classification. A complete integer token takes precedence over word classification. The spellings `def`, `true`, and `false` are reserved.


<!-- cairn:scenario-links:start -->
#### Scenario: CORE-11 for K-SYN-04

- GIVEN the `Core-Bootstrap` profile and every field of `input` in [CORE-11](../../../specs/conformance/cases.json)
- WHEN the `runtime` procedure for case `CORE-11` runs against those inputs
- THEN the observations match every field of `expected` in case `CORE-11`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: CORE-12 for K-SYN-04

- GIVEN the `Core-Bootstrap` profile and every field of `input` in [CORE-12](../../../specs/conformance/cases.json)
- WHEN the `runtime` procedure for case `CORE-12` runs against those inputs
- THEN the observations match every field of `expected` in case `CORE-12`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: K-SYN-05
r[K-SYN-05]

**K-SYN-05.** `run` and `reflect` are the standard operation names. `call` and `reify` are not compatibility aliases. `//` is an ordinary word token, not a comment opener. An explicit user definition can bind these nonreserved word spellings. An unbound spelling produces a resolution error. Tools can offer migration diagnostics but MUST NOT silently rewrite source meaning.

Examples of word tokens are `square`, `fs.read`, `healthy?`, `list.case`, `+`, and `-`. `1+` is not shorthand for `1 +` and is rejected. A negative integer is one literal; subtraction is the separate word `-`.


<!-- cairn:scenario-links:start -->
#### Scenario: CORE-13 for K-SYN-05

- GIVEN the `Core-Bootstrap` profile and every field of `input` in [CORE-13](../../../specs/conformance/cases.json)
- WHEN the `static` procedure for case `CORE-13` runs against those inputs
- THEN the observations match every field of `expected` in case `CORE-13`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: K-SYN-01
r[K-SYN-01]

**K-SYN-01.** Parsing MUST preserve the three semantic expression forms: literal, resolved word invocation, and quotation. There is no infix precedence, implicit process invocation, or shell expansion in this microprofile.


<!-- cairn:scenario-links:start -->
#### Scenario: CORE-18 for K-SYN-01

- GIVEN the `Core-Bootstrap` profile and every field of `input` in [CORE-18](../../../specs/conformance/cases.json)
- WHEN the `static` procedure for case `CORE-18` runs against those inputs
- THEN the observations match every field of `expected` in case `CORE-18`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### 3.2 Literals

An integer literal denotes an `I64` value and MUST be in the inclusive range `-9223372036854775808` through `9223372036854775807`. An out-of-range literal is a static diagnostic; it is not wrapped during parsing.

A text literal is delimited by double quotes. Its content is a sequence of Unicode scalar values. Raw line terminators and invalid UTF-8 are rejected. The supported escapes are `\"`, `\\`, `\n`, `\r`, `\t`, and `\u{H}`, where `H` is one through six hexadecimal digits denoting a Unicode scalar value. Surrogate code points and values above `10FFFF` are rejected. Text values are not implicitly Unicode-normalized.

This revision defines a `Bytes` type but does not select a source literal syntax for it. It can be supplied through typed data constructors or host bindings in a profile. Floats, arbitrary-precision numeric literals, interpolation, and raw strings are outside this microprofile.

### 3.3 Definitions and submissions

The following form is adopted for unannotated, nonrecursive definitions:

```text
def square [ dup * ]
```

A declaration is not an expression that modifies a runtime dictionary. In this draft, a tooling submission is either one definition declaration or one expression body; a script runner may submit a sequence of units. The complete module grammar, imports, and type-annotation concrete syntax remain open.

### Requirement: K-SYN-02
r[K-SYN-02]

**K-SYN-02.** A definition submission MUST be checked without executing the body. A tool MUST install the name mapping only after successful checking and preparation according to its environment policy.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: K-SYN-03
r[K-SYN-03]

**K-SYN-03.** A tool MUST determine the whole submission boundary before executing its expression body. A static error anywhere in that submission prevents execution of all of that submission's body, including effects textually preceding the error.

Previously completed submissions are not rolled back. A runner that checks a whole file as one unit must document that larger boundary.


<!-- cairn:scenario-links:start -->
#### Scenario: CORE-08 for K-SYN-03

- GIVEN the `Core-Bootstrap` profile and every field of `input` in [CORE-08](../../../specs/conformance/cases.json)
- WHEN the `static` procedure for case `CORE-08` runs against those inputs
- THEN the observations match every field of `expected` in case `CORE-08`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

## 4. Values, stacks, and type notation

### 4.1 Stacks

The stack top is written on the right. Juxtaposition of stack elements means ordered stack extension, not a tuple value.

```text
stack shape S ::= empty | rho | S a
runtime stack s ::= [] | s v
```

Here `a` is a value type, `v` a value, and `rho` a stack-tail variable. A stack-tail variable stands for a finite ordered prefix of zero or more values. It is not an untyped container and cannot be inspected by a word that merely threads it onward.

A signature has the form:

```text
S -- T ! e
```

`S` and `T` are input and output stack shapes. `e` is an effect bound. For readability, examples sometimes omit a common untouched prefix:

```text
I64 I64 -- I64
```

means `rho I64 I64 -- rho I64 ! {}`.

### Requirement: K-TYPE-01
r[K-TYPE-01]

**K-TYPE-01.** Input and output stack shapes MUST be checked in order and at their full arity. An unmentioned prefix MUST pass through without being implicitly read, duplicated, or discarded.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### 4.2 Types and schemes

The draft uses these semantic forms:

```text
a, b ::= I64 | Bool | Bytes | Text | Unit
       | Pair<a,b> | Sum<a,b> | List<a>
       | Program<S,T,e> | Syntax
       | Resource<ResourceTypeId>
       | DeclaredType<TypeId, arguments...>
       | alpha

e, f ::= {} | {HostOpId, ...} | epsilon | union(e,f)
```

`DeclaredType` represents the single algebraic-data mechanism whose full declaration syntax remains open. `Pair`, `Sum`, and `List` are bootstrap data schemas, not separate expression forms or a commitment that every schema requires a compiler special case. `Option` and `Result` are ordinary variants supplied by schemas, not privileged control effects.

A named definition may have a rank-1 type scheme quantifying value-type, stack-tail, and effect variables, together with built-in eligibility constraints. First-class program values have instantiated interfaces. This revision does not introduce higher-rank polymorphism, impredicative types, a dynamic `Any`, general subtyping, or user-defined inference rules.

### 4.3 Eligibility predicates

The checker uses built-in predicates rather than an extensible trait-solving language:

| Predicate | Meaning |
|---|---|
| `Data(a)` | Values of `a` may be duplicated and discarded. |
| `Capture(a)` | Values of `a` are data with a defined portable semantic representation and may be embedded in a program recipe. |

The scalar types, `Unit`, and `Syntax` satisfy both predicates. An algebraic type satisfies a predicate only when every possible payload permitted by its instantiated schema satisfies that predicate. `List<a>` inherits the predicate from `a`.

Every valid v0 `Program<S,T,e>` is data and capturable: its own environment is restricted to capturable data. This remains true when the program's *input interface* mentions a resource. A reusable operation that expects a resource does not itself own that resource.

No live resource type satisfies either predicate. Wrapping a resource in a pair, variant, list, or user-defined record does not remove that restriction. For example, `Sum<Resource<R>,I64>` is not data merely because its present value happens to select the integer alternative; elimination can expose the data branch at its narrower payload type.

### Requirement: K-TYPE-02
r[K-TYPE-02]

**K-TYPE-02.** The checker MUST enforce eligibility recursively. A representation change or opaque wrapper MUST NOT launder a resource into capturable data.

A portable semantic representation is defined abstractly in section 11. It does not promise an interoperable binary encoding; section 12 explicitly leaves that encoding open.


<!-- cairn:scenario-links:start -->
#### Scenario: CORE-15 for K-TYPE-02

- GIVEN the `Core-Bootstrap` profile and every field of `input` in [CORE-15](../../../specs/conformance/cases.json)
- WHEN the `static` procedure for case `CORE-15` runs against those inputs
- THEN the observations match every field of `expected` in case `CORE-15`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: ADAPT-02 for K-TYPE-02

- GIVEN the `Resources-Draft` profile and every field of `input` in [ADAPT-02](../../../specs/conformance/adaptation-cases.json)
- WHEN the `static` procedure for case `ADAPT-02` runs against those inputs
- THEN the observations match every field of `expected` in case `ADAPT-02`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: ADAPT-03 for K-TYPE-02

- GIVEN the `Core-Bootstrap` profile and every field of `input` in [ADAPT-03](../../../specs/conformance/adaptation-cases.json)
- WHEN the `admission` procedure for case `ADAPT-03` runs against those inputs
- THEN the observations match every field of `expected` in case `ADAPT-03`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: WORKER-02 for K-TYPE-02

- GIVEN the `Worker-Design` profile and every field of `input` in [WORKER-02](../../../specs/conformance/worker-cases.json)
- WHEN the `static` procedure for case `WORKER-02` runs against those inputs
- THEN the observations match every field of `expected` in case `WORKER-02`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: OCTET-03 for K-TYPE-02

- GIVEN the `Octet-Adoption-Design` profile and every field of `input` in [OCTET-03](../../../specs/conformance/octet-adoption-cases.json)
- WHEN the `static` procedure for case `OCTET-03` runs against those inputs
- THEN the observations match every field of `expected` in case `OCTET-03`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### 4.4 Numeric semantics

### Requirement: K-NUM-01
r[K-NUM-01]

**K-NUM-01.** In this draft, `+`, `-`, and `*` operate on two `I64` values and wrap modulo `2^64`, interpreted back as a signed 64-bit value. Operand order is the deeper value followed by the top value. The result MUST NOT depend on debug versus optimized compilation.

For a mathematical result `z`, the signed result is:

```text
((z + 2^63) mod 2^64) - 2^63
```

`=` in the bootstrap numeric vocabulary compares two `I64` values and returns `Bool`. It is not an implicit generic equality operator for every type. Division, floating point, and the detailed checked-arithmetic library are not selected in this revision.

**Draft decision D-03.** Wrapping arithmetic selects a candidate from the RFC. Checked operations returning `Result` remain the intended library complement; their names and full contract are deferred.


<!-- cairn:scenario-links:start -->
#### Scenario: CORE-14 for K-NUM-01

- GIVEN the `Core-Bootstrap` profile and every field of `input` in [CORE-14](../../../specs/conformance/cases.json)
- WHEN the `runtime` procedure for case `CORE-14` runs against those inputs
- THEN the observations match every field of `expected` in case `CORE-14`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

## 5. Evaluation and observable behavior

### 5.1 Evaluation order

### Requirement: K-EVAL-01
r[K-EVAL-01]

**K-EVAL-01.** An expression body MUST execute left to right. A literal pushes its value. Invoking a word performs its declared stack transformation. A quotation pushes a program value without executing its enclosed body.


<!-- cairn:scenario-links:start -->
#### Scenario: CORE-02 for K-EVAL-01

- GIVEN the `Core-Bootstrap` profile and every field of `input` in [CORE-02](../../../specs/conformance/cases.json)
- WHEN the `runtime` procedure for case `CORE-02` runs against those inputs
- THEN the observations match every field of `expected` in case `CORE-02`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: K-EVAL-02
r[K-EVAL-02]

**K-EVAL-02.** A run MUST use the resolved references in its program. It MUST NOT perform a fresh lookup of source names in a mutable namespace.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: K-EVAL-03
r[K-EVAL-03]

**K-EVAL-03.** Evaluation of one word MUST finish normally before the next sequential word begins. Suspending in an explicitly asynchronous host operation does not authorize reordering subsequent words in the same sequential computation.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### 5.2 Abstract operational model

The following rules define behavior, not an interpreter implementation. Let:

```text
<C ; s ; H ; t>
```

be a configuration with pending code `C`, data stack `s`, host state `H`, and observable host-event trace `t`. Let `D` be the immutable store of resolved definitions. A semicolon inside `C` denotes the remaining sequence.

Representative steps are:

```text
<Literal(v); C ; s ; H ; t>
    -> <C ; s v ; H ; t>

<Quotation(B); C ; s ; H ; t>
    -> <C ; s program(B) ; H ; t>

<Invoke(d); C ; s ; H ; t>
    -> <D[d].body; C ; s ; H ; t>

<run; C ; s p ; H ; t>
    -> <recipe(p); C ; s ; H ; t>

<compose; C ; s p q ; H ; t>
    -> <C ; s composed(p,q) ; H ; t>

<quote; C ; s v ; H ; t>
    -> <C ; s push-program(v) ; H ; t>
```

The checker has already verified the interfaces and constraints for these steps. Recipe expansion in this mathematical notation does not require an AST evaluator in production; a compiled Wasm call must have the same specified behavior.

A host step consumes the arguments specified by its declared interface, makes the authorized request, and returns its declared results, possibly with a changed host state and an appended event trace. Exact host event formats belong to the host contract. A host's nondeterministic response is not silently treated as a pure function result.

### 5.3 Outcomes and sequencing laws

Normal execution yields an output stack and a host trace. Execution may instead diverge or terminate abnormally. Ordinary domain failures represented as `Result` values are normal execution, not automatic traps.

### Requirement: K-EVAL-04
r[K-EVAL-04]

**K-EVAL-04.** If `p` terminates abnormally, the second program `q` in `compose(p,q)` MUST NOT start. Earlier host effects are not rolled back by the language.

The composition laws use failure-propagating sequencing:

```text
run(empty, s) = success(s)
run(compose(p,q), s) = run(p,s) then-on-success run(q)
run(quote(v), s) = success(s v)
```

Host traces concatenate in order. Where both executions terminate normally under the same host responses, composition is associative and the empty program is an identity. These laws do not equate elapsed time, allocation strategy, or behavior under different resource quotas. Recipe associativity is separately specified in section 11.


<!-- cairn:scenario-links:start -->
#### Scenario: CORE-17 for K-EVAL-04

- GIVEN the `Core-Bootstrap` profile and every field of `input` in [CORE-17](../../../specs/conformance/cases.json)
- WHEN the `runtime` procedure for case `CORE-17` runs against those inputs
- THEN the observations match every field of `expected` in case `CORE-17`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### 5.4 Errors and limits

### Requirement: K-ERROR-01
r[K-ERROR-01]

**K-ERROR-01.** A static rejection MUST prevent execution of the rejected submission. At minimum, tools MUST distinguish parsing, resolution, stack/type incompatibility, eligibility/resource-use violations, and effect-bound violations in their diagnostics. Exact wording and numeric diagnostic codes are not frozen.


<!-- cairn:scenario-links:start -->
#### Scenario: CORE-18 for K-ERROR-01

- GIVEN the `Core-Bootstrap` profile and every field of `input` in [CORE-18](../../../specs/conformance/cases.json)
- WHEN the `static` procedure for case `CORE-18` runs against those inputs
- THEN the observations match every field of `expected` in case `CORE-18`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: K-ERROR-02
r[K-ERROR-02]

**K-ERROR-02.** A well-checked invocation with conforming host bindings MUST NOT encounter a language-level stack underflow, wrong operand type, or use of a consumed resource on a normal path. Those outcomes indicate an implementation or binding defect, not a permitted fallback to dynamic typing.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: K-ERROR-03
r[K-ERROR-03]

**K-ERROR-03.** Empty effect bounds do not guarantee termination or freedom from allocation failure, quotas, traps, or side channels. Such failures MUST NOT be misreported as ordinary successful results.

A host/execution profile specifies handling of traps, cancellation, and resource exhaustion. This draft does not specify recoverable exceptions or user-defined continuation handlers.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

## 6. Program construction and execution

### 6.1 Program interface

A program has type `Program<S,T,e>`. `S` and `T` describe the stack at invocation; they do not describe the current stack on which the program value is constructed. `e` is latent until invocation.

### Requirement: K-PROG-01
r[K-PROG-01]

**K-PROG-01.** A checked program MUST carry or be associated with sufficient trusted type and executable information to support its interface, and sufficient recipe information for reflection. These need not be stored redundantly or in a particular memory layout.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### 6.2 Quotation literal

If body `B` checks as `S -- T ! e`, the expression `[ B ]` has effect-free construction behavior:

```text
R -- R Program<S,T,e> ! {}
```

`R` is the surrounding construction stack. It is independent of the body's invocation stack.

### Requirement: K-PROG-02
r[K-PROG-02]

**K-PROG-02.** Quotation literals MUST NOT capture the current data stack implicitly. Missing inputs in a quotation body become requirements of its program interface; they are not obtained from values underneath the quotation during construction.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: K-PROG-03
r[K-PROG-03]

**K-PROG-03.** The entire quotation body MUST check even when the quotation is never called or is later discarded. Unchecked code is represented as `Syntax`, not as a quotation literal accepted without checking.


<!-- cairn:scenario-links:start -->
#### Scenario: CORE-08 for K-PROG-03

- GIVEN the `Core-Bootstrap` profile and every field of `input` in [CORE-08](../../../specs/conformance/cases.json)
- WHEN the `static` procedure for case `CORE-08` runs against those inputs
- THEN the observations match every field of `expected` in case `CORE-08`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### 6.3 `run`

```text
run : S Program<S,T,e> -- T ! e
```

`run` consumes the program value and invokes it against the remaining stack. Consuming a reusable data value does not invalidate any independently duplicated copy of it.

### Requirement: K-PROG-04
r[K-PROG-04]

**K-PROG-04.** `run` MUST invoke an already prepared executable program. It MUST NOT implicitly compile source, fetch code, install interfaces, acquire capabilities, or reinterpret arbitrary data as instructions. An implementation may use its Wasm engine's ordinary execution preparation internally; that is not a hidden call to the language's `code.prepare` service.


<!-- cairn:scenario-links:start -->
#### Scenario: CORE-01 for K-PROG-04

- GIVEN the `Core-Bootstrap` profile and every field of `input` in [CORE-01](../../../specs/conformance/cases.json)
- WHEN the `runtime` procedure for case `CORE-01` runs against those inputs
- THEN the observations match every field of `expected` in case `CORE-01`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### 6.4 `compose`

```text
compose : R Program<A,B,e> Program<B,C,f>
          -- R Program<A,C,union(e,f)> ! {}
```

The deeper program runs first, then the top program. `compose` produces a new program value. It does not run either operand.

### Requirement: K-PROG-05
r[K-PROG-05]

**K-PROG-05.** The shared intermediate stack `B` MUST unify completely, including resource types and positions. An incompatible pair is rejected statically, not converted to a runtime compatibility test.


<!-- cairn:scenario-links:start -->
#### Scenario: CORE-02 for K-PROG-05

- GIVEN the `Core-Bootstrap` profile and every field of `input` in [CORE-02](../../../specs/conformance/cases.json)
- WHEN the `runtime` procedure for case `CORE-02` runs against those inputs
- THEN the observations match every field of `expected` in case `CORE-02`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: CORE-07 for K-PROG-05

- GIVEN the `Core-Bootstrap` profile and every field of `input` in [CORE-07](../../../specs/conformance/cases.json)
- WHEN the `static` procedure for case `CORE-07` runs against those inputs
- THEN the observations match every field of `expected` in case `CORE-07`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: K-PROG-06
r[K-PROG-06]

**K-PROG-06.** Composing values selected or constructed at runtime MUST work over their statically known interfaces without a source compiler service. A literal-only implementation does not conform.


<!-- cairn:scenario-links:start -->
#### Scenario: CORE-03 for K-PROG-06

- GIVEN the `Core-Bootstrap` profile and every field of `input` in [CORE-03](../../../specs/conformance/cases.json)
- WHEN the `runtime` procedure for case `CORE-03` runs against those inputs
- THEN the observations match every field of `expected` in case `CORE-03`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: CORE-04 for K-PROG-06

- GIVEN the `Core-Bootstrap` profile and every field of `input` in [CORE-04](../../../specs/conformance/cases.json)
- WHEN the `runtime` procedure for case `CORE-04` runs against those inputs
- THEN the observations match every field of `expected` in case `CORE-04`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: WORKER-03 for K-PROG-06

- GIVEN the `Worker-Design` profile and every field of `input` in [WORKER-03](../../../specs/conformance/worker-cases.json)
- WHEN the `runtime` procedure for case `WORKER-03` runs against those inputs
- THEN the observations match every field of `expected` in case `WORKER-03`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### 6.5 `quote`

```text
quote : R a -- R Program<S,S a,{}> ! {}
        where Capture(a)
```

`quote` captures the top value explicitly and produces a program that pushes that value when called. The fresh invocation prefix `S` is unrelated to the construction prefix `R` unless later type constraints relate them.

### Requirement: K-PROG-07
r[K-PROG-07]

**K-PROG-07.** `quote` MUST accept only capturable data, MUST preserve the captured value immutably, and MUST NOT publish or otherwise export it. A program may contain private data, including secrets represented as ordinary text; capturability is not a confidentiality guarantee.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: K-PROG-08
r[K-PROG-08]

**K-PROG-08.** A live resource MUST NOT be captured, including through a containing value or a program environment. A quotation may instead require that resource as an invocation input.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### 6.6 Ordinary higher-order definitions

```text
def twice [ dup compose ]
```

Its schematic scheme is:

```text
forall S e R.
R Program<S,S,e> -- R Program<S,S,e> ! {}
```

The resulting program runs its input twice; its effect bound remains `e` because sets are idempotent. An effect bound records possible kinds of operations, not the number of occurrences.

**Example.** `20 [ 1 + ] twice run` yields `22`. This definition is not applicable to every program; it requires matching input and output stack shapes.

### 6.7 Runtime-selected interfaces and transition libraries

### Requirement: K-PROG-09
r[K-PROG-09]

**K-PROG-09.** A runtime-selected program MUST retain one statically established, instantiated interface. Selection MUST NOT erase stack arity, schema identity, resource obligations, or effect bounds. Heterogeneous programs require explicit typed adapters or exhaustive variants before they share a collection or dispatch interface.


<!-- cairn:scenario-links:start -->
#### Scenario: WORKER-01 for K-PROG-09

- GIVEN the `Worker-Design` profile and every field of `input` in [WORKER-01](../../../specs/conformance/worker-cases.json)
- WHEN the `runtime` procedure for case `WORKER-01` runs against those inputs
- THEN the observations match every field of `expected` in case `WORKER-01`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: WORKER-02 for K-PROG-09

- GIVEN the `Worker-Design` profile and every field of `input` in [WORKER-02](../../../specs/conformance/worker-cases.json)
- WHEN the `static` procedure for case `WORKER-02` runs against those inputs
- THEN the observations match every field of `expected` in case `WORKER-02`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: WORKER-03 for K-PROG-09

- GIVEN the `Worker-Design` profile and every field of `input` in [WORKER-03](../../../specs/conformance/worker-cases.json)
- WHEN the `runtime` procedure for case `WORKER-03` runs against those inputs
- THEN the observations match every field of `expected` in case `WORKER-03`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: K-PROG-10
r[K-PROG-10]

**K-PROG-10.** An adapter MUST obey ordinary checking and ownership rules. Representation equality MUST NOT establish schema compatibility. A failed interface match MUST NOT trigger argument guessing, an implicit conversion, or an `Any` fallback.

A library can describe a pure transition with this schematic interface:

```text
State Event -- State List<Action> ! {}
```

`State`, `Event`, and `Action` denote application-owned, resolved data schemas. They are not kernel types or new syntax. The shell supplies each event and retains the explicit state between invocations. Repeated shell invocation does not require guest recursion or a captured continuation.

For this resource-free example, all three schemas satisfy `Data` and `Capture`. An action is an inert request description, not a capability or an execution receipt. The shell admits each action under its own authority policy. Time, model responses, and other external observations enter as explicit inputs.

Resource-bearing programs remain possible through explicit resource inputs and ownership-preserving results. This example does not permit a live resource inside a capturable worker environment. [Worker conformance](../../../specs/WORKER-CONFORMANCE.md) defines a bounded application of these existing mechanisms.


<!-- cairn:scenario-links:start -->
#### Scenario: WORKER-01 for K-PROG-10

- GIVEN the `Worker-Design` profile and every field of `input` in [WORKER-01](../../../specs/conformance/worker-cases.json)
- WHEN the `runtime` procedure for case `WORKER-01` runs against those inputs
- THEN the observations match every field of `expected` in case `WORKER-01`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: WORKER-02 for K-PROG-10

- GIVEN the `Worker-Design` profile and every field of `input` in [WORKER-02](../../../specs/conformance/worker-cases.json)
- WHEN the `static` procedure for case `WORKER-02` runs against those inputs
- THEN the observations match every field of `expected` in case `WORKER-02`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: WORKER-03 for K-PROG-10

- GIVEN the `Worker-Design` profile and every field of `input` in [WORKER-03](../../../specs/conformance/worker-cases.json)
- WHEN the `runtime` procedure for case `WORKER-03` runs against those inputs
- THEN the observations match every field of `expected` in case `WORKER-03`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

## 7. Stack operations, data, and control

### 7.1 Stack vocabulary

The following operations define the draft behavior. Whether an implementation lowers one directly or derives it from a smaller internal basis is not observable here.

```text
dup  : S a -- S a a ! {}        where Data(a)
drop : S a -- S ! {}            where Data(a)
swap : S a b -- S b a ! {}
dip  : S a Program<S,T,e> -- T a ! e
```

`dip` holds `a` outside the called program's visible stack, runs the program, and restores `a` above its normal output. Unlike `dup` and `drop`, `swap` and `dip` do not require data eligibility: they may move a resource without duplicating or discarding it.

### Requirement: K-STACK-01
r[K-STACK-01]

**K-STACK-01.** A value held by `dip` MUST be inaccessible to the enclosed program. It MUST be restored exactly once after normal return. On an abnormal outcome, a held resource remains subject to host abort cleanup; it is not silently resurrected as a valid session value.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### 7.2 Products and sums

Bootstrap schemas have the following stack operations:

```text
unit   : S -- S Unit ! {}
pair   : S a b -- S Pair<a,b> ! {}
unpair : S Pair<a,b> -- S a b ! {}
inl    : S a -- S Sum<a,b> ! {}
inr    : S b -- S Sum<a,b> ! {}

case : S Sum<a,b>
       Program<S a,T,e> Program<S b,T,f>
       -- T ! union(e,f)
```

The selected payload is moved to the chosen branch. The left program is used for `inl`, the right for `inr`. Both branches have the same output stack shape. The unselected program is safe to discard because all v0 program environments contain only capturable data.

Branch-local payload types do not introduce general union subtyping. Different output types require explicit sum construction before a common join. SPEC-B001 defines branch diagnostics and independently checked derivations.

### Requirement: K-DATA-01
r[K-DATA-01]

**K-DATA-01.** Constructing and eliminating data MUST move its payloads without implicit duplication or loss of resources. No hidden mutable cell is introduced by constructing a pair or variant.


<!-- cairn:scenario-links:start -->
#### Scenario: ADAPT-02 for K-DATA-01

- GIVEN the `Resources-Draft` profile and every field of `input` in [ADAPT-02](../../../specs/conformance/adaptation-cases.json)
- WHEN the `static` procedure for case `ADAPT-02` runs against those inputs
- THEN the observations match every field of `expected` in case `ADAPT-02`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### 7.3 Boolean branching

The bootstrap `if` contract is:

```text
if : S Bool Program<S,T,e> Program<S,T,f>
     -- T ! union(e,f)
```

The first program runs for `true`; the second runs for `false`. This is a word, not a fourth expression form. It may be derived through the boolean schema and a sum eliminator.

### Requirement: K-CONTROL-01
r[K-CONTROL-01]

**K-CONTROL-01.** Only the selected branch executes, but both branches MUST type-check and their requirements MUST contribute to the conservative effect bound. A resource may follow different valid operations in the branches, but each branch must account for every incoming ownership obligation and return the same interface.


<!-- cairn:scenario-links:start -->
#### Scenario: ADAPT-01 for K-CONTROL-01

- GIVEN the `Core-Bootstrap` profile and every field of `input` in [ADAPT-01](../../../specs/conformance/adaptation-cases.json)
- WHEN the `static` procedure for case `ADAPT-01` runs against those inputs
- THEN the observations match every field of `expected` in case `ADAPT-01`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: ADAPT-02 for K-CONTROL-01

- GIVEN the `Resources-Draft` profile and every field of `input` in [ADAPT-02](../../../specs/conformance/adaptation-cases.json)
- WHEN the `static` procedure for case `ADAPT-02` runs against those inputs
- THEN the observations match every field of `expected` in case `ADAPT-02`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### 7.4 Recursive collections

`List<a>` is an ordinary recursive schema with empty and nonempty cases. Bootstrap contracts are:

```text
nil  : S -- S List<a> ! {}
cons : S a List<a> -- S List<a> ! {}

list.case : S List<a>
            Program<S,T,e>
            Program<S a List<a>,T,f>
            -- T ! union(e,f)
```

For a nonempty list, the head is pushed before the tail, so the tail is on top. For an empty list, the empty-case program receives no payload. Payload order is normative.

### Requirement: K-DATA-02
r[K-DATA-02]

**K-DATA-02.** Lists of compatible programs MUST be supported. Their elements share one instantiated program interface; a list is not a means to erase heterogeneous stack effects into `Any`.

General `map`, `fold`, `filter`, and iteration libraries are not defined by this revision. Their later definitions should use the same data and program mechanisms.


<!-- cairn:scenario-links:start -->
#### Scenario: CORE-05 for K-DATA-02

- GIVEN the `Core-Bootstrap` profile and every field of `input` in [CORE-05](../../../specs/conformance/cases.json)
- WHEN the `runtime` procedure for case `CORE-05` runs against those inputs
- THEN the observations match every field of `expected` in case `CORE-05`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

## 8. Static checking and effect requirements

### 8.1 Judgments

Write:

```text
Gamma ; Constraints |- body : S -- T ! e
```

for checking a body under resolved signatures `Gamma` and built-in eligibility/effect constraints. Sequencing connects matching intermediate stack shapes; quotation introduction separates construction effects from body effects; word invocation instantiates its rank-1 scheme with fresh variables before unification.

Representative rules are:

```text
EMPTY
    Gamma |- []body : S -- S ! {}

SEQUENCE
    Gamma |- p : A -- B ! e
    Gamma |- q : B -- C ! f
    ------------------------------------------
    Gamma |- p q : A -- C ! union(e,f)

QUOTATION
    Gamma |- B : A -- B' ! e
    ------------------------------------------------
    Gamma |- [ B ] : R -- R Program<A,B',e> ! {}
```

`[]body` in the first rule is an empty expression sequence, not a source quotation value.

### Requirement: K-CHECK-01
r[K-CHECK-01]

**K-CHECK-01.** Word schemes MUST be instantiated freshly at independent named uses. Values duplicated from one first-class program share its instantiated type constraints; duplication MUST NOT grant two independent polymorphic instantiations of a monomorphic value.


<!-- cairn:scenario-links:start -->
#### Scenario: CORE-04 for K-CHECK-01

- GIVEN the `Core-Bootstrap` profile and every field of `input` in [CORE-04](../../../specs/conformance/cases.json)
- WHEN the `runtime` procedure for case `CORE-04` runs against those inputs
- THEN the observations match every field of `expected` in case `CORE-04`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: K-CHECK-02
r[K-CHECK-02]

**K-CHECK-02.** Nonrecursive named definitions may generalize eligible variables not fixed by their environment. Recursive definitions require an explicit signature and check recursive calls against that signature without polymorphic recursion. The full inference algorithm and formal soundness argument are required follow-up work; this draft does not claim them established.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: K-CHECK-03
r[K-CHECK-03]

**K-CHECK-03.** Implementations MUST reject ill-founded type equations rather than constructing infinite stack or value types implicitly. Whether the complete solver uses a specific unification algorithm is not frozen.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### 8.2 Effects

Effect bounds are conservative sets of stable host-operation identities, with variables in generic signatures. Union is associative, commutative, and idempotent as a *summary*. Execution order remains sequential and noncommutative.

### Requirement: K-EFFECT-01
r[K-EFFECT-01]

**K-EFFECT-01.** A program's effect bound MUST include every host-operation requirement that its body may perform under the declared contracts. Construction, `compose`, `quote`, and `reflect` do not inherit the latent effects of the programs they manipulate.


<!-- cairn:scenario-links:start -->
#### Scenario: CORE-06 for K-EFFECT-01

- GIVEN the `Core-Bootstrap` profile and every field of `input` in [CORE-06](../../../specs/conformance/cases.json)
- WHEN the `runtime` procedure for case `CORE-06` runs against those inputs
- THEN the observations match every field of `expected` in case `CORE-06`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: OCTET-07 for K-EFFECT-01

- GIVEN the `Octet-Adoption-Design` profile and every field of `input` in [OCTET-07](../../../specs/conformance/octet-adoption-cases.json)
- WHEN the `admission` procedure for case `OCTET-07` runs against those inputs
- THEN the observations match every field of `expected` in case `OCTET-07`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: K-EFFECT-02
r[K-EFFECT-02]

**K-EFFECT-02.** A checker may widen an effect bound conservatively, but MUST NOT narrow it without establishing that all possible body requirements remain included. This restricted effect-bound relation is not general value subtyping.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: K-EFFECT-03
r[K-EFFECT-03]

**K-EFFECT-03.** Calling a program propagates its latent bound. Invoking a higher-order word propagates the effects of programs it may call according to that word's signature. Discarding a checked quotation without calling it does not perform its latent effects.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: K-EFFECT-04
r[K-EFFECT-04]

**K-EFFECT-04.** An effect annotation, a successful check, and possession of a program value MUST NOT be treated as authorization to perform the named operation. The host enforces authority separately.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: K-EFFECT-05
r[K-EFFECT-05]

**K-EFFECT-05.** A checked program-interface adaptation can widen a latent effect bound only through an explicit acceptance derivation. Its input/output stacks and resolved schema identities MUST remain unchanged. The derivation MUST establish inclusion of the original bound in the destination bound. Duplication MUST retain the adapted interface rather than create independent instantiations.

The adaptation creates a new program value and leaves the original interface unchanged. It retains the ordered recipe and does not execute it or grant authority. It requires no source compilation or proof search during runtime selection.

Retained interface witnesses record the destination bound. Identity comparison includes those witnesses under section 12. A narrower bound requires a separate derivation under K-EFFECT-02. A runtime collection cannot narrow a member's bound by inspecting only its display name.

No generic handler syntax, continuation capture, or multi-shot resumption is defined. A host can supply a mock implementation of an interface, but this does not introduce source-level algebraic handlers implicitly.


<!-- cairn:scenario-links:start -->
#### Scenario: WORKER-02 for K-EFFECT-05

- GIVEN the `Worker-Design` profile and every field of `input` in [WORKER-02](../../../specs/conformance/worker-cases.json)
- WHEN the `static` procedure for case `WORKER-02` runs against those inputs
- THEN the observations match every field of `expected` in case `WORKER-02`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: WORKER-03 for K-EFFECT-05

- GIVEN the `Worker-Design` profile and every field of `input` in [WORKER-03](../../../specs/conformance/worker-cases.json)
- WHEN the `runtime` procedure for case `WORKER-03` runs against those inputs
- THEN the observations match every field of `expected` in case `WORKER-03`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### 8.3 Verification boundary

### Requirement: K-CHECK-04
r[K-CHECK-04]

**K-CHECK-04.** Inference and elaboration SHALL produce an explicit resolved candidate for an acceptance checker. The checker MUST validate the relevant interfaces, references, witnesses, eligibility and effect constraints without trusting a candidate's claimed signature. The complete algorithm and witness encoding remain required follow-up specifications.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: K-CHECK-05
r[K-CHECK-05]

**K-CHECK-05.** Optional behavioral contracts MUST NOT be premises used to bypass the core stack, resource or effect checks. A checker timeout, unsupported construct or internal failure MUST NOT produce acceptance.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: K-CHECK-06
r[K-CHECK-06]

**K-CHECK-06.** An acceptance candidate MUST contain a finite resolved node graph, claimed node interfaces, generic instantiations, schema references, and effect/eligibility witnesses. It MUST identify the candidate format and semantic revision. Each witness MUST bind the exact node and premises that it describes. Acceptance MUST derive the conclusion from those premises rather than trust an asserted result.

The consumer supplies the expected interface, admitted environment, supported subset, and resource limits independently of the candidate. References resolve to exact contracts in that environment. The checker rejects absent premises, foreign owner-scoped references, cyclic type equations, and unsupported witnesses. A scoped recursive definition reference is not permission for a cyclic witness derivation.


<!-- cairn:scenario-links:start -->
#### Scenario: WORKER-05 for K-CHECK-06

- GIVEN the `Worker-Design` profile and every field of `input` in [WORKER-05](../../../specs/conformance/worker-cases.json)
- WHEN the `admission` procedure for case `WORKER-05` runs against those inputs
- THEN the observations match every field of `expected` in case `WORKER-05`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: WORKER-07 for K-CHECK-06

- GIVEN the `Worker-Design` profile and every field of `input` in [WORKER-07](../../../specs/conformance/worker-cases.json)
- WHEN the `admission` procedure for case `WORKER-07` runs against those inputs
- THEN the observations match every field of `expected` in case `WORKER-07`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: K-CHECK-07
r[K-CHECK-07]

**K-CHECK-07.** Admission MUST enforce finite limits before each bounded allocation or traversal step. Limits MUST cover input bytes, decoded nodes, nesting, dependency traversal, type/stack size, checking work, memory, and diagnostic output. Decoding, resolution, and acceptance MUST share an accounted request budget. A nested stage MUST NOT silently reset that budget.

Admission distinguishes accepted, invalid, unsupported, exhausted, and internal-failure outcomes. Only accepted candidates become trusted programs. Other outcomes publish no partial program and issue no candidate-body requests. Exhaustion does not prove invalidity or divergence. Diagnostics retain the primary outcome even if their output budget expires.

Exact witness encodings and complete inference rules remain G-01 deliverables. This contract does not claim a finished algorithm or bounded completion of arbitrary proof search.

The reference checking relation SHALL be formalized in Lean 4. Soundness of the mathematical checker, refinement of an actual Rust implementation, and trust in extraction/build tools are separate obligations under SPEC-V001 and IMPL-V001.


<!-- cairn:scenario-links:start -->
#### Scenario: WORKER-06 for K-CHECK-07

- GIVEN the `Worker-Design` profile and every field of `input` in [WORKER-06](../../../specs/conformance/worker-cases.json)
- WHEN the `admission` procedure for case `WORKER-06` runs against those inputs
- THEN the observations match every field of `expected` in case `WORKER-06`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: WORKER-07 for K-CHECK-07

- GIVEN the `Worker-Design` profile and every field of `input` in [WORKER-07](../../../specs/conformance/worker-cases.json)
- WHEN the `admission` procedure for case `WORKER-07` runs against those inputs
- THEN the observations match every field of `expected` in case `WORKER-07`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: OCTET-07 for K-CHECK-07

- GIVEN the `Octet-Adoption-Design` profile and every field of `input` in [OCTET-07](../../../specs/conformance/octet-adoption-cases.json)
- WHEN the `admission` procedure for case `OCTET-07` runs against those inputs
- THEN the observations match every field of `expected` in case `OCTET-07`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

## 9. Resource discipline

### 9.1 Ownership invariant

### Requirement: K-RES-01
r[K-RES-01]

**K-RES-01.** A live guest resource handle has one ownership obligation. On every normal path it MUST be moved onward, consumed by a declared operation, or explicitly released. Generic data discard is not an implicit resource release operation.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: K-RES-02
r[K-RES-02]

**K-RES-02.** `dup`, generic `drop`, `quote`, and resource serialization MUST be rejected for resource-bearing values. Moving, storing in an ownership-preserving data structure, destructuring, and passing a resource as an invocation input are permitted when the obligations remain accounted for.


<!-- cairn:scenario-links:start -->
#### Scenario: CORE-15 for K-RES-02

- GIVEN the `Core-Bootstrap` profile and every field of `input` in [CORE-15](../../../specs/conformance/cases.json)
- WHEN the `static` procedure for case `CORE-15` runs against those inputs
- THEN the observations match every field of `expected` in case `CORE-15`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: K-RES-03
r[K-RES-03]

**K-RES-03.** A host binding's signature MUST account for resource ownership on every normal result alternative. A successful `Result` and an error `Result` are both normal returns for this purpose.

For example, a borrowing operation may expose a move-threaded interface:

```text
fs.read : S Directory Text
          -- S Directory Result<Bytes,FsError> ! {fs.read}
```

Both success and failure return the directory handle exactly once. A different operation may consume a handle and encode replacement ownership in its result, but that different contract must be explicit.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### 9.2 Branches and aggregate data

Both branches must account for their incoming resources. This does not require identical side effects or returning the same physical handle. It requires a valid ownership transition and a common output type on each branch. A released handle cannot remain usable in the output stack.

A returned aggregate containing a handle carries its ownership obligation with it. The language must not hide such a handle in a value whose type claims `Data` or `Capture`.

### 9.3 Abnormal termination

### Requirement: K-RES-04
r[K-RES-04]

**K-RES-04.** The host MUST retain sufficient ownership accounting to retire invocation-owned handles when guest execution aborts. Cleanup MUST NOT depend on successfully resuming the guest. Handle retirement must be idempotent with respect to double release; the host must not expose a retired handle as live.

This is a requirement on local handle accounting, not a guarantee that a remote cleanup request succeeds exactly once. External effects, network partitions, revocation, and process failure remain host-protocol concerns. Cleanup timing, session ownership boundaries, and recovery policies must be fixed in the execution profile before release.

**Rationale.** This is a move-only, explicit-release normal-path discipline. It is not accurately described as unrestricted affine discard, nor does it prove global exclusivity of the underlying external resource.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

## 10. Definitions and resolution

### Requirement: K-DEF-01
r[K-DEF-01]

**K-DEF-01.** Checking a submission MUST resolve names against an explicit, immutable namespace snapshot. A successful definition contains exact dependency identities, not late-bound source-name lookups.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: K-DEF-02
r[K-DEF-02]

**K-DEF-02.** Renaming or rebinding a source name MUST NOT mutate an existing definition or the meaning of an already prepared program. Editing a body creates a new resolved definition; dependent definitions are changed only through explicit rebuilding or replacement.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: K-DEF-03
r[K-DEF-03]

**K-DEF-03.** A definition declaration does not execute its body. Namespace changes are tooling operations outside ordinary pure guest evaluation.

For self-recursion, a checked declaration carries an explicit interface and a distinguished self reference within its identity scope. This revision permits no mutual-recursion groups. Concrete annotation syntax and canonical encoding of self references are release-blocking open items. A reflected self reference that leaves its owner context must retain or be resolved with its owner identity; a bare self token cannot be rebound accidentally during preparation.

The source-name spelling of a definition is not the definition's semantic identity. Type schemas, builtin semantics, host contracts, and dependency identities participate in the identity rules in section 12.

---


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

# Part B — Programs as data and identity

## 11. Reflection and checked preparation

### 11.1 Abstract data model

The following is a semantic schema, not a Rust representation, serialization format, or additional source grammar:

```text
Syntax = Sequence<Node>

Node = Literal(DataValue)
     | Invoke(Reference)
     | Quotation(Syntax)

Reference = UnresolvedName(Text)
          | Definition(DefinitionId)
          | Builtin(BuiltinId)
          | Host(HostOpId)
          | ScopedSelf(Owner)
```

A checked recipe uses the resolved subset of these references, with required schema/type witnesses associated with its nodes and interface. `DataValue` includes the schema information needed to distinguish values of distinct declared types. It cannot contain a live resource handle.

The complete type-witness encoding is not frozen. It must distinguish necessary instantiations of generic definitions and preserve program interfaces; an implementation must not discard information needed to recheck or transport a recipe merely because executable machine code is already present.

Source spans, comments, display names, and formatting may be retained as metadata outside the canonical semantic representation.

### 11.2 Recipe invariants

### Requirement: P-RECIPE-01
r[P-RECIPE-01]

**P-RECIPE-01.** Every first-class program in the programs-as-data subset MUST have a finite, inspectable recipe describing its language-level composition. A reference may identify another definition without expanding that definition's body. Recursion must not force infinite expansion during inspection.


<!-- cairn:scenario-links:start -->
#### Scenario: ADAPT-10 for P-RECIPE-01

- GIVEN the `Backend-Experiment` profile and every field of `input` in [ADAPT-10](../../../specs/conformance/adaptation-cases.json)
- WHEN the `runtime` procedure for case `ADAPT-10` runs against those inputs
- THEN the observations match every field of `expected` in case `ADAPT-10`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: P-RECIPE-02
r[P-RECIPE-02]

**P-RECIPE-02.** Reflection MUST NOT expose or depend on executable addresses, closure memory layout, allocation identities, or optimizer-generated instruction layout. Optimizing execution MUST preserve the specified recipe observation.


<!-- cairn:scenario-links:start -->
#### Scenario: CORE-09 for P-RECIPE-02

- GIVEN the `Core-Bootstrap` profile and every field of `input` in [CORE-09](../../../specs/conformance/cases.json)
- WHEN the `runtime` procedure for case `CORE-09` runs against those inputs
- THEN the observations match every field of `expected` in case `CORE-09`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: ADAPT-10 for P-RECIPE-02

- GIVEN the `Backend-Experiment` profile and every field of `input` in [ADAPT-10](../../../specs/conformance/adaptation-cases.json)
- WHEN the `runtime` procedure for case `ADAPT-10` runs against those inputs
- THEN the observations match every field of `expected` in case `ADAPT-10`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: P-RECIPE-03
r[P-RECIPE-03]

**P-RECIPE-03.** For compatible programs, the recipe of `compose(p,q)` MUST be the ordered concatenation of their recipe sequences, modulo the specified structural normalization. Association of runtime composition adapters MUST NOT be observable as additional language instructions.

The empty recipe is the concatenation identity. Concatenation grouping is normalized; a quotation boundary is not flattened into its enclosing sequence.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: P-RECIPE-04
r[P-RECIPE-04]

**P-RECIPE-04.** For ordinary non-program data `v`, the recipe of `quote(v)` is one literal node containing `v` with its semantic schema information. For a program value `p`, the recipe of `quote(p)` is one quotation node representing `p`'s recipe and required interface witnesses. A program nested inside an aggregate is encoded as program data, not as an instruction to run it.

This normalization aligns a captured program with a source quotation that pushes that program. It does not inline named definition bodies or perform algebraic simplification. For example, `[ 1 1 + ]` and `[ 2 ]` may have the same normal result but different recipes and definition identities.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### 11.3 `reflect`

```text
reflect : R Program<S,T,e> -- R Syntax ! {}
```

### Requirement: P-REFLECT-01
r[P-REFLECT-01]

**P-REFLECT-01.** `reflect` MUST return an inert, resolved view of the program's recipe without executing it. It consumes one program value; callers may use `dup reflect` to retain a reusable copy.


<!-- cairn:scenario-links:start -->
#### Scenario: CORE-09 for P-REFLECT-01

- GIVEN the `Core-Bootstrap` profile and every field of `input` in [CORE-09](../../../specs/conformance/cases.json)
- WHEN the `runtime` procedure for case `CORE-09` runs against those inputs
- THEN the observations match every field of `expected` in case `CORE-09`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: P-REFLECT-02
r[P-REFLECT-02]

**P-REFLECT-02.** Reflecting a reference MUST NOT implicitly fetch or expand an external definition. Explicit code-store access requires its own host operation. A native host function is reflected as its declared host-operation identity and interface contract, not as inspectable Rust implementation code.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: P-REFLECT-03
r[P-REFLECT-03]

**P-REFLECT-03.** Editing a returned syntax value MUST NOT mutate the checked program from which it was obtained. No syntax constructor, edit, deserializer, or annotation grants executable status by itself.

For example, reflecting `[ 1 + ]` yields a sequence containing a literal `1` and the resolved integer-addition reference. Replacing the integer literal with text produces syntax that must fail checking for that addition; it does not change the old program.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### 11.4 Typed builders versus edited syntax

`quote` and `compose` construct checked program values under static constraints. Arbitrary syntax transformations need not preserve these constraints and therefore cross a checking boundary before execution.

### Requirement: P-STAGE-01
r[P-STAGE-01]

**P-STAGE-01.** `run` MUST reject a `Syntax` value as a static type error. There is no automatic coercion from syntax, source text, a list of nodes, or unverified artifact metadata to `Program`.


<!-- cairn:scenario-links:start -->
#### Scenario: CORE-10 for P-STAGE-01

- GIVEN the `Core-Bootstrap` profile and every field of `input` in [CORE-10](../../../specs/conformance/cases.json)
- WHEN the `static` procedure for case `CORE-10` runs against those inputs
- THEN the observations match every field of `expected` in case `CORE-10`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: P-STAGE-02
r[P-STAGE-02]

**P-STAGE-02.** Deserializing portable code MUST produce an untrusted description/package until validation and preparation establish its executable interface. A claimed signature or digest in the payload is not a proof.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### 11.5 Preparation service

A development host may provide an explicit compiler service. Its language-level adapter has the schematic contract:

```text
prepare<ExpectedInput,ExpectedOutput,AllowedEffects> :
    R CompilerCapability Syntax
    -- R CompilerCapability
       Result<Program<ExpectedInput,ExpectedOutput,AllowedEffects>,Diagnostic>
    ! {code.prepare}
```

This is a generic language adapter contract, not generic WIT source syntax. Concrete generic-application syntax and WIT specialization belong to later documents.

### Requirement: P-PREPARE-01
r[P-PREPARE-01]

**P-PREPARE-01.** Preparation MUST resolve references using an explicit context, validate structure and schemas, check stack interfaces and resource eligibility, and verify that inferred requirements are within `AllowedEffects`. A failed check MUST return a diagnostic without running the candidate body.


<!-- cairn:scenario-links:start -->
#### Scenario: WORKER-01 for P-PREPARE-01

- GIVEN the `Worker-Design` profile and every field of `input` in [WORKER-01](../../../specs/conformance/worker-cases.json)
- WHEN the `runtime` procedure for case `WORKER-01` runs against those inputs
- THEN the observations match every field of `expected` in case `WORKER-01`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: P-PREPARE-02
r[P-PREPARE-02]

**P-PREPARE-02.** Successful preparation MUST return an executable program at the expected instantiated interface. It MUST NOT silently reinterpret a different stack transformation through `Any` or runtime argument guessing.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: P-PREPARE-03
r[P-PREPARE-03]

**P-PREPARE-03.** Preparing code is an explicit host operation. Its permitted code-store access, compilation, allocation, and cache writes belong to the compiler capability's documented contract. These service effects are distinct from the candidate program's latent effects. The compiler service MUST NOT run arbitrary user code as an undocumented checking or macro-expansion step.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: P-PREPARE-04
r[P-PREPARE-04]

**P-PREPARE-04.** The service MUST thread or consume its resource capability according to its declared success and failure contract. The schematic adapter above returns the compiler capability on both normal alternatives.

A development tool may infer an unknown candidate signature for display. A statically checked guest must still choose an expected interface before obtaining a callable value. A minimal execution-only host need not expose preparation at all.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### 11.6 Programs are not ambient authority

### Requirement: P-AUTH-01
r[P-AUTH-01]

**P-AUTH-01.** Captured program data MUST NOT contain live capability handles. A program may name a host contract and require resources as inputs, but naming the contract does not create the resource or permission to use it.

A portable program can still contain sensitive bytes or text, including credentials whose use a host may allow. The resource restriction is not an information-flow or secret-detection system. Hosts must consider indirect authority through filesystem, process, network, or credential access rather than checking only whether an import has a particular name.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### 11.7 Optional behavioral verification

### Requirement: P-VERIFY-01
r[P-VERIFY-01]

**P-VERIFY-01.** Successful `prepare` establishes the required executable interface, resource eligibility and effect bound; it MUST NOT be treated as a proof of an arbitrary behavioral postcondition. Editing and rechecking syntax MUST NOT automatically transfer evidence about the original program.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: P-VERIFY-02
r[P-VERIFY-02]

**P-VERIFY-02.** Ordinary `quote`, `compose`, `run` and `reflect` MUST NOT invoke proof search. Companion operations can replay bounded, previously proved rules for runtime-built programs under SPEC-V002. External proof generation requires an explicit verification service. Neither path changes ordinary builder semantics.

Typed contracts and first-class proof companions are specified in [SPEC-V002](../program-contracts/spec.md). Partial correctness, prefix safety and total correctness remain distinct claims. Proof-required invocation needs applicable evidence, satisfied preconditions, artifact correspondence, and independent host authorization. Verifying untrusted proof source is not an undocumented part of preparation.


<!-- cairn:scenario-links:start -->
#### Scenario: CONTRACT-11 for P-VERIFY-02

- GIVEN the `Contracts-Draft` profile and every field of `input` in [CONTRACT-11](../../../specs/conformance/contract-cases.json)
- WHEN the `runtime` procedure for case `CONTRACT-11` runs against those inputs
- THEN the observations match every field of `expected` in case `CONTRACT-11`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### 11.8 Round-trip and package contracts

### Requirement: P-ROUND-01
r[P-ROUND-01]

**P-ROUND-01.** Under the same resolved context and expected instantiated interface, successful preparation of an unchanged reflected recipe MUST preserve its normalized recipe and interface witnesses. This preservation includes captured data, schema identities, dependency identities, and scoped recursive owners. The resulting program MUST retain the same abstract program-value identity.

The law requires supported features, available exact dependencies, authority for preparation, and sufficient preparation resources. It does not guarantee that preparation succeeds on every host. It does not require identical artifact bytes, allocation behavior, or execution-budget outcomes. Applicable semantic preservation follows the existing compiler/backend obligations, not digest comparison alone.


<!-- cairn:scenario-links:start -->
#### Scenario: WORKER-04 for P-ROUND-01

- GIVEN the `Worker-Design` profile and every field of `input` in [WORKER-04](../../../specs/conformance/worker-cases.json)
- WHEN the `identity` procedure for case `WORKER-04` runs against those inputs
- THEN the observations match every field of `expected` in case `WORKER-04`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: P-ROUND-02
r[P-ROUND-02]

**P-ROUND-02.** A receiving host MUST treat exported code as an untrusted description until local admission succeeds. Successful import under an unchanged semantic context MUST preserve the same recipe, captures, interface, and abstract identity. Import MUST NOT transfer unchecked acceptance flags, behavioral evidence applicability, or live authority.

A changed capture, dependency, schema, or interface requires admission for the changed subject. An incompatible context produces an explicit rejection or unsupported result. Hosts MUST NOT silently replace a missing dependency with another version. Local round-trip evidence does not establish portable binary interoperability before G-03 closes.


<!-- cairn:scenario-links:start -->
#### Scenario: WORKER-04 for P-ROUND-02

- GIVEN the `Worker-Design` profile and every field of `input` in [WORKER-04](../../../specs/conformance/worker-cases.json)
- WHEN the `identity` procedure for case `WORKER-04` runs against those inputs
- THEN the observations match every field of `expected` in case `WORKER-04`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: WORKER-05 for P-ROUND-02

- GIVEN the `Worker-Design` profile and every field of `input` in [WORKER-05](../../../specs/conformance/worker-cases.json)
- WHEN the `admission` procedure for case `WORKER-05` runs against those inputs
- THEN the observations match every field of `expected` in case `WORKER-05`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: P-PACK-01
r[P-PACK-01]

**P-PACK-01.** A portable package MUST describe its format/semantic revision, root recipe, captures, interface witnesses, and exact definition, schema, builtin, and host-contract dependencies. Each transitive dependency MUST be present or resolve from an explicitly admitted store. Shared references and scoped recursion MUST use a finite graph, not infinite body expansion.

The dependency closure contains code and contracts, not host implementations or live resources. Export requires explicit authority for its destination and captured information. Reflection and capturability alone do not authorize publication.


<!-- cairn:scenario-links:start -->
#### Scenario: WORKER-04 for P-PACK-01

- GIVEN the `Worker-Design` profile and every field of `input` in [WORKER-04](../../../specs/conformance/worker-cases.json)
- WHEN the `identity` procedure for case `WORKER-04` runs against those inputs
- THEN the observations match every field of `expected` in case `WORKER-04`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: WORKER-05 for P-PACK-01

- GIVEN the `Worker-Design` profile and every field of `input` in [WORKER-05](../../../specs/conformance/worker-cases.json)
- WHEN the `admission` procedure for case `WORKER-05` runs against those inputs
- THEN the observations match every field of `expected` in case `WORKER-05`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: WORKER-12 for P-PACK-01

- GIVEN the `Worker-Design` profile and every field of `input` in [WORKER-12](../../../specs/conformance/worker-cases.json)
- WHEN the `runtime` procedure for case `WORKER-12` runs against those inputs
- THEN the observations match every field of `expected` in case `WORKER-12`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: P-PACK-02
r[P-PACK-02]

**P-PACK-02.** Import MUST validate dependency closure, resolved identities, schemas, witnesses, feature support, and artifact correspondence under bounded local admission. A package digest or claimed signature MUST NOT bypass those checks. Dependency retrieval requires an explicit compiler or loading service with a declared capability contract. `run` and `reflect` MUST NOT fetch absent code.

Packages MUST declare executable artifacts separately from semantic recipes. Canonical bytes, digest algorithms, transport framing, and concrete service interfaces remain G-03/G-05 deliverables. Registry discovery, replication, and live-environment migration remain outside this contract.


<!-- cairn:scenario-links:start -->
#### Scenario: WORKER-04 for P-PACK-02

- GIVEN the `Worker-Design` profile and every field of `input` in [WORKER-04](../../../specs/conformance/worker-cases.json)
- WHEN the `identity` procedure for case `WORKER-04` runs against those inputs
- THEN the observations match every field of `expected` in case `WORKER-04`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: WORKER-05 for P-PACK-02

- GIVEN the `Worker-Design` profile and every field of `input` in [WORKER-05](../../../specs/conformance/worker-cases.json)
- WHEN the `admission` procedure for case `WORKER-05` runs against those inputs
- THEN the observations match every field of `expected` in case `WORKER-05`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

## 12. Structural identity

### 12.1 Separate identities

This specification distinguishes the following concepts:

| Identity | Identifies |
|---|---|
| `DefinitionId` | A canonical resolved definition and the language semantics necessary to interpret it. |
| `ProgramValueId` | A portable program value, including its captured data and required schema/interface information. |
| `BuildKey` | A compilation request: code/dependencies, compiler identity, target profile, interfaces/adapters, and relevant options. |
| `ArtifactId` | Exact emitted artifact bytes. |

`ProgramValueId` names a distinction already required by the RFC's captured-value discussion; this revision supplies an explicit label. A closure adding two and a closure adding three do not become the same value merely because their executable code template is shared.

### Requirement: P-ID-01
r[P-ID-01]

**P-ID-01.** Renaming source names, changing formatting or source spans, and changing optimizer layout MUST NOT change a definition's semantic identity. Changes to its canonical resolved body, relevant type/schema identities, builtin/host semantics, or resolved dependencies MUST be reflected in that identity.


<!-- cairn:scenario-links:start -->
#### Scenario: ID-01 for P-ID-01

- GIVEN the `ProgramsData-Draft` profile and every field of `input` in [ID-01](../../../specs/conformance/identity-cases.json)
- WHEN the `identity` procedure for case `ID-01` runs against those inputs
- THEN the observations match every field of `expected` in case `ID-01`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: P-ID-02
r[P-ID-02]

**P-ID-02.** A compiler revision or optimization choice may change the build key and artifact identity without changing the source definition identity. Artifact bytes MUST NOT be used as a substitute for source-definition identity.


<!-- cairn:scenario-links:start -->
#### Scenario: ID-02 for P-ID-02

- GIVEN the `ProgramsData-Draft` profile and every field of `input` in [ID-02](../../../specs/conformance/identity-cases.json)
- WHEN the `identity` procedure for case `ID-02` runs against those inputs
- THEN the observations match every field of `expected` in case `ID-02`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: P-ID-03
r[P-ID-03]

**P-ID-03.** Different captured data MUST remain distinguishable in portable program-value identity. Function-template identity alone is insufficient.


<!-- cairn:scenario-links:start -->
#### Scenario: ID-05 for P-ID-03

- GIVEN the `ProgramsData-Draft` profile and every field of `input` in [ID-05](../../../specs/conformance/identity-cases.json)
- WHEN the `identity` procedure for case `ID-05` runs against those inputs
- THEN the observations match every field of `expected` in case `ID-05`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### 12.2 Canonicalization requirements

Canonicalization must specify node ordering, literal encodings, schema identities, alpha-renaming of type/stack/effect variables, effect-set ordering, resolved references, quotation nesting, and distinguished recursive references. Concatenation grouping must normalize as required by `P-RECIPE-03`.

It must exclude incidental source names, locations, allocation addresses, and optimized machine layout. It must not attempt to equate arbitrary programs merely because they appear to compute equivalent functions.

### Requirement: P-ID-08
r[P-ID-08]

**P-ID-08.** Semantic identity MUST include resolved operation, schema, and dependency identities. Changing a referenced versioned WIT operation changes the definition identity and dependent program-value identity. Build-only changes do not alter semantic identity when the resolved Noble contract remains unchanged. See SPEC-W001 for adapter-only changes.


<!-- cairn:scenario-links:start -->
#### Scenario: ID-03 for P-ID-08

- GIVEN the `Component-Draft` profile and every field of `input` in [ID-03](../../../specs/conformance/identity-cases.json)
- WHEN the `identity` procedure for case `ID-03` runs against those inputs
- THEN the observations match every field of `expected` in case `ID-03`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: ID-04 for P-ID-08

- GIVEN the `Component-Draft` profile and every field of `input` in [ID-04](../../../specs/conformance/identity-cases.json)
- WHEN the `identity` procedure for case `ID-04` runs against those inputs
- THEN the observations match every field of `expected` in case `ID-04`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: P-ID-04
r[P-ID-04]

**P-ID-04.** Until a canonical byte format, encoding version, hash algorithm, and test vectors are specified, implementations MUST label stored IDs and encodings experimental and MUST NOT claim cross-implementation stable content hashes. The repository package's file-integrity checksum is not a choice of language hash algorithm.


<!-- cairn:scenario-links:start -->
#### Scenario: WORKER-04 for P-ID-04

- GIVEN the `Worker-Design` profile and every field of `input` in [WORKER-04](../../../specs/conformance/worker-cases.json)
- WHEN the `identity` procedure for case `WORKER-04` runs against those inputs
- THEN the observations match every field of `expected` in case `WORKER-04`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: P-ID-05
r[P-ID-05]

**P-ID-05.** Definition identity MUST NOT be treated as a signature, provenance certificate, authorization token, or justification for caching an effectful result. Trust and execution policy are separate checks.

Untrusted packages must have their contents validated against their claimed identities and contracts under the selected loading policy. A loader must not trust a type/effect manifest solely because it accompanies valid Wasm bytes.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### 12.3 Namespace and storage policy

A name maps to an immutable identity within a namespace snapshot. Updating a snapshot does not mutate that identity. Local files, a database, and a remote content-addressed store may implement the same abstract lookup interface; none is a mandatory language data structure.

This revision does not specify registry discovery, synchronization, garbage collection of code stores, signing policy, or distributed updates. No running program is automatically hot-replaced when a name is rebound.

### 12.4 Verification evidence and identity

### Requirement: P-ID-06
r[P-ID-06]

**P-ID-06.** Optional behavioral contracts and proof evidence SHALL be associated separately from canonical program identity. Adding a proof of unchanged code MUST NOT alter its recipe or definition/program-value identity. Evidence MUST nevertheless bind the exact subject or quantified family, captures/instantiation, semantic context, claim and assumptions to which it applies.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: P-ID-07
r[P-ID-07]

**P-ID-07.** A proof about a recipe MUST NOT be treated as proof about independently supplied executable bytes. The source/recipe-to-artifact correspondence and runtime authorization remain separate acceptance conditions. Canonical evidence transport is not standardized by this extension.

---


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

# Part C — Wasm execution and host contracts

## 13. Wasm execution requirements

### 13.1 Execution substrate

### Requirement: W-EXEC-01
r[W-EXEC-01]

**W-EXEC-01.** The production execution target of this specification is WebAssembly. A conforming `Wasm-Draft` implementation MUST compile checked language computation to Wasm execution rather than require a second production AST or bytecode evaluator to implement source semantics.

Compiler-generated adapters, allocation routines, closures, data structures, cleanup support, and dispatch over already compiled operations are permitted support code. The absence of a second VM does not mean the absence of memory management or language support machinery.

A reference evaluator may exist strictly as a testing oracle. It is not evidence that the Wasm requirements have been implemented.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### 13.2 Runtime-built program values

### Requirement: W-EXEC-02
r[W-EXEC-02]

**W-EXEC-02.** `quote`, `compose`, and `run` MUST support genuinely runtime-created values over the prepared vocabulary. This includes a captured scalar supplied after compilation, a runtime-selected program, and a program retrieved from a homogeneous collection. The implementation MUST NOT invoke the source compiler to perform those operations.


<!-- cairn:scenario-links:start -->
#### Scenario: CORE-03 for W-EXEC-02

- GIVEN the `Core-Bootstrap` profile and every field of `input` in [CORE-03](../../../specs/conformance/cases.json)
- WHEN the `runtime` procedure for case `CORE-03` runs against those inputs
- THEN the observations match every field of `expected` in case `CORE-03`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: CORE-05 for W-EXEC-02

- GIVEN the `Core-Bootstrap` profile and every field of `input` in [CORE-05](../../../specs/conformance/cases.json)
- WHEN the `runtime` procedure for case `CORE-05` runs against those inputs
- THEN the observations match every field of `expected` in case `CORE-05`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: W-EXEC-03
r[W-EXEC-03]

**W-EXEC-03.** Program execution and recipe reflection MUST remain consistent when optimization is disabled or the builder operands are not constant. Constant folding alone is insufficient evidence of first-class program support.


<!-- cairn:scenario-links:start -->
#### Scenario: CORE-09 for W-EXEC-03

- GIVEN the `Core-Bootstrap` profile and every field of `input` in [CORE-09](../../../specs/conformance/cases.json)
- WHEN the `runtime` procedure for case `CORE-09` runs against those inputs
- THEN the observations match every field of `expected` in case `CORE-09`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: W-EXEC-04
r[W-EXEC-04]

**W-EXEC-04.** Compiled program values MUST have a specified calling convention and environment representation before ABI conformance can be claimed. The implementation MUST NOT assume that arbitrary Wasm function signatures are interchangeable.

The precise convention is open. Candidate techniques must account for monomorphic program interfaces, generic definition instantiations, captured data, dynamically long compositions, recipe sharing, reentrancy, and host/resource inputs. This document does not select a heap layout, GC strategy, or closure allocation algorithm.

[SPEC-BE001](../backend-experiments/spec.md) requires an early comparison of WasmGC and managed linear memory. Typed function references, multi-value returns, and specialization are experiment candidates, not permission to discard stack inputs or observable recipes.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### 13.3 Definitions versus components

### Requirement: W-COMP-01
r[W-COMP-01]

**W-COMP-01.** Definition identity and deployment/isolation granularity MUST be separable. A definition having an identity MUST NOT require each invocation to cross a component boundary. Multiple definitions may compile into one component.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: W-COMP-02
r[W-COMP-02]

**W-COMP-02.** Internal `Program<S,T,e>` values MUST NOT be exported as untyped integer addresses or unspecified generic function values. A cross-component interface must use a separately specified concrete adapter, resource contract, or portable code package.

Ordinary internal program calls and code transport are separate operations. A destination may need to resolve exact dependencies, prepare executable code, and provide authorized resources before a transported description is executable.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### 13.4 Compiler and implementation organization

The intended implementation language is Rust. A compiler library, an embedding library, and a command-line tool may share the same checker and backend without creating separate source semantics.

The supplied reference set is carried forward from RFC 0001: `wasm-tools` for backend tooling, `wit-bindgen` as a guest-binding reference, Wasmtime as the intended execution engine, `wstd` as an async library/API reference, and `cap-std` as a host-adapter reference. These are intended roles, not a version-compatibility claim. This revision pins no crate or interface versions.

The language specification does not require every application to expose the same filesystem or network interfaces. Application-specific modules are adapters, not kernel extensions.

### 13.5 Component Model, WIT, and WASI profile

The standard Noble interoperability profile uses WebAssembly components and WIT as its external interface contract. This profile is specified in detail by `SPEC-W001`. WIT does not replace Noble's internal type system: `Program<S,T,e>`, stack-tail polymorphism, effect bounds, recipes, program identity, and proof evidence remain Noble concepts.

### Requirement: W-WIT-01
r[W-WIT-01]

**W-WIT-01.** A `Component-Draft` implementation MUST accept versioned WIT packages/worlds as typed compiler inputs and MUST generate WIT-compatible imports/exports without requiring handwritten duplicate host declarations.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: W-WIT-02
r[W-WIT-02]

**W-WIT-02.** WIT type lowering/lifting MUST be lossless with respect to the selected external contract. Where no identical Noble kernel type exists, the binding layer MUST use an exact boundary/declared type or checked conversion rather than silently narrowing or reinterpreting values.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: W-WIT-03
r[W-WIT-03]

**W-WIT-03.** A generated import for a WIT function MUST have a statically known Noble stack interface. Imported WIT functions are conservatively effectful unless a stronger reviewed Noble contract establishes otherwise; a WIT signature alone is not a purity, determinism, termination, or authority proof.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: W-WIT-04
r[W-WIT-04]

**W-WIT-04.** A Noble export implementing a WIT function MUST present a closed WIT-lowerable interface. Component adapters MUST NOT expose an ambient Noble stack tail, internal addresses, closure pointers, or unspecified runtime representations across the component boundary.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: W-WIT-05
r[W-WIT-05]

**W-WIT-05.** Owned WIT resources MUST lower to Noble move-only resources. Borrowed WIT resources MUST be scoped non-owning boundary values that cannot be captured, serialized, persisted, released as owned, or allowed to escape their declared borrow scope. [RESOURCE-ADAPTERS.md](../resource-adapters/spec.md) specifies the first synchronous adapter subset and rejects unsupported borrow patterns.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: W-WIT-06
r[W-WIT-06]

**W-WIT-06.** The WIT world and exact imported/exported interface versions are compilation inputs and MUST participate in compatibility and build identity. WIT interface identity MUST NOT replace `DefinitionId` or `ProgramValueId`.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: W-WIT-07
r[W-WIT-07]

**W-WIT-07.** Interface availability and Noble effect information MUST NOT be treated as sufficient runtime authority. Linked host/component implementations and concrete resource/capability values enforce authority independently. Profiles SHOULD prefer resource-scoped/capability-scoped interfaces where available.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: W-WIT-08
r[W-WIT-08]

**W-WIT-08.** `Program<S,T,e>` is not automatically a WIT function, closure, or resource. Portable program transport remains an explicit package/validation/preparation problem; no raw generic function address or unspecified program handle may cross the boundary.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: W-WASI-01
r[W-WASI-01]

**W-WASI-01.** Stable WASI interfaces SHALL be the preferred standard Noble host interfaces when they adequately represent the required capability. A Noble-specific duplicate standard system ABI SHOULD require a documented semantic reason.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: W-WASI-02
r[W-WASI-02]

**W-WASI-02.** The preferred standard host profile targets the stable WASI 0.3 family. Exact WIT/WASI package versions and relevant Component Model feature/runtime/toolchain versions MUST be pinned by the build/profile and included where applicable in `BuildKey`. WASI 0.2 MAY be supported through an explicitly identified compatibility profile.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: W-WASI-03
r[W-WASI-03]

**W-WASI-03.** Noble's standard cross-component async ABI MUST use Component Model native `async func`, `stream<T>`, and `future<T>`. This requirement does not add `async`/`await` source forms or make asynchronous suspension imply concurrent execution.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: W-WASI-04
r[W-WASI-04]

**W-WASI-04.** Direct-style Noble evaluation order MUST be preserved across async WIT calls. `stream<T>` and `future<T>` are typed profile-level values; when they carry live runtime state they are conservatively non-`Data` and non-`Capture` until stronger duplication/capture semantics are specified and proved.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: W-WASI-05
r[W-WASI-05]

**W-WASI-05.** The async host profile MUST specify cancellation, completion/error behavior, outstanding resource ownership, cleanup, quotas/backpressure where applicable, and abnormal termination. The Component Model async ABI is not by itself Noble's scheduling or concurrency semantics.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: W-INTEROP-01
r[W-INTEROP-01]

**W-INTEROP-01.** Syndicate/Synit remains the standard concurrency/service model and Preserves remains the default protocol/schema representation for that layer. Where these facilities cross component boundaries, Noble SHOULD expose versioned WIT packages; Preserves values and WIT resources/capabilities remain distinct and require explicit validated conversion.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: W-INTEROP-02
r[W-INTEROP-02]

**W-INTEROP-02.** Lifting/lowering, WIT decoding, resource tables, host callbacks, and component loading are hostile or semi-trusted safety boundaries under SPEC-S001. Public wrappers MUST validate all preconditions not already established by an independently verified caller.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### 13.6 Implementation verification profile

### Requirement: W-VERIFY-01
r[W-VERIFY-01]

**W-VERIFY-01.** The implementation MUST follow IMPL-V001: Lean 4 reference definitions and an Aeneas-first Rust implementation across the project. The entire semantic kernel MUST use the mandatory Charon → Aeneas → Lean route. Non-kernel exceptions require explicit review, exact source scope, and separate evidence. Results MUST retain extraction assumptions and correspondence status. No automatic Verus-to-Lean proof interchange is assumed.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: W-VERIFY-02
r[W-VERIFY-02]

**W-VERIFY-02.** Verification of Rust implementation components MUST NOT be reported as source-to-Wasm semantic preservation without evidence for the relevant compiler, artifact-loading and execution boundaries. Public/FFI/Wasm entry wrappers MUST validate preconditions not established by a verified caller.

Selected tools and compatible pinned versions are different decisions. This extension selects roles but supplies no tested version matrix or verified tool binaries.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

## 14. Embedding, host boundaries, and sessions

### 14.1 Host contracts

### Requirement: H-HOST-01
r[H-HOST-01]

**H-HOST-01.** Host operations MUST have stable declared interfaces covering argument/results, effects, and resource disposition. Bindings MUST validate incoming resources and return values consistent with those contracts.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: H-HOST-02
r[H-HOST-02]

**H-HOST-02.** Runtime authorization MUST be enforced independently of guest type/effect claims. Forged, stale, cross-context, or wrongly typed resource handles MUST NOT grant access to a host object.


<!-- cairn:scenario-links:start -->
#### Scenario: WORKER-12 for H-HOST-02

- GIVEN the `Worker-Design` profile and every field of `input` in [WORKER-12](../../../specs/conformance/worker-cases.json)
- WHEN the `runtime` procedure for case `WORKER-12` runs against those inputs
- THEN the observations match every field of `expected` in case `WORKER-12`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: OCTET-02 for H-HOST-02

- GIVEN the `Octet-Adoption-Design` profile and every field of `input` in [OCTET-02](../../../specs/conformance/octet-adoption-cases.json)
- WHEN the `runtime` procedure for case `OCTET-02` runs against those inputs
- THEN the observations match every field of `expected` in case `OCTET-02`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: H-HOST-03
r[H-HOST-03]

**H-HOST-03.** The host MUST describe indirect authority exposed through its operations. Omitting a specifically named import is not sufficient isolation when another permitted operation can perform the same consequential action.

A guest's opaque directory handle may designate a constrained scope. An implementation must enforce that scope in the host rather than convert the handle into an unchecked path string. Details of path resolution, read-only policies, revocation, and filesystem behavior belong to the later filesystem profile.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### 14.1.1 Protected effects and authorization witnesses

### Requirement: H-AUTH-01
r[H-AUTH-01]

**H-AUTH-01.** An execution profile MUST classify each outbound host operation as protected or explicitly unprotected. An unprotected classification MUST name its scope and review rationale. A protected operation MUST use distinct plan, authorization, attempt, observation, and receipt contracts.

These are boundary roles, not new kernel types or syntax:

| Role | Meaning |
|---|---|
| Plan | Immutable description of the requested operation and arguments |
| Authorization | Host-established, scoped permission for that plan |
| Attempt | Record of one admitted executor invocation |
| Observation | Validated evidence of an operation outcome at a declared boundary |
| Receipt | Description of a scoped claim supported by the observation |

A plan does not imply authorization. An authorized attempt does not imply success. A pure policy function can return an allow/deny decision from explicit facts. That decision remains data and cannot create live authority.


<!-- cairn:scenario-links:start -->
#### Scenario: OCTET-01 for H-AUTH-01

- GIVEN the `Octet-Adoption-Design` profile and every field of `input` in [OCTET-01](../../../specs/conformance/octet-adoption-cases.json)
- WHEN the `runtime` procedure for case `OCTET-01` runs against those inputs
- THEN the observations match every field of `expected` in case `OCTET-01`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: H-AUTH-02
r[H-AUTH-02]

**H-AUTH-02.** Only an authorized host boundary can establish a live authorization witness. It MUST validate the authority facts independently of producer-supplied plan or decision claims. The witness MUST bind the exact operation contract, plan and arguments, actor/owner context, policy revision, and applicable validity or quota constraints.

Before protected admission, the host MUST validate those bindings and required current revocation, expiry, and quota facts. Missing required facts MUST fail closed. The profile MUST specify the synchronization point and freshness assumptions for this decision. Local admission does not promise atomic authorization across remote systems.


<!-- cairn:scenario-links:start -->
#### Scenario: OCTET-01 for H-AUTH-02

- GIVEN the `Octet-Adoption-Design` profile and every field of `input` in [OCTET-01](../../../specs/conformance/octet-adoption-cases.json)
- WHEN the `runtime` procedure for case `OCTET-01` runs against those inputs
- THEN the observations match every field of `expected` in case `OCTET-01`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: OCTET-02 for H-AUTH-02

- GIVEN the `Octet-Adoption-Design` profile and every field of `input` in [OCTET-02](../../../specs/conformance/octet-adoption-cases.json)
- WHEN the `runtime` procedure for case `OCTET-02` runs against those inputs
- THEN the observations match every field of `expected` in case `OCTET-02`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: H-AUTH-03
r[H-AUTH-03]

**H-AUTH-03.** The initial protected-effect contract uses a one-shot witness. Protected admission MUST atomically consume its availability and register the attempt before the protected operation starts. A failed preflight MUST issue no protected operation and preserve any valid, unconsumed caller obligation under the declared result contract.

After admission commits, failure, cancellation, or an unknown external outcome MUST NOT restore that witness or authorize an automatic retry. A repeat requires fresh authorization. Input and result resources retain their separate ownership contracts. A future reusable witness contract requires explicit scope, cardinality, revocation, quota, and lifetime rules. Ordinary move-threaded capabilities do not become one-shot merely because they can request such witnesses.


<!-- cairn:scenario-links:start -->
#### Scenario: OCTET-01 for H-AUTH-03

- GIVEN the `Octet-Adoption-Design` profile and every field of `input` in [OCTET-01](../../../specs/conformance/octet-adoption-cases.json)
- WHEN the `runtime` procedure for case `OCTET-01` runs against those inputs
- THEN the observations match every field of `expected` in case `OCTET-01`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: OCTET-02 for H-AUTH-03

- GIVEN the `Octet-Adoption-Design` profile and every field of `input` in [OCTET-02](../../../specs/conformance/octet-adoption-cases.json)
- WHEN the `runtime` procedure for case `OCTET-02` runs against those inputs
- THEN the observations match every field of `expected` in case `OCTET-02`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: H-AUTH-04
r[H-AUTH-04]

**H-AUTH-04.** Any guest-visible live authorization witness MUST use the existing resource discipline. It MUST NOT satisfy `Data` or `Capture`, or permit generic duplication, discard, serialization, or unchecked construction. Constructors, imports, and conversions MUST NOT turn an allow record, digest, identity, or declared type tag into live authority.

The Rust host MUST keep witness fields opaque and expose only approved constructors and executor interfaces. Runtime context, generation, liveness, and rights checks still apply. Static opacity does not establish credential validity or revocation freshness.


<!-- cairn:scenario-links:start -->
#### Scenario: OCTET-02 for H-AUTH-04

- GIVEN the `Octet-Adoption-Design` profile and every field of `input` in [OCTET-02](../../../specs/conformance/octet-adoption-cases.json)
- WHEN the `runtime` procedure for case `OCTET-02` runs against those inputs
- THEN the observations match every field of `expected` in case `OCTET-02`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: OCTET-03 for H-AUTH-04

- GIVEN the `Octet-Adoption-Design` profile and every field of `input` in [OCTET-03](../../../specs/conformance/octet-adoption-cases.json)
- WHEN the `static` procedure for case `OCTET-03` runs against those inputs
- THEN the observations match every field of `expected` in case `OCTET-03`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### 14.1.2 Observation and receipt construction

### Requirement: H-RECEIPT-01
r[H-RECEIPT-01]

**H-RECEIPT-01.** A trusted observation MUST bind the exact attempt, operation, observed outcome, source boundary, and relevant execution context. Only approved observation constructors or explicit evidence admission can establish that trusted status. Receipt constructors MUST validate observation applicability to the requested claim.

A success receipt requires an approved success observation for that same claim and attempt. A plan, witness, attempt, failed observation, absent observation, or unknown outcome MUST NOT substitute for success. Denial, failure, and unknown outcomes require distinct receipt claims. Schema-valid bytes alone do not establish an observation's provenance.


<!-- cairn:scenario-links:start -->
#### Scenario: OCTET-01 for H-RECEIPT-01

- GIVEN the `Octet-Adoption-Design` profile and every field of `input` in [OCTET-01](../../../specs/conformance/octet-adoption-cases.json)
- WHEN the `runtime` procedure for case `OCTET-01` runs against those inputs
- THEN the observations match every field of `expected` in case `OCTET-01`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: OCTET-04 for H-RECEIPT-01

- GIVEN the `Octet-Adoption-Design` profile and every field of `input` in [OCTET-04](../../../specs/conformance/octet-adoption-cases.json)
- WHEN the `adapter` procedure for case `OCTET-04` runs against those inputs
- THEN the observations match every field of `expected` in case `OCTET-04`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: H-RECEIPT-02
r[H-RECEIPT-02]

**H-RECEIPT-02.** Receipts MUST distinguish operation success from invocation success and state their observation scope. A confirmed late external success can support an operation receipt while the cancelled invocation remains cancelled. Such evidence MUST NOT restore a guest owner or permit successful task-result delivery after cancellation.

A receipt remains data, not permission to execute or a proof of arbitrary program behavior. Exported receipt descriptions MUST NOT carry unchecked trust flags. Import requires explicit provenance, context, and claim validation before trusted use. Artifact integrity alone does not establish signer authenticity or host authorization.

These contracts refine H-HOST-02 and H-OUTCOME-01/02. They add no implicit authorizer, receipt service, distributed retry policy, or proof requirement for ordinary programs. [Octet adoption](../../../specs/OCTET-ADOPTION.md) records their source and implementation gates.


<!-- cairn:scenario-links:start -->
#### Scenario: OCTET-01 for H-RECEIPT-02

- GIVEN the `Octet-Adoption-Design` profile and every field of `input` in [OCTET-01](../../../specs/conformance/octet-adoption-cases.json)
- WHEN the `runtime` procedure for case `OCTET-01` runs against those inputs
- THEN the observations match every field of `expected` in case `OCTET-01`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: OCTET-04 for H-RECEIPT-02

- GIVEN the `Octet-Adoption-Design` profile and every field of `input` in [OCTET-04](../../../specs/conformance/octet-adoption-cases.json)
- WHEN the `adapter` procedure for case `OCTET-04` runs against those inputs
- THEN the observations match every field of `expected` in case `OCTET-04`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### 14.2 Sessions and the REPL

### Requirement: H-REPL-01
r[H-REPL-01]

**H-REPL-01.** A REPL MUST retain persistent stack values in explicit session-owned storage. Its semantics MUST NOT rely on a returned Wasm function leaving a persistent operand stack available for the next submission.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: H-REPL-02
r[H-REPL-02]

**H-REPL-02.** A statically rejected submission MUST leave the prior session stack and namespace snapshot unchanged and MUST execute no user body effects. Compilation/cache activity within an explicitly invoked tool is not a body effect and must be documented separately.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: H-REPL-03
r[H-REPL-03]

**H-REPL-03.** A runtime failure after checking MUST NOT be represented as an automatic rollback of earlier host effects. The tool MUST report failure and apply the execution profile's resource retirement and session recovery policy before allowing further use of affected values.

The exact recovery policy is open: a future profile must specify which state survives, which invocation-owned resources are retired, and how a resumed session obtains a sound stack type. It may not merely reuse potentially consumed handles because their previous stack positions are known.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### 14.3 Limits and loading

### Requirement: H-LIMIT-01
r[H-LIMIT-01]

**H-LIMIT-01.** An execution profile MUST state its policies for memory, execution budget, host-operation duration, and outstanding resources. Language `Data` or empty effects do not imply unbounded allocation is safe.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: H-LIMIT-02
r[H-LIMIT-02]

**H-LIMIT-02.** A bounded execution profile MUST define accounting units, interruption points, and a maximum guest-work interval between interruption checks. Coverage MUST include pure loops, calls, runtime builders, data traversal, and allocation. An empty effect bound MUST NOT exempt a computation from limits. Nested invocations MUST remain within their enclosing allocation and execution budgets.

Host operations require separate duration and outstanding-work limits. A host deadline does not prove that native work stopped. Unstoppable work retains an owner and consumes its declared quota until retirement completes. The profile MUST specify bounded diagnostics and observation storage. Exhaustion MUST NOT silently increase a budget or become an ordinary success.


<!-- cairn:scenario-links:start -->
#### Scenario: WORKER-09 for H-LIMIT-02

- GIVEN the `Worker-Design` profile and every field of `input` in [WORKER-09](../../../specs/conformance/worker-cases.json)
- WHEN the `runtime` procedure for case `WORKER-09` runs against those inputs
- THEN the observations match every field of `expected` in case `WORKER-09`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: WORKER-10 for H-LIMIT-02

- GIVEN the `Worker-Design` profile and every field of `input` in [WORKER-10](../../../specs/conformance/worker-cases.json)
- WHEN the `runtime` procedure for case `WORKER-10` runs against those inputs
- THEN the observations match every field of `expected` in case `WORKER-10`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: H-LIMIT-03
r[H-LIMIT-03]

**H-LIMIT-03.** A profile MUST identify its accounting revision and distinguish deterministic work exhaustion from host deadlines and cancellation. Equal programs need not consume equal budgets across backends. A replay claim MUST identify all applicable accounting, toolchain, host-response, and ordering inputs. These requirements do not establish durable replay or distributed fairness.


<!-- cairn:scenario-links:start -->
#### Scenario: WORKER-09 for H-LIMIT-03

- GIVEN the `Worker-Design` profile and every field of `input` in [WORKER-09](../../../specs/conformance/worker-cases.json)
- WHEN the `runtime` procedure for case `WORKER-09` runs against those inputs
- THEN the observations match every field of `expected` in case `WORKER-09`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: H-LOAD-01
r[H-LOAD-01]

**H-LOAD-01.** A loader MUST separate artifact validation, source/type provenance policy, interface compatibility, and resource authorization. A valid Wasm module is not by itself proof that a separately supplied language recipe, effect bound, or reflection manifest is truthful.

The precise trust chain between source checking, recipe metadata, compiler output, and loaded artifacts must be specified before accepting arbitrary third-party artifacts as trusted `Program` values.



<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### 14.4 Safety boundaries

### Requirement: H-SAFE-01
r[H-SAFE-01]

**H-SAFE-01.** Every public transition from unverified Rust, FFI, Wasm, external wire data, or host callback into trusted Noble runtime state MUST validate all preconditions not already guaranteed by a proved caller relation.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: H-SAFE-02
r[H-SAFE-02]

**H-SAFE-02.** A host MUST reject stale, forged, wrong-kind, cross-context, or unauthorized resource representations before performing the requested operation. Such representations MUST NOT become valid merely because the guest can manufacture the underlying integer/bytes representation.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: H-SAFE-03
r[H-SAFE-03]

**H-SAFE-03.** Deserialization and portable-artifact loading MUST produce untrusted representations until schema/interface, identity/provenance policy, and preparation/loading validation establish the claimed trusted form. Canonical encoding or digest equality alone is insufficient.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: H-SAFE-04
r[H-SAFE-04]

**H-SAFE-04.** Native implementation unsafety MUST remain behind narrow reviewed adapters. Releases advertising the affected Noble-safe claim MUST publish the relevant unsafe/native boundary inventory and current assurance status.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### 14.5 Async execution and streams

Component Model native async is the selected external ABI under SPEC-W001. Noble scheduling, complete async ownership rules, fairness, and stream surface vocabulary remain open.

The first synchronous adapter subset rejects suspension with an active borrow. Later async profiles must specify pending ownership, cancellation, completion, buffering, and cleanup before claiming support. A Rust async function does not cross the component boundary without the selected adapter.

Typed streams are a library/interface objective, not an additional core expression node. An execution-only host may omit async facilities and still support the pure quotation subset.

### 14.6 Invocation outcomes and observation

### Requirement: H-OUTCOME-01
r[H-OUTCOME-01]

**H-OUTCOME-01.** An execution profile MUST distinguish normal return, permitted suspension, trap, budget exhaustion, cancellation, host deadline, and internal failure. A domain `Result` error remains a normal typed return. A non-return outcome MUST NOT expose a guessed output stack as `T`.

The host outcome record identifies the invocation, exact program/build context, execution profile, primary outcome, and ownership disposition. Suspension retains an explicit owner for pending state. It is not completion or a transferable guest continuation. Recovery uses the profile's explicit state policy, not an automatic reconstruction of consumed inputs.


<!-- cairn:scenario-links:start -->
#### Scenario: WORKER-01 for H-OUTCOME-01

- GIVEN the `Worker-Design` profile and every field of `input` in [WORKER-01](../../../specs/conformance/worker-cases.json)
- WHEN the `runtime` procedure for case `WORKER-01` runs against those inputs
- THEN the observations match every field of `expected` in case `WORKER-01`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: WORKER-08 for H-OUTCOME-01

- GIVEN the `Worker-Design` profile and every field of `input` in [WORKER-08](../../../specs/conformance/worker-cases.json)
- WHEN the `adapter` procedure for case `WORKER-08` runs against those inputs
- THEN the observations match every field of `expected` in case `WORKER-08`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: WORKER-09 for H-OUTCOME-01

- GIVEN the `Worker-Design` profile and every field of `input` in [WORKER-09](../../../specs/conformance/worker-cases.json)
- WHEN the `runtime` procedure for case `WORKER-09` runs against those inputs
- THEN the observations match every field of `expected` in case `WORKER-09`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: WORKER-10 for H-OUTCOME-01

- GIVEN the `Worker-Design` profile and every field of `input` in [WORKER-10](../../../specs/conformance/worker-cases.json)
- WHEN the `runtime` procedure for case `WORKER-10` runs against those inputs
- THEN the observations match every field of `expected` in case `WORKER-10`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: H-OUTCOME-02
r[H-OUTCOME-02]

**H-OUTCOME-02.** The host MUST distinguish requested, denied, admitted, completed, and externally unknown operation outcomes in its bounded observation record. Operation observations MUST retain invocation and operation-attempt correlation. A request or effect plan MUST NOT count as completed external work. Cancellation MUST NOT erase earlier observations or imply rollback of earlier effects.

If an observation limit prevents complete retention, the record MUST mark the omitted scope explicitly. Missing observations MUST NOT imply that no external action occurred. The profile MUST retain the primary invocation outcome and whether operation outcomes remain unknown. Observation export requires explicit authority and a declared policy for sensitive data.

The ownership transitions in [SPEC-R001](../resource-adapters/spec.md) govern cancellation/completion races for the selected async extension. These host records add no catch syntax, general handler, persistence guarantee, or automatic retry rule.

---


<!-- cairn:scenario-links:start -->
#### Scenario: WORKER-01 for H-OUTCOME-02

- GIVEN the `Worker-Design` profile and every field of `input` in [WORKER-01](../../../specs/conformance/worker-cases.json)
- WHEN the `runtime` procedure for case `WORKER-01` runs against those inputs
- THEN the observations match every field of `expected` in case `WORKER-01`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: WORKER-10 for H-OUTCOME-02

- GIVEN the `Worker-Design` profile and every field of `input` in [WORKER-10](../../../specs/conformance/worker-cases.json)
- WHEN the `runtime` procedure for case `WORKER-10` runs against those inputs
- THEN the observations match every field of `expected` in case `WORKER-10`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: OCTET-04 for H-OUTCOME-02

- GIVEN the `Octet-Adoption-Design` profile and every field of `input` in [OCTET-04](../../../specs/conformance/octet-adoption-cases.json)
- WHEN the `adapter` procedure for case `OCTET-04` runs against those inputs
- THEN the observations match every field of `expected` in case `OCTET-04`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

# Part D — Library boundary and conformance

## 15. Libraries and deferred features

The first library layer should provide data and program combinators over the same types and evaluation rules: collection traversal, predicates, folds, syntax transforms, and stack conveniences. No separate macro language is required to express `Program -> Program` builders or `Syntax -> Syntax` transformations.

[CALCULATOR.md](../calculator/spec.md) selects the first AI-authoring reference application and its exact numeric library direction. Arbitrary-precision integers and rationals remain library contracts outside bootstrap. The calculator expression grammar does not change Noble syntax, and exact division does not change core `I64` operators.

This revision deliberately does not define shell-compatible parsing, external-process syntax, reader macros, arbitrary compile-time I/O, general algebraic handlers, higher-rank programs, resource-capturing closures, a full borrow checker, distributed location types, live-resource migration, or durable workflows.

Remote execution may later accept portable code plus immutable input data. It must separately address capability delegation, dependency loading, retries, and effect semantics. Portable code does not imply transparent migration of a live environment.

These boundaries retain the RFC's focus. They are not assertions that the deferred features are impossible or undesirable.

## 16. Conformance requirements and release gates

### 16.1 Fixture format

The accompanying [conformance/cases.json](../../../specs/conformance/cases.json) contains newly authored bootstrap expectations. The safety and WIT files contain additional profile scenarios. [EVIDENCE.md](../evidence/spec.md) defines their schema and status rules. The missing historical core fixture package is not reconstructed or claimed as recovered. Each case has an identifier, applicable subsets, requirement references, and either a source submission or a structured harness scenario. Structured scenarios are specifications of test setup, not new language syntax.

Expected stacks are ordered bottom to top. A JSON integer fixture is identified explicitly as `I64`. Programs and syntax are represented by harness descriptions, not portable wire encodings. Production adapters must validate and prepare any injected programs; test setup does not authorize bypassing the invariants.

A negative static test succeeds only when checking rejects it before any body effect. A runtime/domain error test instead observes the declared execution outcome. Unsupported cases must be reported, not counted as passing.

### 16.2 Required behavior families

### Requirement: C-TEST-01
r[C-TEST-01]

**C-TEST-01.** Conformance testing MUST include nonconstant program construction, higher-order passing/return, collection storage, effect-order preservation, unused quotation checking, reflection stability, immutable resolution, and recursive resource eligibility. Literal arithmetic examples alone are insufficient.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: C-TEST-02
r[C-TEST-02]

**C-TEST-02.** Resource tests MUST cover success, domain-error, and abnormal-exit paths. Merely rejecting `dup` on a bare handle is insufficient if a pair, variant, closure, or suspension can lose its ownership accounting.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: C-TEST-03
r[C-TEST-03]

**C-TEST-03.** Backend tests MUST demonstrate that runtime `quote` and `compose` do not invoke `code.prepare`, and that the reflected recipe matches the executed program even when generated Wasm is optimized.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: C-TEST-04
r[C-TEST-04]

**C-TEST-04.** A report MUST distinguish semantic test execution, static document/fixture validation, and proof obligations. No result in this package establishes compiler conformance or type-system soundness.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### 16.3 Release gates

Before a stable v0 language specification can be declared, the project must complete and review:

| Gate | Required deliverable |
|---|---|
| G-01 | Full inference/checking algorithm, including rank-1 generalization, stack rows, effect constraints and eligibility; explicit candidate/acceptance boundary. Stable-core soundness evidence is required by G-08. |
| G-02 | Complete module, signature, and algebraic-type declaration syntax with resolution rules. |
| G-03 | Canonical node/value/type encoding, identity algorithm/versioning, recursive-reference handling, package dependency closure, round-trip laws, and cross-implementation vectors. |
| G-04 | Wasm calling convention, data/closure layout, memory management, generic lowering, and runtime composition prototype, with the SPEC-BE001 comparison record. |
| G-05 | Implement and validate the selected Component Model/WIT/WASI profile: lossless WIT type mapping, owned/borrowed resource adapters, world import/export compilation, pinned WASI 0.3/version matrix, native async boundary semantics, artifact/recipe trust policy, and import authorization rules. |
| G-06 | Trap/cancellation cleanup, async ownership races, session recovery, bounded execution/admission, observable failure records, and concrete resource-adapter contracts. |
| G-07 | Executed conformance suite and negative/property-based tests against a real compiler and Wasm backend. |
| G-08 | Reviewed Lean-checked metatheory and acceptance-checker soundness/termination for the complete subset declared stable, as scoped by SPEC-V001; a plan alone does not complete this gate. |
| G-09 | For advertised verification claims: exact coverage, evidence/assumption ledger, pinned tools, and explicit Rust/extraction/backend/host boundaries. `Contracts-Draft` additionally requires VC-GATE-01 through VC-GATE-03. |
| G-10 | Noble-safe claim closure: SPEC-S001 safety theorem coverage for the advertised subset, hostile-boundary validation tests, unsafe/native boundary inventory, and explicit standard-concurrency safety status where applicable. |
| G-11 | Component-profile conformance: executed WIT import/export, resource ownership/borrow, async/stream/future, WASI versioning, and cross-language interoperability tests against the selected Component/WASI profile. |

These gates are acknowledged design work. They are not permission for an implementation to violate the draft's established semantic contracts.

### 16.4 Verification conformance

### Requirement: C-VERIFY-01
r[C-VERIFY-01]

**C-VERIFY-01.** A verification report MUST identify the baseline specification revision and this verification extension revision. It MUST separate expected fixtures, executed tests, proof obligations and accepted proofs. The included verification package reports no completed language or implementation proofs.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: C-VERIFY-02
r[C-VERIFY-02]

**C-VERIFY-02.** Verification coverage MUST include runtime builder operands, reflection-sensitive observations, wrapping arithmetic, effect prefixes, recursive eligibility, hidden resource state, ordinary error results and applicable abnormal-cleanup boundaries. Excluded constructs MUST remain visible; a theorem for a smaller subset does not establish stable full-core coverage.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: C-VERIFY-03
r[C-VERIFY-03]

**C-VERIFY-03.** A claim of a verified Rust acceptance checker additionally requires correspondence for the actual implementation/configuration. A claim reaching Wasm additionally requires the relevant backend/loading correspondence. Assumptions, solver failures, timeouts and unsupported cases MUST NOT be reported as proved obligations.

See [SPEC-V001](../verification/spec.md), [SPEC-V002](../program-contracts/spec.md), [IMPL-V001](../verification-toolchain/spec.md) and [DEC-N001](../../../specs/DECISIONS.md) for the selected obligations, tool roles, policy and open work.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### 16.5 Component interface conformance

### Requirement: C-WIT-01
r[C-WIT-01]

**C-WIT-01.** Component-profile testing MUST include generated WIT imports, WIT-compatible Noble exports, lossless boundary type mapping, exact version/world selection, and rejection of incompatible export interfaces.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: C-WIT-02
r[C-WIT-02]

**C-WIT-02.** Resource tests MUST include owned-handle move-only behavior, attempted capture/duplication, borrowed-handle escape rejection, forged/stale/wrong-context handle rejection, and cleanup on normal and abnormal adapter exits.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: C-WIT-03
r[C-WIT-03]

**C-WIT-03.** Imported WIT functions MUST contribute their declared conservative Noble operation/effect requirements, and a type-correct import/effect declaration MUST NOT fabricate runtime authority absent from the linked host/profile.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: C-WIT-04
r[C-WIT-04]

**C-WIT-04.** WASI 0.3/native-async tests MUST include an `async func`, a `stream<T>`, and a `future<T>` crossing a component boundary while preserving Noble sequential ordering and the profile's cancellation/resource-cleanup rules.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: C-WIT-05
r[C-WIT-05]

**C-WIT-05.** Compatibility testing MUST distinguish WASI 0.2 and 0.3 profiles and MUST NOT count silent interface/version substitution as conformance. Exact package/profile versions used by a build must be reported.


<!-- cairn:scenario-links:start -->
Test design remains open for this requirement. No scenario or execution evidence is supplied.
<!-- cairn:scenario-links:end -->

### Requirement: C-WIT-06
r[C-WIT-06]

**C-WIT-06.** Interoperability testing MUST exercise at least one Noble component against an independently generated or implemented Component Model peer, and MUST keep Preserves protocol values distinct from live WIT/Noble resource authority.

---


<!-- cairn:scenario-links:start -->
#### Scenario: WI-15 for C-WIT-06

- GIVEN the `Component-Sync-Bootstrap` profile and every field of `input` in [WI-15](../../../specs/conformance/wit-wasi-cases.json)
- WHEN the `runtime` procedure for case `WI-15` runs against those inputs
- THEN the observations match every field of `expected` in case `WI-15`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

# Appendix A — Worked examples (informative)

## A.1 Construction is not invocation

```text
41 [ 1 + ]
```

The resulting stack contains `41` and a program requiring an integer input. No addition has run. Appending `run` yields `42`.

```text
[ "audit" test.emit ] drop
```

In a test environment declaring `test.emit : Text -- ! {test.emit}`, the quotation is checked and discarded without issuing the event. The declaration being available is distinct from executing its body.

## A.2 Composition order

```text
20 [ 1 + ] [ 2 * ] compose run
```

This yields `42`. Reversing the two programs yields `41` for input `20`, demonstrating that composition is ordered even though effect summaries use set union.

## A.3 Runtime capture

Suppose a compiled word receives a runtime integer `n` on top of its construction stack:

```text
quote [ + ] compose
```

It returns a program that adds `n` to a later integer input. Choosing `n = 2` after compilation and invoking the returned program on `40` yields `42`. This operation does not need `code.prepare`.

## A.4 Program returned by an ordinary word

Install the separate declaration:

```text
def twice [ dup compose ]
```

Then submit:

```text
20 [ 1 + ] twice run
```

The result is `22`. `twice` is an ordinary checked definition, not a parser feature.

## A.5 Program stored in a list

```text
20 nil [ 1 + ] swap cons [ ] [ drop run ] list.case
```

The nonempty branch receives the program followed by the empty tail. It drops the data-only tail and calls the program on `20`, yielding `21`. A conformance harness must also test a list assembled or selected after compilation.

## A.6 Resource input without resource capture

Given the illustrative `fs.read` contract, this is a reusable program:

```text
[ "main.rs" fs.read ]
```

It requires a directory input and returns that directory plus a result. The same kind of program can be used with different authorized directory inputs. Quoting the directory itself is rejected.

## A.7 Invalid code cannot hide in a discarded quotation

```text
[ "wrong" 1 + ] drop
```

The quotation body is ill-typed and the whole submission is rejected. Representing that text as inert `Syntax` would be allowed, but calling the syntax directly would still be rejected.

## A.8 Reflection and editing

```text
[ 1 + ] dup reflect
```

The stack retains the program and an inert resolved syntax value. A transform of that syntax may produce a new candidate. It cannot mutate the retained program; preparing the candidate must check it anew against an expected interface.

---

# Appendix B — Source traceability (informative)

This draft is grounded in RFC 0001. It does not replace the RFC's record of rationale with claims that all new details were already agreed.

| RFC section | Content carried forward | Specification sections |
|---|---|---|
| 1–2 | Small composable core; layer and application boundaries | 1–2, 15 |
| 3 | Left-to-right stack semantics; three expression forms; resolved definitions | 3, 5, 10 |
| 4 | Program interfaces, `run`, `compose`, `quote`, explicit capture, first-class guarantee | 4, 6 |
| 5 | Recipes, `Syntax` versus `Program`, reflection, checked preparation | 11 |
| 6 | Wasm-only execution, runtime composition, staging, session state | 13–14 |
| 7 | Rank-1 static core, basic data, effect bounds; numeric behavior initially open | 4, 7–8 |
| 8 | Move-only resources, no capture, explicit release and abort cleanup | 9, 14 |
| 9 | Small control basis, ordinary libraries, deferred stream semantics | 7, 15 |
| 10 | Names-independent identities; separate definition/build/artifact concepts | 10, 12 |
| 11 | Semantic laws and acceptance examples | 5, 16, conformance fixtures |
| 12–13 | Four-document sequence and unresolved release decisions | Parts A–D, Appendix C |
| 14 | Architectural reference catalogue | Original RFC; intended roles summarized in 13.4 |

New elaborations in this revision include the lexical microprofile, wrapping arithmetic selection, bootstrap eliminator payload order, requirement identifiers, formal judgment notation, diagnostic categories, structural normalization for quoted program values, and fixture format. Their status and rationale are recorded in `DECISIONS.md`.

# Appendix C — Open decisions (informative)

The most immediate next document is the checker specification: explicit judgments and solving/generalization rules for stack tails, first-class monomorphic programs, eligibility constraints, and effect bounds. The current requirements deliberately do not assert that these rules form a complete inference algorithm or that soundness has been proved.

Canonical serialization and the internal Noble Wasm program ABI should follow as separate reviewable specifications. The external component boundary is selected by SPEC-W001: WIT + the Component Model, with the stable WASI 0.3 family as the preferred host profile. In particular, stable program identity, closed generic instantiations, recursive references, and runtime-built composition must be tested together rather than chosen independently.

The concrete host/resource profile must then settle exception-free normal ownership, trap retirement, REPL recovery, and authorization at imported interfaces. Async streams and remote code transport follow those foundations rather than enlarge this kernel draft.

The language's public name, file extension, package namespace, release governance, and implementation repository layout remain unassigned. This document does not silently choose them from the enclosing project's name.

---

*End of SPEC-0001, revision 0.1.0-draft.5.*
