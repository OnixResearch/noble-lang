# Resident Wasm binary engine

## ADDED Requirements

### Requirement: BE-LIVE-01
r[BE-LIVE-01]

The opt-in emitter MUST validate deterministic pinned Wasm binary version, opcodes, control/stack typing, imports, exact source/build correspondence and selected Node/V8 ABI in process without wasm-tools/Binaryen per reload. Node/V8 may JIT; a separately pinned proven no-JIT real Wasm interpreter MUST fail closed until independently compatible, never fall back to V8 or a custom Noble bytecode evaluator.
