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
`acceptance-2026-11-20-r3.json` its retained round-3 successor,
`acceptance-2026-11-20-r4.json` the receipt the first five promotions cited, and
`acceptance-2026-11-23-pb.json` the receipt the current promotions cite. A run never writes,
overwrites or binds a retained receipt — every retained receipt file is excluded from the receipt's
manifest, and promoting copies a run's new receipt to a new retained name. The raw mirror of a
promoted run keeps the lane evidence and the run's own command outputs.
`acceptance-2026-11-21-b1.json` (sha256 `e240cdf6…`, mirror `raw/b1-054806`) re-runs the gate on
the merged Cairn batch-1 tree `05bac95a` (source revision `sha256:0c80b13f…`, binary
`d0c98cc0…`): the same five cases are promotable, LSLOT-04 and LSLOT-07 are refused, LSLOT-05 and
LSLOT-08 are not executed, and there are no mismatches or integrity issues. The case records keep
citing the r4 receipt.
`acceptance-2026-11-21-b2.json` (sha256 `ba83e84f…`, mirror `raw/b2-175231`) re-runs the gate on
the Cairn batch-2 tree `7d910d87` (source revision `sha256:a319bebd…`, binary `1e8f444a…`), after
the gate learned to list the batch-1 receipt: the same five cases are promotable, LSLOT-04 and
LSLOT-07 are refused, LSLOT-05 and LSLOT-08 are not executed, and there are no mismatches or
integrity issues. The case records keep citing the r4 receipt.
`acceptance-2026-11-21-b3.json` (sha256 `f5b0744d…`, mirror `raw/b3-215133`) re-runs the gate on
the combined Cairn tree `151fa703` (batch 2 plus LIVE round 3; source revision
`sha256:b9dd56f1…`, binary `c9b21ea8…`). That tree first refused LSLOT-09's
`historical_LIVE_01_to_10_changed`, because owner decision 30 dropped LIVE-05's `missing-import`
variant; `live-baseline.mjs` now admits exactly that drop beside decision 21a's LIVE-02 change.
The same five cases are promotable, LSLOT-04 and LSLOT-07 are refused, LSLOT-05 and LSLOT-08 are
not executed, and there are no mismatches or integrity issues. The case records keep citing the
r4 receipt.
`acceptance-2026-11-23-pb.json` (sha256 `455597be…`, mirror `raw/pb-071700`) runs the gate on the
Cairn push-B tree `0fffc7d8` (source revision `sha256:f0827fca…`, binary `f82595ad…`), which adds
the LSLOT-05 selected-target evidence lane, the LSLOT-04/07 evidence conjuncts on a proof-required
slot (integrator decision 46) and LSLOT-08's replay from two actual baselines (integrator decision
45). All nine cases are promotable, with no mismatches or integrity issues. LSLOT-04, 05, 07 and 08
are promoted from it, and the records of LSLOT-01, 02, 03, 06 and 09 now cite it (their designs and
observed values are unchanged).

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
| LSLOT-04 | `live_slot_cas_policy.mjs`, with the rollback `evidence` conjunct from `live_slot_evidence_policy.mjs` | yes: the conjunct is observed on a proof-required slot whose evidence is the LSLOT-05 lane's (integrator decision 46) |
| LSLOT-05 | `live_slot_selected_target_evidence.mjs` | yes: the selected returned target's exact evidence record, rechecked by the `noble build --require-proof` consumer (integrator decision 43); the host-rendered closed subject is an explicit assumption |
| LSLOT-06 | `live_slot_resource_ref_static.mjs` | yes, under the owner decisions 19 and 25: `serialize`/`forge` and `retain` are dropped as impossible by design, `guest_requests` counts the rejected attempts only, and `aggregate-capture`/`export-or-return` accept their earliest refusal |
| LSLOT-07 | `live_slot_cas_policy.mjs`, with `publication-without-grant` from `live_slot_evidence_policy.mjs` | yes: the admitted proof present in `publication-without-grant` is the LSLOT-05 lane's exact record (integrator decision 46) |
| LSLOT-08 | `live_slot_quota_replay.mjs` | yes, as amended by integrator decision 45: the replay variants run from two actual baselines (the captured `adder` target and the effectful `traced` target); exact replay eligibility is neither behavioral proof nor real-effect authority |
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
