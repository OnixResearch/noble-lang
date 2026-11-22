# Integrator self-review: abnormal-exit pin release (LREF-EXIT-01), 2026-11-22

This record is an integrator self-review under owner decision 40 ("merge into the batch 4+5 push
now; your veto at the push is the only review, as in decision 38"). It is not a fresh-context
review: the integrator wrote the reviewed commits.

## Reviewed commits

- `cc23fb84`: `SlotRegistry.abortRoot` releases every frame pin still held from the host ledger
  (innermost first, each exactly once, counted as `released_on_abort`), then the map pin; the slot
  engine continues the session only on a provably exact release (owner decision 6) and reports
  `exit_release`; the LREF-CASE-01 lane and the updated OWN-CASE-03/05, registry and coherent-maps
  lanes.
- `c1d9c76a`: OWN-CASE-03/05 amendments (integrator decision 39), the OWN gate's new comparisons and
  `verification/abnormal-exit/receipt.mjs`.
- `8b62827a`: the OWN gate compares objects independently of key order (found by the first gate
  run on the merge `da8d475e`, which reported OWN-CASE-05 unmet only on key order).

## Checks performed

| Check | Result |
|---|---|
| 28 live-slot lanes against the new CLI (`node crates/noble-cli/tests/live_slot_*.mjs`) | all pass except `live_slot_selected_retention.mjs`, which needs compiler-metadata arguments its Rust host supplies and is unaffected |
| `cargo test -p noble-cli --test live_slot_direct_quote` | 6 passed, including the new LREF-CASE-01 host |
| `node crates/noble-cli/tests/live_slot_registry.mjs` | passed (abort releases the one pin left; a second abort and a late `leaveFrame` are refused) |
| OWN gate `runs/20261122-b5` at `8b62827a` | `all-passed`, OWN-CASE-01..07 promotable |
| LREF receipt `verification/abnormal-exit/receipt.json` | fixed CLI passes; the prior CLI (`b4c63a1e`) fails at the save-time root, terminating a session whose release was exact |

## Exactness conditions (the risk this change takes)

Continuing a session after a failed root is new. The engine continues only when every condition
holds; any other exit poisons as before:

1. every cleanup step (frozen replay owner, provisional saved owners, trace inspection) succeeded;
2. no guest call threw (a trap can stop the backend between two of its own updates without a flag);
3. the shared guest `failure` global is zero (a recorded guest failure is terminal: its operand and
   continuation roots can never be released);
4. no continuation (`cp`), reflection walk (`rp`), dispatch (the in-progress word) or borrowed frame
   (current-frame word, `live_frame`) is open;
5. no frame pin was released on abort (such a pin keeps its backend reference);
6. `clear_stack` released the operand roots, and the retirement sweep succeeded;
7. the session was not poisoned during the root (checkpoint failure, donor change, retention
   failure).

In practice only host-side failures after the guest settled continue: a failure while saving
outputs, or a refusal before the guest runs (for example a root reference with no pinned published
slot). Every guest failure and every trap still terminates the session.

## Findings

- F1 (fixed, `8b62827a`): the OWN gate compared objects by key order.
- F2 (accepted): OWN-CASE-03's save-time refusals and the coherent-maps half-published root now
  keep the session; their lanes release every earlier owner and run another root, or close with
  exit 0, so a leak or a poisoned session fails them.
- F3 (open, recorded): host abort and worker cancellation are not executable through the shipped
  CLI; the trap path with frames still entered is exercised in process only.
- F4 (side finding, unrelated): installing `[ ] dup compose run quote` as a live-slot target fails
  with an internal failure ("actual installed Program cell differs from independently checked
  metadata"); tracked separately.

## Verdict

ACCEPT-WITH-NOTES for ticking the LREF-EXIT-01 implementation task on the receipt, and for the
OWN-CASE-03/05 promotions on the OWN gate run. No proof is claimed.
