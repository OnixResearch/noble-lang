## Why

S-CASE-02 requires the exact `000102` three-byte Resources-Draft region to refuse offset 3, length 1 before any protected read. Core's `values.mjs` observes a separate, 16-page Wasm memory, in which that slice is valid. A generic array slice cannot establish the required guest-callable adapter or authority boundary. r[S-MEM-01] r[S-LANG-04]

## What Changes

- Add a distinct versioned synchronous WIT region world and production `noble component read-region COMPONENT HOST_BUFFER_HEX OFFSET LENGTH` host ingress. The host invoker, not the component, chooses immutable bounded bytes and registers the exact invocation-owned resource with read right; the component receives only an owned resource and signed offsets, borrows it for `region.read`, then consumes it through `regions.release`. r[S-MEM-01]
- Validate actual retained owner identity/context/kind/generation/right, signed arguments, checked end, region extent and output quota before any protected byte read. Return `bounds-reject` for canonical `(3,1)` with zero protected operations or region reads, and actual bytes `0102` for `(1,2)`; admit `(3,0)` as an empty in-bounds read. Keep Wasm linear memory and host-native addresses outside the selected region contract. r[S-LANG-04]
- Record separate source-bound evidence from real compiled guest and production host, including lifecycle, overflow, outside-boundary and invalid-owner controls. Preserve every prior S-CASE-16/DX06/six-case historical receipt and keep proof open. r[S-MEM-01] r[S-LANG-04]

## Impact

- **Files**: `.cairn/changes/safety-bounded-region-adapter/`, native safety/resource-adapter specifications, a narrow WIT world, production Rust component host ingress, the canonical S-CASE-02 state/evidence only after acceptance, generated views and a new verification gate/receipt.
- **Testing**: Compile and invoke the exact source under pinned WIT/Wasmtime toolchain; compare all canonical fields and independently measured protected-region accesses, positive data and release, negative bounds/overflow/handle mutations; freeze source and retained command outputs. Cairn validation does not imply guest execution or proof.
