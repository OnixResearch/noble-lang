# Host-retained Directory lifecycle

## MODIFIED Requirements

### Requirement: RA-STATE-01
r[RA-STATE-01]

A `Live` to `Busy` transition MUST validate kind, context, generation, and rights before protected work. It consumes guest availability atomically with scope registration. In the selected bounded-region adapter the host MUST also validate signed nonnegative and overflow-free offset/length against the retained byte extent and output quota before taking that borrow/native pin. The host MUST retain the resource through `begin`, validate native access, read only the checked range and `complete` it before the guest's explicit release; invalid arithmetic or claims MUST leave the protected read count at zero.

The separate S-CASE-06 filesystem adapter retains its logical `Directory` owner over one preopened regular File in the host (no guest borrow, WIT Directory representation or OS directory preopen). It MUST validate that host table's owner identity, invocation context, kind, generation and liveness, and independent host-granted READ with `Table::validate`, plus the exact preopened-file key `main.rs`, before a protected file-content read. This retained Table right is the selected authorization boundary; it does not claim use of the service-specific `Authority::authorize_from_trusted_host` one-shot Plan/facts. A denied authorization, invalid owner or unmapped key has no protected read or native pin. For an approved read, the host begins its protected table scope, keeps the owner/file alive and returns at most 4096 bytes; it MAY read at most one extra bounded sentinel byte to reject an oversized file without returning those bytes. The host completes the scope and accounts for the operation before retiring the owner; neither a live owner nor a valid key is sufficient authority without READ.

#### Scenario: S-CASE-06 refused and granted file reads

- GIVEN the canonical live host-owned Directory with no READ grant and one vetted regular file at exact guest key `main.rs`, plus a separate READ-granted invocation
- WHEN the same compiled guest requests `fs.read` through the production host
- THEN the canonical invocation denies before protected content access, whereas the independently granted one returns only that preopened file's at-most-4096 bytes and retires its owner

### Requirement: RA-CLEAN-01
r[RA-CLEAN-01]

Abnormal cleanup MUST not depend on guest continuation. The host retires all invocation-owned obligations under the execution profile. It does not return a guessed pre-call session stack. For S-CASE-06 the host, not the guest, retains the live `Directory` throughout the invocation. Normal denied/successful result, domain error and guest trap MUST each settle that owner's custody or retain explicitly accounted cleanup debt until safe release; repeated cleanup, late callback and invalid claims MUST NOT release twice, revive a retired owner or gain a protected file read.

#### Scenario: Host Directory settlement on refusal and completion

- GIVEN an invocation-owned host Directory with no READ and a separately granted invocation of the same compiled guest
- WHEN a denied result, successful bounded read or abnormal guest exit completes
- THEN each owner's retirement is accounted exactly once and no later duplicate or stale event regains read authority
