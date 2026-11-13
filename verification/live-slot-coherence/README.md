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
version `A` to callee version `E`, with identical stack results and reflected sites, and runs
in the `live_slot` test target.

## Issue #47 follow-up (2026-11-13)

`crates/noble-cli/tests/live_slot_caller_emits.mjs` is the exact caller/callee relocation fixture:
the same checked caller slot `A` changes from emitting before its call to delegating, while the
callee slot `E` changes from inert to emitting. Its asserted observation is the effect row's
`slotId` and `programValueId`, with the same reflected root site and stack result.

`crates/noble-cli/tests/live_slot_relocation_controls.mjs` measures the remaining controls:

- A root pinned to a coherent all-old pair emits once, and a fresh root after both publications
  emits once from the coherent all-new pair.
- The test-only current-map build is held after selecting old caller `A`, then receives compatible
  replacements of `E` and `A` during the hold. In the caller-to-callee direction, the old caller
  and new callee each emit (two rows); in the reverse direction, the old emitting callee is
  replaced before selection by an inert callee (zero rows). The ordinary build retains its old
  map and emits exactly once in both cases.
- Without the test seam, a root is actually invoked after only the callee publication and before
  the caller publication. Its trace shows old caller `A` plus new callee `E` and two effect rows;
  after the next publication, the all-new pair emits once. The intermediate root is a real mixed
  compatible map, not the earlier empty-slot failed invocation.

The counterfactual is isolated behind Cargo feature `live-test-current-map`, which is not a default
feature. Only that feature-built worker substitutes current-map selection into its embedded host;
the default `live_slot` test scans the selected default `noble` binary for the unique seam marker
and requires it absent. A feature-enabled integration test runs the duplicate/omission lane against
the deliberately broken binary. Do not enable this feature in shipped builds. Neither these finite
observations nor the receipt prove correspondence with `LiveDispatch.lean`.

The source/model/artifact-bound dated receipt is
[`receipts/2026-11-13/receipt.json`](receipts/2026-11-13/receipt.json). It binds the clean source
commit, both CLI digests, lane outputs and source digests while keeping model results, runtime
observations and non-claims separate. Reproduce it from a clean commit with the pinned Node 24:

```sh
env -i PATH=<pinned Node 24 directory>:/run/current-system/sw/bin HOME=<home directory> \
  <pinned Node 24 executable> verification/live-slot-coherence/receipt-2026-11-13.mjs \
  <absolute path of default noble CLI> <absolute path of feature-built test CLI> \
  <new directory outside the repository>
```

The default binary must not contain `noble-test-current-map-seam/v1`; the explicitly
feature-built counterfactual must contain it. The receipt does not promote an LSLOT case.
