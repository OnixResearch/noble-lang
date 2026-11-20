## Why

S-CASE-13 fixes two independent Wasm-Draft execution limits: one unit of guest engine fuel and 64 bytes of generated guest heap. Core's current Node route exposes managed runtime step and allocation budgets but not Wasmtime instruction fuel; the component host fixes its fuel and does not expose the generated Core heap setter through WIT. A 64-byte Store memory limit would prevent its fixed 16-page memory from instantiating, not demonstrate the requested execution failure. r[S-LANG-03]

## What Changes

- Compile one versioned, import-free WIT world from checked Noble sources and invoke its unchanged emitted Core Wasm through a production Wasmtime route. Instantiate at normal 16-page memory before independently setting the selected guest-call fuel and generated-heap allocation limit. r[S-LANG-03]
- Expose a private generated allocator diagnostic set only by the quota guard. Classify `Trap::OutOfFuel` with exhausted fuel and the diagnosed guest allocator trap separately; an unrelated `unreachable` or failed instantiation is not `specified-quota-failure`. Clean up the generated heap after abnormal execution and preserve zero host callbacks. r[S-LANG-03]
- Admit the exact canonical 1/64 failure variants only after source-bound execution, positive I64(3), byte-exact heap boundary controls and retained raw observations. Proof and general engine/backend correspondence remain open. r[S-LANG-03]

## Impact

- **Files**: versioned quota WIT and Noble source fixtures, generated-Core allocator diagnostic, production quota-core CLI route, native safety S-LANG-03 delta, new S-CASE-13 gate/receipt and canonical state/evidence only after acceptance.
- **Testing**: Compiled guest/actual Wasmtime traps, exact 56/57/65/76 generated-heap boundaries, post-trap cleanup, no imported callbacks, native Cairn gates/validation, then a separately source-bound later-source replay.
