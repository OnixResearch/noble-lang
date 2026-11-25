# Live-reference lifetime design evidence

## ADDED Requirements

### Requirement: LREF-EV-01
r[LREF-EV-01]

Every LREF-CASE scenario design MUST remain `implementation: absent`, `execution: not-run`, `proof: open`, `trust: unassessed`, `evidence: []` until a separate implementation and actual execution or proof are independently recorded. LREF evidence MUST NOT promote or inherit LSLOT, LIVE or OWN case status, and LSLOT evidence MUST NOT be presented as LREF evidence.

Later evidence MUST bind the following, in addition to the `EV-LSLOT-01` fields:

- every abnormal-exit path and its pin-release counts;
- every exhausted counter value and refusal;
- the package members and owner kinds retained per version;
- the retirement step that reclaimed each charge.

Diagnostic receipts from the experimental live-slot CLI MUST NOT be copied into canonical case evidence.

#### Scenario: LREF-CASE-06 design state stays unexecuted

- GIVEN the LREF-CASE-01..06 designs and the LSLOT mapping recorded in the live-reference lifetime design
- WHEN the change is validated without implementation or execution
- THEN every LREF case remains absent/not-run/open/unassessed with no evidence, and no LSLOT case status changes
