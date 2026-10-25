# MW2 Worker outcome and receipt semantics

## MODIFIED Requirements

### Requirement: H-OUTCOME-01
r[H-OUTCOME-01]

For the selected MW2 Worker shell, permitted suspension MUST retain the Pending task as the explicit owner and expose no typed output. A host deadline while native work remains pinned MUST retain host retirement ownership until native stop and cleanup; it MUST remain distinct from deterministic source work/fuel exhaustion. r[H-OUTCOME-01]

### Requirement: H-OUTCOME-02
r[H-OUTCOME-02]

At cancellation acknowledgement, an admitted native send without a terminal host observation MUST be reported as externally unknown and retain its invocation/attempt correlation. Cancellation MUST preserve prior observations and MUST NOT imply rollback. A late terminal observation MUST update the operation attempt without changing the cancelled invocation. r[H-OUTCOME-02]

### Requirement: H-RECEIPT-02
r[H-RECEIPT-02]

A confirmed late native success after WORKER-01 cancellation MAY support an operation-success receipt. The receipt MUST retain the exact attempt, plan and cancelled invocation outcome; it MUST NOT restore a guest owner/witness, authorize retry, deliver a task result or create a late worker event. r[H-RECEIPT-02]
