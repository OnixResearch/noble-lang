# Backend representation experiments

<!-- cairn:purpose:start -->
## Purpose

This accepted specification records Noble draft contracts, not completed implementation.
Original requirement IDs, explanatory prose, examples, and open decisions remain authoritative.
Scenario clauses declare designs; the conformance ledger records execution and evidence.

## Requirements

<!-- cairn:purpose:end -->


Document: SPEC-BE001  
Revision: 0.1.0-draft.5  
Depends on: SPEC-0001, SPEC-B001 and SPEC-S001 at 0.1.0-draft.5  
Status: Bounded M3 comparison complete with managed-linear-memory selected for M4; optional post-M3 layout study unexecuted, universal backend refinement open

## Scope and selection

The [external study](../../../review/EXTERNAL-DESIGN-STUDY.md) motivates this experiment. Zena supplies design examples, not a required dependency or Noble compatibility evidence.

### Requirement: BE-LIVE-01
r[BE-LIVE-01]

**BE-LIVE-01.** The first `Live-Wasm-Draft` target MUST reuse the currently selected persistent Node 24.13.0/V8 13.6.233.17-node.37 Core WebAssembly engine and managed-linear-memory program representation, but implement a *new opt-in* in-process deterministic Wasm binary emission and validation path from independently accepted source. The existing Core engine prepares WAT using external `wasm-tools parse/validate`, optionally Binaryen `wasm-opt`, and `WebAssembly.Module` on each submission; that existing path is not instant reload and installing a candidate directly against its shared live imports is not transactional staging. The live emitter MUST pin a format/ABI revision and exact supported Wasm feature/opcode set, canonical instruction encodings and control/stack validation; the selected VM MUST validate exact bytes and exact import/export types and reject unknown versions, unsupported features, start sections, unauthorized imports, excessive module/section/function/table/memory sizes, mismatched ABI and invalid control/operand stacks before publication. No externally supplied Wasm, claimed manifest or arbitrary bytes become executable from a digest alone. The generation/admission record and build key MUST bind exact source/import bytes, resolved definitions, types, effects, capability contracts, limits, compiler/ABI/engine revision and emitted bytes; language-level semantic definition identities instead follow canonical resolved bodies and dependencies under P-ID-01/02/04, so formatting or build-only changes do not change them. Encoding and verification cost and VM compilation/instantiation time MUST be bounded and observed separately from execution.

The binary envelope is standard WebAssembly core magic plus binary version `1` (little-endian `01 00 00 00`), not a Noble bytecode format. The first selected feature vocabulary is wasm32 memory32, `i32`/`i64`, finite control blocks/branches, locals/globals, calls and typed `call_indirect`, `funcref` table, and explicitly validated passive data/element bulk-memory initialization used by the selected managed-memory backend. Arithmetic uses WebAssembly's defined wrapping/trap semantics as constrained by checked Noble `I64`; source-level stack/effect typing remains the independent kernel's responsibility. SIMD, shared-memory threads/atomics, WasmGC, memory64, exceptions, tail calls and components are outside this core profile and MUST refuse rather than depend on an engine's broader support. Before implementation acceptance, the exact normative Wasm core/feature revision, allowed opcode inventory and byte encodings MUST be frozen alongside the emitter/validator and exercised by positive and unsupported-opcode controls; this draft does not pretend an as-yet-unwritten encoder has passed that gate.

Node/V8 WebAssembly execution can compile/JIT Wasm; its pinned `--no-liftoff --no-wasm-lazy-compilation --no-wasm-tier-up` settings do not prove interpretation. A claim of *strictly interpreted* execution requires separately selected and pinned actual Wasm interpreter engine (Wasmi is a candidate, not an accepted or currently selected engine), demonstrated no-JIT mode, exact feature/ABI parity and same source-bound negative/positive conformance before `noble live repl --engine interpreter` is enabled. Missing or incompatible interpreter MUST explicitly refuse that engine choice; it MUST NOT silently fall back to V8, Wasmtime JIT or a custom Noble evaluator.

The selected V8 path has no assumed native Wasm instruction fuel. Its binary emitter and verifier MUST enforce explicit bounded metering on all paths including backedges, nested calls and generated helpers, with a separately bounded recursion/operand stack and accounted allocations. A host watchdog provides a last-resort timeout by poisoning/ending the affected session, not claiming safe in-process rollback or transparent restart. The separately selected interpreter, if implemented, MUST account its actual steps under an equivalently declared budget; observed wall time and semantic step counts are different measures.

No cross-arena `Program` bridge is selected. Only candidate staging has an isolated shadow VM; after admission, the same accepted no-init bytes install into disjoint slots of the long-lived *shared* live memory/table/global arena at a top-level safepoint. Old checked Program handles remain local to that single arena and retain old table indices and immutable captures, including inside aggregates. The emitter MUST exclude start functions, active data/element segments and any initialization-time mutation of imported state; its bounded host-owned fresh-slot installation must either complete without effects or restore all new slots before refusal, never overwrite an old callable slot. The host MUST retain old instances while referenced and reclaim only unreferenced new/old slots under exact generation ownership and a finite session table/cell budget. Stage-time host imports are inert, and live imported forwarding callbacks remain authorization-gated after publication: no preparation-stage host request, newly acquired authority, or stale grant survives a rebind.

<!-- cairn:scenario-links:start -->
#### Scenario: LIVE-02 for BE-LIVE-01

- GIVEN the `Live-Wasm-Draft` profile and every field of `input` in [LIVE-02](../../../specs/conformance/live-wasm-cases.json)
- WHEN the `runtime` procedure for case `LIVE-02` runs against those inputs
- THEN the observations match every field of `expected` in case `LIVE-02`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: LIVE-04 for BE-LIVE-01

- GIVEN the `Live-Wasm-Draft` profile and every field of `input` in [LIVE-04](../../../specs/conformance/live-wasm-cases.json)
- WHEN the `admission` procedure for case `LIVE-04` runs against those inputs
- THEN the observations match every field of `expected` in case `LIVE-04`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: LIVE-07 for BE-LIVE-01

- GIVEN the `Live-Wasm-Draft` profile and every field of `input` in [LIVE-07](../../../specs/conformance/live-wasm-cases.json)
- WHEN the `admission` procedure for case `LIVE-07` runs against those inputs
- THEN the observations match every field of `expected` in case `LIVE-07`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: BE-COMPARE-01
r[BE-COMPARE-01]

**BE-COMPARE-01.** M3 MUST compare `wasm-gc` with `managed-linear-memory` for the same declared bootstrap workload. Each candidate needs an executed result or an evidenced blocker. The selection record MUST NOT present an unsupported candidate as a measured success.

M3 does not require two production backends. A documented blocker can end one candidate's trial. At least one candidate must satisfy the existing M3 execution gates before M3 closes.

WasmGC is not selected by this document. Neither a small binary nor an upstream benchmark establishes suitability for Noble.


<!-- cairn:scenario-links:start -->
#### Scenario: ADAPT-12 for BE-COMPARE-01

- GIVEN the `Implementation-Policy` profile and every field of `input` in [ADAPT-12](../../../specs/conformance/adaptation-cases.json)
- WHEN the `review` procedure for case `ADAPT-12` runs against those inputs
- THEN the observations match every field of `expected` in case `ADAPT-12`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: BE-CONFIG-01
r[BE-CONFIG-01]

**BE-CONFIG-01.**

Each run MUST record immutable tool revisions, target, workload, optimization mode, resource limits, and enabled Wasm features. The record names GC, function-reference, multi-value, tail-call, and component support separately. Unsupported instrumentation MUST remain explicit rather than produce a zero measurement.

M3 selects an experimental resource-free, monomorphic compiled-continuation configuration. Its comparison uses the same checked workload and bounded graph/continuation policy for WasmGC structs and managed-linear-memory records. The Rust lowering/emitter, owned runtime assets, verification-only engine host, assembler and optimizer MUST have separately identified source/tool subjects and trust boundaries. Runtime inputs MUST arrive after module compilation, and module execution MUST NOT import a source-preparation or compiler service.

The experiment MUST distinguish required module features from unused capabilities enabled by the selected engine. Guest logical allocation charges, retained linear-memory bytes, and process/engine memory observations MUST NOT be conflated. In particular, the common logical size assigned to a GC object MUST NOT be reported as a measured physical GC allocation.


<!-- cairn:scenario-links:start -->
#### Scenario: ADAPT-12 for BE-CONFIG-01

- GIVEN the `Implementation-Policy` profile and every field of `input` in [ADAPT-12](../../../specs/conformance/adaptation-cases.json)
- WHEN the `review` procedure for case `ADAPT-12` runs against those inputs
- THEN the observations match every field of `expected` in case `ADAPT-12`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

#### Scenario: Pinned optimized and unoptimized trials

- GIVEN one checked workload, immutable assembler/optimizer/engine identities and declared resource limits
- WHEN both representation candidates execute under optimization off and on
- THEN each result retains its actual configuration, complete correctness outcomes and explicit unknown instrumentation without treating a blocked or failed candidate as a success

#### Scenario: Runtime input after preparation

- GIVEN an already compiled module with no source-preparation imports
- WHEN runtime captures, collection selections and composition-tree shapes vary
- THEN the compiled operations and Wasm type set remain fixed while outputs and normalized recipes match the declared workload

### Requirement: BE-DYNAMIC-01
r[BE-DYNAMIC-01]

**BE-DYNAMIC-01.** Runtime builders MUST reuse compiled operations over an explicit supported interface set. They MUST NOT require Noble source preparation, recipe interpretation, or a new Wasm type for each runtime composition tree.

A candidate can use typed entry points, immutable capture environments, and shared recipes. A Rust-written compiler can emit WasmGC directly. An ordinary Rust-to-Wasm build does not select that guest representation automatically.


<!-- cairn:scenario-links:start -->
#### Scenario: CORE-03 for BE-DYNAMIC-01

- GIVEN the `Core-Bootstrap` profile and every field of `input` in [CORE-03](../../../specs/conformance/cases.json)
- WHEN the `runtime` procedure for case `CORE-03` runs against those inputs
- THEN the observations match every field of `expected` in case `CORE-03`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: CORE-05 for BE-DYNAMIC-01

- GIVEN the `Core-Bootstrap` profile and every field of `input` in [CORE-05](../../../specs/conformance/cases.json)
- WHEN the `runtime` procedure for case `CORE-05` runs against those inputs
- THEN the observations match every field of `expected` in case `CORE-05`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: ADAPT-09 for BE-DYNAMIC-01

- GIVEN the `Backend-Experiment` profile and every field of `input` in [ADAPT-09](../../../specs/conformance/adaptation-cases.json)
- WHEN the `runtime` procedure for case `ADAPT-09` runs against those inputs
- THEN the observations match every field of `expected` in case `ADAPT-09`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: ADAPT-10 for BE-DYNAMIC-01

- GIVEN the `Backend-Experiment` profile and every field of `input` in [ADAPT-10](../../../specs/conformance/adaptation-cases.json)
- WHEN the `runtime` procedure for case `ADAPT-10` runs against those inputs
- THEN the observations match every field of `expected` in case `ADAPT-10`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: ADAPT-17 for BE-DYNAMIC-01

- GIVEN the `Backend-Experiment` profile and every field of `input` in [ADAPT-17](../../../specs/conformance/adaptation-cases.json)
- WHEN the `runtime` procedure for case `ADAPT-17` runs against those inputs
- THEN the observations match every field of `expected` in case `ADAPT-17`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: BE-ARITY-01
r[BE-ARITY-01]

**BE-ARITY-01.** Calling conventions and generated adapters MUST preserve complete ordered stack interfaces and ownership obligations. They MUST NOT discard extra arguments to make incompatible interfaces appear compatible.

Explicit checked words can consume their declared inputs. This rule prohibits hidden arity adaptation, not ordinary `drop` on eligible data.


<!-- cairn:scenario-links:start -->
#### Scenario: ADAPT-08 for BE-ARITY-01

- GIVEN the `Backend-Experiment` profile and every field of `input` in [ADAPT-08](../../../specs/conformance/adaptation-cases.json)
- WHEN the `static` procedure for case `ADAPT-08` runs against those inputs
- THEN the observations match every field of `expected` in case `ADAPT-08`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: BE-REFLECT-01
r[BE-REFLECT-01]

**BE-REFLECT-01.** Optimization and reachability analysis MUST preserve required recipe observations and semantic identity. A program reachable only through a collection, returned value, or reflection path remains observable. Required recipe information MUST NOT be stripped as optional debug metadata.


<!-- cairn:scenario-links:start -->
#### Scenario: CORE-05 for BE-REFLECT-01

- GIVEN the `Core-Bootstrap` profile and every field of `input` in [CORE-05](../../../specs/conformance/cases.json)
- WHEN the `runtime` procedure for case `CORE-05` runs against those inputs
- THEN the observations match every field of `expected` in case `CORE-05`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: CORE-09 for BE-REFLECT-01

- GIVEN the `Core-Bootstrap` profile and every field of `input` in [CORE-09](../../../specs/conformance/cases.json)
- WHEN the `runtime` procedure for case `CORE-09` runs against those inputs
- THEN the observations match every field of `expected` in case `CORE-09`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: ADAPT-09 for BE-REFLECT-01

- GIVEN the `Backend-Experiment` profile and every field of `input` in [ADAPT-09](../../../specs/conformance/adaptation-cases.json)
- WHEN the `runtime` procedure for case `ADAPT-09` runs against those inputs
- THEN the observations match every field of `expected` in case `ADAPT-09`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: ADAPT-10 for BE-REFLECT-01

- GIVEN the `Backend-Experiment` profile and every field of `input` in [ADAPT-10](../../../specs/conformance/adaptation-cases.json)
- WHEN the `runtime` procedure for case `ADAPT-10` runs against those inputs
- THEN the observations match every field of `expected` in case `ADAPT-10`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: ADAPT-17 for BE-REFLECT-01

- GIVEN the `Backend-Experiment` profile and every field of `input` in [ADAPT-17](../../../specs/conformance/adaptation-cases.json)
- WHEN the `runtime` procedure for case `ADAPT-17` runs against those inputs
- THEN the observations match every field of `expected` in case `ADAPT-17`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: BE-LIMIT-01
r[BE-LIMIT-01]

**BE-LIMIT-01.** Each candidate MUST define limits for allocation, retained recipes, composition depth, and execution-stack use. Limit failures MUST produce specified outcomes with accounted cleanup. Reports distinguish construction failure from invocation failure.

Tail calls do not eliminate every pending continuation in arbitrary composition trees. Compiled trampolines or explicit continuation storage remain candidates. Dispatch over compiled operations is permitted by W-EXEC-01.

The comparison uses left-associated, right-associated, and balanced trees over the same ordered leaves. Successful runs preserve output, effect order, and normalized recipes. Quota outcomes can differ across representations but cannot count as successful executions.


<!-- cairn:scenario-links:start -->
#### Scenario: ADAPT-09 for BE-LIMIT-01

- GIVEN the `Backend-Experiment` profile and every field of `input` in [ADAPT-09](../../../specs/conformance/adaptation-cases.json)
- WHEN the `runtime` procedure for case `ADAPT-09` runs against those inputs
- THEN the observations match every field of `expected` in case `ADAPT-09`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: ADAPT-17 for BE-LIMIT-01

- GIVEN the `Backend-Experiment` profile and every field of `input` in [ADAPT-17](../../../specs/conformance/adaptation-cases.json)
- WHEN the `runtime` procedure for case `ADAPT-17` runs against those inputs
- THEN the observations match every field of `expected` in case `ADAPT-17`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: ADAPT-18 for BE-LIMIT-01

- GIVEN the `Backend-Experiment` profile and every field of `input` in [ADAPT-18](../../../specs/conformance/adaptation-cases.json)
- WHEN the `runtime` procedure for case `ADAPT-18` runs against those inputs
- THEN the observations match every field of `expected` in case `ADAPT-18`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

## Data, resources, and components

### Requirement: BE-RESOURCE-01
r[BE-RESOURCE-01]

**BE-RESOURCE-01.** GC or reference-counting reachability MUST NOT discharge live-resource or native-pin obligations. Representation changes MUST preserve recursive `Data`/`Capture` checks and SPEC-R001 retirement rules. REPL values remain in explicit session-owned storage.


<!-- cairn:scenario-links:start -->
#### Scenario: RA-CASE-05 for BE-RESOURCE-01

- GIVEN the `Component-Sync-Bootstrap` profile and every field of `input` in [RA-CASE-05](../../../specs/conformance/resource-cases.json)
- WHEN the `adapter` procedure for case `RA-CASE-05` runs against those inputs
- THEN the observations match every field of `expected` in case `RA-CASE-05`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

### Requirement: BE-BOUNDARY-01
r[BE-BOUNDARY-01]

**BE-BOUNDARY-01.** A component compatibility probe MUST account for lifting/lowering, memory allocation, copies, and cleanup under a pinned ABI. Internal WasmGC support MUST NOT imply zero-copy WIT strings or lists.

The historical component probe was nonblocking for resource-free M3. Its incomplete results are not M5 evidence, and M5 cannot inherit conformance from that probe. The separate [M5 gate](../../../verification/m5/gate.mjs) exercises the pinned `noble-test:sync/bootstrap@1.0.0` world against an independent Rust/Wasmtime 40.0.2 component peer. Lossless UTF-8 string and byte-list round trips are paired with unchanged core-Wasm hostile-import probes for allocation failure, malformed UTF-8 and truncated ranges, including cleanup without partial trusted results. Core instrumentation is not independent component-peer evidence and does not measure all engine allocations or physical copies.

M5 also exercises cancellation and unexpected-suspension retirement with actual guest GC and a separately retained native pin, followed by late and duplicate callbacks. This bounded local control does not implement native async or make GC responsible for resource retirement. The [resource-transition correspondence](../../../proofs/m5/M5Resources.lean) has a narrower claim than native release, authenticated host callbacks or universal backend/ABI refinement. Neither these scopes nor M3's representation comparison establish full `Component-Draft` or WASI conformance.


<!-- cairn:scenario-links:start -->
#### Scenario: ADAPT-11 for BE-BOUNDARY-01

- GIVEN the `Component-Sync-Bootstrap` profile and every field of `input` in [ADAPT-11](../../../specs/conformance/adaptation-cases.json)
- WHEN the `adapter` procedure for case `ADAPT-11` runs against those inputs
- THEN the observations match every field of `expected` in case `ADAPT-11`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

## Comparison record

### Requirement: BE-REPORT-01
r[BE-REPORT-01]

**BE-REPORT-01.**

The comparison MUST report emitted bytes by section, preparation time, execution time, allocation counts, and peak memory where observable. It also records depth limits, correctness outcomes, recipe observations, and trust boundaries. Engine memory and guest logical allocation measurements MUST remain distinguishable.

Repeated timing samples need their sample count and aggregation method. Missing measurements remain unknown. No performance claim can omit a failed correctness case for the same claimed workload.

Existing CORE-03, CORE-05, and CORE-09 cover dynamic inputs and reflection. Adaptation scenarios in `specs/conformance/adaptation-cases.json` add depth, arity, reachability, boundary, and selection-policy designs. Their state and evidence fields record scoped execution separately from these requirements. M3 comparison evidence does not close M4, component interoperability, or backend proof obligations.


<!-- cairn:scenario-links:start -->
#### Scenario: ADAPT-09 for BE-REPORT-01

- GIVEN the `Backend-Experiment` profile and every field of `input` in [ADAPT-09](../../../specs/conformance/adaptation-cases.json)
- WHEN the `runtime` procedure for case `ADAPT-09` runs against those inputs
- THEN the observations match every field of `expected` in case `ADAPT-09`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

#### Scenario: ADAPT-12 for BE-REPORT-01

- GIVEN the `Implementation-Policy` profile and every field of `input` in [ADAPT-12](../../../specs/conformance/adaptation-cases.json)
- WHEN the `review` procedure for case `ADAPT-12` runs against those inputs
- THEN the observations match every field of `expected` in case `ADAPT-12`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

#### Scenario: Retained measurements and unknown instrumentation

- GIVEN a completed trial with repeated samples, exact module sections, runtime recipes and cleanup observations
- WHEN a comparison record is produced
- THEN it retains the full trial artifacts and timing aggregation while unavailable physical GC and isolated engine measurements remain explicitly unknown

#### Scenario: Correctness precedes representation selection

- GIVEN candidate records with exact configurations and declared workload outcomes
- WHEN the selection policy evaluates passing, blocked, missing-pin, fabricated-metric and failed-correctness records
- THEN only correctly executed candidates with valid scoped evidence are eligible and no failed or unsupported record becomes a performance success

### Requirement: BE-LAYOUT-01
r[BE-LAYOUT-01]

**BE-LAYOUT-01.**

An optional post-M3 experiment MAY compare the selected managed-linear-memory Wasm lowering with a type-directed finite algebraic-data layout and A-normal/call-lowering alternative over the same accepted, resource-free typed programs. Each candidate MUST retain exact ordered stack/call semantics, checked source and instantiated interface, resolved schema/program identity, observable normalized recipes and captures, and effect/request ordering. It MUST compare runtime-supplied inputs after compilation under matching declared limits and pinned compiler/optimizer/engine/feature configurations, both optimization off and on. The record MUST distinguish normal results, traps and construction/invocation quota failures; record guest logical allocation charges and retained linear-memory bytes separately, expose cleanup, and mark unavailable engine/process memory observations unknown or unsupported rather than report zero. It MUST include finite two-arm variant, nested product/list, runtime quotation/composition and reflection shapes, plus hostile quota and malformed-boundary controls. Type-directed specialization MUST NOT smuggle a new source-preparation step, change portable program identity, omit a dynamically reachable recipe, or erase a checked effect/owner obligation.

Any later resource/component-positive candidate MUST independently preserve WIT canonical lowering/lifting, exact ABI ownership/post-return or trap cleanup, native-pin retirement and host authorization; resource-free measurements MUST NOT be advertised as this evidence. No unconditional observational equivalence, universal backend theorem or performance win follows from Bend's published examples or a faster finite trial. M3's existing completed WasmGC-versus-managed-memory results remain historical and unchanged; this study is not a reopened M3 gate.


<!-- cairn:scenario-links:start -->
#### Scenario: ADAPT-17 for BE-LAYOUT-01

- GIVEN the `Backend-Experiment` profile and every field of `input` in [ADAPT-17](../../../specs/conformance/adaptation-cases.json)
- WHEN the `runtime` procedure for case `ADAPT-17` runs against those inputs
- THEN the observations match every field of `expected` in case `ADAPT-17`

This is a scenario design, not an execution result. The case's `state` and `evidence` fields record its status.

<!-- cairn:scenario-links:end -->

#### Scenario: ADAPT-17 finite layout comparison

- GIVEN identical checked resource-free workloads, fixed post-compilation inputs, two pinned managed-memory lowerings and declared resource limits
- WHEN both configurations execute optimized and unoptimized runs with reflection, normal paths, traps and quota boundaries
- THEN each run records complete ordered value/recipe/effect and allocation/quota observations; discrepancies remain visible, with no resource-positive or universal-proof promotion
