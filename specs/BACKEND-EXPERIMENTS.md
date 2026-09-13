<!-- Generated compatibility view. Edit .cairn/specs/backend-experiments/spec.md instead. -->
# Backend representation experiments

Document: SPEC-BE001  
Revision: 0.1.0-draft.5  
Depends on: SPEC-0001, SPEC-B001 and SPEC-S001 at 0.1.0-draft.5  
Status: Required M3 investigation, not a selected ABI or completed backend

## Scope and selection

The [external study](../review/EXTERNAL-DESIGN-STUDY.md) motivates this experiment. Zena supplies design examples, not a required dependency or Noble compatibility evidence.

**BE-COMPARE-01.** M3 MUST compare `wasm-gc` with `managed-linear-memory` for the same declared bootstrap workload. Each candidate needs an executed result or an evidenced blocker. The selection record MUST NOT present an unsupported candidate as a measured success.

M3 does not require two production backends. A documented blocker can end one candidate's trial. At least one candidate must satisfy the existing M3 execution gates before M3 closes.

WasmGC is not selected by this document. Neither a small binary nor an upstream benchmark establishes suitability for Noble.

**BE-CONFIG-01.** Each run MUST record immutable tool revisions, target, workload, optimization mode, resource limits, and enabled Wasm features. The record names GC, function-reference, multi-value, tail-call, and component support separately. Unsupported instrumentation MUST remain explicit rather than produce a zero measurement.

## Program representation

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

The component probe is nonblocking for resource-free M3. Its incomplete results remain M5 work. M5 requires independent-peer checks and cannot inherit conformance from this probe.

## Comparison record

**BE-REPORT-01.** The comparison MUST report emitted bytes by section, preparation time, execution time, allocation counts, and peak memory where observable. It also records depth limits, correctness outcomes, recipe observations, and trust boundaries. Engine memory and guest logical allocation measurements MUST remain distinguishable.

Repeated timing samples need their sample count and aggregation method. Missing measurements remain unknown. No performance claim can omit a failed correctness case for the same claimed workload.

Existing CORE-03, CORE-05, and CORE-09 cover dynamic inputs and reflection. [Adaptation scenarios](conformance/adaptation-cases.json) add depth, arity, reachability, boundary, and selection-policy designs. All remain unexecuted.
