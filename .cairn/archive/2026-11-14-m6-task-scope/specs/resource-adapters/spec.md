## ADDED Requirements

### Requirement: RA-GROUP-01
r[RA-GROUP-01]

A host scope MUST be bounded at two children and MUST be identified by a
host-issued scope identity that no copied handle, callback, snapshot, decision,
slot index, generation or serialized descriptor can substitute for. Membership
MUST be retained as an ordered list of child references (scope identity, slot,
generation), appended in attachment order, and MUST be granted only through the
scope's own move-only admitted child value. Duplicate attachment, attachment of a
child from a foreign table or scope, attachment of a stale generation, attachment
after close and fabricated membership MUST be refused, and a refusal MUST leave
both the membership and the child's task unchanged. The scope MUST remain a
host-side composition layer: the kernel table's per-task state machine is not
changed and no new kernel surface is added.

#### Scenario: a copied handle does not attach

- GIVEN a child admitted into a scope whose move-only value has already been
  consumed by attachment, and a copied descriptive handle naming the same slot
  and generation
- WHEN attachment is attempted through the copied handle
- THEN it MUST be refused and the retained membership MUST NOT change

#### Scenario: stale and post-close attachment are refused

- GIVEN a scope holding a live child and a task naming an older generation of
  that slot, and separately a scope that has already closed
- WHEN attachment is attempted with the stale task, and with a valid task after
  the close
- THEN both MUST be refused, and the refused task MUST remain usable by its
  current owner

### Requirement: RA-GROUP-02
r[RA-GROUP-02]

Scope admission MUST be incremental, one child at a time, because the kernel's
`Admission` value holds the table exclusively; no more than one scope-issued
child value may await attachment, and whole-group preflight MUST NOT be claimed
or implied. Dropping an unattached scope-issued value MUST transfer its
retirement to the host registry and leave the scope fail-closed. A refused
admission MUST preserve every previously committed
child as a member with its custody, reservations, pins and native obligations
unchanged, and MUST return every caller-owned input unchanged. A refused or
failed later admission MUST NOT be described as rollback of native work, of a
consumed authorization, or of a committed child. Each admitted child's footprint
MUST be charged once by the existing `Request`/`Limits` accounting, and the scope
MUST NOT introduce a second accounting of the same capacity.

#### Scenario: the first child survives a refused second admission

- GIVEN a scope holding one committed child and a table whose remaining capacity
  or generation budget cannot admit the second child
- WHEN the second admission is attempted and refused
- THEN the first child MUST remain a member, its accounting and retirement
  obligations MUST be unchanged, and the caller's second input MUST be returned
  unchanged

#### Scenario: no whole-group preflight

- GIVEN a host that would prefer to preflight both children together
- WHEN the scope's admission policy is inspected
- THEN it MUST be incremental and MUST NOT claim atomic group admission, because
  the second preparation cannot exist while the first holds the table

### Requirement: RA-GROUP-03
r[RA-GROUP-03]

The scope MUST report child results in attachment order regardless of the order
in which the children complete, and MUST NOT report a result the kernel table has
not delivered. The scope's primary failure MUST be the first abnormal outcome in
the host's serialized order and MUST stick: a later success, a later failure, a
late completion of a cancelled child and a duplicate callback MUST NOT replace it.
A cancellation acknowledgement MUST be recorded as neither a result nor a
settlement. Membership MUST be released exactly once per member, and a repeated
release MUST be refused without changing accounting.

#### Scenario: attachment order, not completion order

- GIVEN two members attached in one order whose completions arrive in the
  opposite order
- WHEN the scope reports its results
- THEN the reported order MUST be the attachment order, and the same results MUST
  be reported when the completion order is reversed

#### Scenario: the sticky primary failure

- GIVEN a member that fails abnormally and a later member success, and separately
  a cancelled child whose late completion arrives afterwards
- WHEN the scope selects its failure
- THEN it MUST retain the first abnormal outcome, and the late completion MUST
  NOT deliver a result or replace that failure

### Requirement: RA-GROUP-04
r[RA-GROUP-04]

Each undelivered child MUST be retired on cancellation, deadline, trap and host
failure, and a native cancellation request, a native-stop observation, pin
settlement and cleanup MUST remain separate decisions rather than one shielded
operation. An unsettled child MUST keep the scope unsettled. Dropping the scope
wrapper MUST NOT invent cleanup: the retained host owner (the scope registry)
MUST inherit the outstanding retirement, because dropping a task value performs no
cleanup in the kernel. The host-retained input payload MUST remain owned while
cleanup is outstanding and MUST be dropped only after the table accepts cleanup;
the host registry MUST inherit that payload if the wrapper is dropped earlier. If
the table snapshot confirms an empty pin set after native stop, the host MUST
record the empty settlement without issuing a zero-mask table operation, and MUST
still acknowledge cleanup separately.

#### Scenario: cancellation is not settlement

- GIVEN a member cancelled before its result was delivered, with native work still
  running and pins outstanding
- WHEN the scope reports its state
- THEN it MUST report the retirement and MUST NOT report settlement until the
  native stop, pin settlement and cleanup obligations are discharged

#### Scenario: a dropped wrapper leaves retirement to the host

- GIVEN a scope wrapper dropped while one member still owes native stop, pins or
  cleanup
- WHEN retirement is asked for afterwards
- THEN the host scope registry MUST own it, and no cleanup MUST be reported as
  complete merely because the wrapper no longer exists

#### Scenario: cleanup releases the host input only when accepted

- GIVEN a child whose input payload remains host-owned while native stop, pins or
  cleanup are outstanding
- WHEN table cleanup is refused or succeeds
- THEN refusal MUST preserve the payload, and successful cleanup MUST release it

#### Scenario: an empty pin set needs no fabricated table release

- GIVEN a native-stopped child whose table snapshot has no outstanding pins
- WHEN the host acknowledges pin settlement
- THEN it MUST record the empty settlement without changing table accounting, and
  cleanup MUST remain a separate required step

### Requirement: RA-GROUP-05
r[RA-GROUP-05]

Final close MUST require every member to satisfy the existing M6 finalization
contract — a successful `finish` after native stop, pin settlement and complete
cleanup — together with settled membership accounting; otherwise the close MUST be
refused. A repeated close after a successful close MUST be idempotent and MUST NOT
release accounting a second time. Close MUST NOT manufacture completion, delivery,
native stop or a result.

#### Scenario: early close is refused

- GIVEN a scope with a member that has not passed `finish`, or whose membership
  accounting is not settled
- WHEN the scope is closed
- THEN the close MUST be refused and the scope MUST remain able to settle

#### Scenario: repeated close is idempotent

- GIVEN a scope closed after every member finalized
- WHEN close is requested again
- THEN it MUST succeed idempotently, report the same settled state and change no
  accounting

### Requirement: RA-GROUP-06
r[RA-GROUP-06]

The scope policy MUST be exercised by a finite bounded model — one scope, at most
two children, a declared action alphabet and a declared depth — compared against
an adapter driving the actual Rust M6 table API and reusing the completed #46
interleaving study. Every result MUST identify whether it establishes scope
policy, transition composition or adapter correspondence, and model proofs,
runtime tests and host assumptions MUST remain separately identified. Evidence
MUST bind the source revision, the scope policy, the model revision and the
declared bounds. No eventual native completion, cleanup after host death,
fairness or liveness is claimed, and a model obligation MUST NOT be reported as an
adapter guarantee.

#### Scenario: model and adapter are labelled separately

- GIVEN a scope trace proved in the model and the same trace replayed through the
  adapter
- WHEN the evidence is reported
- THEN the model obligation and the adapter replay MUST be reported as separate
  results, and neither MUST be described as the other

#### Scenario: the bound is declared with the evidence

- GIVEN retained evidence for the scope policy
- WHEN its bindings are inspected
- THEN the source revision, the change revision, the model module and the
  declared alphabet and depth MUST be recorded
