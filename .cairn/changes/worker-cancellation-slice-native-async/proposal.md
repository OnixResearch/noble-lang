# MW2 worker cancellation slice and native async interfaces

## Why

MW1's Worker Program v2 owns interface checking, preparation, packages, bounded synchronous execution and authority-separated observations. Its active change still has one task that mixes those finished MW1 obligations with native task ownership, cancellation, permitted suspension and a deadline while native work stays pinned. Those are MW2 obligations under RA-ASYNC-02 through RA-ASYNC-05 and must not block archiving the MW1 change or be implied by a synchronous WIT owner. r[RA-ASYNC-05]

The repository has two near-complete prototypes: production `noble worker shell` over an in-process datagram, and the independent async Component Model peer under `verification/worker-send/`. Both run the typed Submit/Cancelled transitions and kernel task/resource/authority tables, but neither is canonical WORKER-01 evidence: cancellation is committed after the Cancelled transition, the external outcome is a constant `not-attempted`, no delivery-first worker path exists, and WORKER-09's suspension/deadline-native-work rows are not run. r[RA-ASYNC-04]

## What Changes

- Select `noble-test:worker-send/worker-shell@1.0.0` as MW2's native async shell boundary and add an explicit await/result operation. The move-only future is consumed by cancellation or delivery; cancellation acknowledges revoked guest access, never native stop. r[RA-ASYNC-05]
- Integrate explicit preparation, Submit, independent send authority/admission, cancellation commit, the post-commit Cancelled transition, a late native completion and accounted retirement. Record authority observations and an operation-success receipt without changing the cancelled invocation. r[H-RECEIPT-02]
- Execute both cancellation-before-delivery and delivery-before-cancel, plus permitted suspension and a host deadline while native work remains pinned. Promote WORKER-01 and finish WORKER-09 only from source-bound receipts. r[RA-ASYNC-04]
- Define and close the `native-async-interfaces` entry gate from concrete handle disposition, await/result/error, cancellation observation and cleanup ownership. WORKER-08's accepted M6 receipt remains immutable; append worker-shell evidence rather than rewriting its case. r[RA-ASYNC-05]

## Impact

- **Files**: `worker-send.wit`, the independent peer and gate, typed worker fixtures/service handoff, optional production shell reporting, WORKER-01/09 evidence, ROADMAP/STATUS.
- **Verification**: production compiler and CLI digests, independently built async peer, all WORKER-01 transition controls, both orderings, late completion/receipt, suspension, deadline-native-work, the M6 gate replay, Octet/inventory and all fourteen main gates. r[RA-ASYNC-05]
