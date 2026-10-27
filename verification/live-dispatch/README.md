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

## First attempt at the fixture, and where it stopped

The fixture was actually written into the LSLOT-02 lane and run against a real
CLI (`/var/tmp/noble-target/debug/noble`, pinned Node, `env -i`). It is **not**
kept: the attempt stopped short and the lane is unchanged, so nothing above is
promoted to execution evidence. What the attempt measured, in order:

1. A caller that both invokes and emits needs the dispatch effect declared:
   installing it under `effects: ['test.emit']` alone refused with
   `unavailable selected host effect`.
2. With the caller declaring `LiveRef<I64,I64,live.dispatch+test.emit>`, bootstrap
   accepted the pair, but the invocation refused
   `root reference binding disagrees with checked caller or slot`: the borrowed
   reference's declared `(input, output, ceiling)` must equal the bound slot's own
   contract exactly
   (`crates/noble-cli/src/core/live_slot.rs:3348-3367`).
3. With the reference narrow enough to match the slot, the dispatch row appears in
   `request_trace` but the guest completes with **status 5**
   (`crates/noble-cli/src/core/runtime/slot-engine.mjs:2200-2205`), and with the
   emit placed before the dispatch the trace is empty and the same status returns.
   `5` is the pre-select failure code
   (`crates/noble-wasm/src/source/emit/live.rs:28-30`), so this is a guest-side
   refusal at the caller's own effect site, not a host or policy refusal.
4. Two binding positions were tried: `position` is the **formal index** (`1` for
   the second formal — bindings that disagree are refused at step 2), so the
   `position: ordinal + 1` shape used by the LSLOT-02 fixtures applies to their
   deeper stacks, not to a two-formal caller.

What is not settled: whether the `test.emit` word needs an explicit host binding
(`require emit Text -- ! test.emit`) inside the live-slot source fragment, which
the quota-replay fixture obtains through a definition source rather than a bare
program fragment. That is the next thing to check, and it is the reason this task
stays open rather than reported done.

### Addendum: isolation probes after the attempt

Two throwaway probes (not committed, run against the same real CLI) narrowed the
status-5 result without settling it:

- A bare live-slot fragment that emits — `'"relocated" test.emit drop 21'` —
  **installs** cleanly, so the `test.emit` word is accepted at ingress. That rules
  out "the fragment needs a `require` header to parse the word".
- Bootstrap requires **two or three** checked slots (`bootstrap requires two or
  three checked slots`), so a single-slot emit probe is not constructible; with a
  second inert slot the same emitting candidate is refused at bootstrap with
  `checked bootstrap candidate differs from selected slot interface or effect
  ceiling` — for ceilings `['test.emit']` and `['live.dispatch', 'test.emit']`
  alike. The publish path the quota-replay fixture uses (`publish` at epoch 0)
  does not share that bootstrap comparison, which is why its emitting program
  reaches execution there.

So the unresolved question is now narrower: whether a **bootstrapped** emitting
candidate can match a slot ceiling at all, or whether the relocation witness has to
be built on the publish path. Both remaining options are inside declared
operations; neither is claimed done.

### Addendum 2: the status-5 blocker is the emit site, not the borrowed reference

A publish-path probe with **no borrowed references at all** — two slots, the
emitting program `drop "relocated" test.emit drop 21` published into its slot —
publishes and then fails guest execution with the same **status 5** and an empty
`request_trace`. So the blocker recorded above is **not** about binding a borrowed
reference: it is the `test.emit` word's own execution in a live-slot program
fragment.

Two further facts from the same probes:

- the fragment shape matters at publication: `"relocated" test.emit drop 21` is
  refused for a ceiling mismatch while `drop "relocated" test.emit drop 21` is
  published, so the checked interface of a fragment does not simply follow the
  stack one writes;
- the definition route (`def traced [ … ]` plus `install … selected_name`) is the
  shape the quota-replay fixture uses, and installing `run` behind it refuses
  `stack/program join: expected ?stack Program<?stack -- ?stack>; actual [] I64`,
  i.e. that route needs a `Program` value on the stack, which a bare
  `inputs: ['I64']` install does not provide.

Consequence for VC-LIVE-03: the relocation witness needs the definition/`Program`
route rather than a bare emitting fragment, and until that is built the four tasks
stay open. This is a narrower, more useful statement than "status 5 unexplained".
