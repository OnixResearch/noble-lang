## Phase 1: Design selection building on canonical Live-Slot-Design (no runtime acceptance)

- [x] [serial] Identify which issue #1 semantics the canonical LSLOT contract already selects. Separate the experimental runtime facts at 44329dc and isolate four lifetime gaps without changing `explicit-live-references`. r[LREF-PKG-01] r[LREF-EXIT-01]
- [x] [serial] Answer each "Questions to settle" item. Define the version package retention unit and retirement order, abnormal-exit pin release, non-wrapping counters and escaped-result semantics, related to the #3 borrow and reclamation contract and the #5 quotas. r[LREF-PKG-01] r[LREF-EXIT-01] r[LREF-CTR-01] r[LREF-ESCAPE-01]
- [x] [serial] Map the issue outcome scenarios to the registered LSLOT designs and design LREF-CASE-01..06 as unexecuted. Run document validation and the proposal/design/tasks gates only. r[LREF-EV-01]

## Phase 2: Future implementation and evidence gates (open; not undertaken here)

- [ ] [serial] Implement abnormal-exit pin release with exact ledger accounting for a continuing host, and execute LREF-CASE-01. r[LREF-EXIT-01]
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
