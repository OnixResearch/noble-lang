## ADDED Requirements

### Requirement: VC-REF-01
r[VC-REF-01]

A kernel-checked ServiceRef reference MUST specify the admitted value type, the exact byte grammar, the name alphabet and the limit parameters, and MUST prove value round-trip, canonicality and full-input consumption as named theorems.

The model MUST state that encoding is a function of the admitted value, that invalid names and unadmitted configured depths are explicit schema and depth failures, and that the decoder consumes the whole input rather than promoting a prefix decoder to a document decoder.

#### Scenario: VC-REF-CASE-01 round trip and canonicality

- GIVEN an admitted name and readiness tag and an admitted configured depth
- WHEN the model decodes the canonical encoding produced from that value
- THEN the decoded value is that value, and every accepted byte sequence is exactly the canonical encoding of the value it decodes to

### Requirement: VC-REF-02
r[VC-REF-02]

The comparison against the Rust codec MUST use expectations that are independent of the implementation under test, and MUST cover the alphabet edges, the name and wire bounds, the configured depths, truncation at every offset, appended bytes, one-byte substitutions, invalid UTF-8 and seeded generated values.

Where the model and the implementation decide differently, the difference MUST be reported as a divergence rather than pinned as agreement.

#### Scenario: VC-REF-CASE-02 boundary and mutation coverage

- GIVEN the generated case file written from the Lean model
- WHEN the Rust decoder is run against every case
- THEN each decision matches its independent expectation, except the documented bracket-scan divergence, which the lane asserts as a divergence

### Requirement: VC-REF-03
r[VC-REF-03]

Receipts MUST bind the model, the toolchain and the tested implementation to a clean source commit, and MUST state the correspondence route and every remaining open edge.

The reference MUST NOT claim an extraction or refinement proof it does not have, and MUST NOT claim whole-parser security, zero-copy performance or whole-compiler behaviour.

#### Scenario: VC-REF-CASE-03 receipt scope

- GIVEN the retained receipts
- WHEN their scope statements are read
- THEN they name the clean commit, the Lean, Mathlib and Rust pins, the host and target, the case-file digest and the open Rust-to-model edge
