# External design study for Noble

Date: 2026-09-12  
Noble basis: canonical specifications at `0.1.0-draft.2`  
Status: recommendations, not adopted language changes or implementation evidence

**Keep the small checked core. Adopt the boundary discipline, and prototype the backend ideas before selecting a representation.**

## 1. Recommended disposition

### Near-term recommendations

| Idea | Source | Disposition | Noble application |
|---|---|---|---|
| Explicit safety arguments for native code | Zerocopy | Adopt the practice | M1 boundary inventory, cited safety arguments, negative tests, and scoped Miri runs |
| Typed byte views before semantic acceptance | Zerocopy | Adapt the architecture | Separate representation checks, candidate checks, artifact trust, and authority |
| Branch-local type information | Blog and Zena | Adapt through existing eliminators | Precise stack diagnostics and `case` payload types, without implicit union joins |
| Ownership separate from memory reclamation | Zena and blog | Retain and strengthen existing direction | Immutable data can use GC. Live resources still require explicit ownership and retirement |
| Contracts over initial and final state | Blog | Retain existing contract layer | Lean metadata and ghost snapshots, not new assertion syntax or implicit proof assumptions |

### Experiments and later work

| Idea | Disposition | Reason |
|---|---|---|
| WasmGC data, typed closure environments, and multi-value returns | M3 comparison experiment | Strong fit, but dynamic composition, quotas, and component costs remain untested |
| Zerocopy crate for selected binary headers | Prototype when a concrete format exists | Good implementation candidate, not a dependency needed by the initial semantic checker |
| Distinct and opaque types | Rust newtypes now, guest types later | Useful identity separation. Guest declaration and identity rules are not yet complete |
| Scoped resource convenience and async cancellation | M5/M6 follow-up | Cleanup effects and native pins must remain explicit |
| Self-hosting, class hierarchies, general unions, macros, and units of measure | Defer | These features do not close the first checker or Wasm milestones |

No recommendation changes Noble's Rust implementation, WIT boundary, WASI 0.3 direction, or Syndicate/Preserves direction.

## 2. What the sources actually establish

### Zena: useful mechanisms, mixed maturity

The [homepage](https://zena-lang.dev/) describes a WasmGC-native language with monomorphized generics, typed closures, ownership, and component integration. It also warns that the project is immature and contains bugs.

The [Wasm guide](https://zena-lang.dev/guide/web-assembly/) explains concrete representations. The [performance guide](https://zena-lang.dev/guide/performance/) explains specialization and reachability. Neither page establishes performance for Noble.

Three inspected pages are placeholders: [correctness](https://zena-lang.dev/guide/correctness/), [WASI](https://zena-lang.dev/guide/wasi/), and [inference](https://zena-lang.dev/reference/inference/). The [resource guide](https://zena-lang.dev/guide/resources/) distinguishes implemented foundations from unfinished ownership and borrow checks.

The source inspection used commit `a74a6939833b2f5fa8107c3e0c7f8ee3032457d0`:

| Inspected source | Observation |
|---|---|
| [Closure lowering](https://github.com/elematic/zena/blob/a74a6939833b2f5fa8107c3e0c7f8ee3032457d0/packages/zena-compiler/zena/lib/codegen/ir/lowering.zena#L174-L242) | Function adaptation reads closure context and function fields. It checks representation assumptions before emission |
| [Function-value fixture](https://github.com/elematic/zena/blob/a74a6939833b2f5fa8107c3e0c7f8ee3032457d0/tests/language/execution/functions/function_values_reachability.zena) | Covers direct calls, argument passing, fields, returned functions, and local bindings |
| [Narrowing fixture](https://github.com/elematic/zena/blob/a74a6939833b2f5fa8107c3e0c7f8ee3032457d0/tests/language/semantics/type-narrowing/assignment_invalidates_narrowing.zena) | Explicit negative expectations cover invalidated branch facts |
| [Component ABI implementation](https://github.com/elematic/zena/blob/a74a6939833b2f5fa8107c3e0c7f8ee3032457d0/packages/stdlib/zena/component/abi.zena#L1-L155) | Strings cross between GC arrays and linear memory through allocation and copy loops |
| [Ownership implementation](https://github.com/elematic/zena/blob/a74a6939833b2f5fa8107c3e0c7f8ee3032457d0/packages/stdlib/zena/core/ownership.zena#L1-L165) | Defines disposal and ownership regimes. Comments and published guide spellings differ in places |

The [component design](https://github.com/elematic/zena/blob/a74a6939833b2f5fa8107c3e0c7f8ee3032457d0/docs/design/component-emission.md) contains upstream execution claims and concrete examples. Those runs were not reproduced here.

The homepage's zero-marshaling language is not a safe assumption. The inspected implementation explicitly copies strings. [Canonical ABI GC support](https://github.com/WebAssembly/component-model/issues/525) was an open pre-proposal at retrieval.

### Zerocopy: representation safety, not language semantics

The repository now places its crate under `zerocopy/`. Its root page rendered community guidelines rather than the crate README.

The source inspection used commit `f2c99f928beb0356efe8c0765d177dad5167d899`. The inspected [manifest](https://github.com/google/zerocopy/blob/f2c99f928beb0356efe8c0765d177dad5167d899/zerocopy/Cargo.toml) declares version `0.8.57`. This is an observed source version, not a selected Noble dependency or a release audit.

The [README](https://github.com/google/zerocopy/blob/f2c99f928beb0356efe8c0765d177dad5167d899/zerocopy/README.md) and selected implementations distinguish these properties:

| Property | Meaning | Does not establish |
|---|---|---|
| `TryFromBytes` | Content-dependent conversion with runtime validity checks | Valid Noble schemas, dependencies, effects, or authority |
| `FromBytes` | Every initialized bit pattern is valid for the Rust type | Sufficient length or alignment for a requested reference |
| `FromZeros` | An all-zero representation is valid | All nonzero representations are valid |
| `IntoBytes` | Safe access to initialized representation bytes | Canonical, portable, or authenticated serialization |
| `KnownLayout`, `Immutable`, `Unaligned` | Layout knowledge, restricted interior mutation, or alignment of one | Immutable external storage, stable lifetimes, or semantic trust |

The [checked reference conversion](https://github.com/google/zerocopy/blob/f2c99f928beb0356efe8c0765d177dad5167d899/zerocopy/src/lib.rs#L1968-L1991) performs a layout conversion followed by content validation. The [unconditional conversion](https://github.com/google/zerocopy/blob/f2c99f928beb0356efe8c0765d177dad5167d899/zerocopy/src/lib.rs#L4246-L4258) still returns layout errors.

The [soundness policy](https://github.com/google/zerocopy/blob/f2c99f928beb0356efe8c0765d177dad5167d899/zerocopy/POLICIES.md) requires safety arguments tied to Rust documentation. It also states exceptions and target assumptions.

The inspected CI contains [Miri configurations](https://github.com/google/zerocopy/blob/f2c99f928beb0356efe8c0765d177dad5167d899/.github/workflows/ci.yml#L588-L654) for Stacked Borrows and Tree Borrows. This job excludes some targets, including Wasm, and skips pull-request events. Its [Kani configuration](https://github.com/google/zerocopy/blob/f2c99f928beb0356efe8c0765d177dad5167d899/.github/workflows/ci.yml#L724-L746) uses a restricted feature selection. CI configuration is not evidence that every function, target, or feature is proved.

### Blog: three useful ideas, one incorrect example

Pranoy Dutta's [article](https://prydt.xyz/blog/a-few-good-ideas-in-pl/) covers flow typing, borrow checking, and contracts. It is an idea survey, not a formal specification.

The [TypeScript handbook](https://www.typescriptlang.org/docs/handbook/2/narrowing.html#control-flow-analysis) corroborates branch refinement and declared-type checks. The article's [Rust reference](https://doc.rust-lang.org/1.8.0/book/references-and-borrowing.html) is the old Rust 1.8 book. Its lexical-scope examples are not a current Rust implementation specification.

The article's deposit postcondition is:

```text
balance == balance + amount
```

Both occurrences refer to final state. For an initial balance of 100 and deposit of 10, it asks whether 110 equals 120. The intended relation needs an initial-state snapshot:

```text
balance_after = balance_before + amount
```

Steel evaluated this arithmetic counterexample. No D program was compiled. Noble already supports ghost snapshots conceptually in [PROGRAM-CONTRACTS.md](../specs/PROGRAM-CONTRACTS.md).

D's [contract specification](https://dlang.org/spec/contracts.html) and [function contracts](https://dlang.org/spec/function.html#contracts) also permit disabled runtime checks and describe invalid-state consequences. Those semantics do not fit Noble's no-undefined-behavior contract.

## 3. Wasm backend: adapt the mechanism, not the whole language

**Recommendation: compare a WasmGC backend representation with a managed linear-memory representation during M3.**

Noble's immutable data fits immutable GC fields. Monomorphic `Program<S,T,e>` interfaces fit typed function references. Multiple stack results fit Wasm multi-value returns.

A Rust-written compiler can emit these Wasm features directly. Compiling an ordinary Rust runtime to Wasm does not automatically produce a WasmGC representation for Noble values.

Treat Zena as a design reference, not a required compiler dependency. Prefer compatible, pinned Rust backend components over a port of its compiler or WIT parser.

An experimental program representation can pair a compiled entry point with an immutable capture environment and a retained recipe. The exact layout remains open. Its entry signature must preserve the instantiated input and output stacks.

Runtime composition cannot require a new Wasm type or source compilation for each composition tree. Reusable compiled composition adapters need bounded, typed representations for their children. The supported interface set must be explicit.

### Adaptation constraints

1. **Preserve recipes.** Dead-code elimination must retain identities and recipe information observable through `reflect`. Reflection metadata is not optional debug metadata.
2. **Preserve exact arity.** Zena's callback adapters can discard unused arguments. Noble cannot copy that rule across ownership-bearing stack positions.
3. **Keep resources separate.** GC reachability cannot discharge file, component, or native-pin obligations. GC allocation does not make resources capturable.
4. **Measure boundary costs.** WIT strings and lists can require linear-memory allocation and copies. Zerocopy does not eliminate that representation mismatch.
5. **Measure limits.** Deep composition, retained recipes, host stack use, GC allocation, and cancellation require explicit budgets and failure outcomes.

A recursive composition closure can consume one host frame per nested composition. Tail calls alone do not remove every pending continuation in arbitrary composition trees. A compiled trampoline or explicit continuation representation is another candidate, not a selected ABI.

The REPL still needs session-owned storage. A returned Wasm function does not leave a persistent operand stack for the next submission.

### M3 comparison cases

| Case | Required observation |
|---|---|
| Post-compilation scalar capture and runtime program selection | Correct output without source preparation or recipe interpretation |
| Program retrieved from a homogeneous collection | Exact same instantiated interface and behavior |
| Long and differently shaped compositions | Bounded accounting, correct order, and a specified quota failure |
| Optimization enabled and disabled | Same required recipe observations, effects, and semantic identity |
| String/list component round trip and allocation failure | Exact values, measured copies, no leaked boundary allocation |

The first four fit M3. The component row is an early compatibility probe, not full M5 conformance. It must not delay the resource-free M3 experiment.

Compare emitted bytes by section, preparation time, allocation counts, peak memory, composition depth, and execution time. Pin the engine and its GC, function-reference, and tail-call feature configuration. No speedup or size advantage is established yet.

## 4. Zerocopy: use a narrow decoding adapter

**Recommendation: use zerocopy only where a concrete binary layout benefits from it. Start the semantic checker with ordinary owned Rust data.**

A safe architecture is:

```text
bounded, stable input bytes
  -> checked layout view
  -> untrusted candidate data
  -> semantic acceptance against an explicit interface and environment
  -> checked program
  -> artifact correspondence and authority checks
  -> permitted execution
```

Every arrow establishes a different property. A successfully decoded header can still advertise false effects, unknown dependencies, cyclic nodes, or a forged resource index.

Good initial targets include a future artifact header or a fixed-format cache envelope. Preserves remains the selected protocol direction. Zerocopy does not replace its decoder or justify a new interchange format.

### Adapter constraints

1. Use fixed-width fields and explicit byte order. Avoid `usize`, pointers, native enum layouts, and padding as wire contracts.
2. Separate layout errors from semantic rejection. Prefer derived implementations over handwritten unsafe trait implementations.
3. Bound lengths, offsets, recursion, and total work before allocation or traversal. Reject unsupported versions and malformed tags.
4. Keep backing storage stable for the entire view lifetime. Across guest callbacks, suspension, or shared-memory mutation, copy unless a reviewed ownership contract prevents invalidation.
5. Keep wire views outside accepted semantic state. Inventory the crate, derive macros, features, and native assumptions at the boundary.

`Immutable` does not prevent another process or guest thread from changing shared storage. A valid header can also contain an out-of-bounds payload offset. Neither property follows from a Rust representation trait.

Noble's `Data` and `Capture` predicates describe language eligibility, not Rust bit validity. No zerocopy derive can grant those predicates or manufacture a live capability.

Raw `IntoBytes` output is not a substitute for canonical program encoding. The encoding still needs versioning, ordering, normalization, and cross-implementation vectors.

### Smallest useful experiment

A paired decoder experiment can compare explicit field reads with derived byte views over one experimental header. Both paths must produce the same untrusted candidate or rejection.

| Positive path | Negative path |
|---|---|
| Valid, correctly sized header | Truncation and offset arithmetic overflow |
| Defined byte order | Misalignment and swapped byte order |
| Valid tag and schema version | Invalid boolean/tag and unknown version |
| Stable immutable backing buffer | Attempted view escape or invalidation across a callback |
| Valid representation and semantic contract | Valid bytes with forged effects or a stale handle |

Pin a reviewed crate version and dependency lock only after this experiment warrants the dependency. Its `no_std` support does not establish Charon/Aeneas compatibility.

## 5. Flow typing: precise branch stacks, not implicit unions

**Recommendation: improve elaboration and diagnostics without adding another typing relation to the bootstrap core.**

Noble already tracks ordered stack types at each word. Its `case` operation moves a selected sum payload into the corresponding branch. This gives useful branch-local type information without mutable variables or general union subtyping.

For example, this existing contract exposes an integer only in the integer branch:

```text
case : S Sum<Resource<R>,I64>
       Program<S Resource<R>,T,e>
       Program<S I64,T,f>
       -- T ! union(e,f)
```

The whole sum remains non-`Data` and noncapturable. The integer payload can satisfy `Data` after elimination. The resource branch must account for its owner independently.

Both branches still produce the same complete stack `T`. An `I64` result on one branch and `Text` on another require explicit sum construction, not an implicit `I64 | Text` join.

Useful M2 additions are branch-local stack displays, expected-versus-actual stack diagnostics, and contextual information for quotation elaboration. The independent acceptance checker still checks every derived node.

A future pattern-matching surface can elaborate into existing eliminators. Guards need explicit effects, failure paths, and ownership rules. An arbitrary user predicate must not become trusted refinement evidence.

**Discriminating cases:** mismatched branch arity, reordered resource slots, a missing constructor case, and a guard that consumes an owner before another branch needs it. These are proposed test designs, not additional executed conformance cases.

## 6. Ownership: borrow the separation, reject the escape regime

**Recommendation: retain adapter-local, owner-threaded borrows for M5. Do not add a general guest borrow checker now.**

Zena's separation of ordinary data, owners, short-lived borrows, and scoped computations is useful. Noble already has a stricter initial contract in [RESOURCE-ADAPTERS.md](../specs/RESOURCE-ADAPTERS.md).

A WIT borrow is not automatically a Rust shared reference or a proof that the external object cannot mutate. Noble's host table must still check kind, context, generation, rights, and active scope.

Zena also offers `Unmanaged<T>`, `disown`, and `adopt`. Its source explicitly accepts lost leak-freedom and compile-time use-after-dispose detection in that regime. This is a deliberate Zena trade-off, not evidence of a defect there.

That trade-off conflicts with Noble's recursive ownership accounting and ban on generic resource duplication and capture. Do not import it as a guest escape mechanism.

A later scoped cleanup combinator is possible. It needs these properties:

1. Cleanup calls have declared effects and a specified order.
2. Success and ordinary `Result` errors return or transfer ownership according to the same explicit contract.
3. Traps and cancellation trigger host retirement without a resumed guest continuation.
4. Native work retains its accounted pin until it cannot access storage.
5. Repeated cleanup cannot release twice or revive an owner.

A shielded cleanup region cannot guarantee that an unavailable host eventually completes. `Retiring` remains necessary. General scoped futures belong behind the M6 contract gate, not inside M2.

## 7. Contracts and nominal distinctions

### Contracts: strengthen examples, not syntax

The blog reinforces [VC-LOGIC-01/02](../specs/PROGRAM-CONTRACTS.md), snapshot-based reasoning, and the difference between a defect and an expected domain failure.

The first contract examples can cover initial/final stack relations, wrap-aware arithmetic, runtime-built programs, and ordinary `Result` alternatives. A passing example or runtime assertion is not a proof for all inputs.

Mandatory boundary checks must remain active in release builds. Optional proof metadata cannot authorize execution or remove checks on hostile input.

If runtime contract guards are added later, their cost, effects, failure outcome, and identity consequences need a separate contract. A false guard must cause a defined failure, not permit undefined behavior.

The deposit example also does not justify adding floating point to Noble. Business arithmetic can use explicit units and checked bounds over the selected numeric semantics.

### Nominal distinctions: use Rust newtypes first

Zena's distinct and opaque types reinforce a useful implementation discipline. `DefinitionId`, `ProgramValueId`, `BuildKey`, host-operation IDs, and owner contexts must not interchange accidentally.

Rust newtypes with restricted constructors can provide this separation in M1/M2. Deserialization and public constructors must not bypass the relevant checks. A typed identifier is still not authority.

Guest nominal types and units of measure can follow the algebraic declaration design. Their schema identities and eligibility rules must participate in checking and canonical identity. They do not need a second object system.

## 8. Evidence, limits, and next action

### Review method

This review used three correlated passes by one reviewer: language semantics, runtime representation, and adversarial boundary analysis. No subagents participated.

The bounded retrieval used 22 scraper calls and two Git checkouts for source inspection. Several scraper calls returned navigation or incomplete selectors. The relevant material came from corrected fetches or pinned local source reads.

The terminal result is a completed recommendation study. Backend and dependency selection remain blocked on experiments, not on more agreement between descriptions.

### Checks performed

- Read all three requested sources and the cited supporting sections.
- Inspected selected code and fixtures at the two recorded Git revisions.
- Evaluated the deposit arithmetic counterexample with Steel.
- Ran the existing Noble document validator and its 21 regression tests successfully before and after this report.

No upstream test suite, benchmark, Miri run, Kani proof, or Noble execution was performed. Selected source functions received a focused read, not a complete security audit. No external code or dependency entered Noble's implementation.

The canonical specifications remain unchanged. The existing 66 scenarios remain unexecuted, and the 27 proof obligations remain open.

### Next action

**Approve the M3 representation comparison, not a permanent WasmGC commitment.** Keep M1 as the next implementation milestone. During M1, add the cited native-boundary review practice to the existing quality gates.
