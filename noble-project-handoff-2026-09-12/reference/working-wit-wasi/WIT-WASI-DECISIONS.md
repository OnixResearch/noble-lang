# WIT/WASI Integration Decision Register

Document: DEC-W001  
Revision: 0.1.0-draft.1  
Date: 2026-09-12  
Status: Architecture selected; implementation and conformance remain open

## 1. Selected decisions

| ID | Decision | Consequence |
|---|---|---|
| WD-01 | Component Model components are Noble's standard portable deployment/interoperability artifact | Core Wasm may remain an internal/lower-level profile, not the default external contract |
| WD-02 | WIT is Noble's standard component/host interface language | Compiler consumes WIT directly and emits WIT-compatible exports |
| WD-03 | Noble remains richer than WIT internally | `Program<S,T,e>`, stack polymorphism, effects, recipes, proofs, and identity do not become WIT constructs |
| WD-04 | WIT worlds are explicit compilation contracts | Imports/exports and exact versions participate in compatibility and build identity |
| WD-05 | Imported WIT functions are conservatively effectful | WIT signatures do not prove purity or determinism |
| WD-06 | WIT resources inherit Noble ownership | `own` becomes move-only; `borrow` is scoped/noncapturable and may not escape |
| WD-07 | Interface availability/effects remain separate from runtime authority | Linking/types do not forge host capabilities |
| WD-08 | Stable WASI interfaces are the preferred standard host APIs | Avoid parallel Noble filesystem/network/clock/etc. ABIs without a semantic reason |
| WD-09 | WASI 0.3 is the preferred standard host family | Exact 0.3.x packages/toolchains are pinned; 0.2 is an explicit compatibility profile |
| WD-10 | Component Model native async is Noble's standard cross-component async ABI | No requirement for source-level `async`/`await`; direct-style sequential semantics remain |
| WD-11 | `stream<T>`/`future<T>` are profile-level typed values, conservatively noncapturable when live | Async ABI does not bypass resource/lifetime safety |
| WD-12 | `Program<S,T,e>` is not automatically a WIT function/resource | Portable program transport remains separately specified and validated |
| WD-13 | Syndicate/Synit component crossings use WIT; Preserves remains protocol data | WIT and Preserves are complementary rather than competing data/interface systems |
| WD-14 | WIT/WASI exact versions participate in BuildKey, not DefinitionId | Interface upgrades do not silently change source semantic identity |
| WD-15 | Component adapters are safety boundaries | Lifting/lowering, resource tables, callbacks and loading require validation/testing/assurance |

## 2. Baseline changes

This decision resolves the architectural direction of baseline open item O-07: the standard cross-component lowering is the WebAssembly Component Model with WIT, and the standard host-family target is WASI 0.3. It does **not** complete O-07 operationally: exact type mappings, resource adapters, runtime/toolchain matrices, code generation, and conformance tests remain open deliverables.

Baseline G-05 is therefore strengthened from “choose WIT/component lowering” to “implement and validate the selected WIT/Component/WASI profile.”

## 3. Deliberate non-decisions

This revision does not:

- make every Noble definition a component;
- constrain internal Noble types to WIT types;
- define a WIT serialization for arbitrary `Program<S,T,e>` values;
- claim a WIT function is pure because it lacks an effect annotation;
- make an imported interface equivalent to unrestricted ambient authority;
- add Noble `async`/`await` syntax;
- make native async a scheduler/concurrency semantics;
- adopt an exact WIT/WASI patch version forever;
- claim the compiler/runtime currently passes the new conformance suite.

## 4. Immediate implementation order

1. Add WIT parsing/package identity to the compiler and build graph.
2. Generate typed import bindings with conservative `WitOpId` effects.
3. Implement closed-stack export adapters.
4. Implement exact owned/borrowed resource bridging.
5. Emit/load Component Model components for one small world.
6. Target a pinned WASI 0.3 profile in Wasmtime.
7. Exercise one native `async func`, one `stream<T>`, and one `future<T>` boundary.
8. Add a small Syndicate WIT boundary using Preserves payloads.
9. Execute cross-language and hostile-boundary tests.
10. Add translation-validation/proof work for the selected boundary.
