# MW2 native async worker shell

## MODIFIED Requirements

### Requirement: RA-ASYNC-02
r[RA-ASYNC-02]

For WORKER-01 the independently authorized send plan, witness consumption, attempt registration, resource transfer and task admission MUST remain separate obligations at one serialized boundary. A failed preflight starts no native work and returns every owner. After commit, failure enters retirement and never restores authority. r[RA-ASYNC-02]

### Requirement: RA-ASYNC-03
r[RA-ASYNC-03]

The selected Worker shell MUST use the production task table's complete state/event decision matrix. The integrated cancellation-before-delivery and delivery-before-cancel modes add worker-service evidence without changing the accepted WORKER-08 projection or M6 receipt. r[RA-ASYNC-03]

### Requirement: RA-ASYNC-05
r[RA-ASYNC-05]

For the selected MW2 Worker shell, `noble-test:worker-send/worker-shell@1.0.0` MUST define a move-only future/task disposition and explicit asynchronous finish operation returning `result<s64,string>`. Cancellation or finish consumes the guest handle; cancellation acknowledgement means guest access is revoked while native cleanup remains host-owned. The host MUST report task/resource ownership, pins, attempt outcome, invocation outcome and cleanup disposition at each boundary. r[RA-ASYNC-05]

A permitted suspension MUST retain a Pending task as the owner and expose no typed output. A host deadline while native work remains pinned MUST retain host retirement ownership until native stop, resource completion, pin settlement, cleanup and finish. Store or handle drop is not native-stop evidence. r[H-OUTCOME-01]

#### Scenario: native async Worker interface

- GIVEN a checked pure Worker transition that returns one inert Send action, an independent exact send grant, a bounded native operation and the selected async component shell
- WHEN cancellation-before-delivery, delivery-before-cancel, suspend-awaiting-send and deadline-with-native-outstanding run
- THEN each handle is consumed exactly once; preflight, Pending, Retiring/Ready and Retired ownership and pins match the selected table; cancellation-first never delivers, delivery-first delivers once, suspension retains the task owner, and deadline retains host retirement ownership until cleanup

### Requirement: RA-ASYNC-04
r[RA-ASYNC-04]

For WORKER-01, the shell MUST commit cancellation before supplying the corresponding Cancelled event to the worker. At acknowledgement the task MUST be Retiring with one native pin and the operation outcome externally unknown. A late native success MAY produce an operation receipt, but the invocation MUST remain cancelled; it MUST NOT deliver a result, restore an owner/witness, retry, roll back the external action or emit a second worker event. r[H-RECEIPT-02]

#### Scenario: WORKER-01 integrated cancellation order

- GIVEN candidate source supplied after coordinator compilation, a separate explicit preparation service, the exact State/Event/Action schema, initial state count zero and independently authorized send authority
- WHEN Submit returns Send(attempt-1,"ping"), the shell admits and starts it, commits cancellation, runs Cancelled(attempt-1), then receives late native success
- THEN the states/actions and all WORKER-01 expected ownership, pin, delivery, event, observation and rollback fields match; preparation and evaluation traces stay separate and worker evaluation emits zero host requests
