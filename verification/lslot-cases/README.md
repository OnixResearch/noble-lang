# Canonical LSLOT case gate

`gate.mjs` executes the live-slot cases registered in
`specs/conformance/live-reference-cases.json` and projects the runtime evidence onto every
machine-expressible `input`, `expected` and variant field. It promotes nothing by itself: it writes
one immutable `acceptance.json` and reports which cases are promotable.

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
`canonical-fixture.json`, every command's raw stdout/stderr and `acceptance.json` there; lane
evidence directories are created under `TMPDIR` and hashed into the receipt.

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
| LSLOT-03 | `live_slot_resource_ref_admission.mjs` | no: four variants did not reach their canonical attempt |
| LSLOT-04 | `live_slot_cas_policy.mjs` | no: the rollback `evidence` conjunct has no runtime representation |
| LSLOT-05 | not run | no: waits on runtime template identity |
| LSLOT-06 | `live_slot_resource_ref_static.mjs` | no: `serialize`/`forge` have no selected source word, and three more variants are partial |
| LSLOT-07 | `live_slot_cas_policy.mjs` | no: `publication-without-grant` needs an admitted proof to be present |
| LSLOT-08 | not run | no: waits on runtime template identity |
| LSLOT-09 | `live_slot_legacy_refusal.mjs`, `live_repl_slot_exclusion.mjs` | yes |

`verification/live-slot-current-source/gate.mjs` and `verification/live-update-corpus` assert the
pre-promotion states. They are historical receipts for the source they ran on; this gate does not
edit or regenerate them.
