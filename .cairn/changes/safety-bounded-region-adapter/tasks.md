## Phase 1: Contract and production adapter

- [x] [serial] Select a separate versioned WIT region resource and user-facing component host ingress; verify the actual compiler's owner/borrow/result source before accepting the interface. r[S-MEM-01] r[S-LANG-04]
- [x] [serial] Bind independently host-selected immutable region bytes to a live invocation-owned typed resource and explicit right; validate identity, context, kind, generation, rights, signed checked bounds and output quota before any protected byte read. Return and release the owner exactly once, including denial and abnormal cleanup. r[S-MEM-01] r[S-LANG-04]
- [x] [serial] Pass canonical `(000102,3,1)` through compiled Noble guest and production host, positive `(1,2)` and `(3,0)`, out-of-range/negative/overflow and forged/cross-context/wrong-kind/retired probes with observed zero-read invariants. r[S-MEM-01] r[S-LANG-04]

## Phase 2: Evidence and release

- [x] [serial] Pass native Cairn proposal/design/tasks gates, preview and sync bounded safety/resource-adapter deltas, regenerate views and validate specifications without promoting document validation as runtime evidence. r[S-MEM-01] r[S-LANG-04]
- [x] [serial] Run a new immutable source-bound gate for S-CASE-02 with retained raw commands and proof-open assumptions. Only then promote its canonical state/evidence and refresh views/ledger; preserve historical receipts and coordinate a fresh later source freeze/replay. r[S-MEM-01] r[S-LANG-04]
- [x] [serial] Correct nonreused host table/context identities, run a NEW source-bound cross-host owner gate and append evidence[1] only after actual passing compiled guest/host replay. Preserve evidence[0], its original runner and receipt byte-identically; renew final source/replay after canonical views settle. r[S-MEM-01] r[S-LANG-04]

- [ ] [serial] Archival blocker (recorded 2026-10-25): the frozen release-source replay trees `verification/adapt15-release-source/source.mjs`, `verification/wi03-release-source/source.mjs`, `verification/wi03-final-source/source.mjs` and `verification/scase07-current-source/source.mjs` enumerate `.cairn/changes/<slug>` as live inputs, so a dated move breaks their scope description. Archiving requires those trees to be regenerated against the archive layout, or an owner decision to freeze them as historical-only. All task boxes in this change are checked and its requirement IDs are already canonical. r[S-LANG-04]
