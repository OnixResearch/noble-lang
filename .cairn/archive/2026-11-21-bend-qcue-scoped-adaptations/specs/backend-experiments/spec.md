# Optional managed-memory layout study

## ADDED Requirements

### Requirement: BE-LAYOUT-01
r[BE-LAYOUT-01]

An optional post-M3 experiment MAY compare the selected managed-linear-memory Wasm lowering with a type-directed finite algebraic-data layout and A-normal/call-lowering alternative over the same accepted, resource-free typed programs. Each candidate MUST retain exact ordered stack/call semantics, checked source and instantiated interface, resolved schema/program identity, observable normalized recipes and captures, and effect/request ordering. It MUST compare runtime-supplied inputs after compilation under matching declared limits and pinned compiler/optimizer/engine/feature configurations, both optimization off and on. The record MUST distinguish normal results, traps and construction/invocation quota failures; record guest logical allocation charges and retained linear-memory bytes separately, expose cleanup, and mark unavailable engine/process memory observations unknown or unsupported rather than report zero. It MUST include finite two-arm variant, nested product/list, runtime quotation/composition and reflection shapes, plus hostile quota and malformed-boundary controls. Type-directed specialization MUST NOT smuggle a new source-preparation step, change portable program identity, omit a dynamically reachable recipe, or erase a checked effect/owner obligation.

Any later resource/component-positive candidate MUST independently preserve WIT canonical lowering/lifting, exact ABI ownership/post-return or trap cleanup, native-pin retirement and host authorization; resource-free measurements MUST NOT be advertised as this evidence. No unconditional observational equivalence, universal backend theorem or performance win follows from Bend's published examples or a faster finite trial. M3's existing completed WasmGC-versus-managed-memory results remain historical and unchanged; this study is not a reopened M3 gate.

#### Scenario: ADAPT-17 finite layout comparison

- GIVEN identical checked resource-free workloads, fixed post-compilation inputs, two pinned managed-memory lowerings and declared resource limits
- WHEN both configurations execute optimized and unoptimized runs with reflection, normal paths, traps and quota boundaries
- THEN each run records complete ordered value/recipe/effect and allocation/quota observations; discrepancies remain visible, with no resource-positive or universal-proof promotion
