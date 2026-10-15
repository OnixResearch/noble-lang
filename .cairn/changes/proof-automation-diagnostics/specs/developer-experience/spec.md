# Proof goal and context view

## ADDED Requirements

### Requirement: PAUTO-GOAL-01
r[PAUTO-GOAL-01]

A proof goal/context view MUST present, at a source-located site:

- The independently generated expected claim and the exact subject/module revision.
- The complete ordered input, intermediate and output stacks, including the preserved bottom tail.
- Typed local values, proof premises, explicit assumptions and the model/profile.
- At a failed composition, the precise remaining bridge or result-join obligation, with both incompatible predicates or stacks.
- Inserted arguments and the selected rule/theorem identities.

Every displayed item MUST link to its source span or report that provenance is unavailable.

**Status.** Status indicators MUST derive only from the independent checker for the exact statement digest. They MUST distinguish checked, incomplete/unknown, unsupported, refuted, exhausted/timeout, internal failure and stale, and MUST retain the original claim and assumptions. A checked normal-return partial-correctness result MUST NOT be presented as termination, artifact correspondence or host authorization. Tested-only evidence MUST be shown separately and never changes the outcome.

**Holes.** A proof hole MAY exist only in editor transport. It MUST NOT count as an accepted theorem or satisfy proof-required admission.

**Staleness.** A change to source, subject, contract, statement, transitive dependency, reviewed library or policy MUST mark prior status stale without deleting saved immutable evidence for its original subject.

**Output bounds.** Diagnostic output MUST be bounded and MUST report truncation explicitly.

#### Scenario: PAUTO-CASE-11 stale status after subject change

- GIVEN an accepted `increment-correct` result for `increment [ 1 + ]`
- WHEN the subject changes to `[ 2 + ]` or a `use` dependency's model or checker revision changes
- THEN the site shows `stale` rather than `checked` or `refuted`, the old evidence remains listed for its original subject only, and no Lean check runs until explicitly requested

#### Scenario: PAUTO-CASE-07 checker timeout distinct from falsity

- GIVEN the PAUTO-CASE-01 template candidate checked with a 1 ms Lean deadline
- WHEN the consumer deadline expires
- THEN the outcome is `timeout` with the checker pool named, the claim and assumptions remain displayed, and no refutation is reported
