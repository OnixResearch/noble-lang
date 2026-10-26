## ADDED Requirements

### Requirement: VC-OWN-01
r[VC-OWN-01]

A kernel-checked ownership-accounting reference MUST select one declared small grammar with an executable checker and an independent declarative judgment, and MUST prove checker acceptance implies the judgment.

The reference MUST state that eligibility is recursive over wrapping depth, that a data reference and a must-use resource are distinct accounting classes, that dropping a data reference is not releasing a host resource, and that the opaque segment's must-use obligations are conserved. It MUST state the two-arm accounting rule and the ordinary error-return rule as separate obligations.

#### Scenario: VC-OWN-CASE-01 accounting and conservation

- GIVEN a data payload and a wrapped data payload, and a must-use resource at both depths
- WHEN the checker runs the duplicate, drop, swap and quote fragment over them
- THEN exactly the data payloads are duplicable, no wrapping depth makes the resource eligible, and no admitted program consumes the segment's must-use obligations

### Requirement: VC-OWN-02
r[VC-OWN-02]

The comparison against the production rules MUST use expectations written from the model rather than from the implementation under test, and MUST classify every corpus case as an executable production control or as model-only with a stated reason.

Every declared checker mutation MUST be caught by at least one retained expectation, and the unmutated checker MUST agree with every expectation.

#### Scenario: VC-OWN-CASE-02 mutation detection

- GIVEN the declared mutations that duplicate or release a must-use resource, ignore swap eligibility, consume a quoted operand, ignore branch accounting or admit an error return with an outstanding operand
- WHEN each mutation is run over the retained corpus
- THEN each mutation produces at least one mismatch and the unmutated checker produces none

### Requirement: VC-OWN-03
r[VC-OWN-03]

Receipts MUST bind the model, the toolchain, the case file and the tested implementation to a clean source commit, and MUST state the correspondence route and every remaining open edge and coverage limit.

The reference MUST NOT claim an extraction or refinement proof it does not have, MUST NOT count model-only cases as executable controls, and MUST NOT claim host resource release or whole-compiler safety.

#### Scenario: VC-OWN-CASE-03 receipt scope

- GIVEN the retained receipts
- WHEN their scope statements are read
- THEN they name the clean commit, the Lean and Rust pins, the host and target, the case-file and lane digests, the model-only classifications and the open Rust-to-model edge
