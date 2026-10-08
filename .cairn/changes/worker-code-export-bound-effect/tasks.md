# WORKER-12 exact bound effect

## Source and native code

- [x] [serial] Add reserved kernel effect, strict Text/Text required operation registration, owner-bound validation, source syntax and independent compiler replay. r[H-HOST-02]
- [x] [serial] Emit typed slot-only `noble/test_code_export_bound` host import on invocation; leave the Wasm invocation poisoned on any host return and never pass Text operands. r[H-HOST-02]
- [x] [serial] Add scoped source/kernel/Wasm checks for exact interface and forged effect or adapter row (execution pending numeric slot). r[H-HOST-02]

## Validation and selected evidence

- [ ] [serial] Run scoped code tests and real compiled guest after numeric slot is released; no real execution or proof is inferred from source edits. r[H-HOST-02]
- [ ] [serial] Validate Cairn with the pinned wrapper when permitted; WORKER-12 promotion requires independent component/guest host evidence beyond this change. r[H-HOST-02]
