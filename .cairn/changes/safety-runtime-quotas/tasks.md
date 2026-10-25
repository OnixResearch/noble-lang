## Phase 1: Production execution

- [x] [serial] Reproduce the missing user-facing fuel/heap selection and independently establish that the compiled import-free Noble world has an actual 16-page memory and a separate generated heap quota. r[S-LANG-03]
- [x] [serial] Implement a real Wasmtime Core host call with separately selected fuel/heap limits and private allocator-only diagnostic; classify only matching execution traps, preserve zero imports/callbacks and clean up after failure. r[S-LANG-03]
- [x] [serial] Exercise compiled Noble compute and 56/57/65-byte text results with fuel 1/ample and generated quota 64/76, including positive I64(3) and byte-exact results. r[S-LANG-03]

## Phase 2: Source-bound acceptance

- [x] [serial] Pass Cairn proposal/design/tasks gates and sync a bounded S-LANG-03 delta, then regenerate authoritative views and validate without treating document checks as runtime execution. r[S-LANG-03]
- [x] [serial] Run a NEW immutable source-bound S-CASE-13 gate against the production CLI and pinned actual Noble compiler, retain raw commands/identity/failures, and promote only exact canonical state/evidence after passing both execution variants and positive controls. r[S-LANG-03]
- [x] [serial] Freeze the corrected source/views and coordinate a fresh chronological DX-06 and six-case release; all older receipts stay immutable historical source scopes and owner-law/Octet/Aeneas proof remain open. r[S-LANG-03]

- [ ] [serial] Archival blocker (recorded 2026-10-25): the frozen release-source replay trees `verification/adapt15-release-source/source.mjs`, `verification/wi03-release-source/source.mjs`, `verification/wi03-final-source/source.mjs` and `verification/scase07-current-source/source.mjs` enumerate `.cairn/changes/<slug>` as live inputs, so a dated move breaks their scope description. Archiving requires those trees to be regenerated against the archive layout, or an owner decision to freeze them as historical-only. All task boxes in this change are checked and its requirement IDs are already canonical. r[S-LANG-03]
