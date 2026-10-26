## Why

The selected live-reference design pins one immutable registry map per root,
selects from that map at every live dispatch, and checks current authority
independently at each dispatch and effect. Saved exact Programs stay immutable.
The existing LSLOT-02 controls show nested and transitive roots hold across leaf
replacement and inspect exact selected versions, but no model names *why* a
root's selections cannot move, how a frozen-entry-map reference relates to the
runtime, or where coordinated-version coherence stops. This change adds the
bounded live-dispatch model and its proven properties (issue #47). It is a
verification follow-on, not a redesign of immutable Programs and not a claim
that live-slot machinery is absent.

## What Changes

- `proofs/foundation/NobleFoundation/LiveDispatch.lean` defines bounded roots,
  ordered entry maps, single-slot global-epoch publications, guest-visible
  events and separately identifiable control events. It proves that a single
  publication changes only its own slot's selection, that a root's selections
  come from its own pinned map, that execution through a root equals a
  frozen-entry-map reference for the same request sequence, that two separate
  publications are not one atomic replacement, that a denied dispatch produces
  no guest event, and that a finite abnormal prefix keeps its committed events.
  r[VC-LIVE-01]
- The retained kernel receipt `verification/foundation/receipts/d2aa7a382/`
  binds the model: 261 theorems, nine of them live-dispatch, clean source and
  axiom sets inside the allow-list. r[VC-LIVE-02]
- The interface-compatible two-version fixture, the broken per-dispatch-current-map
  control and the sequential-publication witness stay Phase 2 work: they bind a
  JavaScript runtime fixture to an existing supported observation, which this
  phase does not claim. r[VC-LIVE-03]

## Impact

- **Files**: `proofs/foundation/NobleFoundation/LiveDispatch.lean`,
  `proofs/foundation/NobleFoundation.lean`, `proofs/foundation/inventory.json`,
  `verification/foundation/receipts/d2aa7a382/`.
- **Not claimed**: no correspondence between this model and
  `crates/noble-cli/src/core/runtime/slot-registry.mjs`; no event-count claim
  under any authorization outcome; no atomic multi-slot replacement, state or
  resource migration, new contextual-effect inference, liveness or
  exactly-once effect claim; canonical LSLOT status is not promoted.
