# OWN-CASE acceptance gate

`gate.mjs` builds the production `noble` binary from a clean checkout and runs the committed lanes
for OWN-CASE-01, 02, 03, 04 (non-borrow variants), 05, 06 and 07. It binds the source snapshot,
the built binary, the exact case projections and every raw observation. It is derived from the
settled conformance-case-promotion recipe; it promotes nothing by itself.

## Retained runs

| Run | Command | Result |
|---|---|---|
| [`runs/20261120c`](runs/20261120c) | `node verification/own-cases/gate.mjs --out <new dir>` at commit `5c9e38f4` | `completed`; no failures |
| [`runs/20261120-failed`](runs/20261120-failed) | the same gate before its row parser was fixed | `failed`; OWN-CASE-04 rows were misparsed after the test harness prefix |

[`acceptance.json`](acceptance.json) is the retained passing receipt (`sha256 e873df58…`,
`source_revision sha256:3d24…`, binary `d1e084df…`).

## Promoted cases

OWN-CASE-01, 02, 03, 04, 06 and 07 are promoted `implemented/passed/open/explicit`, each with one
evidence record that binds this receipt. Proof stays open for every case; no lowering, runtime or
host-ledger correspondence proof is supplied.

- **OWN-CASE-03** now runs the balanced, left-deep and right-deep 8-leaf trees. The anonymous
  capture summary had used a smaller budget (64 nodes/1000 bytes) than the selected-origin and
  retained-graph paths (32 nodes/4096 bytes) for the same graph; it now uses the same exact bound.
  Retention accounting is unchanged: `observeProgramGraph` already walks up to 4096 nodes, and a
  40-leaf control is still refused at compose 16 by the 32-node exact-summary bound.
- **OWN-CASE-04** exercises the kernel acceptance boundary directly; the six non-borrow variants
  are rejected at the check stage before any submission or emission.

## Withheld: OWN-CASE-05

The designed `test.abort` trap cannot be produced in the live-slot source profile. Live-slot
sessions use the kernel environment, which ends at `test.emit`; the ordinary Core environment
appends the abort row in `adapt_environment`. Three kernel gates block a noble-cli/noble-contracts
fix:

1. `bootstrap_matches` fixes a declared-module environment's effect list to `test.emit` plus
   `live.dispatch`, so the abort effect id cannot be added.
2. `acceptance/parts/instantiate.rs` rejects any candidate effect absent from `env.effects`.
3. The bootstrap table has no scheme row for Definition 23, so inference reaches the internal
   `inconsistent frontend arena` diagnostic.

Admitting `test.abort` therefore needs an environment-rule change in the M4-locked
`crates/noble-kernel`; this slice did not take a contract escape around it.

The host ledger's abnormal-exit path is measured in `crates/noble-cli/tests/live_slot_own_trap.mjs`:
`abortRoot` clears entered dispatch frames, a normal `finishRoot` is refused while frames are
entered, and the aborted root's captured map pin retires exactly once. Backend per-root frame-pin
counters are not exposed by any operator request or registry accessor, so "frame pins released
exactly once" is not observed and the case stays unexecuted/unpromoted.

## Non-claims

- Reclamation is observed through owner refusal and survivors running alike; the runtime exposes no
  live-cell count.
- The receipt is finite behavioral evidence for one binary built from the recorded commit, not a
  universal proof of ownership, reclamation or compiler correctness.
