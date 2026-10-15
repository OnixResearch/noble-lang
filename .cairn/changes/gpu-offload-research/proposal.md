# Type/effect-directed GPU offloading for numeric Programs (issue #2)

## Why

Issue [#2](https://github.com/OnixResearch/noble-lang/issues/2) asks whether checked `Program<S,T,e>` interfaces can support GPU execution of a constrained pure numeric subset. It asks for a design recommendation, not an implementation.

The current language gives typed Programs and conservative latent effect bounds. It also specifies that an empty effect bound does not exempt work from limits (H-LIMIT-02) and that effect bounds may be widened conservatively (K-EFFECT-02). An empty bound therefore says nothing about termination, bounded memory, independence or profitability.

Core-Bootstrap also has no map primitive and no recursion:

- The word table stops at `list.case`, `test.emit` and `test.abort` (`crates/noble-contracts/src/program/words.rs` L87-L112).
- Recursive definitions remain open (README L535).
- A `List<I64>` uses two 48-byte cells per element: a cons cell plus a boxed scalar head (`crates/noble-wasm/runtime/operations.wat` L46-L54; `stack.wat` L46-L52).

Without a defined eligible subset, an explicit dispatch boundary, an I64-exact device lowering, a cost model and an independent test plan, any GPU work would risk changing K-NUM-01 arithmetic or the observable quota/fallback semantics. r[GPU-SCOPE-01] r[GPU-NUM-01]

## What Changes

- Add a research/design record (`design.md`) recommending:
  - a small **explicit, typed batch-map boundary** over statically known, independently checked, effect-free `I64` Programs, as the first and only entry point (inference of parallel regions is deferred);
  - **compile-time eligibility and kernel generation**, separated from **runtime device selection**, with device-independent precharged quota;
  - per-device-API feasibility for exact wrapping signed `I64`, an end-to-end cost model and an independent correctness test plan.

  r[GPU-SCOPE-01] r[GPU-DISPATCH-01] r[GPU-NUM-01] r[GPU-FALLBACK-01] r[GPU-GATE-01]
- Add five `backend-experiments` requirements. They apply only if a later change prototypes GPU offload:
  - the eligible subset and explicit boundary; r[GPU-SCOPE-01]
  - eligibility does not imply selection, and selection does not change observable results or quota; r[GPU-DISPATCH-01]
  - exact K-NUM-01 lowering with no division or floating point; r[GPU-NUM-01]
  - fallback, launch-failure, interruption and quota behaviour; r[GPU-FALLBACK-01]
  - independent evidence before any selection. r[GPU-GATE-01]

## Impact

- **Files**: `.cairn/changes/gpu-offload-research/{proposal,design,tasks}.md`, `metadata.json` and the ADDED delta `specs/backend-experiments/spec.md`. Canonical specs, conformance JSON, the manifest, roadmap/status, generated views, the ledger and source code are unchanged.
- **Testing**: Pinned Cairn `validate` and `gate proposal|design|tasks gpu-offload-research`. These are structural only. No GPU, kernel or Noble execution occurs, and the GPU-PLAN cases in `design.md` are unexecuted.
- **Coordination**: #4 (CPU fallback route and its accounting; `cranelift-backend-evaluation`), #14 (numeric-graph stack-to-graph extraction precedent; that issue owns FP32 and training), #3/#5 (packed batch buffers, device-buffer lifetime and allocator), active `bend-qcue-scoped-adaptations` DX-PURE-PAR-01 (pure fork/join admission precedent) and `runtime-program-template-identity` (template identity as a possible kernel key).

## Out of Scope

Implementation; selecting a GPU API, crate or driver; new syntax or a packed array type; floating point, division or checked arithmetic; automatic parallel-region inference; resources, host effects, live slots or live-REPL integration; general arena reclamation or lifetime redesign; benchmarks; proof or conformance-state promotion.
