## Why

Noble issue #1 asks whether an explicitly live reference can coexist with immutable captured Programs. Ordinary `[ n ]` permanently resolves its dependency, while the existing `Live-Wasm-Draft` reload design rebuilds dependents and forbids live slots in that profile. Neither contract supplies a checked, reusable caller whose target is deliberately selected at invocation time. This change selects a separate opt-in design without rewriting those contracts or claiming an implementation. r[K-LSLOT-01]

## What Changes

- Add `Live-Slot-Design`: an immutable, generic checked caller contains typed `slot.invoke`, borrowing a host-issued invocation-input `LiveRef<I,O,E>`; the ref and slot ID are not authority, captures, portable code or serialization. Dispatch declares `E ∪ {live.dispatch}`. Ordinary `[ n ]` remains captured v1 after any slot publication. r[K-LSLOT-01] r[K-LSLOT-02]
- Pin one immutable persistent registry map/global epoch per root and reuse it for nested calls with explicitly host-issued, typed A/B/C input refs passed through ordered callee interfaces. Admit exact interfaces and effect ceilings without executing candidates; publish with authorized epoch-and-slot-incarnation/generation CAS, retaining old versions under quota until their final map/frame/saved-Program owner releases. Check current authorization per dispatch and actual effect. r[K-LSLOT-02] r[H-LSLOT-01] r[S-LSLOT-01]
- Bind optional selected-target evidence to its actual identity, captures, context, claim and assumptions; preserve inert reflection and exact-version trace/replay, rejecting wrong frozen snapshot or host request/response/accounting even with admitted artifacts. Add nine unexecuted conformance designs and explicit unsupported boundaries for Core and guarded `Live-Wasm-Draft`. r[VC-LSLOT-01] r[EV-LSLOT-01] r[DX-LSLOT-01] r[B-LSLOT-01]

## Impact

- **Files**: this active Cairn change's language, safety, program-contracts, evidence, developer-experience and core-bootstrap deltas; the narrowly scoped existing `live-wasm-reload` exclusion; `specs/conformance/live-reference-cases.json`; supporting native README/roadmap/status only if needed to distinguish design from implementation.
- **Testing**: document gates/validation, document self-tests and scoped document tests, plus JSON/state inspection. These establish structural design validity only; no LSLOT case is executed or proven. Runtime implementation, source-bound evidence, sync/promotion, archive and issue closure require later independent review.
