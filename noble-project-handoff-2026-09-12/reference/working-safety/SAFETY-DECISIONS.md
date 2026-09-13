# Noble Safety Decision Register

Document: DEC-S001  
Revision: 0.1.0-draft.1  
Date: 2026-09-12  
Status: Safety architecture selected; implementation and proof obligations remain open

## 1. Selected decisions

| ID | Decision | Consequence |
|---|---|---|
| SD-01 | Noble guest code has no `unsafe` escape hatch | Low-level unsafety is confined to compiler/runtime/host implementation boundaries |
| SD-02 | Undefined behavior is not a Noble semantic outcome | Safety violations become static rejection or specified runtime failure, never arbitrary semantics |
| SD-03 | Raw machine addresses are not ordinary guest values | No guest pointer forging, arbitrary dereference, or representation transmute |
| SD-04 | Preserve `Program<S,T,e>` as the checked execution boundary | `Syntax`, bytes, artifacts, and manifests require explicit validation/preparation |
| SD-05 | Effects remain separate from authority | Effect/type/proof information never grants capabilities |
| SD-06 | Live resources/capabilities remain move-only, recursively non-`Data`/non-`Capture` | No duplication, silent discard, capture, persistence, or serialization by generic operations |
| SD-07 | The standard concurrency profile forbids direct shared mutable Noble memory | Noble-visible data races are excluded by the concurrency model rather than synchronized by user pointers |
| SD-08 | Syndicate/Synit + Preserves define the standard concurrency/protocol safety direction | Dataspace/facet scope, capability checks, and schema validation become safety obligations |
| SD-09 | Host/native/FFI/Wasm boundaries validate hostile representations | Verified internal preconditions cannot be assumed from unverified callers |
| SD-10 | Memory/type/resource/concurrency safety are mandatory; total correctness is separate | “Safe” does not silently promise termination, liveness, or business correctness |
| SD-11 | Stable safety is a mechanized release gate | Lean metatheory + actual checker correspondence + executed tests + trust ledger are required |
| SD-12 | Future dependent/higher-rank/subtyping/refinement features remain allowed | Initial simplicity is sequencing, not a permanent expressiveness ban |

## 2. Relationship to the baseline

These decisions strengthen existing baseline commitments rather than replacing them. SPEC-0001 already rejects stack/type fallback, separates effects from authority, makes live resources move-only/noncapturable, and requires host-side resource validation and explicit preparation. SPEC-V001 already requires finite-prefix safety claims, a Lean reference model, actual Rust-checker correspondence, explicit trusted boundaries, and stronger source-to-Wasm evidence for end-to-end claims.

The safety extension makes the missing project-wide policy explicit: there is no guest opt-out from those guarantees, and undefined behavior is not part of Noble's source-language semantics.

## 3. Open work

| ID | Deliverable |
|---|---|
| OS-01 | Complete declarative checker/inference rules and explicit candidate witness representation |
| OS-02 | Lean proof of SO-01 through SO-05 for the first stable core subset |
| OS-03 | Actual Rust acceptance-checker refinement via the selected Charon/Aeneas route |
| OS-04 | Verus-backed or equivalently assured resource/capability table and public wrappers |
| OS-05 | Concrete Noble-to-Wasm safety correspondence/translation-validation strategy |
| OS-06 | Standard Syndicate/Synit concurrency profile and formal isolation/scope/capability semantics |
| OS-07 | Bounded Preserves decoder/schema adapter profile for hostile wire input |
| OS-08 | Inventory of unsafe/native compiler/runtime/host implementation boundaries |
| OS-09 | Executed negative/property/fuzz tests for malformed artifacts, handles, and wire values |
| OS-10 | Release claim ledger defining exactly which subset is Noble-safe |
