# Language Specification
## Version 0.1.0-draft.1 — Minimal typed programs-as-data core

| Field | Value |
|---|---|
| Document identifier | SPEC-0001 |
| Revision | 0.1.0-draft.1 |
| Date | 2026-09-09 |
| Project | noble |
| Language name | Unassigned; the project name is not a language-naming decision |
| Status | Initial working draft in the official specification workstream |
| Basis | [RFC 0001 — Minimal typed programs-as-data core](rfc-0001-minimal-programs-as-data.md) |
| Scope of this revision | Kernel semantics and type obligations; programs-as-data contract; execution requirements |
| Not yet frozen | Complete inference algorithm, module/type-declaration syntax, canonical byte encoding, Wasm ABI, host profiles |

This document begins the specification; it is not a release announcement or a claim that a conforming compiler exists. Its requirements are normative for the draft subsets it defines. Publication of this draft does not establish a soundness proof, ratify unresolved decisions, or amend the original RFC.

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

**MUST** and **MUST NOT** state requirements for the applicable draft subset. **SHOULD** states a recommendation for which a documented reason may justify a different choice. **MAY** identifies a permitted choice. Explanatory paragraphs marked **Rationale** or **Example** are informative and do not add requirements.

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

`Core-Draft` covers the expression microprofile, stack semantics, program builders, basic data/control contracts, and static-checking obligations. `ProgramsData-Draft` adds reflection, immutable resolution, and structural identity invariants. `Resources-Draft` adds the ownership obligations in section 9. `Preparation-Draft` adds the explicit compiler-service contract. `Wasm-Draft` adds the execution requirements in section 13.

These are development and reporting labels, not five different source languages. A report MUST list the subsets it implements. This revision does not permit an unqualified claim of full language or cross-implementation binary conformance: several required release gates remain open.

**K-STATUS-01.** A test report MUST distinguish executed results from expected outcomes and unsupported cases. The fixture file accompanying this draft defines expected behavior; it does not report execution of a language implementation.

## 2. Scope and semantic commitments

**K-SCOPE-01.** The kernel consists of values, ordered stack transformations, program construction and calls, data construction and elimination, definition references, and effect/resource-use rules. Domain objects such as terminals, clusters, deployments, and agents MUST NOT be required kernel syntax or semantics.

**K-SCOPE-02.** Pure data and program values MUST be immutable. A transform creates a new value; it cannot mutate an existing checked program or a definition that an invocation already references.

**K-SCOPE-03.** Programs MUST be first-class: they may be arguments, results, and elements of homogeneous data collections when their types permit it. Correct execution MUST NOT depend on a caller recognizing a quotation literal syntactically.

**K-SCOPE-04.** Sequential composition MUST NOT imply concurrency, distribution, transactionality, retry, or persistence. Those behaviors require explicitly specified operations and profiles.

**K-SCOPE-05.** Constructing or inspecting a program MUST NOT execute its latent host operations, acquire authority, or publish its captured values.

**Rationale.** Smallness means a small number of independent mechanisms. Library vocabulary is not counted as additional syntax merely because it requires an explicit host facility.

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

A source unit is UTF-8. Outside text literals, the microprofile recognizes ASCII space, tab, carriage return, and line feed as whitespace. `//` begins a comment outside a text literal and continues to the next line terminator or end of input. Non-ASCII whitespace is not silently normalized into a separator.

A word token begins with an ASCII letter, underscore, or one of `+ - * / = < > ? !`. Its remaining characters may additionally include ASCII digits and `.`. The maximal token is scanned before it is classified; a complete integer token has precedence over word classification. `//` always starts a comment outside a string. The spellings `def`, `true`, and `false` are reserved. Additional module keywords are not defined here.

Examples of word tokens are `square`, `fs.read`, `healthy?`, `list.case`, `+`, and `-`. `1+` is not shorthand for `1 +` and is rejected. A negative integer is one literal; subtraction is the separate word `-`.

**K-SYN-01.** Parsing MUST preserve the three semantic expression forms: literal, resolved word invocation, and quotation. There is no infix precedence, implicit process invocation, or shell expansion in this microprofile.

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

**K-SYN-02.** A definition submission MUST be checked without executing the body. A tool MUST install the name mapping only after successful checking and preparation according to its environment policy.

**K-SYN-03.** A tool MUST determine the whole submission boundary before executing its expression body. A static error anywhere in that submission prevents execution of all of that submission's body, including effects textually preceding the error.

Previously completed submissions are not rolled back. A runner that checks a whole file as one unit must document that larger boundary.

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

**K-TYPE-01.** Input and output stack shapes MUST be checked in order and at their full arity. An unmentioned prefix MUST pass through without being implicitly read, duplicated, or discarded.

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

**K-TYPE-02.** The checker MUST enforce eligibility recursively. A representation change or opaque wrapper MUST NOT launder a resource into capturable data.

A portable semantic representation is defined abstractly in section 11. It does not promise an interoperable binary encoding; section 12 explicitly leaves that encoding open.

### 4.4 Numeric semantics

**K-NUM-01.** In this draft, `+`, `-`, and `*` operate on two `I64` values and wrap modulo `2^64`, interpreted back as a signed 64-bit value. Operand order is the deeper value followed by the top value. The result MUST NOT depend on debug versus optimized compilation.

For a mathematical result `z`, the signed result is:

```text
((z + 2^63) mod 2^64) - 2^63
```

`=` in the bootstrap numeric vocabulary compares two `I64` values and returns `Bool`. It is not an implicit generic equality operator for every type. Division, floating point, and the detailed checked-arithmetic library are not selected in this revision.

**Draft decision D-03.** Wrapping arithmetic selects a candidate from the RFC. Checked operations returning `Result` remain the intended library complement; their names and full contract are deferred.

## 5. Evaluation and observable behavior

### 5.1 Evaluation order

**K-EVAL-01.** An expression body executes left to right. A literal pushes its value. Invoking a word performs its declared stack transformation. A quotation pushes a program value without executing its enclosed body.

**K-EVAL-02.** A call MUST use the resolved references in its program. It MUST NOT perform a fresh lookup of source names in a mutable namespace.

**K-EVAL-03.** Evaluation of one word finishes normally before the next sequential word begins. Suspending in an explicitly asynchronous host operation does not authorize reordering subsequent words in the same sequential computation.

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

<call; C ; s p ; H ; t>
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

**K-EVAL-04.** If `p` terminates abnormally, the second program `q` in `compose(p,q)` MUST NOT start. Earlier host effects are not rolled back by the language.

The composition laws use failure-propagating sequencing:

```text
run(empty, s) = success(s)
run(compose(p,q), s) = run(p,s) then-on-success run(q)
run(quote(v), s) = success(s v)
```

Host traces concatenate in order. Where both executions terminate normally under the same host responses, composition is associative and the empty program is an identity. These laws do not equate elapsed time, allocation strategy, or behavior under different resource quotas. Recipe associativity is separately specified in section 11.

### 5.4 Errors and limits

**K-ERROR-01.** A static rejection MUST prevent execution of the rejected submission. At minimum, tools MUST distinguish parsing, resolution, stack/type incompatibility, eligibility/resource-use violations, and effect-bound violations in their diagnostics. Exact wording and numeric diagnostic codes are not frozen.

**K-ERROR-02.** A well-checked invocation with conforming host bindings MUST NOT encounter a language-level stack underflow, wrong operand type, or use of a consumed resource on a normal path. Those outcomes indicate an implementation or binding defect, not a permitted fallback to dynamic typing.

**K-ERROR-03.** Empty effect bounds do not guarantee termination or freedom from allocation failure, quotas, traps, or side channels. Such failures MUST NOT be misreported as ordinary successful results.

A host/execution profile specifies handling of traps, cancellation, and resource exhaustion. This draft does not specify recoverable exceptions or user-defined continuation handlers.

## 6. Program construction and execution

### 6.1 Program interface

A program has type `Program<S,T,e>`. `S` and `T` describe the stack at invocation; they do not describe the current stack on which the program value is constructed. `e` is latent until invocation.

**K-PROG-01.** A checked program MUST carry or be associated with sufficient trusted type and executable information to support its interface, and sufficient recipe information for reflection. These need not be stored redundantly or in a particular memory layout.

### 6.2 Quotation literal

If body `B` checks as `S -- T ! e`, the expression `[ B ]` has effect-free construction behavior:

```text
R -- R Program<S,T,e> ! {}
```

`R` is the surrounding construction stack. It is independent of the body's invocation stack.

**K-PROG-02.** Quotation literals MUST NOT capture the current data stack implicitly. Missing inputs in a quotation body become requirements of its program interface; they are not obtained from values underneath the quotation during construction.

**K-PROG-03.** The entire quotation body MUST check even when the quotation is never called or is later discarded. Unchecked code is represented as `Syntax`, not as a quotation literal accepted without checking.

### 6.3 `call`

```text
call : S Program<S,T,e> -- T ! e
```

`call` consumes the program value and invokes it against the remaining stack. Consuming a reusable data value does not invalidate any independently duplicated copy of it.

**K-PROG-04.** `call` MUST invoke an already prepared executable program. It MUST NOT implicitly compile source, fetch code, install interfaces, acquire capabilities, or reinterpret arbitrary data as instructions. An implementation may use its Wasm engine's ordinary execution preparation internally; that is not a hidden call to the language's `code.prepare` service.

### 6.4 `compose`

```text
compose : R Program<A,B,e> Program<B,C,f>
          -- R Program<A,C,union(e,f)> ! {}
```

The deeper program runs first, then the top program. `compose` produces a new program value. It does not run either operand.

**K-PROG-05.** The shared intermediate stack `B` MUST unify completely, including resource types and positions. An incompatible pair is rejected statically, not converted to a runtime compatibility test.

**K-PROG-06.** Composing values selected or constructed at runtime MUST work over their statically known interfaces without a source compiler service. A literal-only implementation does not conform.

### 6.5 `quote`

```text
quote : R a -- R Program<S,S a,{}> ! {}
        where Capture(a)
```

`quote` captures the top value explicitly and produces a program that pushes that value when called. The fresh invocation prefix `S` is unrelated to the construction prefix `R` unless later type constraints relate them.

**K-PROG-07.** `quote` MUST accept only capturable data, MUST preserve the captured value immutably, and MUST NOT publish or otherwise export it. A program may contain private data, including secrets represented as ordinary text; capturability is not a confidentiality guarantee.

**K-PROG-08.** A live resource MUST NOT be captured, including through a containing value or a program environment. A quotation may instead require that resource as an invocation input.

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

**Example.** `20 [ 1 + ] twice call` yields `22`. This definition is not applicable to every program; it requires matching input and output stack shapes.

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

**K-STACK-01.** A value held by `dip` MUST be inaccessible to the enclosed program. It MUST be restored exactly once after normal return. On an abnormal outcome, a held resource remains subject to host abort cleanup; it is not silently resurrected as a valid session value.

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

**K-DATA-01.** Constructing and eliminating data MUST move its payloads without implicit duplication or loss of resources. No hidden mutable cell is introduced by constructing a pair or variant.

### 7.3 Boolean branching

The bootstrap `if` contract is:

```text
if : S Bool Program<S,T,e> Program<S,T,f>
     -- T ! union(e,f)
```

The first program runs for `true`; the second runs for `false`. This is a word, not a fourth expression form. It may be derived through the boolean schema and a sum eliminator.

**K-CONTROL-01.** Only the selected branch executes, but both branches MUST type-check and their requirements MUST contribute to the conservative effect bound. A resource may follow different valid operations in the branches, but each branch must account for every incoming ownership obligation and return the same interface.

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

**K-DATA-02.** Lists of compatible programs MUST be supported. Their elements share one instantiated program interface; a list is not a means to erase heterogeneous stack effects into `Any`.

General `map`, `fold`, `filter`, and iteration libraries are not defined by this revision. Their later definitions should use the same data and program mechanisms.

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

**K-CHECK-01.** Word schemes MUST be instantiated freshly at independent named uses. Values duplicated from one first-class program share its instantiated type constraints; duplication MUST NOT grant two independent polymorphic instantiations of a monomorphic value.

**K-CHECK-02.** Nonrecursive named definitions may generalize eligible variables not fixed by their environment. Recursive definitions require an explicit signature and check recursive calls against that signature without polymorphic recursion. The full inference algorithm and formal soundness argument are required follow-up work; this draft does not claim them established.

**K-CHECK-03.** Implementations MUST reject ill-founded type equations rather than constructing infinite stack or value types implicitly. Whether the complete solver uses a specific unification algorithm is not frozen.

### 8.2 Effects

Effect bounds are conservative sets of stable host-operation identities, with variables in generic signatures. Union is associative, commutative, and idempotent as a *summary*. Execution order remains sequential and noncommutative.

**K-EFFECT-01.** A program's effect bound MUST include every host-operation requirement that its body may perform under the declared contracts. Construction, `compose`, `quote`, and `reify` do not inherit the latent effects of the programs they manipulate.

**K-EFFECT-02.** A checker may widen an effect bound conservatively, but MUST NOT narrow it without establishing that all possible body requirements remain included. This restricted effect-bound relation is not general value subtyping.

**K-EFFECT-03.** Calling a program propagates its latent bound. Invoking a higher-order word propagates the effects of programs it may call according to that word's signature. Discarding a checked quotation without calling it does not perform its latent effects.

**K-EFFECT-04.** An effect annotation, a successful check, and possession of a program value MUST NOT be treated as authorization to perform the named operation. The host enforces authority separately.

No generic handler syntax, continuation capture, or multi-shot resumption is defined. A host can supply a mock implementation of an interface, but this does not introduce source-level algebraic handlers implicitly.

## 9. Resource discipline

### 9.1 Ownership invariant

**K-RES-01.** A live guest resource handle has one ownership obligation. On every normal path it MUST be moved onward, consumed by a declared operation, or explicitly released. Generic data discard is not an implicit resource release operation.

**K-RES-02.** `dup`, generic `drop`, `quote`, and resource serialization MUST be rejected for resource-bearing values. Moving, storing in an ownership-preserving data structure, destructuring, and passing a resource as an invocation input are permitted when the obligations remain accounted for.

**K-RES-03.** A host binding's signature MUST account for resource ownership on every normal result alternative. A successful `Result` and an error `Result` are both normal returns for this purpose.

For example, a borrowing operation may expose a move-threaded interface:

```text
fs.read : S Directory Text
          -- S Directory Result<Bytes,FsError> ! {fs.read}
```

Both success and failure return the directory handle exactly once. A different operation may consume a handle and encode replacement ownership in its result, but that different contract must be explicit.

### 9.2 Branches and aggregate data

Both branches must account for their incoming resources. This does not require identical side effects or returning the same physical handle. It requires a valid ownership transition and a common output type on each branch. A released handle cannot remain usable in the output stack.

A returned aggregate containing a handle carries its ownership obligation with it. The language must not hide such a handle in a value whose type claims `Data` or `Capture`.

### 9.3 Abnormal termination

**K-RES-04.** The host MUST retain sufficient ownership accounting to retire invocation-owned handles when guest execution aborts. Cleanup MUST NOT depend on successfully resuming the guest. Handle retirement must be idempotent with respect to double release; the host must not expose a retired handle as live.

This is a requirement on local handle accounting, not a guarantee that a remote cleanup request succeeds exactly once. External effects, network partitions, revocation, and process failure remain host-protocol concerns. Cleanup timing, session ownership boundaries, and recovery policies must be fixed in the execution profile before release.

**Rationale.** This is a move-only, explicit-release normal-path discipline. It is not accurately described as unrestricted affine discard, nor does it prove global exclusivity of the underlying external resource.

## 10. Definitions and resolution

**K-DEF-01.** Checking a submission MUST resolve names against an explicit, immutable namespace snapshot. A successful definition contains exact dependency identities, not late-bound source-name lookups.

**K-DEF-02.** Renaming or rebinding a source name MUST NOT mutate an existing definition or the meaning of an already prepared program. Editing a body creates a new resolved definition; dependent definitions are changed only through explicit rebuilding or replacement.

**K-DEF-03.** A definition declaration does not execute its body. Namespace changes are tooling operations outside ordinary pure guest evaluation.

For self-recursion, a checked declaration carries an explicit interface and a distinguished self reference within its identity scope. This revision permits no mutual-recursion groups. Concrete annotation syntax and canonical encoding of self references are release-blocking open items. A reflected self reference that leaves its owner context must retain or be resolved with its owner identity; a bare self token cannot be rebound accidentally during preparation.

The source-name spelling of a definition is not the definition's semantic identity. Type schemas, builtin semantics, host contracts, and dependency identities participate in the identity rules in section 12.

---

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

**P-RECIPE-01.** Every first-class program in the programs-as-data subset MUST have a finite, inspectable recipe describing its language-level composition. A reference may identify another definition without expanding that definition's body. Recursion must not force infinite expansion during inspection.

**P-RECIPE-02.** Reflection MUST NOT expose or depend on executable addresses, closure memory layout, allocation identities, or optimizer-generated instruction layout. Optimizing execution MUST preserve the specified recipe observation.

**P-RECIPE-03.** For compatible programs, the recipe of `compose(p,q)` MUST be the ordered concatenation of their recipe sequences, modulo the specified structural normalization. Association of runtime composition adapters MUST NOT be observable as additional language instructions.

The empty recipe is the concatenation identity. Concatenation grouping is normalized; a quotation boundary is not flattened into its enclosing sequence.

**P-RECIPE-04.** For ordinary non-program data `v`, the recipe of `quote(v)` is one literal node containing `v` with its semantic schema information. For a program value `p`, the recipe of `quote(p)` is one quotation node representing `p`'s recipe and required interface witnesses. A program nested inside an aggregate is encoded as program data, not as an instruction to run it.

This normalization aligns a captured program with a source quotation that pushes that program. It does not inline named definition bodies or perform algebraic simplification. For example, `[ 1 1 + ]` and `[ 2 ]` may have the same normal result but different recipes and definition identities.

### 11.3 `reify`

```text
reify : R Program<S,T,e> -- R Syntax ! {}
```

**P-REFLECT-01.** `reify` MUST return an inert, resolved view of the program's recipe without executing it. It consumes one program value; callers may use `dup reify` to retain a reusable copy.

**P-REFLECT-02.** Reifying a reference MUST NOT implicitly fetch or expand an external definition. Explicit code-store access requires its own host operation. A native host function is reflected as its declared host-operation identity and interface contract, not as inspectable Rust implementation code.

**P-REFLECT-03.** Editing a returned syntax value MUST NOT mutate the checked program from which it was obtained. No syntax constructor, edit, deserializer, or annotation grants executable status by itself.

For example, reifying `[ 1 + ]` yields a sequence containing a literal `1` and the resolved integer-addition reference. Replacing the integer literal with text produces syntax that must fail checking for that addition; it does not change the old program.

### 11.4 Typed builders versus edited syntax

`quote` and `compose` construct checked program values under static constraints. Arbitrary syntax transformations need not preserve these constraints and therefore cross a checking boundary before execution.

**P-STAGE-01.** `call` MUST reject a `Syntax` value as a static type error. There is no automatic coercion from syntax, source text, a list of nodes, or unverified artifact metadata to `Program`.

**P-STAGE-02.** Deserializing portable code MUST produce an untrusted description/package until validation and preparation establish its executable interface. A claimed signature or digest in the payload is not a proof.

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

**P-PREPARE-01.** Preparation MUST resolve references using an explicit context, validate structure and schemas, check stack interfaces and resource eligibility, and verify that inferred requirements are within `AllowedEffects`. A failed check MUST return a diagnostic without running the candidate body.

**P-PREPARE-02.** Successful preparation MUST return an executable program at the expected instantiated interface. It MUST NOT silently reinterpret a different stack transformation through `Any` or runtime argument guessing.

**P-PREPARE-03.** Preparing code is an explicit host operation. Its permitted code-store access, compilation, allocation, and cache writes belong to the compiler capability's documented contract. These service effects are distinct from the candidate program's latent effects. The compiler service MUST NOT run arbitrary user code as an undocumented checking or macro-expansion step.

**P-PREPARE-04.** The service MUST thread or consume its resource capability according to its declared success and failure contract. The schematic adapter above returns the compiler capability on both normal alternatives.

A development tool may infer an unknown candidate signature for display. A statically checked guest must still choose an expected interface before obtaining a callable value. A minimal execution-only host need not expose preparation at all.

### 11.6 Programs are not ambient authority

**P-AUTH-01.** Captured program data MUST NOT contain live capability handles. A program may name a host contract and require resources as inputs, but naming the contract does not create the resource or permission to use it.

A portable program can still contain sensitive bytes or text, including credentials whose use a host may allow. The resource restriction is not an information-flow or secret-detection system. Hosts must consider indirect authority through filesystem, process, network, or credential access rather than checking only whether an import has a particular name.

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

**P-ID-01.** Renaming source names, changing formatting or source spans, and changing optimizer layout MUST NOT change a definition's semantic identity. Changes to its canonical resolved body, relevant type/schema identities, builtin/host semantics, or resolved dependencies MUST be reflected in that identity.

**P-ID-02.** A compiler revision or optimization choice may change the build key and artifact identity without changing the source definition identity. Artifact bytes MUST NOT be used as a substitute for source-definition identity.

**P-ID-03.** Different captured data MUST remain distinguishable in portable program-value identity. Function-template identity alone is insufficient.

### 12.2 Canonicalization requirements

Canonicalization must specify node ordering, literal encodings, schema identities, alpha-renaming of type/stack/effect variables, effect-set ordering, resolved references, quotation nesting, and distinguished recursive references. Concatenation grouping must normalize as required by `P-RECIPE-03`.

It must exclude incidental source names, locations, allocation addresses, and optimized machine layout. It must not attempt to equate arbitrary programs merely because they appear to compute equivalent functions.

**P-ID-04.** Until a canonical byte format, encoding version, hash algorithm, and test vectors are specified, implementations MUST label stored IDs and encodings experimental and MUST NOT claim cross-implementation stable content hashes. The repository package's file-integrity checksum is not a choice of language hash algorithm.

**P-ID-05.** Definition identity MUST NOT be treated as a signature, provenance certificate, authorization token, or justification for caching an effectful result. Trust and execution policy are separate checks.

Untrusted packages must have their contents validated against their claimed identities and contracts under the selected loading policy. A loader must not trust a type/effect manifest solely because it accompanies valid Wasm bytes.

### 12.3 Namespace and storage policy

A name maps to an immutable identity within a namespace snapshot. Updating a snapshot does not mutate that identity. Local files, a database, and a remote content-addressed store may implement the same abstract lookup interface; none is a mandatory language data structure.

This revision does not specify registry discovery, synchronization, garbage collection of code stores, signing policy, or distributed updates. No running program is automatically hot-replaced when a name is rebound.

---

# Part C — Wasm execution and host contracts

## 13. Wasm execution requirements

### 13.1 Execution substrate

**W-EXEC-01.** The production execution target of this specification is WebAssembly. A conforming `Wasm-Draft` implementation MUST compile checked language computation to Wasm execution rather than require a second production AST or bytecode evaluator to implement source semantics.

Compiler-generated adapters, allocation routines, closures, data structures, cleanup support, and dispatch over already compiled operations are permitted support code. The absence of a second VM does not mean the absence of memory management or language support machinery.

A reference evaluator may exist strictly as a testing oracle. It is not evidence that the Wasm requirements have been implemented.

### 13.2 Runtime-built program values

**W-EXEC-02.** `quote`, `compose`, and `call` MUST support genuinely runtime-created values over the prepared vocabulary. This includes a captured scalar supplied after compilation, a runtime-selected program, and a program retrieved from a homogeneous collection. The implementation MUST NOT invoke the source compiler to perform those operations.

**W-EXEC-03.** Program execution and recipe reflection MUST remain consistent when optimization is disabled or the builder operands are not constant. Constant folding alone is insufficient evidence of first-class program support.

**W-EXEC-04.** Compiled program values MUST have a specified calling convention and environment representation before ABI conformance can be claimed. The implementation MUST NOT assume that arbitrary Wasm function signatures are interchangeable.

The precise convention is open. Candidate techniques must account for monomorphic program interfaces, generic definition instantiations, captured data, dynamically long compositions, recipe sharing, reentrancy, and host/resource inputs. This document does not select a heap layout, GC strategy, or closure allocation algorithm.

### 13.3 Definitions versus components

**W-COMP-01.** Definition identity and deployment/isolation granularity MUST be separable. A definition having an identity MUST NOT require each invocation to cross a component boundary. Multiple definitions may compile into one component.

**W-COMP-02.** Internal `Program<S,T,e>` values MUST NOT be exported as untyped integer addresses or unspecified generic function values. A cross-component interface must use a separately specified concrete adapter, resource contract, or portable code package.

Ordinary internal program calls and code transport are separate operations. A destination may need to resolve exact dependencies, prepare executable code, and provide authorized resources before a transported description is executable.

### 13.4 Compiler and implementation organization

The intended implementation language is Rust. A compiler library, an embedding library, and a command-line tool may share the same checker and backend without creating separate source semantics.

The supplied reference set is carried forward from RFC 0001: `wasm-tools` for backend tooling, `wit-bindgen` as a guest-binding reference, Wasmtime as the intended execution engine, `wstd` as an async library/API reference, and `cap-std` as a host-adapter reference. These are intended roles, not a version-compatibility claim. This revision pins no crate or interface versions.

The language specification does not require every application to expose the same filesystem or network interfaces. Application-specific modules are adapters, not kernel extensions.

## 14. Embedding, host boundaries, and sessions

### 14.1 Host contracts

**H-HOST-01.** Host operations MUST have stable declared interfaces covering argument/results, effects, and resource disposition. Bindings MUST validate incoming resources and return values consistent with those contracts.

**H-HOST-02.** Runtime authorization MUST be enforced independently of guest type/effect claims. Forged, stale, cross-context, or wrongly typed resource handles MUST NOT grant access to a host object.

**H-HOST-03.** The host MUST describe indirect authority exposed through its operations. Omitting a specifically named import is not sufficient isolation when another permitted operation can perform the same consequential action.

A guest's opaque directory handle may designate a constrained scope. An implementation must enforce that scope in the host rather than convert the handle into an unchecked path string. Details of path resolution, read-only policies, revocation, and filesystem behavior belong to the later filesystem profile.

### 14.2 Sessions and the REPL

**H-REPL-01.** A REPL MUST retain persistent stack values in explicit session-owned storage. Its semantics MUST NOT rely on a returned Wasm function leaving a persistent operand stack available for the next submission.

**H-REPL-02.** A statically rejected submission MUST leave the prior session stack and namespace snapshot unchanged and MUST execute no user body effects. Compilation/cache activity within an explicitly invoked tool is not a body effect and must be documented separately.

**H-REPL-03.** A runtime failure after checking MUST NOT be represented as an automatic rollback of earlier host effects. The tool MUST report failure and apply the execution profile's resource retirement and session recovery policy before allowing further use of affected values.

The exact recovery policy is open: a future profile must specify which state survives, which invocation-owned resources are retired, and how a resumed session obtains a sound stack type. It may not merely reuse potentially consumed handles because their previous stack positions are known.

### 14.3 Limits and loading

**H-LIMIT-01.** An execution profile MUST state its policies for memory, execution budget, host-operation duration, and outstanding resources. Language `Data` or empty effects do not imply unbounded allocation is safe.

**H-LOAD-01.** A loader MUST separate artifact validation, source/type provenance policy, interface compatibility, and resource authorization. A valid Wasm module is not by itself proof that a separately supplied language recipe, effect bound, or reflection manifest is truthful.

The precise trust chain between source checking, recipe metadata, compiler output, and loaded artifacts must be specified before accepting arbitrary third-party artifacts as trusted `Program` values.

### 14.4 Async execution and streams

No async ABI or stream surface API is frozen here. A future profile must specify suspension, cancellation propagation, ownership of pending resources, completion failures, fairness, bounded buffering, and ordering. A Rust async function or stream must not be assumed to cross the component boundary unchanged.

Typed streams are a library/interface objective, not an additional core expression node. An execution-only host may omit async facilities and still support the pure quotation subset.

---

# Part D — Library boundary and conformance

## 15. Libraries and deferred features

The first library layer should provide data and program combinators over the same types and evaluation rules: collection traversal, predicates, folds, syntax transforms, and stack conveniences. No separate macro language is required to express `Program -> Program` builders or `Syntax -> Syntax` transformations.

This revision deliberately does not define shell-compatible parsing, external-process syntax, reader macros, arbitrary compile-time I/O, general algebraic handlers, higher-rank programs, resource-capturing closures, a full borrow checker, distributed location types, live-resource migration, or durable workflows.

Remote execution may later accept portable code plus immutable input data. It must separately address capability delegation, dependency loading, retries, and effect semantics. Portable code does not imply transparent migration of a live environment.

These boundaries retain the RFC's focus. They are not assertions that the deferred features are impossible or undesirable.

## 16. Conformance requirements and release gates

### 16.1 Fixture format

The accompanying `conformance/cases.json` contains expected results and rejection conditions. Each case has an identifier, applicable subsets, requirement references, and either a source submission or a structured harness scenario. Structured scenarios are specifications of test setup, not new language syntax.

Expected stacks are ordered bottom to top. A JSON integer fixture is identified explicitly as `I64`. Programs and syntax are represented by harness descriptions, not portable wire encodings. Production adapters must validate and prepare any injected programs; test setup does not authorize bypassing the invariants.

A negative static test succeeds only when checking rejects it before any body effect. A runtime/domain error test instead observes the declared execution outcome. Unsupported cases must be reported, not counted as passing.

### 16.2 Required behavior families

**C-TEST-01.** Conformance testing MUST include nonconstant program construction, higher-order passing/return, collection storage, effect-order preservation, unused quotation checking, reflection stability, immutable resolution, and recursive resource eligibility. Literal arithmetic examples alone are insufficient.

**C-TEST-02.** Resource tests MUST cover success, domain-error, and abnormal-exit paths. Merely rejecting `dup` on a bare handle is insufficient if a pair, variant, closure, or suspension can lose its ownership accounting.

**C-TEST-03.** Backend tests MUST demonstrate that runtime `quote` and `compose` do not invoke `code.prepare`, and that the reflected recipe matches the executed program even when generated Wasm is optimized.

**C-TEST-04.** A report MUST distinguish semantic test execution, static document/fixture validation, and proof obligations. No result in this package establishes compiler conformance or type-system soundness.

### 16.3 Release gates

Before a stable v0 language specification can be declared, the project must complete and review:

| Gate | Required deliverable |
|---|---|
| G-01 | Full inference/checking algorithm, handling of rank-1 generalization, stack rows, effect constraints, and eligibility; soundness argument or clearly scoped proof plan. |
| G-02 | Complete module, signature, and algebraic-type declaration syntax with resolution rules. |
| G-03 | Canonical node/value/type encoding, identity algorithm/versioning, recursive-reference handling, and cross-implementation vectors. |
| G-04 | Wasm calling convention, data/closure layout, memory management, generic lowering, and runtime composition prototype. |
| G-05 | WIT/component lowering, artifact/recipe trust policy, version/profile matrix, and import authorization rules. |
| G-06 | Trap/cancellation cleanup, session recovery, quotas, and concrete resource-adapter contracts. |
| G-07 | Executed conformance suite and negative/property-based tests against a real compiler and Wasm backend. |

These gates are acknowledged design work. They are not permission for an implementation to violate the draft's established semantic contracts.

---

# Appendix A — Worked examples (informative)

## A.1 Construction is not invocation

```text
41 [ 1 + ]
```

The resulting stack contains `41` and a program requiring an integer input. No addition has run. Appending `call` yields `42`.

```text
[ "audit" test.emit ] drop
```

In a test environment declaring `test.emit : Text -- ! {test.emit}`, the quotation is checked and discarded without issuing the event. The declaration being available is distinct from executing its body.

## A.2 Composition order

```text
20 [ 1 + ] [ 2 * ] compose call
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
20 [ 1 + ] twice call
```

The result is `22`. `twice` is an ordinary checked definition, not a parser feature.

## A.5 Program stored in a list

```text
20 nil [ 1 + ] swap cons [ ] [ drop call ] list.case
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
[ 1 + ] dup reify
```

The stack retains the program and an inert resolved syntax value. A transform of that syntax may produce a new candidate. It cannot mutate the retained program; preparing the candidate must check it anew against an expected interface.

---

# Appendix B — Source traceability (informative)

This draft is grounded in RFC 0001. It does not replace the RFC's record of rationale with claims that all new details were already agreed.

| RFC section | Content carried forward | Specification sections |
|---|---|---|
| 1–2 | Small composable core; layer and application boundaries | 1–2, 15 |
| 3 | Left-to-right stack semantics; three expression forms; resolved definitions | 3, 5, 10 |
| 4 | Program interfaces, `call`, `compose`, `quote`, explicit capture, first-class guarantee | 4, 6 |
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

Canonical serialization and the Wasm ABI should follow as separate reviewable specifications. In particular, stable program identity, closed generic instantiations, recursive references, and runtime-built composition must be tested together rather than chosen independently.

The concrete host/resource profile must then settle exception-free normal ownership, trap retirement, REPL recovery, and authorization at imported interfaces. Async streams and remote code transport follow those foundations rather than enlarge this kernel draft.

The language's public name, file extension, package namespace, release governance, and implementation repository layout remain unassigned. This document does not silently choose them from the enclosing project's name.

---

*End of SPEC-0001, revision 0.1.0-draft.1.*
