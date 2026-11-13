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

`receipt.json` and `lane.stdout` in this directory are one such run. No `LSLOT-*` conformance case
is promoted here.

## Retained current-map mutant (VC-LIVE-03 control)

[`mutant.mjs`](mutant.mjs) builds two CLIs from one `git archive` of a clean commit: the
unpatched source, and a copy whose `dispatch` selects from the registry's **current** map
(`this.#current.map`) instead of the root's pinned map (`root.state.map`) in
`crates/noble-cli/src/core/runtime/slot-registry.mjs`. It runs this lane and the two-selection lane
`crates/noble-cli/tests/live_slot_two_selections.mjs` against both, refuses a dirty tree, and
records the patch, both binary digests, the build recipe, the lane digests and every outcome:

```sh
env -i HOME="$HOME" PATH=/run/current-system/sw/bin <node> verification/live-slot-coherence/mutant.mjs \
  --work <new directory> --cargo <pinned cargo wrapper> --jobs 6
```

[`mutant-ebb10b47.json`](mutant-ebb10b47.json) is that run from clean commit `ebb10b47`. In the
two-selection lane one root is pinned at the dispatcher selection and held at its first leaf
selection; a publication of slot `E` lands during the hold; the root then selects the leaf twice.

| Direction | Pinned version's events | Unpatched CLI, held root | Current-map CLI, held root |
|---|---|---|---|
| `emitting-old` (old leaf emits, republished leaf is inert) | 2 | 2 | **0**: both events omitted |
| `inert-old` (old leaf is inert, republished leaf emits) | 0 | 0 | **2**: two events the pinned version never performs |

In both directions the current-map CLI ran the *republished* Program for both selections of a root
pinned on the old map (its held Program equals the next root's). The unpatched CLI passes both lanes;
the current-map CLI fails the coherence lane at the pinned-version assertion and the two-selection
lane at the pinned-event-count assertion. This is the implementation-side counterpart of the model's
`current_map_selection_duplicates_and_omits`: it shows the root pin is what removes the omitted and
extra events. It is a finite control on one build, not a correspondence proof between
`NobleFoundation/LiveDispatch.lean` and `slot-registry.mjs`.

A caller version that both emits and dispatches is expressible after all (an earlier recon said
callers never emit; that holds only for a root frame).
`crates/noble-cli/tests/live_slot_caller_emits.mjs` moves the distinguished event from caller
version `A` into callee version `E`, with identical stack results and reflected sites, and runs
in the `live_slot` test target.
