# Opt-in live Wasm source reload

## Why

The existing persistent Core session executes Wasm on selected Node/V8 but launches external wasm-tools (and optionally Binaryen) per submission, retains immutable namespace snapshots, and has no safe transactional watch/reload. A Scheme-like edit/reload/next-call workflow needs fresh new top-level bindings without changing checked old definitions or saved Programs. r[K-LIVE-01] r[BE-LIVE-01]

## What Changes

- Specify separate `noble live repl` and `noble live watch` entry points over explicit bounded source/module snapshots, with a new in-process Wasm binary emitter/validator after independent kernel acceptance. No custom Noble bytecode interpreter or changed `noble run/session` default. r[DX-LIVE-01] r[B-LIVE-01]
- Stage candidate modules in isolation without linking live memory/table/globals or granting host authority, check the complete transitive dependent definition graph and ABI, then publish a new immutable generation at a top-level safepoint. Old frames and Programs keep exact old dependencies. r[K-LIVE-01] r[H-LIVE-02]
- Add a separately host-granted guest self-edit lane: checked live-only words build a bounded pure `Program` candidate during real Wasm execution, enqueue it under the originating immutable definition identity and expected source generation, and let the host independently reify/check/stage and CAS-publish only after return. Guest code receives no direct file, compiler or permission authority; old Programs and selected file bytes remain unchanged. r[K-LIVE-02] r[H-LIVE-04]
- Record bounded preparation, VM execution and old-code retention, exact negative/positive designs and evidence status; selected V8 can JIT, and a separate genuine pinned interpreter requires its own acceptance. r[S-LIVE-01] r[BE-LIVE-01] r[EV-LIVE-01]

## Impact

- **Files**: canonical language/bootstrap/backend/developer/safety/verification/evidence specifications, a separately unexecuted live conformance design, roadmap/status and generated compatibility views.
- **Evidence**: none yet; all LIVE-01 through LIVE-10 cases are absent/not-run and all proof obligations remain open. Historical Core/M4/MC1/MC2/DXM1 results remain unchanged. r[V-LIVE-01] r[EV-LIVE-01]
