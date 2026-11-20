# Canonical LSLOT case gate

`gate.mjs` executes the live-slot cases registered in
`specs/conformance/live-reference-cases.json` and projects the runtime evidence onto every
machine-expressible `input`, `expected` and variant field. It promotes nothing by itself: it writes
a new immutable receipt file and reports which cases are promotable.

## Running it

The gate refuses a dirty tree, a different Node than the one selected by
`crates/noble-cli/src/core/runtime/config.json`, and a target directory inside the repository. It
also refuses an environment that already carries a loader or interpreter preload variable, so run
it with a clean environment (`env -i`) and the pinned Node:

```sh
env -i HOME="$HOME" USER="$USER" LOGNAME="$USER" LANG=C.UTF-8 TMPDIR=/some/private/scratch \
  NOBLE_LSLOT_TARGET_DIR=/some/external/cargo-target \
  PATH=/pinned/node/bin:/run/current-system/sw/bin:/usr/bin:/bin \
  /pinned/node/bin/node verification/lslot-cases/gate.mjs /a/new/output/directory
```

`ABSOLUTE_NEW_OUTPUT_DIRECTORY` must not exist. The gate writes `source.json`,
`canonical-fixture.json`, every command's raw stdout/stderr and a new `receipt-<UTC>.json` (with its
`.sha256`) there; lane evidence directories are created under `TMPDIR` and hashed into the receipt.

Retained receipts live beside the gate: `acceptance.json` is the round-1 receipt,
`acceptance-2026-11-20-r3.json` its retained round-3 successor, and
`acceptance-2026-11-20-r4.json` the receipt the current promotions cite. A run never writes,
overwrites or binds a retained receipt — every retained receipt file is excluded from the receipt's
manifest, and promoting copies a run's new receipt to a new retained name. The raw mirror of a
promoted run keeps the lane evidence and the run's own command outputs.

## What a promotion means

- `observed`: a lane measured the field and it equals the canonical value.
- `declared`: prose/design text that no runtime field can carry. Never counted as an observation.
- `unobserved`: a machine-expressible premise whose canonical attempt did not run. It refuses the
  case and names the field.
- `mismatch`: a measured value that differs. It fails the gate.

A case is promotable only with `observed` and `declared` entries. The promoted case records in
`specs/conformance/live-reference-cases.json` keep `proof: open` and `trust: explicit`: a receipt
executes behavior, it does not prove it.

## Case coverage

| Case | Lane(s) | Promoted |
|---|---|---|
| LSLOT-01 | `live_slot_capture_legacy.mjs`, plus `live_slot_late_control.mjs` and `live_slot_selected_origin.mjs` as controls | yes |
| LSLOT-02 | `live_slot_nested_roots.mjs` | yes |
| LSLOT-03 | `live_slot_resource_ref_admission.mjs` | yes, under the owner decision 19 amendment: the unselectable `fs.write` and impossible under-declare variants are the drops, the input-only variant is replaced by the both-sides `Account@2` candidate, and `generic_dispatch_effect` is observed from a constructible generic caller's reflected site |
| LSLOT-04 | `live_slot_cas_policy.mjs` | no: the rollback `evidence` conjunct has no runtime representation |
| LSLOT-05 | not run | no: the gate has no projector for the positive selected-target proof; the positive target and proof-carrying publication are still absent |
| LSLOT-06 | `live_slot_resource_ref_static.mjs` | yes, under the owner decisions 19 and 25: `serialize`/`forge` and `retain` are dropped as impossible by design, `guest_requests` counts the rejected attempts only, and `aggregate-capture`/`export-or-return` accept their earliest refusal |
| LSLOT-07 | `live_slot_cas_policy.mjs` | no: `publication-without-grant` needs an admitted proof to be present |
| LSLOT-08 | not run | no: the gate has no projector for the exact replay case; the positive target and proof-carrying publication are still absent |
| LSLOT-09 | `live_slot_legacy_refusal.mjs`, `live_repl_slot_exclusion.mjs` | yes |

## Regressions the gate runs before projecting

Every run re-checks that the checks can fail, and a failed expectation sets the receipt result to
`failed`:

- **In-root publication.** A copy of this run's LSLOT-02 evidence with the old root's publication
  event removed must project `input.base.interleaving` as `mismatch`; the unmodified evidence must
  project it as `observed`.
- **LIVE contract baseline.** The LIVE-01..10 contract fields are compared with the full baseline
  commit `8075d3c083474884f9a7c72b5b1a483f73eec314`. The comparison must accept the original
  LIVE-02 `input.after_reload_ack`, accept exactly the owner-approved decision-21a value
  (`["20 swap run 20 twice swap"]`), reject decision 21's superseded script, and reject any change
  to another LIVE case.

`verification/live-slot-current-source/gate.mjs` and `verification/live-update-corpus` assert the
earlier case states. They are historical receipts for the source they ran on (owner decisions 4 and
20); this gate does not edit or regenerate them.
