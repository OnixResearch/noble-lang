# Live-dispatch host-lane recon (VC-LIVE-03)

Read-only reconnaissance for `.cairn/changes/root-pinned-live-dispatch` task
`r[VC-LIVE-03]`: whether an interface-compatible two-version fixture — one version
emitting a distinguished event in the caller, the later one emitting the same
event in the callee — can be bound to an **existing** host observation, as the
requirement demands, or whether it would need a new host API.

**Verdict: constructible with the existing host API.** No new host surface is
required. Nothing below is execution evidence: no fixture exists yet, the four
`VC-LIVE-03` tasks stay open, and this file claims no event count.

## The distinguished event is already observable

- The live-slot host accepts `test.emit` as an authority effect and switches the
  frontend to the test-host session when a granted slot's ceiling carries it:
  `crates/noble-cli/src/core/live_slot.rs:543-560`
  (`test_emit_selected`, `new_live_slots_with_test_hosts`).
- The accepted effect vocabulary is exactly `test.emit`, `test.abort`,
  `live.dispatch`: `crates/noble-cli/src/core/live_slot.rs:1076-1084`.
- An emitted effect is an **ordered trace row with its own text and response**,
  not only a counter: `crates/noble-cli/tests/live_slot_quota_replay.mjs:236-240`
  asserts `request_trace` rows with `operation` `dispatch`/`effect`, `request`
  `A`/`B` and `response` `emitted:A`/`emitted:B` under one root epoch, from a
  definition source that contains `"A" test.emit` (`:22-23`), installed at the
  type `LiveRef<I64,I64,test.emit>` (`:199`).
- Rows carry the selected program and epoch, so a relocated event is
  distinguishable per version: `programValueId`, `artifactSha256`, `generation`
  (`live_slot_quota_replay.mjs:137-161`), `rootEpoch`
  (`live_slot_nested_roots.mjs:150-168`).
- The worker report already carries `request_trace` through to the host:
  `crates/noble-cli/src/core/live.rs:1020`.

## What the fixture needs, using only declared operations

Two generations of a caller slot `A` and a callee slot `B`, each with ceiling
`test.emit` and an `effect` grant, and authority effects `['test.emit']` — the
manifest shape `crates/noble-cli/tests/live_slot_quota_replay.mjs:21-24` already
uses.

- **old pair**: caller `A1` source invokes `B` and emits the distinguished text;
  callee `B1` is inert (`'1 +'`-shaped). One `effect` row with the caller's
  `programValueId`.
- **new pair**: caller `A2` only invokes; callee `B2` emits the same text. One
  `effect` row with the **callee's** `programValueId`. That difference is the
  relocation witness.

Controls the requirement asks for, all expressible through install/publish/
invoke/bootstrap:

1. coherent all-old and all-new roots ⇒ **exactly one** `effect` row with the
   distinguished text, asserted as a count over `request_trace`.
2. **broken per-dispatch-current-map control**: publish `A2` while the pinned root
   still selects `B1` (or the reverse) ⇒ the same distinguished text appears
   **twice** (old caller emits and new callee emits) or **zero times** (new caller
   delegates to an inert old callee). The control must expose which of the two it
   is rather than assert "diverged".
3. **denied dispatch or effect before the event**: with the effect grant withheld
   for the emitting slot, the dispatch is refused *and* no `effect` row exists, so
   no event-count conclusion applies; the spec's own clause
   (`.cairn/changes/root-pinned-live-dispatch/specs/live-dispatch-reference/spec.md:29-40`)
   forbids counting that run.
4. **abnormal prefix**: a stopped execution keeps the committed `effect` rows and
   is presented as a prefix, never as rollback.

## Open points this recon does not settle

- The relocation above moves an event between caller and callee of the *same*
  slot pair; whether a single `request_trace` row's `programValueId` alone proves
  "callee rather than caller" for a nested `slot.invoke` chain is unresolved here
  and must be settled by the fixture, not by this note.
- Task 9's "existing supported observation **in** `live_slot_nested_roots.mjs`"
  is satisfiable by extension, but the fixture may equally live in a sibling mjs
  target if that keeps the LSLOT-02 lane's own scope intact; the Cairn task text
  names the nested-roots file, so an extension there is the default.
- Nothing here binds `NobleFoundation/LiveDispatch.lean` to these rows; the model
  proof, the implementation comparison and the host correspondence stay separate
  receipts.
