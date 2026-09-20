## Why

M3 must establish an executed Wasm representation choice before M4 can deliver an end-to-end core. Existing checker and MC1 proof results do not establish Wasm execution, dynamic program construction, recipe preservation, or runtime bounds.

## What Changes

- Compare actual WasmGC and managed-linear-memory modules on one checked, resource-free workload (BE-COMPARE-01).
- Compile primitive operations and continuations in pure Rust; construct, compose, retain, reflect, and invoke programs after module compilation without a compiler service or recipe interpreter (BE-DYNAMIC-01, BE-ARITY-01, BE-REFLECT-01).
- Implement explicit allocation, recipe, composition-depth and execution-stack limits, with construction/invocation outcomes and accounted region cleanup (BE-LIMIT-01).
- Pin the experiment host and toolchain, preserve failed cases and unknown instrumentation, and select a representation only after the complete correctness matrix passes (BE-REPORT-01). Select the experimental configuration under r[BE-CONFIG-01].

## Impact

- **Files**: `crates/noble-wasm`, existing CLI/workspace/Nix/policy integration, `tools` and `verification/m3-wasm`, canonical backend specification and generated views, conformance records, roadmap and evidence.
- **Testing**: actual engine execution of CORE-03/05/09 and ADAPT-08/09/10/12; optimization on/off; independent wrapping/recipe oracles; exact and exceeded quotas; cleanup/reuse; selection-policy rejection controls; Rust extraction, affected regressions, source inventory, quality and Cairn gates. Resource/component interoperability remains M5; universal compiler correctness and M4 core delivery are not claimed.
