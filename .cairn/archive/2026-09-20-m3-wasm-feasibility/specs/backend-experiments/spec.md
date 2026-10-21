# M3 experimental execution configuration

## MODIFIED Requirements

### Requirement: BE-CONFIG-01
r[BE-CONFIG-01]

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

### Requirement: BE-REPORT-01
r[BE-REPORT-01]

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
