# Nested session protocol model (WIT-SESSION-01, WIT-SESSION-02)

Model-theorem check for `.cairn/changes/nested-session-protocol`. It builds
`proofs/m6/NestedSession.lean` in an isolated project with the pinned Lean 4.31 toolchain, then checks
every inventoried theorem's compiled statement and transitive axioms.

## Command

```sh
env -i HOME="$HOME" USER="$USER" LOGNAME="$USER" PATH=/run/current-system/sw/bin:/usr/bin:/bin \
  "$(selected node)" verification/nested-session/check.mjs --output /tmp/nested-session-check
```

Same refusals as the region-trace harness: a caller environment carrying `LEAN*`, `LAKE*`, `BASH*`,
`GIT_*`, `NIX_*` or `ELAN*` is refused, an output directory inside the source tree is refused, and a
dirty worktree needs the explicit `--allow-dirty` flag (which the receipt then records).

## What is modelled and proved

The grammar is one finite set of nodes — `finished`, typed `send`, typed `recv`, internal `choose`,
external `offer`, and a named `nested` constructor with explicit parameters — resolved against a closed
definitions table under a **bounded** unfolding depth. The endpoint is an opaque move-only resource
carrying channel identity, side, protocol identity, current state and generation.

| Theorem | Claim |
|---|---|
| `dual_is_involutive`, `dual_swaps_send_and_recv`, `dual_swaps_choose_and_offer` | duality is definitional, involutive, and swaps Send/Recv and internal/external choice on the same message/branch |
| `resolve_refuses_exhaustion`, `resolve_unfolds_with_depth`, `resolve_refuses_unknown_definition` | exhaustion and an unknown index are **refusals**, never acceptances |
| `endpoint_copy_is_refused`, `endpoint_discard_is_refused`, `endpoint_capture_is_refused` | copy, discard and capture each have their own named refusal |
| `stale_generation_is_refused`, `matching_generation_is_accepted`, `double_close_is_refused` | identity alone is not authority; the matching generation is the non-vacuous control; a second close is refused |
| `closed_endpoint_cannot_send`, `send_step_advances_to_the_continuation` | a closed endpoint cannot send, and one step advances exactly to the send node's continuation with peer/channel/protocol/generation unchanged |
| `send_step_does_not_observe_a_peer` | a send step returns only the endpoint's own successor: **receipt is not modelled** |
| `admitOne_is_bounded`, `admission_is_bounded`, `full_mailbox_is_unchanged` | admission keeps the mailbox depth inside the declared limit (given a well-formed session) and a full mailbox refuses without a silent drop |
| `admission_preserves_the_endpoint_inventory`, `no_endpoint_duplication` | no operation in this model increases the endpoint inventory |
| `trace_send_then_recv`, `trace_recursive_entry_unfolds_to_the_send_node`, `trace_orderly_close`, `trace_bounded_buffering_refuses_the_third_payload` | positive traces over one closed table: sequencing, recursive entry unfolding to its definition, orderly close once, bounded buffering stopping at the limit |

`verification/nested-session/receipt.json` is the retained receipt of one clean-source run: **24** model
theorems, `propext` the only reported axiom and inside the standard allow-list. Its `bindings` block
records `helper`, `runtime` and `peer` as explicitly absent with the reason, and its `claims` block
labels the verdict.

## Non-claims

The definitions table is handwritten: there is no lowering, runtime, scheduler, queue or peer
correspondence here, **no progress statement**, and endpoint delegation (passing an endpoint as a
message) is excluded, so ownership obligations stay outside this model. Cancellation, mailbox
fullness and cleanup facts are not modelled yet. WIT-SESSION-03 through WIT-SESSION-06 remain open.
