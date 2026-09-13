# Noble reference addendum — Bend and Ambient

Document: REF-BA001  
Revision: 0.1.0-draft.1  
Date: 2026-09-09  
Status: References recorded; adaptations proposed, not implemented or ratified  
Basis: supplied SPEC-0001, RFC 0001, SPEC-V001, SPEC-V002, and IMPL-V001

## 1. Reference registration

This document adds two architectural references without modifying the original RFC, baseline specification, verification addendum, or parallel syntax/tooling work. It does not register either project as an implementation dependency. The existing Project source files have not been overwritten or replaced.

**REF-BEND — HigherOrderCO/Bend**  
Repository: <https://github.com/HigherOrderCo/Bend>  
Intended role: independent-computation structure, recursive traversal and generation, functional parallelism, and performance experiments. Study HVM-based execution as a separate research reference, not Noble's selected production runtime. [B1–B3]

**REF-AMBIENT — OnixResearch/ambient**  
Repository: <https://github.com/OnixResearch/ambient>  
Intended role: file-oriented content addressing, object stores, explicit live-resolution boundaries, effect tooling, remote package protocols, and lessons about type/trait complexity. Do not adopt its bytecode VM, identity encoding, handler machinery, or deployment security posture by implication. [A1–A4]

The observations below come from the projects' primary documentation retrieved on the date above. They are not reports of executed benchmarks, passing tests, security audits, or verified implementations. The source register records observed file-object identifiers rather than claiming a tested release or pinned compatible toolchain.

## 2. Overall recommendation

Ambient is the closer architectural reference for Noble's code model and development environment. Bend is most useful for the structure of pure computation and possible future execution optimizations.

Adopt the mechanisms that fit Noble's existing boundaries: a small typed quotation core, immutable resolved recipes, explicit effects and resources, and Wasm execution. Do not turn the combined references into a requirement for a second VM, general continuations, a distributed runtime, or automatic parallel execution in the first implementation.

The proposed adaptations allocate no new punctuation. In particular, they do not repurpose `#` away from the user's comment-syntax direction. The supplied baseline's older lexical profile is not being reaffirmed by this addendum.

## 3. Bend adaptations

### 3.1 Make recursive traversal and generation ordinary library tools

Bend documents `fold` as recursive consumption/transformation and `bend` as recursive generation. Its examples expose independent tree branches rather than forcing every algorithm into a sequential accumulator. [B2, B3]

**Proposal:** prioritize ordinary typed list/tree `map`, `fold`, and `unfold` operations, followed by a divide-and-conquer helper. Begin with concrete data schemas and the selected rank-1 program interfaces. Do not add a higher-kinded recursion-scheme system or a new expression form merely to abstract over every recursive datatype immediately.

A left fold retains its specified order. A separate tree reduction or an established associativity law is needed before reassociating a reduction. Noble's wrapping `I64` addition supplies a useful first law to prove; arbitrary user combiners do not acquire that law from their stack signature.

Useful early examples are tree summation, tree mapping, balanced aggregation, and construction followed by traversal. They exercise data, recursion, quotations, and inference together. Their acceptance depends on the remaining checker and declaration work; the names here are proposed library vocabulary, not implemented Noble APIs.

### 3.2 Expose independent work without changing `compose`

Bend's guide distinguishes a computation that consumes another's result from two independent computations joined later. Its parallelism claim relies on that distinction, not on making a sequential algorithm independent by renaming it. [B2]

Noble's existing example remains sequential:

```text
20 [ 1 + ] [ 2 * ] compose call
```

The second quotation consumes the first quotation's output. Its result is 42 under the draft semantics. [N1 §6.4]

**Proposal:** make independent branches easy to describe with ordinary program/data combinators. A future parallel profile can execute suitable branches concurrently while returning results in a specified order. A sequential implementation is the first reference behavior. The full stack-polymorphic signatures and resource isolation rules must be designed, rather than hidden by schematic arrow notation.

An internal typed dependency graph may help identify independent regions. That graph is an optimizer representation, not the public program recipe. Optimized code must retain the recipe observed by `reify`, including program values returned inside other data. [N1 §11.2; N2 §4.5]

### 3.3 Treat parallelism as a measured and verified optimization

Bend uses HVM2 and documents CPU/GPU execution choices. Its README also cautions about single-core performance. These are reasons to experiment, not evidence that adopting its evaluator will make Noble workloads faster. [B1]

**Proposal:** keep Wasm as the production target. An interaction-net experiment can remain outside the conforming implementation until it has an explicit lowering/execution design and evidence for Noble's semantics. Installing an interaction-net evaluator inside Wasm would not by itself satisfy the existing no-second-production-evaluator requirement. [N1 §13.1]

The initial benchmark set should compare sequential chains and balanced branches, scalar work and allocation-heavy trees, and realistic small jobs as well as large jobs. Record wall time, single-worker baseline, allocation, peak memory, scheduling overhead, and data-transfer costs where applicable. Speedup without absolute cost is not sufficient.

An empty effect bound is only one eligibility condition. It does not establish termination, lack of traps, independence, or safe speculation. Parallelizing an unchosen branch or exposing a later failure before an earlier divergence can change observations. Any scheduler profile must define failures, cancellation, budgets, ordering, and ownership. Begin optimization experiments with a deliberately restricted, total pure fragment; retain the ordinary sequential path elsewhere. [N1 §§5, 14.4; N2 §§4.2, 4.5]

## 4. Ambient adaptations

### 4.1 File-oriented development plus an inspectable content store

Ambient combines source files with immutable objects, snapshots, named tags, pack transfer, and dependency/diff inspection. Its architecture uses one canonical object representation across storage and transport. [A2, Content-Addressing and The Store]

**Proposal:** use ordinary source files and Git for authoring, with a local store for resolved definitions, recipes, build results, and snapshots. Provide structured inspection of dependencies, captures, effects, interfaces, and evidence. A future browser can render this information without inventing another semantic index.

Keep the existing four identities distinct:

| Identity | Intended Noble meaning |
|---|---|
| `DefinitionId` | Resolved language definition and its semantic dependencies |
| `ProgramValueId` | Exact portable program including immutable captures |
| `BuildKey` | Compiler, target/profile, adapters, options, and checked inputs |
| `ArtifactId` | Exact emitted bytes |

These are existing Noble concepts, not new terminology imported from Ambient. [N1 §12.1]

**Important non-adaptation:** Ambient's object encoding hashes compiled bytecode, and its recursive-group rule orders named members by name. Noble requires source identity to be independent of optimizer output and source renaming. Reuse the store and dependency-closure ideas, not those identity rules. Do not adopt Ambient's hash algorithm or binary encoding without Noble's own canonicalization specification. [A2, Content-Addressing; N1 §12.2]

Mutually recursive groups are a useful later reference, but remain deferred in Noble. Their identity design must address renaming and member ordering explicitly rather than simply copying a name-sorted strongly connected component.

### 4.2 Separate build caching, result caching, and evidence caching

Ambient's README presents reuse of builds and unchanged computations as a benefit of content addressing. [A1, Perfect Caching]

**Proposal:** define three independent cache policies. Build reuse binds all relevant compilation inputs. Computation reuse binds the exact program, immutable inputs, semantics, and any explicitly modeled environment. Evidence reuse binds the subject, proposition, assumptions, verification context, and acceptance policy. [N1 §12; N3 §7]

A cached filesystem test is not fresh evidence about today's filesystem merely because the test's code hash is unchanged. Likewise, a cached timeout does not prove divergence. Begin result caching with deterministic pure computations whose completion and observation policy are explicit, or with tests run against a fully specified mock environment. Captured secrets and result data still require storage/export policy.

### 4.3 Learn from the nominal-type and trait retrospective

Ambient's author identifies nominal-type bookkeeping and trait/overload dependency complexity as design regrets, and flags exhaustiveness under version evolution. [A1, Learnings & Regrets]

**Proposal:** favor explicit ordinary words over implicit conversions, operator-resolution machinery, and a general trait system in v0. Evaluate structural records and transparent data schemas as later ergonomics, while retaining meaningful nominal distinctions where they are needed. This is not a decision to erase Noble's declared type identities or make same-shaped host resources interchangeable.

Separate semantic schema revision from a long-lived nominal anchor. A changed closed variant set must not be injected under an old schema contract. Either the schema identity changes or an explicitly specified extensible representation handles unknown cases. Old code remains pinned to its original schema; crossing versions requires a checked conversion or migration. Identical field shapes are not enough to equate host operations or authority-bearing resource kinds. [N1 §§4.3, 9, 12]

### 4.4 Obtain testing and replay benefits at host boundaries first

Ambient documents effect interception, mocking, and replay as uses of its abilities. Its detailed architecture implements handlers with single-shot delimited continuations. [A1, Programmable Effects; A2, Delimited Continuations]

**Proposal:** initially use explicit Rust host adapters for real execution, deterministic mocks, recording, and replay. This does not require Noble to acquire general source-level effect handlers. The baseline already permits mock host bindings while deferring continuations. [N1 §8.2]

A replay profile needs operation-contract identities, ordered requests and responses, validation of expected inputs, and explicit mismatch failure. Resource handles require invocation-local logical mapping; logged integers must never resurrect live capabilities. Confidentiality and redaction policy apply to recorded data. Concurrent replay additionally needs scheduling and cancellation observations; do not promise arbitrary distributed replay from an effect set alone.

These boundaries provide useful tests now and explicit host models for later Lean contracts. They do not prove that real filesystem or network adapters satisfy those models. [N2 §§4.3–4.4; N3 §2.2]

### 4.5 Use explicit live-resolution boundaries

Ambient keeps hash-pinned internal calls and introduces explicit points that resolve the current generation. Reading the latest binding is an effect; one such read selects a pinned subtree, while separate reads can cross a deployment boundary. [A3, The model and The Live ability]

**Proposal:** retain pinned ordinary `call`. A future development/hosting profile can expose an authorized operation that resolves a named binding once per unit of work and returns an already prepared program at an expected interface. The selected program and its dependencies remain immutable during that invocation. Missing, incompatible, or insufficiently trusted replacements fail explicitly.

Compatibility includes stack interface, schemas, resource disposition, and the allowed effect bound. Optional behavioral guarantees need separate evidence; matching a type is not proof of equivalent behavior. Replacement may not silently widen an existing caller's allowed effects.

State survives only under an explicit host-owned state and migration contract. Runtime capability ownership must still respect Noble's move-only rules. Do not import Ambient's reusable cell-handle behavior as permission to copy or serialize resources. Draining and retiring old generations require a separate failure/cleanup policy. Changing the dispatch pointer is not transactional rollback of effects or state migration. [N1 §§9–10, 14.2; N3 §§5–7]

### 4.6 Make portable packages explicit; keep authority at the receiver

Ambient's remote reference describes dependency negotiation, canonical packs, isolated execution, and receiver-supplied grants. It explicitly excludes captured closures and continuations from the wire-safe subset, despite the broader closure-transfer description in its README. Treat this discrepancy as a limitation to investigate, not a demonstrated portable-closure implementation. [A1, Remote Functions; A4]

**Proposal:** define a Noble portable-program package around a validated recipe, immutable captures, interface/schema witnesses, and exact dependency identities. Receiving it is not `call`: it passes through bounded decoding, dependency validation, checking/preparation or an explicit artifact-trust path, and destination authorization. Remote fetch and execution remain explicit host operations. Code identity confers neither network trust nor authority. [N1 §§11.4–11.6, 13.3, 14.3]

The receiver can request missing objects, but must enforce package size/depth limits and scope code-store access. Source-to-artifact correspondence remains separate from digest verification. Credentials, live sockets, pending continuations, and a local authority environment do not migrate automatically.

Ambient also warns that its deployment interface has no authentication or authorization beyond what the embedding supplies. Noble should not inherit that posture: remote deployment requires an explicit authenticated principal and independently enforced admission policy. This recommendation is an architectural requirement for a future profile, not a claim that a secure protocol is already specified. [A4, Remote deploy]

### 4.7 Share the analysis implementation across tools

Ambient's architecture describes a shared analysis layer used by its checker and language server, including parity testing. [A2, The shared analysis layer]

**Proposal:** make CLI checking, REPL preparation, editor diagnostics, and structural browsing clients of shared parsing, resolution, elaboration, and diagnostic APIs. Preserve Noble's independently validating acceptance checker behind that shared frontend; shared inference is not a substitute for independent validation. [N2 §5.1]

Editor recovery may report a useful partial result, but partial or invalid source must not become a checked executable program. The presentation layer may differ; semantic error classification and resolution should not drift between frontends.

## 5. Recommended integration order

| Priority | Work item | Placement |
|---|---|---|
| Now | Record references and retain the no-new-core-mechanism boundary | This addendum |
| Next | Concrete typed traversal/generation words | Standard-library specification after checker/declaration foundations |
| Next | Canonical identity, local store, snapshots, inspection, cache separation | Programs-as-data and tooling specifications |
| Next | Shared analysis and deterministic host testing | Rust implementation architecture and host-test profile |
| Later | Explicit live resolution and checked state migration | Development/hosting profile |
| Later | Portable packages and authorized remote invocation | Loading/transport profile |
| Experimental | Parallel pure regions and interaction-net comparisons | Separate performance/lowering workstream |

None of these recommendations introduces a new `Program` type parameter, source-level dependent typing, a runtime prover, a general handler mechanism, or a mandatory scheduler. Any eventual core change must identify its impact on Lean semantics, acceptance-checker refinement, recipe observations, ownership, and Wasm correspondence under the existing verification policy. [N2 §2; N4 §§1, 4]

## 6. Candidate validation scenarios

These are proposed expectations; **all are not-run**. They are not the existing Noble conformance suite and do not imply an implementation exists.

| ID | Expected observation |
|---|---|
| BA-01 | Existing sequential `compose` order and abnormal-termination behavior remain unchanged. |
| BA-02 | Traversals check with runtime-supplied program values, not just literal quotations. |
| BA-03 | An execution optimization preserves reified recipes, including programs inside returned aggregates. |
| BA-04 | Empty effects alone do not authorize branch speculation, resource duplication, or arbitrary reduction reassociation. |
| BA-05 | Source renaming preserves semantic identity; compiler/options changes affect the appropriate build/artifact identities. |
| BA-06 | Changed captures change program-value identity; changed host state does not reuse a result cache entry without an applicable environment policy. |
| BA-07 | An added variant cannot be accepted as an old closed schema without explicit compatibility handling. |
| BA-08 | Existing prepared programs remain pinned after name rebinding; explicit live resolution rejects incompatible replacements. |
| BA-09 | A valid package digest alone cannot bypass interface/effect checking, artifact correspondence, or receiver authorization. |
| BA-10 | Replay detects request mismatch and never converts recorded resource identifiers into live authority. |
| BA-11 | Shared frontend semantic diagnostics agree; editor recovery cannot prepare invalid partial source. |
| BA-12 | Remote deployment without required authentication/admission is denied before candidate execution. |

## 7. Source register

The following Git blob identifiers were returned by the GitHub connector. They identify observed file contents, not commit IDs or proof evidence. Branch URLs are discovery locations and may later change. Documentation was reviewed, not the whole implementation. In particular, Ambient architecture and live-upgrade reads were scoped to the sections used above.

| ID | Primary source | Observed Git blob |
|---|---|---|
| B1 | [Bend README](https://github.com/HigherOrderCO/Bend/blob/main/README.md) | `9e78f075389e1529d7cec5645e12c637a3999fbe` |
| B2 | [Bend guide](https://github.com/HigherOrderCO/Bend/blob/main/GUIDE.md) | `f048b9b40ea3ff31377bd2eeadf2c1c95ef718ed` |
| B3 | [Bend features](https://github.com/HigherOrderCO/Bend/blob/main/FEATURES.md) | `ce60841f54f068c1ad539561ca0e824cc60f4368` |
| A1 | [Ambient README](https://github.com/OnixResearch/ambient/blob/main/README.md) | `1f97aacfd9276200c7bafd38baa4fab974800868` |
| A2 | [Ambient architecture](https://github.com/OnixResearch/ambient/blob/main/ref/architecture.md) | `5a37825c4876d93c19985940b28c2b2f3f24a0ef` |
| A3 | [Ambient live upgrade](https://github.com/OnixResearch/ambient/blob/main/ref/live-upgrade.md) | `5cec4d02b97cbc88a6f9b9865d0ff4c7f16180ae` |
| A4 | [Ambient remote execution](https://github.com/OnixResearch/ambient/blob/main/ref/remote-execution.md) | `44c41cb61a63bc8d101a31a681f67bb74e945f69` |

Noble sources supplied in this Project:

- **N1:** `language-specification-0.1.0-draft.1.md` — SPEC-0001, revision 0.1.0-draft.1.
- **N2:** `VERIFICATION.md` — SPEC-V001, revision 0.1.0-draft.1.
- **N3:** `PROGRAM-CONTRACTS.md` — SPEC-V002, revision 0.1.0-draft.1.
- **N4:** `VERIFICATION-TOOLCHAIN.md` — IMPL-V001, revision 0.1.0-draft.1.
- **N5:** `rfc-0001-minimal-programs-as-data.md` — original RFC and its existing reference catalogue; preserved unchanged.

The original SPEC-0001 and verification integration preview retain their existing status. No reference project's documentation silently resolves Noble's open inference, schema encoding, canonical hashing, ABI, ownership-recovery, or proof obligations.
