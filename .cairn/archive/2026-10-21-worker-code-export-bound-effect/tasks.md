# WORKER-12 exact bound effect

## Source and native code

- [x] [serial] Add reserved kernel effect, strict Text/Text required operation registration, owner-bound validation, source syntax and independent compiler replay. r[H-HOST-02]
- [x] [serial] Emit typed slot-only `noble/test_code_export_bound` host import on invocation; leave the Wasm invocation poisoned on any host return and never pass Text operands. r[H-HOST-02]
- [x] [serial] Add scoped source/kernel/Wasm checks for exact interface and forged effect or adapter row (execution pending numeric slot). r[H-HOST-02]

## Validation and selected evidence

- [x] [serial] Run scoped code tests and real compiled guest after numeric slot is released; no real execution or proof is inferred from source edits. Evidence: `verification/worker12-adapter/scoped-tests.json` (8 of 8 scoped tests at `3e5f4a51`, including real Wasmtime runs of the compiled `ledger@1.capture` fixture against a success-reporting and a throwing host import, and the `noble worker program test-host` child guest). r[H-HOST-02]
- [x] [serial] Validate Cairn with the pinned wrapper when permitted; WORKER-12 promotion requires independent component/guest host evidence beyond this change. Evidence: `bun tools/cairn.mjs validate --root .` (Cairn `15f00875`) reported valid with 0 issues and 0 findings; WORKER-12 stays `absent`/`not-run`/`open`/`unassessed`. r[H-HOST-02]
