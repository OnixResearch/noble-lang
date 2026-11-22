## Phase 1: Design selection building on canonical Live-Slot-Design (no runtime acceptance)

- [x] [serial] Identify which issue #1 semantics the canonical LSLOT contract already selects. Separate the experimental runtime facts at 44329dc and isolate four lifetime gaps without changing `explicit-live-references`. r[LREF-PKG-01] r[LREF-EXIT-01]
- [x] [serial] Answer each "Questions to settle" item. Define the version package retention unit and retirement order, abnormal-exit pin release, non-wrapping counters and escaped-result semantics, related to the #3 borrow and reclamation contract and the #5 quotas. r[LREF-PKG-01] r[LREF-EXIT-01] r[LREF-CTR-01] r[LREF-ESCAPE-01]
- [x] [serial] Map the issue outcome scenarios to the registered LSLOT designs and design LREF-CASE-01..06 as unexecuted. Run document validation and the proposal/design/tasks gates only. r[LREF-EV-01]

## Phase 2: Future implementation and evidence gates (open; not undertaken here)

- [x] [serial] Implement abnormal-exit pin release with exact ledger accounting for a continuing host, and execute LREF-CASE-01. r[LREF-EXIT-01]
  Progress 2026-11-22 (implemented; integrator decision 39, owner decision 6):
  - **Release.** `SlotRegistry.abortRoot` releases every frame pin the failed root still holds from
    the host ledger, innermost first and each exactly once, then the root's map pin; the root's
    refs expire with it. The reply's `frame_pins` gains `released_on_abort`, the pins the host
    released itself, as distinct from those the guest's unwinding released through `leaveFrame`.
    A ledger that does not balance afterwards is refused.
  - **Continue or poison.** The slot engine continues the session only when the release is
    provably exact: every cleanup step succeeded, no guest call was interrupted, no guest failure
    was recorded (it is terminal for the shared guest state), no continuation, reflection walk,
    dispatch or borrowed frame is open, no pin was released on abort (its backend reference would
    remain), the operand roots were released through the backend and the retirement sweep
    succeeded. Otherwise it poisons, as before. Replies carry `exit_release` and `session_state`
    (`retained` or `terminated-after-guest-failure`).
  - **LREF-CASE-01.** `crates/noble-cli/tests/live_slot_abnormal_exit.mjs` (hosted by
    `live_slot_direct_quote.rs`) runs the save-time variant (continue: a freshly authorized B-v2
    publishes at epoch 2 in a quota-1 registry, B-v1 retires, a new root selects B-v2) and the
    quota variant (poison, with no pin left), and aborts an in-process root inside both frames
    (2 pins released on abort). Host abort and worker cancellation are not executable through the
    shipped CLI. The receipt tool is `verification/abnormal-exit/receipt.mjs`.
  Ticked 2026-11-22 (Cairn batch 5). [`verification/abnormal-exit/receipt.json`](../../../verification/abnormal-exit/receipt.json)
  (sha256 `70d03b18…`, clean source `8b62827a`): the fixed CLI `7f32e4e1…` passes the lane, and the
  CLI `38d1e33d…` declared built from the commit before the change (`b4c63a1e`) fails at the
  save-time root, terminating a session whose release was exact. The OWN gate run
  `verification/own-cases/runs/20261122-b5` observed OWN-CASE-03/05 as amended. The review is the
  integrator self-review `review-integrator-2026-11-22-lref-exit.md` under owner decision 40 (owner
  veto at the push). No LREF case is registered or promoted; LREF-CASE-02, 04, 05 and 06 stay open.
- [x] [serial] Implement package-level version retention, resolve #3 `OWN-CASE-06`, and execute LREF-CASE-03 through slot replacement. r[LREF-PKG-01]
  Ticked 2026-11-09. `crates/noble-cli/tests/live_slot_package_retention.mjs` runs LREF-CASE-03,
  hosted by a third test in `live_slot_direct_quote.rs`.
  - **OWN-CASE-06.** Resolved by extending module lifetime to saved callers; see
    `.cairn/changes/ownership-lifetime-contract` and `verification/module-root-retention/`.
  - **Package retention.** A saved owner is now charged for every known version whose cell lives
    in a module its graph enters (the module set OWN-CASE-06 keeps alive), as well as for the
    versions it reaches as cells. Before, a Program saved from a dispatch to v1 kept M1 alive
    without holding v1's charge, so a quota-1 registry admitted v2 while it lived.
  - **LREF-CASE-03.** Through `slot.invoke`, v1 returns `[ h ]`, saved as O. At quota 2, v2 is
    published, `20 O run` yields 21, and M1's span retires only when O is released. At quota 1,
    v2 is refused while O lives and admitted after O is released, retiring M1's span.
  - **Evidence.** `verification/package-retention/receipt.json`: the fixed CLI passes from clean
    source `9a922ab9`, and the CLI from the parent commit `eb2d0ba0` admits v2 while O lives.
  - **Not covered.** The package is approximated by the modules whose code the saved graph
    enters; table and capture members count only as far as that graph walk observes them. No
    LREF or LSLOT case is promoted.
- [ ] [serial] Execute LREF-CASE-02, 04, 05 and 06 together with LSLOT-01..09 against an implemented profile, recording source-bound evidence separately per case. r[LREF-CTR-01] r[LREF-ESCAPE-01] r[LREF-EV-01]
