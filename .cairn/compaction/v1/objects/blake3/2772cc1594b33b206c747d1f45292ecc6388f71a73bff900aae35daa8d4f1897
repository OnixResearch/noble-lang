## Context

The acceptance contract is `specs/roadmap.json`, `.cairn/specs/core-bootstrap/spec.md` and `specs/conformance/cases.json` at draft.5. M4 depends on completed M2 and M3. M3 selects managed linear memory; its WasmGC comparison and fixed experimental entry point remain regression evidence, not a second production backend. The current kernel independently derives concrete node interfaces. MC1 contains bounded source parsing/inference infrastructure but deliberately excludes effects and runtime execution. M3 owns a compiled-continuation emitter, tagged stack, arena, program graph and recipe representation, currently restricted to a quotation-only root and scalar capture.

## Decisions

### Decision: Reuse the acceptance boundary, not a fixture dispatcher

**Choice:** Add a bounded Core-Bootstrap source frontend around the existing schemes and inference patterns. Parsing accepts the normative lexical microprofile and either one expression body or one `def name [ body ]` declaration. Resolution snapshots exact dependencies. Inference emits finite untrusted candidates with complete concrete instantiations. Kernel acceptance derives all node interfaces against separately supplied session input types, expected output and environment/effect policy. Backend compilation rechecks its inputs; no acceptance flag or candidate metadata authorizes execution.

**Rationale:** This retains M2's independently checked boundary and MC1's existing machinery without treating an inferred witness as evidence. Definition bodies must be checked without execution and every executable specialization checked independently; a forged scheme cannot authorize a different body. Named uses freshen rank-1 variables; duplicated first-class programs retain one monomorphic interface.

### Decision: Compile ordinary execution and runtime builders

**Choice:** Generalize the managed-linear-memory lowering to arbitrary accepted source bodies, all bootstrap operations, full capturable data, and declared resource-free host effects. Quotation bodies and primitive/continuation functions become Wasm functions. Runtime quote/compose/run schedule compiled entries and carry immutable captures; recipes are inert semantic metadata and never determine instructions. Preserve quotation boundaries and exact resolved identities, including definitions, through optimization.

**Rationale:** This removes M3's experiment-only vocabulary while retaining its bounded graph and ordered-interface conventions. All source text and candidate acceptance stay outside guest execution. Dynamic captures and program selection occur after compilation with no compiler-service import.

### Decision: Transactional preparation and persistent compiled values

**Choice:** Expose a one-submission CLI and a persistent session. The session retains the namespace snapshot, ordered stack types, compiled program bodies and runtime values across submissions. Preparation completes before committing a definition or beginning expression execution. Parse, resolution, typing, acceptance and lowering refusals leave namespace/stack untouched and make zero candidate-body host requests. Previously prepared program values preserve their code, captures and resolved dependencies even after name rebinding. Report runtime traps and exhaustion distinctly; prior host requests cannot be rolled back.

**Rationale:** Replaying source history or interpreting reflected recipes would violate the execution contract. Session persistence must retain actual compiled closures and data, not reconstruct program meaning from a display recipe.

### Decision: Explicit resource-free environment and bounds

**Choice:** Bind supported builtin identities and explicit host contracts (`test.emit`, `test.abort`, plus harness-only resource-free inputs when required) before acceptance. Host effects preserve request order and stop after a trap. Carry finite byte/node/nesting/type/stack/dependency/work/output limits through preparation, and finite allocation/recipe/depth/continuation/execution limits through Wasm. Unknown profiles, host contracts, live resource values, recursion, imports and exhausted budgets return explicit outcomes.

**Rationale:** Resource-bearing types remain available only for negative eligibility checks. Empty latent effects do not exempt pure work from runtime limits. Limits must be charged before bounded allocation/traversal; no fallback execution follows refusal.

### Decision: Separate execution, extraction and proof evidence

**Choice:** Run CORE-01..18 using the exact declared sources and structured harness parameters, with optimization off/on where observables can change. Add targeted controls for ordered interfaces, first-class monomorphism, fresh definitions, immutable rebinding, all data/control words, aggregate captures, lexical edges, traps, hostile candidates and exact/exceeded limits. Re-extract every changed pure production component from actual Rust using pinned Charon/Aeneas, compile the generated Lean, audit dependencies/axioms and exercise refusal controls. Renew the existing source inventory and full quality checks without reducing target/rule coverage.

**Rationale:** Passing examples and successful extraction are not universal soundness, compiler correctness, or backend refinement. Owned Wasm assets, assembler, optimizer, engine, loader and external models remain explicit trust subjects. Open obligations remain open.

## Risks / Trade-offs

- Concrete kernel acceptance does not by itself prove rank-1 inference soundness. Definition installation and specialization must preserve constraints and independently reject forged contracts; universal frontend/refinement claims remain open.
- Cross-submission compiled programs require persistent code/capture/interface identity. Neither source replay nor recipe execution is an acceptable persistence shortcut.
- Full capture/reflection support increases bounded graph traversal and representation coverage; complete type/schema and program-interface metadata must survive aggregates and optimization.
- Runtime traps can leave an already-issued host-effect prefix. Only static/preparation refusal is transactional; runtime state policy must be explicit and tested.
- Extraction/tooling may require narrowly documented compatibility patterns; opaque local bodies or silent exclusions cannot close the changed-code gate.

## Acceptance and closeout

Every implementation and verification task is required before marking M4 complete. Durable receipts bind exact source/tool identities, raw execution artifacts, actual CLI/session output, extraction/dependency/axiom results and full quality gates. Update conformance, status and roadmap only to the scope actually established. Sync and archive the Cairn change only after the tasks pass, then rerun post-archive validation.
