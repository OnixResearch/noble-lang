# Push B integrator review (owner decision 54), 2026-11-22

Scope: the integrator-authored commits of the push-B integration branch (batches 7-10 and
proof-automation task 4), reviewed on the merged tree after `6fe17303`. The owner
answered "you review" (decision 54), superseding decision 51's hold for a fresh-context reviewer.
The reviewer is the author of most of these commits; this record states what was re-read, what
was checked independently of the commit messages, and what still rests on pending evidence.

## Verdict

ACCEPT-WITH-NOTES, conditional on the pending evidence listed at the end. One defect was found and
fixed before this verdict (R-3).

## Findings

| ID | Commit | Severity | Finding | Disposition |
|---|---|---|---|---|
| R-1 | `05218d4b` | note | The closed subject a selected-target claim binds (`anonymous_target_subject`) is rendered by trusted host code from the verified template rows: composition is concatenation, a quoted root input becomes its captured value, a quoted Program becomes a nested quotation and a static Program its checked body. The renderer is bounded (depth 32) and runs only after `Plan::new` and `Graph::validate` accept the actual graph, but there is no independent model of the rendering. | Record it as an explicit assumption of LSLOT-05's evidence; the claim is about that subject, and the installed bytes' correspondence to it stays an artifact premise. |
| R-2 | `05218d4b`, `2238d9bb` | note | The registry freezes an admitted evidence record shallowly (`captures` is copied and frozen; other nested values are not). A caller that mutates a nested value after admission can only make the later `exactEvidence` re-check fail; it cannot make a mismatching record pass. | No change. |
| R-3 | `fa0eb5e3` (PAUTO) | major, fixed | Binding the four interface comparisons of the unary rule to a local made the pinned Aeneas refuse the contracts subject ("Nested borrows are not supported yet"), which blocks the M4 lane. | Fixed in `6fe17303`: the comparison returns from `unary_shape_differs`, as the translated predecessor did; the refusal messages are unchanged. Re-verified by the push-B ladder's pre-discovery. |
| R-4 | `50c4228c`, `c8b5dfc8` | note | `GuardExtracted` proves facts about `NobleContractImpl.Funs`, which the push-B ladder regenerates because PAUTO changed `noble-contracts`. The guard functions themselves are untouched, but a renamed auxiliary would break the MC2 build. | The MC2 lane on the ladder's tree is required evidence before the push. |
| R-5 | template identity (closeout) | minor | The `P-ID-03` delta's trailing status paragraph still describes LSLOT-05/08 as unpromoted and alias/owner semantics as unimplemented; after push B both are false, and the paragraph would become canonical text at sync. | Rewrite it before the decision-50 sync. |

## What was checked

- `2238d9bb` (dispatch): every map entry is created by `bootstrap` or `publish`, and both call the
  candidate check with the host attestation; dispatch re-checks the stored record against the
  selected version (`definitionId`, `exactEvidence`) and skips only the one-shot attestation. A
  proof-required slot cannot hold an unattested entry, so dispatch cannot admit one.
- `05218d4b` (LSLOT-05): every declared evidence field (subject, captures, context, claim digest,
  assumptions, artifact manifest digest) is compared before the proof is rechecked; the recheck is
  the `noble build --require-proof` consumer route; the core then binds the accepted claim to the
  target's closed subject (`bind_artifact` then `release`), and any refusal is reported before the
  worker sees the publication.
- `aaaa13af` (counters): `epoch-exhausted`, `generation-exhausted` and `incarnation-exhausted` are now
  distinct. The only constructor that seeds counters is `counterFixture`; the slot engine builds its
  registry from a literal options object and never passes it, so shipped inputs cannot reach it.
- `3de97362` (projector): every registered LSLOT-08 expected field is measured by `project08`.
- `5bcd5dc4` (T4/T2/T5): the statements are restricted to lifted frames exactly as the module text
  says, and the unrestricted bridge is not claimed; axioms `propext`, `Quot.sound` only.
- `50c4228c`, `c8b5dfc8` (guards): the module states what is not proved (agreement of the structural
  reading with the MC1 export beyond fixed examples; completeness of the bounded `is_type0`), which
  matches the theorems.
- PAUTO: only an accepted Lean refutation sets `disproved`; consumer refusals of the instance are
  `candidate-model-only`, and infrastructure failures are reported as themselves (`not-checked`);
  the compiled run changes only `advertised_as` and `discrepancy`, never the outcome.

## Pending evidence before the push

The push-B ladder (M4 discovery, regenerated subjects, inventory, M4 lock and check), the
service-wire rebind, the lanes (MC1 169 required theorems, M3-Wasm, MC2 with `GuardExtracted`,
service-wire), the LSLOT gate final run, the live-slot lane receipt (LREF-CASE-02/04/05/06,
ID-05), the PAUTO gate rerun on the merged tree, check-specs and all 17 flake checks.

## Evidence outcome (2026-11-23)

The pending evidence above came back clean on the push-B tree:

- Ladder: the pre-discovery after `6fe17303` regenerated the contracts subject (`85ceaa99`); the
  inventory was renewed (`9f921a1b`), the M4 lock renewed (`e2273206`) and checked (`aa8e3521`,
  `M4-IMPLEMENTATION-GATE passed: sha256:30f47a93…`), and service-wire was rebound (`0fffc7d8`).
- Lanes on clean `0fffc7d8`: MC1 passed with 169 required theorems (`sha256:9c8bcc8e…`), M3-Wasm
  passed (`sha256:701738e5…`), MC2 passed CONTRACT-01..15 with the ten `GuardExtracted` theorems
  and its embedded M4 check (`sha256:985b1fde…`; R-4 closed), and service-wire passed with 61
  theorems and all eleven mutation controls refused.
- LSLOT gate on `0fffc7d8`: all nine cases promotable, no mismatch or integrity issue
  (`verification/lslot-cases/acceptance-2026-11-23-pb.json`, sha256 `455597be…`); LSLOT-05's
  promotion record names the host-rendered subject as an explicit assumption (R-1).
- Live-slot lanes receipt (LREF-CASE-02/04/05/06 and ID-05) on `0fffc7d8`
  (`verification/live-slot-lanes/runs/20261123-0fffc7d8/receipt.json`, sha256 `98313669…`).
- PAUTO gate rerun on `0fffc7d8`: `all-passed`, PAUTO-CASE-02..06 promotable (sha256 `58878425…`).
- LIVE and OWN gates on `0fffc7d8`: LIVE-01..10 and OWN-CASE-01..07 all passed with unchanged
  projections.
- The P-ID-03 paragraph was rewritten before the sync (R-5). check-specs and `cairn validate`
  pass after the LREF, GUARD and P-ID-03 syncs. The 17 flake checks run on the final push-B head
  before the fast-forward push.
