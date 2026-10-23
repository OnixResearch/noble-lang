# MW2 worker cancellation slice

## Interface and host

- [ ] [serial] Extend `worker-send.wit` with explicit await/result/error semantics and update the independent component-model-async peer; retain move-only cancellation/delivery disposition. r[RA-ASYNC-05]
- [ ] [serial] Report authority/task/resource observations at admission, cancellation acknowledgement, late completion and final retirement; remove constant external-outcome/rollback claims. r[H-OUTCOME-02]
- [ ] [serial] Execute both cancellation-before-delivery and delivery-before-cancel, with late-success receipt semantics and no owner resurrection, retry or late worker event. r[RA-ASYNC-04] r[H-RECEIPT-02]

## Canonical cases and gate

- [x] [serial] Build a source-bound WORKER-01 gate: explicit preparation in a separate process, Submit then post-cancellation-commit Cancelled transitions, all four transition controls, independent send grant, late native completion and retirement. Evidence: `verification/worker01/acceptance.json` (sha256 `b39990a865b0d8644b2f157df51dd623d35a6772bf2f5fa5c5f9be65f8428314`) at `0a66b72b`: every expected field derives from Worker reports, kernel snapshots/counters/observations and an append-only lifecycle sequence; the operation-success receipt retains a Cancelled invocation. r[RA-ASYNC-02]
- [ ] [serial] Run WORKER-09 `permitted-suspension` and `host-deadline-native-work-outstanding`, retaining a new receipt without rewriting its MW1 receipt; promote only when all ten rows have results. r[H-OUTCOME-01]
- [ ] [serial] Replay WORKER-08's immutable M6 gate and append worker-shell race-order evidence without changing its input or expected projection. r[RA-ASYNC-03]

## Milestone closeout

- [ ] [serial] Record `native-async-interfaces` gate evidence, promote WORKER-01/09, review WORKER-11, update STATUS/ROADMAP, run Cairn/spec self-tests, Octet/inventory and all fourteen main gates, then mark MW2 complete. r[RA-ASYNC-05]
