# Opt-in Text byte cursor tasks

## Native contract

- [x] [serial] Specify the disjoint pure guest profile and byte/status ABI as an active unpromoted native Cairn delta. Do not sync into selected specs or promote old cases. r[DX-TEXT-01]

## Implementation and observation

- [x] [serial] Wire the opt-in kernel/frontend/backend/CLI contract, preserve unflagged and declared/LIVE profiles, and reject forged reserved operations before emission. r[DX-TEXT-01]
- [x] [serial] Run a real compiled Wasm child with opt off/on over Aé, empty, EOF and invalid offsets; check absent host requests, unchanged Text and negative isolation. Retain raw scoped observation without promoting MA1/CALC or source-bound release evidence. r[DX-TEXT-01]

## Closeout

- [x] [serial] Run direct pinned Cairn proposal/design/tasks gates and validation; leave this change active until deliberate review/sync. r[DX-TEXT-01]

- [x] [serial] Resolve the combined-profile conflict before archiving: owner decided on 2026-10-25 to document the shipped capability canonically, so the combined `Declared-Text-v1` profile is now normative in `DX-TEXT-01` (`.cairn/specs/developer-experience/spec.md`), case `TEXT-02` in `specs/conformance/text-byte-cursor-cases.json` is `implemented/passed/open/explicit` and registered in `specs/spec-family.json`, and `verification/text-byte-cursor/observation.json` retains 25 children at binary sha256 `3d0c08be31d768ef4d837c6ecf83e16fd4ffab075d9dfcf24d8a3cb186961b10`, including the combined-profile child. The earlier conflict is retained as a historical finding in `verification/text-byte-cursor/reproduce-failure.json`. r[DX-TEXT-01]
