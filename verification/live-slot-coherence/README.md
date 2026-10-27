# Coherent entry-map receipts (VC-LIVE-03)

`receipt.mjs` runs `crates/noble-cli/tests/live_slot_coherent_maps.mjs` against a selected CLI and
writes a receipt whose sections stay **distinct** on purpose:

- `model` — the named kernel-checked `NobleFoundation.LiveDispatch` theorems and the retained
  foundation receipt `verification/foundation/receipts/d2aa7a382/`. Does not establish that the
  selected runtime behaves this way.
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

`receipt.json` and `lane.stdout` in this directory are one such run. No `LSLOT-*` conformance case
is promoted here, and the deliberately broken per-dispatch-current-map duplicate control remains
not constructible from the fixture authority shape.
