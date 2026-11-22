# Integrator review: OWN-CASE-03 amendment (decision 35), 2026-11-22

This is an **integrator review** under owner decision 28 (reviewer agents unavailable), not a
fresh-context review. The owner keeps a veto before any push.

## Scope

- `specs/conformance/ownership-cases.json`: the OWN-CASE-03 registration and its `amendments`
  entry (decision 35, delegated).
- `crates/noble-cli/tests/live_slot_ownership_cases.mjs`: the lane, parametrized by leaf count.
- `verification/own-cases/gate.mjs`: the OWN-CASE-03 projection of each registered variant.
- `.cairn/changes/ownership-lifetime-contract/tasks.md`: the progress note.

## Findings

1. **The amendment keeps the case's purpose.** OWN-CASE-03 checks that owners of captured-I64
   `compose` trees release independently of shape: every leaf and intermediate owner except the
   root is released, and the root still runs with the same result. The design's 8-leaf deep
   chains cannot run under the reviewed live-slot observation bound (32 nodes / 4,096 bytes per
   anonymous summary child): they are refused at compose 5. Raising that bound would change a
   reviewed live-slot limit to make an ownership case pass and would only move the cliff to the
   next size, so decision 35 keeps the bound. The amendment observes shape independence where it
   fits (4-leaf balanced, left-deep and right-deep, 10 with 6 releases each) and keeps the 8-leaf
   balanced tree (36 with 14 releases). The 8-leaf deep chains are registered as refused, with the
   refusal's compose index, diagnostic and session state, not dropped.
2. **The lane now matches the design's release count.** Before the amendment the deep-chain lane
   released 7 of the 14 owners the design names; it now releases every leaf and intermediate
   except the root (`2n - 2`), as the balanced tree does. This is a lane correction the amendment
   exposed, not a relaxation.
3. **The gate cannot pass a narrowed case.** Each registered variant is projected either as an
   executed tree (result before and after, releases before the root) or as the bound's refusal at
   the stated compose. A refused variant registered as executed, an executed variant registered
   as refused, or any field difference makes the case unmet. The bounded control (40 leaves,
   refused at compose 5, exit 2) is unchanged.
4. **Note.** The anonymous summary bound is a property of the live-slot host observation, not of
   ownership. The amendment records that 8-leaf deep shape independence is not observed; it is
   observed for 4 leaves.

## Method

- Read the amendment, the lane diff and the gate diff (`dabf880d`).
- Ran the OWN gate on the batch-4 tree `88e90add`
  (`verification/own-cases/runs/20261122-b4/acceptance.json`, sha256 `1a5abea4…`): verdict
  `all-passed`. OWN-CASE-03 observed balanced-8 36 before and after 14 releases; left- and
  right-deep-8 refused at compose 5 with session `terminated-after-guest-failure`; balanced-,
  left-deep- and right-deep-4 each 10 before and after 6 releases; the 40-leaf control refused at
  compose 5 with exit 2. OWN-CASE-01, 02, 04, 06 and 07 passed with their registered
  observations; OWN-CASE-05 stays withheld.

## Verdict

ACCEPT. The amendment does not narrow the case: the deep 8-leaf chains are registered as the
bound's refusal, which the gate checks exactly, and the 4-leaf variants observe the property the
case exists for within the reviewed bound. OWN-CASE-03 may be promoted to implemented/passed on the
batch-4 run with proof open.
