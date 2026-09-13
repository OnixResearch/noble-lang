# Noble Project Handoff

## 1. What Noble is

Noble is a small, statically typed, concatenative language centered on **programs as first-class data**, implemented in Rust and targeting WebAssembly.

The core semantic idea is:

> A program is an immutable, typed value with an inspectable composition. Constructing a program and executing its effects are separate operations.

The central type is conceptually:

```text
Program<InputStack, OutputStack, Effects>
```

A checked `Program<S,T,e>` can be passed, returned, composed, stored where its type permits, inspected structurally, and executed. Effects are latent until execution. The language kernel should remain small; terminals, filesystems, services, actors, distributed execution, durability, and similar platform facilities belong in host profiles and libraries rather than becoming unrelated core expression forms.

## 2. Selected core surface

The current selected terminology is:

```text
[ ... ]     construct a checked Program
run         execute a Program
compose     construct sequential composition
quote       capture eligible immutable data as a Program
reflect     expose a Program as inert resolved Syntax
```

Conceptually:

```text
run:
    S Program<S,T,e>
    -- T ! e

compose:
    Program<A,B,e> Program<B,C,f>
    -- Program<A,C,union(e,f)> ! {}

quote:
    a
    -- Program<S,S a,{}> ! {}
    where Capture(a)
```

Canonical examples:

```text
41 [ 1 + ] run
# => 42

20 [ 1 + ] [ 2 * ] compose run
# => 42

40 2 quote [ + ] compose run
# => 42

def twice [ dup compose ]
20 [ 1 + ] twice run
# => 22
```

The runtime-built examples must genuinely work with values chosen after compilation. Constant-folding quotation literals is not sufficient evidence of first-class programs.

## 3. `Program` versus `Syntax`

Keep the staging distinction explicit:

```text
Syntax
    editable, inert, potentially unresolved or ill-typed

Program<S,T,e>
    checked, executable, known interface/effects, inspectable recipe
```

Typed builders such as `quote` and `compose` preserve checked-program invariants. Arbitrary structural rewriting produces `Syntax` and must cross an explicit preparation/checking boundary before becoming executable.

`run` must not implicitly compile source, fetch dependencies, acquire authority, or reinterpret arbitrary data as instructions. `reflect` exposes the resolved language-level recipe, not machine addresses, closure layout, optimizer IR, or Wasm instruction layout.

## 4. Type-system direction

The first implementable core remains deliberately tractable: stack-tail typing, rank-1 named polymorphism, algebraic data, conservative effect sets, and statically known first-class program interfaces.

However, later **dependent types, higher-rank polymorphism, principled subtyping/refinements, and extensible inference mechanisms are desired directions, not rejected features**. Their omission from the first milestone is sequencing. Any such extension must preserve the language safety theorem, resource rules, recipe observations, acceptance-checker architecture, and verification story.

There is no intended dynamic `Any` fallback for unknown program stack transformations.

## 5. Safety posture

The newest safety workstream selects the following project-wide policy:

- no guest `unsafe` escape hatch;
- undefined behavior is not a Noble-language semantic outcome;
- raw machine addresses and unchecked native pointers are not ordinary Noble values;
- malformed external values, stale/forged handles, and representation errors are rejected or fail in a specified way rather than becoming arbitrary execution;
- `Program<S,T,e>` remains the checked execution boundary;
- effects describe possible operations but do not grant authority;
- live resources/capabilities are move-only and recursively excluded from ordinary duplication/capture/serialization;
- host/FFI/Wasm/component boundaries validate hostile or semi-trusted representations;
- the standard concurrency profile should exclude direct shared mutable Noble memory;
- stable “Noble-safe” claims require mechanized semantics/checker correspondence plus executed tests and a disclosed trust ledger.

Safety does **not** silently mean termination, deadlock freedom, service availability, confidentiality against intentionally authorized disclosure, or exactly-once external effects. Those require separate stronger claims.

## 6. Effects, resources, and authority

Noble distinguishes three things that must not be collapsed:

```text
operation/interface availability
    !=
effect requirement
    !=
runtime authority
```

A program may statically require a filesystem or network operation without having permission to use any specific resource. Actual authority is carried by validated host/resource capabilities.

Live resources are invocation inputs rather than portable captured state. Generic `dup`, generic `drop`, `quote`, and serialization must not duplicate or hide ownership of live resources. On normal paths resources must be moved onward, consumed by a declared operation, or explicitly released; abnormal cleanup remains an explicit host/runtime obligation.

## 7. Wasm, Component Model, WIT, and WASI

Wasm remains the production execution substrate. Noble should not require a second production AST/bytecode interpreter.

The newer WIT/WASI workstream strengthens the platform boundary:

- **Component Model components** are the standard portable deployment/interoperability artifact.
- **WIT** is Noble's standard external component/host interface language.
- Noble remains richer internally than WIT: stack polymorphism, `Program<S,T,e>`, effects, recipes, proofs, and Noble identity are not reduced to WIT constructs.
- WIT worlds are explicit compilation/import/export contracts.
- Imported WIT operations are conservatively effectful unless a stronger Noble contract is established; a WIT signature is not a purity proof.
- WIT `own` resources inherit Noble move-only ownership; WIT `borrow` values are scoped/noncapturable and must not escape.
- Interface availability and type correctness do not grant runtime authority.
- Stable WASI interfaces are preferred to inventing duplicate Noble filesystem/clock/random/socket/HTTP/CLI ABIs.
- The selected host-family direction is WASI 0.3, with exact versions pinned per build/profile and 0.2 handled as an explicit compatibility profile.
- Component Model native async (`async func`, `stream<T>`, `future<T>`) is the standard cross-component async ABI, without requiring source-level `async`/`await` syntax.
- `Program<S,T,e>` is not automatically a WIT function/resource. Portable program transport remains separately specified and validated.

Definition identity and component deployment granularity stay separate: many Noble definitions may compile into one component.

## 8. Concurrency direction

The newest selected concurrency direction is **Syndicate/Synit + Preserves**, not a bespoke Noble actor/mailbox system.

Treat this as the default architecture to formalize next:

- Syndicate's dataspace / assertions / interests / messages / facets provide the concurrency and conversational-lifetime model.
- Synit is the concrete system/service-layer reference.
- Preserves is the default protocol/schema representation for Syndicate conversational data.
- BEAM/OTP remains a secondary reference for supervision, fault isolation, restart philosophy, and operational lessons.
- Concurrency remains outside the language kernel; a `Program` is not an actor.
- Sequential `compose` stays sequential.
- The standard profile should forbid direct shared mutable Noble memory between concurrent participants.
- Facet/conversation ownership should scope assertions, observations, subordinate work, and cleanup.
- Capability/authority validation remains runtime-enforced independently of protocol typing.
- At Component Model crossings, Syndicate facilities should be exposed through versioned WIT packages; Preserves and WIT are complementary rather than competing.

The older generated BEAM-first concurrency package should therefore be treated as historical input requiring reconciliation, not as current architectural authority.

## 9. Choreography

Choreographic programming remains an optional higher-level protocol facility outside the kernel.

The older choreography draft contains useful work on roles, transfers, choices, ordering, failure, and projection. Under the newer direction, choreography should preferentially lower onto the Syndicate/Synit concurrency layer rather than create an independent communication runtime. Preserves should be the protocol-data representation at that layer, and WIT should define component crossings.

This reconciliation has not yet been formalized into a merged choreography spec.

## 10. Verification architecture

Verification is a core development commitment, but ordinary Noble execution must not invoke Lean, Aeneas, Verus, SMT, or proof search.

Selected roles:

```text
Lean 4
    reference semantics
    metatheory
    reusable program-contract logic

Rust -> Charon -> Aeneas -> Lean
    acceptance checker
    pure validation/checking utilities
    selected representation utilities

Verus
    selected imperative Rust components
    especially resource/capability handle tables and lifecycle state

Separate backend workstream
    Noble -> Wasm / Component Model correspondence
```

Always distinguish:

1. language metatheory;
2. proof about actual Rust implementation functions;
3. optional behavioral proof about a specific Noble program;
4. source/recipe-to-Wasm/component correspondence;
5. runtime host/adapter assumptions.

One proof does not automatically establish the others.

## 11. Identity model

Keep at least these identities distinct:

```text
DefinitionId
    canonical resolved Noble definition

ProgramValueId
    portable program value including immutable captures

BuildKey
    compilation request including compiler/profile/WIT/WASI/adapters/options

ArtifactId
    exact emitted artifact bytes
```

Names are metadata rather than semantic identity. Renaming should not mutate already-resolved code. WIT/WASI version changes affect build identity rather than source definition identity unless Noble semantics themselves change.

Canonical Noble program encoding/hashing is still unresolved and must not be accidentally inherited from Preserves, WIT, optimized Wasm bytes, or a particular Rust serializer.

## 12. Important references and their roles

- Factor / Mirth: quotations, combinators, typed concatenative design.
- Unison: names-independent definition identity and explicit effects.
- Wasmtime / wasm-tools / wit-bindgen / Component Model: execution and interoperability substrate.
- WIT / WASI: standard external interface and host-profile family.
- Syndicate / Synit / Preserves: default concurrency/service/protocol direction.
- BEAM / OTP / Erlang / Elixir / Gleam: supervision, failure isolation, operational concurrency lessons.
- ChoRus / choreographic programming: global protocol descriptions and endpoint projection.
- Ambient: content-addressed code/store ideas, live-resolution boundaries, tooling architecture.
- Bend: independent pure computations and experimental parallelization ideas.
- Lunatic: Wasm process/isolation/scheduling reference.
- Flawless: durable/replayable execution and ambiguous external-effect recovery reference.

Borrow mechanisms deliberately; do not enlarge Noble's kernel merely because a reference system contains a useful feature.

## 13. Current status

The specification architecture is substantial but draft.

The project-backed baseline and verification documents explicitly report that checker soundness, implementation proofs, backend correspondence, and many conformance scenarios remain open/not-run. The verification decision register says no Lean/Aeneas/Charon/Verus/compiler/runtime proof is currently reported complete in that package.

The newest safety and WIT/WASI packages passed **structural document validation only**. Their runtime scenarios are not-run and their proof obligations remain open.

Conversation history indicates implementation work has occurred, but this handoff set does not contain an authoritative current Rust/Lean repository or executed implementation test report. **Inspect the actual repository and rerun tests before claiming an implementation feature works.**

## 14. Major source-of-truth problem

There is not yet one canonical SPEC-0001 that includes all of:

- baseline kernel/programs-as-data semantics;
- verification extension;
- `run` / `reflect` naming;
- `#` comments;
- safety extension;
- WIT/WASI first-class component profile;
- Syndicate/Synit/Preserves concurrency direction;
- choreography reconciliation.

Safety and WIT/WASI each have separate working integration previews. Syndicate is newer than both. Do not choose one preview and overwrite the others.

## 15. First milestone that matters

The foundational language milestone remains a real Wasm-backed programs-as-data vertical slice demonstrating together:

```text
41 [ 1 + ] run
20 [ 1 + ] [ 2 * ] compose run
runtime_n quote [ + ] compose
20 [ 1 + ] twice run
program reflect
```

and also demonstrating:

- incompatible composition rejected before execution;
- effectful quotation construction performs no latent effect;
- runtime-created programs work without invoking the source compiler;
- reflected recipe corresponds to the executed program;
- supported live resources cannot be duplicated/captured incorrectly;
- actual tests are distinguished from expected fixtures.

Once that core slice is honest and working, the next platform slice should be WIT/Component Model integration, followed by the first formalized Syndicate/Preserves concurrency slice.
