## Why

P-ID-03 and unexecuted ID-05 require `quote [ + ] compose` with runtime capture `I64(2)` versus `I64(3)` to share a definition template while yielding different `ProgramValueId`s. P-RECIPE-04 preserves the captured literal in the exact recipe; none of these rules specifies how an independently admitted returned program acquires its *own* authenticated `DefinitionId`. The source/guest builder's definition is not the returned target. Symbolic P2/D2 in LSLOT-05 and Ptrace/Dtrace in LSLOT-08 cannot justify positive evidence or replay without an actual selected target identity. r[P-ID-03] `VC-LSLOT-01` `EV-LSLOT-01`

## What Changes

- Propose a narrowly modified P-ID-03: derive an anonymous closed target's template identity only at an identity-bearing admission boundary, from independently checked `quote`/`compose` occurrences, an authenticated runtime-capture-slot witness, its normalized resolved body, exact interface/dependencies and scoped owner. Keep the literal-bearing recipe intact and bind each exact capture to a distinct program-value identity; never substitute the builder definition, a guessed `D` string or artifact bytes. `P-ID-01` r[P-ID-03] `P-RECIPE-03` `P-RECIPE-04`
- Require bounded VM reification and independent admission before selecting an anonymous target; leave P-ID-04's stable encoding open and P-ID-06/07, VC-LSLOT-01 and EV-LSLOT-01 proof, installed-Wasm correspondence, replay and host-authority checks independent. No Core/guarded-live slot enablement or positive LSLOT-05/08 outcome is claimed. r[P-ID-03] `P-ID-04` `P-ID-06` `P-ID-07` `VC-LSLOT-01` `EV-LSLOT-01`

## Impact

- **Files**: this new active change's proposal, design, tasks, metadata and language P-ID-03 delta only. Canonical native specs, ID-05, all nine LSLOT cases, generated views/ledger/receipt, status and prior archives remain unchanged pending independent normative review.
- **Testing**: pinned Cairn proposal/design/tasks gates and native validate, document self-tests and scoped document tests; inspect the unchanged canonical case/status/evidence records. These are draft-structure checks, not executable identity, replay, proof or source-bound acceptance.
