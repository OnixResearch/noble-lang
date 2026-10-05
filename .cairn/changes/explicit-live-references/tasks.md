## Phase 1: Design selection (no runtime acceptance)

- [x] [serial] Select the separate typed generic dispatch, ephemeral input/authority boundary, root snapshot, admission/CAS, evidence and reflection contracts as active native requirement deltas. r[K-LSLOT-01] r[K-LSLOT-02] r[H-LSLOT-01] r[S-LSLOT-01] r[VC-LSLOT-01] r[DX-LSLOT-01]
- [x] [serial] Preserve ordinary captures and the existing `Live-Wasm-Draft` no-slot exclusion; design nine explicitly unexecuted LSLOT scenarios and state/status boundaries. r[B-LSLOT-01] r[EV-LSLOT-01]
- [x] [serial] Run proposal/design/tasks gates, native validation and document tests without generated-view sync, evidence promotion or report/ledger refresh. r[EV-LSLOT-01]

## Phase 2: Future implementation and independent acceptance (open)

- [ ] [serial] Implement independently checked `Live-Slot-Design` admission, root-issued typed ref inputs/pass-through, root pinning and nested dispatch with current host authorization. r[K-LSLOT-02] r[S-LSLOT-01] r[B-LSLOT-01]
- [ ] [serial] Implement exact immutable target admission and bounded global-epoch/slot-incarnation/generation CAS publication, rollback-as-publication and last-pin retirement/reclamation across maps, frames and saved Programs. r[H-LSLOT-01] r[S-LSLOT-01]
- [ ] [serial] Execute every LSLOT case and adversarial variants including cross-slot pinning, retry after last owner release and hostile frozen-replay bindings against actual runtime; record separately bound trace, evidence, proof and trust results without inheriting LIVE acceptance. r[EV-LSLOT-01] r[VC-LSLOT-01] r[DX-LSLOT-01]
