<!-- Generated compatibility view. Edit .cairn/specs/backend-experiments/spec.md instead. -->
# Backend representation experiments

Document: SPEC-BE001  
Revision: 0.1.0-draft.5  
Depends on: SPEC-0001, SPEC-B001 and SPEC-S001 at 0.1.0-draft.5  
Status: Bounded M3 comparison complete with managed-linear-memory selected for M4; optional post-M3 layout study unexecuted, universal backend refinement open

## Scope and selection

The [external study](../review/EXTERNAL-DESIGN-STUDY.md) motivates this experiment. Zena supplies design examples, not a required dependency or Noble compatibility evidence.

**BE-COMPARE-01.** M3 MUST compare `wasm-gc` with `managed-linear-memory` for the same declared bootstrap workload. Each candidate needs an executed result or an evidenced blocker. The selection record MUST NOT present an unsupported candidate as a measured success.

M3 does not require two production backends. A documented blocker can end one candidate's trial. At least one candidate must satisfy the existing M3 execution gates before M3 closes.

WasmGC is not selected by this document. Neither a small binary nor an upstream benchmark establishes suitability for Noble.

**BE-CONFIG-01.**

Each run MUST record immutable tool revisions, target, workload, optimization mode, resource limits, and enabled Wasm features. The record names GC, function-reference, multi-value, tail-call, and component support separately. Unsupported instrumentation MUST remain explicit rather than produce a zero measurement.

M3 selects an experimental resource-free, monomorphic compiled-continuation configuration. Its comparison uses the same checked workload and bounded graph/continuation policy for WasmGC structs and managed-linear-memory records. The Rust lowering/emitter, owned runtime assets, verification-only engine host, assembler and optimizer MUST have separately identified source/tool subjects and trust boundaries. Runtime inputs MUST arrive after module compilation, and module execution MUST NOT import a source-preparation or compiler service.

The experiment MUST distinguish required module features from unused capabilities enabled by the selected engine. Guest logical allocation charges, retained linear-memory bytes, and process/engine memory observations MUST NOT be conflated. In particular, the common logical size assigned to a GC object MUST NOT be reported as a measured physical GC allocation.

#### Scenario: Pinned optimized and unoptimized trials

- GIVEN one checked workload, immutable assembler/optimizer/engine identities and declared resource limits
- WHEN both representation candidates execute under optimization off and on
- THEN each result retains its actual configuration, complete correctness outcomes and explicit unknown instrumentation without treating a blocked or failed candidate as a success

#### Scenario: Runtime input after preparation

- GIVEN an already compiled module with no source-preparation imports
- WHEN runtime captures, collection selections and composition-tree shapes vary
- THEN the compiled operations and Wasm type set remain fixed while outputs and normalized recipes match the declared workload

**BE-DYNAMIC-01.** Runtime builders MUST reuse compiled operations over an explicit supported interface set. They MUST NOT require Noble source preparation, recipe interpretation, or a new Wasm type for each runtime composition tree.

A candidate can use typed entry points, immutable capture environments, and shared recipes. A Rust-written compiler can emit WasmGC directly. An ordinary Rust-to-Wasm build does not select that guest representation automatically.

**BE-ARITY-01.** Calling conventions and generated adapters MUST preserve complete ordered stack interfaces and ownership obligations. They MUST NOT discard extra arguments to make incompatible interfaces appear compatible.

Explicit checked words can consume their declared inputs. This rule prohibits hidden arity adaptation, not ordinary `drop` on eligible data.

**BE-REFLECT-01.** Optimization and reachability analysis MUST preserve required recipe observations and semantic identity. A program reachable only through a collection, returned value, or reflection path remains observable. Required recipe information MUST NOT be stripped as optional debug metadata.

**BE-LIMIT-01.** Each candidate MUST define limits for allocation, retained recipes, composition depth, and execution-stack use. Limit failures MUST produce specified outcomes with accounted cleanup. Reports distinguish construction failure from invocation failure.

Tail calls do not eliminate every pending continuation in arbitrary composition trees. Compiled trampolines or explicit continuation storage remain candidates. Dispatch over compiled operations is permitted by W-EXEC-01.

The comparison uses left-associated, right-associated, and balanced trees over the same ordered leaves. Successful runs preserve output, effect order, and normalized recipes. Quota outcomes can differ across representations but cannot count as successful executions.

## Data, resources, and components

**BE-RESOURCE-01.** GC or reference-counting reachability MUST NOT discharge live-resource or native-pin obligations. Representation changes MUST preserve recursive `Data`/`Capture` checks and SPEC-R001 retirement rules. REPL values remain in explicit session-owned storage.

**BE-BOUNDARY-01.** A component compatibility probe MUST account for lifting/lowering, memory allocation, copies, and cleanup under a pinned ABI. Internal WasmGC support MUST NOT imply zero-copy WIT strings or lists.

The historical component probe was nonblocking for resource-free M3. Its incomplete results are not M5 evidence, and M5 cannot inherit conformance from that probe. The separate [M5 gate](../verification/m5/gate.mjs) exercises the pinned `noble-test:sync/bootstrap@1.0.0` world against an independent Rust/Wasmtime 40.0.2 component peer. Lossless UTF-8 string and byte-list round trips are paired with unchanged core-Wasm hostile-import probes for allocation failure, malformed UTF-8 and truncated ranges, including cleanup without partial trusted results. Core instrumentation is not independent component-peer evidence and does not measure all engine allocations or physical copies.

M5 also exercises cancellation and unexpected-suspension retirement with actual guest GC and a separately retained native pin, followed by late and duplicate callbacks. This bounded local control does not implement native async or make GC responsible for resource retirement. The [resource-transition correspondence](../proofs/m5/M5Resources.lean) has a narrower claim than native release, authenticated host callbacks or universal backend/ABI refinement. Neither these scopes nor M3's representation comparison establish full `Component-Draft` or WASI conformance.

## Comparison record

**BE-REPORT-01.**

The comparison MUST report emitted bytes by section, preparation time, execution time, allocation counts, and peak memory where observable. It also records depth limits, correctness outcomes, recipe observations, and trust boundaries. Engine memory and guest logical allocation measurements MUST remain distinguishable.

Repeated timing samples need their sample count and aggregation method. Missing measurements remain unknown. No performance claim can omit a failed correctness case for the same claimed workload.

Existing CORE-03, CORE-05, and CORE-09 cover dynamic inputs and reflection. Adaptation scenarios in `specs/conformance/adaptation-cases.json` add depth, arity, reachability, boundary, and selection-policy designs. Their state and evidence fields record scoped execution separately from these requirements. M3 comparison evidence does not close M4, component interoperability, or backend proof obligations.

#### Scenario: Retained measurements and unknown instrumentation

- GIVEN a completed trial with repeated samples, exact module sections, runtime recipes and cleanup observations
- WHEN a comparison record is produced
- THEN it retains the full trial artifacts and timing aggregation while unavailable physical GC and isolated engine measurements remain explicitly unknown

#### Scenario: Correctness precedes representation selection

- GIVEN candidate records with exact configurations and declared workload outcomes
- WHEN the selection policy evaluates passing, blocked, missing-pin, fabricated-metric and failed-correctness records
- THEN only correctly executed candidates with valid scoped evidence are eligible and no failed or unsupported record becomes a performance success

**BE-LAYOUT-01.**

An optional post-M3 experiment MAY compare the selected managed-linear-memory Wasm lowering with a type-directed finite algebraic-data layout and A-normal/call-lowering alternative over the same accepted, resource-free typed programs. Each candidate MUST retain exact ordered stack/call semantics, checked source and instantiated interface, resolved schema/program identity, observable normalized recipes and captures, and effect/request ordering. It MUST compare runtime-supplied inputs after compilation under matching declared limits and pinned compiler/optimizer/engine/feature configurations, both optimization off and on. The record MUST distinguish normal results, traps and construction/invocation quota failures; record guest logical allocation charges and retained linear-memory bytes separately, expose cleanup, and mark unavailable engine/process memory observations unknown or unsupported rather than report zero. It MUST include finite two-arm variant, nested product/list, runtime quotation/composition and reflection shapes, plus hostile quota and malformed-boundary controls. Type-directed specialization MUST NOT smuggle a new source-preparation step, change portable program identity, omit a dynamically reachable recipe, or erase a checked effect/owner obligation.

Any later resource/component-positive candidate MUST independently preserve WIT canonical lowering/lifting, exact ABI ownership/post-return or trap cleanup, native-pin retirement and host authorization; resource-free measurements MUST NOT be advertised as this evidence. No unconditional observational equivalence, universal backend theorem or performance win follows from Bend's published examples or a faster finite trial. M3's existing completed WasmGC-versus-managed-memory results remain historical and unchanged; this study is not a reopened M3 gate.

#### Scenario: ADAPT-17 finite layout comparison

- GIVEN identical checked resource-free workloads, fixed post-compilation inputs, two pinned managed-memory lowerings and declared resource limits
- WHEN both configurations execute optimized and unoptimized runs with reflection, normal paths, traps and quota boundaries
- THEN each run records complete ordered value/recipe/effect and allocation/quota observations; discrepancies remain visible, with no resource-positive or universal-proof promotion
