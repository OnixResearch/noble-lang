# Current Noble Decisions Snapshot

This file records the **selected project direction as of 2026-09-12**. It is a handoff snapshot, not a substitute for the normative specifications.

## Core language

- Noble is a small typed concatenative language with first-class programs-as-data.
- `Program<S,T,e>` is the checked executable abstraction.
- Quotation construction is distinct from execution.
- Selected surface names: `run` (older drafts: `call`) and `reflect` (older drafts: `reify`).
- `compose` is sequential and ordered; concurrency is not implied.
- Runtime-built `quote`/`compose` values must work without source recompilation.
- `Syntax` and checked `Program` remain distinct staging states.
- `#` is the selected comment direction; older baseline `//` text is historical.

## Type system

- Initial core: stack tails, rank-1 named polymorphism, algebraic data, effect variables/sets, first-class instantiated programs.
- No dynamic `Any` fallback for arbitrary program stack effects.
- Dependent types, higher-rank polymorphism, principled subtyping/refinement, and extensible inference remain desired future capabilities if they preserve the safety/verification architecture.

## Safety

- No guest `unsafe` escape hatch.
- Undefined behavior is not a Noble-language semantic outcome.
- No ordinary raw pointers or address forging.
- Effects do not grant authority.
- Live resources/capabilities are move-only and recursively non-duplicable/noncapturable by generic operations.
- Host/FFI/Wasm/component boundaries validate untrusted representations.
- Standard concurrency forbids direct shared mutable Noble memory.
- Stable safety claims require mechanized evidence and executed tests, not prose alone.

## Wasm / Component Model / WIT / WASI

- Wasm is the production execution substrate.
- Component Model components are the standard portable deployment/interoperability artifact.
- WIT is the standard external interface language.
- Internal Noble types remain richer than WIT.
- WIT worlds are explicit compilation/import/export contracts.
- WIT imports are conservatively effectful unless a stronger verified contract exists.
- WIT resources inherit Noble ownership; `borrow` must be scoped and nonescaping.
- WIT/interface availability does not imply runtime authority.
- Stable WASI interfaces are preferred host APIs; WASI 0.3 is the selected preferred family in the working profile, with exact versions pinned and 0.2 explicit compatibility.
- Native Component Model async is the selected cross-component async ABI without requiring source-level `async`/`await`.
- `Program<S,T,e>` transport remains a separate portable-code protocol problem.

## Concurrency

- Syndicate/Synit + Preserves are the selected de facto concurrency/service/protocol direction.
- Dataspaces, assertions, interests, messages, and facets are the model to adapt.
- Concurrency stays outside the kernel; `Program` is not an actor.
- Preserves is protocol/schema data; WIT is component interface/ABI. They are complementary.
- Syndicate component crossings should use versioned WIT packages.
- BEAM/OTP is a secondary reference for supervision/failure semantics, not the primary concurrency semantic model.
- Older BEAM-first concurrency drafts must be reconciled rather than extended unchanged.

## Choreography

- Choreography remains optional and outside the kernel.
- It should preferentially lower onto the Syndicate/Synit layer.
- Existing choreography semantics are useful but need reconciliation with the newer concurrency direction.

## Verification

- Lean 4: reference semantics, metatheory, program-contract logic.
- Charon/Aeneas: preferred bridge from the actual safe sequential Rust checker to Lean.
- Verus: selected imperative components, especially handle/resource lifecycle machinery.
- Backend/source-to-Wasm/component correspondence is a separate proof/validation lane.
- Language proof, Rust proof, program proof, artifact correspondence, and host trust are separate claims.

## Identity

- `DefinitionId`, `ProgramValueId`, `BuildKey`, and `ArtifactId` are distinct.
- Names do not define semantic identity.
- WIT/WASI versions and component adapters belong in build identity where relevant.
- Canonical Noble program encoding/hashing remains unresolved.

## Explicitly not settled

- Complete inference/checking algorithm and witness representation.
- Complete declaration/module/type syntax.
- Canonical program bytes/hash/versioning.
- Final Wasm program calling convention and memory management.
- Complete WIT-to-Noble mapping/resource adapters/toolchain matrix.
- Portable program package/transport format.
- Standard Syndicate/Synit Noble API and formal semantics.
- Preserves hostile-input adapter profile.
- Choreography-to-Syndicate projection details.
- Concrete repository implementation status unless verified from the actual repo.
