## Why

Noble issue #1 asks whether explicit live references can coexist with immutable captured Programs. Most of its semantics are already selected. The `explicit-live-references` change put the `Live-Slot-Design` contract into the canonical specifications: `K-LSLOT-01/02`, `H-LSLOT-01`, `S-LSLOT-01`, `VC-LSLOT-01`, `DX-LSLOT-01`, `EV-LSLOT-01` and `B-LSLOT-01`, with nine registered LSLOT-01..09 scenario designs that remain absent/not-run. That change stays active for future implementation and is not modified here.

Four gaps remain where #1 meets the ownership and lifetime contract of #3:

- The canonical text retires a "code version" when its last map, frame or saved-Program owner releases it. It does not define what that version's lifetime unit contains.
- Root exit is covered only for normal completion.
- Epoch, incarnation and generation counters are required to be monotonic, but nothing says what happens at exhaustion.
- A Program that escapes from a slot-selected target has no stated identity, liveness or retention rule.

The experimental live-slot runtime already shows these paths: abnormal-root release, counter exhaustion refusals, transitive saved-graph version counting and module-root sweeps. None of them is a canonical obligation. This change adds those obligations and answers each "Questions to settle" item by citing the existing contract and the new requirements. r[LREF-PKG-01] r[LREF-EXIT-01]

## What Changes

- Define each published or pinned slot version's retention unit as the complete executable package from #3 (`OWN-PKG-01`), including transitive exact named dependencies reached through module globals and table entries. Fix the retirement order: map unpin, version owners, package-member owners, then code-span clear. A failure after commit poisons the session rather than un-committing. r[LREF-PKG-01]
- Require every abnormal root exit (trap, quota, host abort, cancellation, transport failure) to release the root's map pin and frame pins exactly once on the host side, without resuming the guest. No saved owner may be issued for that root's outputs, and the root's refs must expire. r[LREF-EXIT-01]
- Require registry epoch, slot incarnation, slot generation and code-retention revision counters to be non-wrapping, with explicit refusal at exhaustion that leaves pins and saved Programs intact. r[LREF-CTR-01]
- Specify that a Program escaping a slot-selected target is an ordinary exact immutable Program. It keeps its own lineage identity and its package, never follows a later publication, and never carries a `LiveRef`. A saved generic caller still needs fresh host-issued refs and a fresh pin for each root. r[LREF-ESCAPE-01]
- Map the issue's five outcome scenarios to the registered LSLOT cases. Add six unexecuted LREF-CASE designs for the new obligations. r[LREF-EV-01]

## Impact

- **Files**: only this new active change: `proposal.md`, `design.md`, `tasks.md`, and ADDED deltas under `specs/language`, `specs/safety` and `specs/evidence`. The `explicit-live-references` and `live-wasm-reload` changes, canonical specs, `specs/conformance/live-reference-cases.json`, generated views, the ledger, the roadmap and status files are unchanged.
- **Coordination**: depends on `ownership-lifetime-contract` (#3) for the package unit, borrow shape and reclamation conditions. Retained-version budgets stay host-ledger quotas, distinct from guest-cell allocation (`allocation-strategy-contract`, #5). #14 keeps live-slot dispatch out of numeric graphs.
- **Testing**: document validation and the proposal/design/tasks gates only. No live-slot runtime path, LSLOT/LREF case, proof or acceptance is executed or claimed. The experimental CLI and its diagnostic gate are not canonical evidence.
