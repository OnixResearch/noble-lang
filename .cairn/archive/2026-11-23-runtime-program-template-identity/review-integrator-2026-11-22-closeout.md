# runtime-program-template-identity: integrator closeout review (decision 50), 2026-11-22

Owner decision 50 routes this change's closeout through an integrator review plus the owner's
veto, then the `P-ID-03` sync and the archive. This record reviews the delta against the evidence
that exists after Cairn batch 6 and push B; the push-B receipt values were filled in on 2026-11-23
from the final runs on clean source `0fffc7d8`.

## Verdict

ACCEPT-WITH-NOTES for the sync, after the delta's trailing status paragraph is rewritten (N-1).

## The delta against the evidence

| Delta clause | Evidence | Status |
|---|---|---|
| Admission derives the target's own `DefinitionId` from the returned program, not the builder's, a caller's, a label or bytes | tasks 1-4 (batch 6): `OriginTemplate` structural rows derived from checked source lineage; host and worker re-derive and compare | implemented for the bounded selected I64 builder subset |
| Fixed literals and checked computed constants are fixed operands; only varying root input creates capture slots | `selected_target_projection_classifies_fixed_computed_and_root_bindings`; the `live_slot_selected_origin` lane (fixed/computed 2→3 D-different, root-input 2→3 same-D/different-P) | implemented |
| Logical repeated-slot alias follows source-binding lineage, not VM sharing; independent equal-valued bindings stay distinct | `selected_template_projection_separates_logical_bindings_from_vm_sharing`; `shared-program` versus `repeat-root`, `renamed-alias` | implemented |
| Semantic lexical recursive owner distinguishes templates; VM owners, handles and pins do not | `template_projection_requires_a_classified_recursive_owner`; `SelfNamed` | implemented |
| Unverifiable, missing, truncated, forged or ambiguous mappings refuse identity-bearing admission | `returned_target_refuses_absent_forged_truncated_and_ambiguous_template_metadata`; `selected_template_projection_refuses_absent_forged_truncated_and_stale_metadata` | implemented |
| The selected `ProgramValueId` binds template, ordered typed captures, interface/effects and dependencies; installed Wasm corresponds separately | v4 experimental identities; composite manifest (`template_sha256`, captures, classes, donors) recomputed by the worker; LSLOT-05 binds the artifact digest as a declared field | implemented; correspondence stays an artifact premise |
| ID-05: same template, different program values for captures 2 and 3 | ID-05 capture lane (`live_slot_capture_identity.mjs`), receipt `verification/live-slot-lanes/runs/20261123-0fffc7d8/receipt.json` (sha256 `98313669…`) | executed in push B |
| P-ID-06/07, VC-LSLOT-01, EV-LSLOT-01 govern proof, replay and authority separately | LSLOT-05 (selected-target evidence) and LSLOT-08 (replay-only, two baselines), promoted from `verification/lslot-cases/acceptance-2026-11-23-pb.json` (sha256 `455597be…`) | executed in push B |

## Notes

- N-1 (minor): the delta's last paragraph still says the vectors refine an unexecuted ID-05 relation
  "without ... promotion of LSLOT-05/08" and that the control supplies "neither general logical
  alias/lexical-owner semantics nor implemented returned-target P/D"; after push B both statements
  are false, and the paragraph would become canonical text at sync. Replace it with the current
  scope, keeping P-ID-04, general returned-target admission and a proof of the derivation open.
- N-2: the derivation is implemented for the bounded selected subset only; unsupported shapes stay
  opaque and receive no identity. The canonical text must not read as general admission.
- N-3: no proof of the identity derivation exists; the evidence is behavioral (tests and lanes).

## Closeout steps

Rewrite the paragraph (N-1), tick task 5 with the push-B receipts and task 6 with this record and
the owner's veto, `cairn sync` the `P-ID-03` MODIFIED requirement, regenerate the views and the
ledger, run check-specs and `cairn validate`, then archive.

## Owner answer

Campaign decision 55 (2026-11-22): no veto. The paragraph is rewritten as proposed, `P-ID-03` is
synced in the push that carries the LSLOT-05/08 receipts, and the archive follows with the next
ladder (the change is referenced from `crates/` tests and the live-slot lane receipt tool).
