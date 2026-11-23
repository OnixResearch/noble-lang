# Abnormal root exit (LREF-CASE-01)

`receipt.mjs` runs `crates/noble-cli/tests/live_slot_abnormal_exit.mjs` against two CLIs and binds
the clean source tree it ran from (`.cairn/archive/2026-11-23-live-reference-lifetimes`, r[LREF-EXIT-01],
owner decision 6):

- **fixed**, built from the recorded source: the lane must pass;
- **prior**, built from the commit before the change: the lane must fail at the save-time root,
  whose release is exact but whose session the prior host still terminates.

The lane runs a two-frame root (caller A dispatching slot B through `slot.invoke`) in a quota-1
registry:

| Variant | Exit | Observed |
|---|---|---|
| save-time (continue) | saving the root's output fails the 32-node observation bound after B performed `test.emit` | `exit_release` exact, session retained, frame pins 2/2/0 with 0 released on abort, no saved owner, the effect reported and not rolled back; a freshly authorized B-v2 then publishes at epoch 2 (the quota-1 registry admits it only without the root's map pin), B-v1's code span retires and a new root selects B-v2 |
| quota (poison) | B exhausts the 100000-step root quota | `exit_release` not exact (the guest's recorded failure is terminal for the shared guest state), frame pins 2/2/0 with no pin left, session terminated, CLI exit 2 |
| abandoned frames (in process) | a root aborted inside both frames | both frame pins released on abort, the map pin released once (B-v1 retired exactly once) |

Host abort and worker cancellation are not executed: `test.abort` is not installable in the
live-slot profile, and the shipped binary has no fault model for cancellation (a failed checkpoint
poisons the session).

## Retained receipt

[`receipt.json`](receipt.json) (`sha256 70d03b18…`): source commit `8b62827a`, clean; fixed CLI
`7f32e4e1…` (status 0); prior CLI `38d1e33d…` declared built from `b4c63a1e` (status 1), whose
save-time root reports `session_state` `terminated-after-guest-failure`, no `exit_release` and
frame pins without `released_on_abort`. The lane output of both runs is in
[`fixed.stdout`](fixed.stdout) and [`prior.stdout`](prior.stdout). Selected host evidence, not a
proof and not a conformance promotion: the LREF-CASE designs are recorded in the change's design,
not registered in conformance.
