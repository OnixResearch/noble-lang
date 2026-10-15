# Ownership-safe reclamation and scratch reset

## ADDED Requirements

### Requirement: OWN-RECLAIM-01
r[OWN-RECLAIM-01]

A guest storage reclamation algorithm that does not trace from roots MUST count every owning edge kind before releasing storage. The edge kinds are:

- heap child edges;
- operand and continuation slots;
- frame pins and held `dip`/borrow-scope owners;
- saved-owner tokens and module roots;
- registry map or root pins;
- code-to-global and code-to-table dependency edges of executable packages.

Its acyclicity and lifetime argument MUST use a logical birth or admission order that is never recycled. Numeric handles that may be reused, content hashes, or immutability alone MUST NOT suffice. An intra-package self reference MUST NOT be counted as an owner edge. Owner counters and handle epochs MUST have fixed width and MUST refuse or fail closed at saturation rather than wrap. Collection MUST run only when no uncounted guest or host reference can exist. Storage reclamation MUST NOT discharge, perform or imply any host-resource release.

#### Scenario: OWN-CASE-02 shared duplicates released in either order

- GIVEN two saved owners of one shared Program released in each order with an invocation between releases, and the counter-saturation variant of OWN-CASE-10
- WHEN each owner is released and collection runs
- THEN the surviving owner keeps working, storage is reclaimed only after the last owner releases, and saturation refuses without wrapping

### Requirement: OWN-SCRATCH-01
r[OWN-SCRATCH-01]

An invocation-local scratch region MUST NOT be reset, rolled back or compacted while any retained owner references a cell in it. Retained owners include root outputs, saved owners, module roots, map pins, frames and held owners. Every value crossing an escape boundary MUST be promoted, transferred or allocated as retained storage before the boundary completes. Escape boundaries include root outputs left in session storage, saved-owner issue, host park, live-slot candidate staging and export. On trap, cancellation or failed construction, the host MUST release frame and map pins and retire invocation-owned resources exactly once without resuming the guest. It MUST issue no saved owner for an output of the failed invocation, and retained packages MUST remain unchanged.

#### Scenario: OWN-CASE-05 trap during an escaping save

- GIVEN a root that saves a Program and then traps before returning, and the escaping-capture reset variant of OWN-CASE-08
- WHEN the host performs abnormal cleanup and any scratch reset
- THEN no saved owner is issued, pins are released exactly once, retained Programs read unchanged captures, and no retained owner reaches reset storage
