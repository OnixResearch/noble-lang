## Context

Node 24.13.0/V8 13.6.233.17-node.37 is the selected Core Wasm runtime. Its persistent memory/table/globals and immutable Program captures are real; its current external WAT assembly/validation per submission is not immediate reload. Instantiating on live shared imports before commitment can mutate shared state. K-DEF-01/02 freeze existing definition identities/dependencies and prohibit editing already prepared Program meaning. r[BE-LIVE-01] r[K-LIVE-01]

## Decisions

### Decision: Wasm bytecode, not a second Noble instruction set

**Choice:** A new opt-in checked source-to-in-process deterministic Wasm binary encoder and two-layer admission (independent Noble kernel plus exact VM byte validation/import/ABI correspondence) target the existing persistent Node/V8 engine. Existing Core WAT/external tools continue unchanged. Wasm bytecode does not imply interpretation: `--engine interpreter` is refused until an actual pinned no-JIT Wasm interpreter passes exact compatibility tests. r[B-LIVE-01] r[BE-LIVE-01] r[S-LIVE-01]

### Decision: Rebuild dependencies and swap snapshots, never mutate a Program

**Choice:** Reload checks a complete stable, host-selected file/import snapshot, re-resolves and rebuilds all transitively affected definitions with new immutable identities, validates their ordered stack, effect and capability ABI, prepares Wasm in isolated nonauthoritative state, and publishes one namespace generation only at a top-level safepoint. The next new top-level lookup sees new bodies; existing frames, old Programs/captures and their nested invocations see old resolved dependencies. No typed live slot inside old code, no re-execution or implicit state migration. r[K-LIVE-01] r[H-LIVE-01] r[H-LIVE-02] r[H-LIVE-03]

**Shared live ABI:** Shadow-stage against separate state, then instantiate the same accepted no-start/no-active-segment bytes against the selected long-lived shared live arena at a top-level safepoint. Pre-reserve disjoint bounded table/cell/data slots, install fresh functions through checked host-controlled writes, restore new slots on any prepublication failure, and publish only after complete no-effect install and final file-hash recheck. Old Programs, including aggregate captures, retain their original table slots in this same arena. Host callbacks are inert during staging and authorization-gated after publication; active frames finish before the safepoint. r[H-LIVE-02] r[H-LIVE-03]

### Decision: Resource-free first profile with explicit quotas and refusal

**Choice:** Preserve checked stack when ABI-identical; refuse resource-bearing stack or resource-positive import instead of inventing ownership migration. Cap source/read retries, check work, binary bytes, execution fuel/steps, recursion, memory/table/cells and retained old instances; retire unreferenced generations under accounting. Invalid snapshot, incompatible code or exhausted budget leaves old namespace/stack/authorizations untouched and runs zero candidate bodies. Host effects of already running code are real and remain subject to independent policy. r[S-LIVE-01] r[H-LIVE-03]

## Risks / Trade-offs

- A new in-process binary emitter/validation path is required; no current implementation or source-to-Wasm proof exists for it. Isolated staging must not reuse the current `install` against shared imports.
- Active calls delay publication; a finite wait/timeout refusal is honest, whereas zero-latency promise or stale next-call result is not. Saved Programs pin old code and may exhaust bounded retention.
- Strictly interpreted execution remains an optional separately gated engine, not a description of V8 flags or Wasmtime. Native C AOT, if specified separately, is not this Wasm live session. r[BE-LIVE-01] r[V-LIVE-01]
