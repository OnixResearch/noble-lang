# Coherent entry-map receipts (VC-LIVE-03)

`receipt.mjs` runs `crates/noble-cli/tests/live_slot_coherent_maps.mjs` against a selected CLI and
writes a receipt whose sections stay **distinct** on purpose:

- `model` — the named kernel-checked `NobleFoundation.LiveDispatch` theorems, including the
  event-relocation run and the broken current-map control, and the retained foundation receipt
  `verification/foundation/receipts/1d4113e6c/`. Does not establish that the selected runtime
  behaves this way.
- `implementation_comparison` — the measured lane, its runner and the observed counts. Does not
  establish the model theorems.
- `host_correspondence` — the selected CLI bytes and the session identifiers. Does not establish
  anything about a differently built CLI.
- `inherited_coverage` — revocation, abnormal prefix, stale-CAS/ABA, nested-root and retention
  lanes bound by digest only; those lanes keep their own claims and were not re-run here.

Run it with the pinned Node and a fresh receipt directory:

```sh
env -i PATH=/run/current-system/sw/bin:<node>/bin <node> verification/live-slot-coherence/receipt.mjs \
  /tmp/<target>/debug/noble /tmp/<fresh-receipt-directory>
```

The lane measures retention directly. A root is held at its first `live_select`, and a
compare-and-set publication of a new emitting version of slot `E` lands and is acknowledged
during the hold. The held root still dispatches once and emits once from the version it was
pinned on. The next root does the same from the new version. A publication without
`expected_incarnation`/`expected_generation` stale-rejects with no root pinned, so that
refusal is recorded as a control, not as retention evidence. An earlier version of the lane
used it as retention evidence.

Mutation check, not retained here: a CLI built with `dispatch` selecting from the current
registry map instead of the root's pinned map (`slot-registry.mjs`) fails this lane at the
held-root assertion. The previous lane passed against the same mutant.

`receipt.json` and `lane.stdout` in this directory are one such run. No `LSLOT-*` conformance case
is promoted here, and the deliberately broken per-dispatch-current-map duplicate control remains
not constructible from the fixture authority shape.
