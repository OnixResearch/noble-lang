## ADDED Requirements

### Requirement: WREF-STEP-01
r[WREF-STEP-01]

The bounded Wasm correspondence lane MUST define Mathlib-free small-step
semantics for the declared emitted instructions and reached helper bodies.
It MUST relate typed source stacks to Wasm state, including untouched tails,
and distinguish normal results from faults and profile exhaustion. Excluded
opcodes, helpers, profiles and resource states MUST be listed.

#### Scenario: Ordered arithmetic with a preserved tail

- **GIVEN** a checked opt-off pure I64 subtraction quotation and a nonempty tail
- **WHEN** the bounded step relation executes the lowered body with sufficient quota
- **THEN** the result is left minus right modulo 2^64 and the tail is unchanged

### Requirement: WREF-EMIT-01
r[WREF-EMIT-01]

The lowering theorem MUST refer to the pinned Rust emitter's extracted
structured output and an independently stated `Exec` relation. The admitted
subset, helpers, input premises, and exact recipe MUST bind to the same subject.
The theorem MUST be non-vacuous. Tests and extraction success alone MUST NOT
count as simulation evidence.

#### Scenario: Body or recipe changes below the same claim

- **GIVEN** a checked pure I64 quotation and an independently stated execution claim
- **WHEN** the compared body or its reflected recipe is independently substituted
- **THEN** the changed subject does not supply the original correspondence premise

### Requirement: WREF-CHAIN-01
r[WREF-CHAIN-01]

Receipts MUST distinguish structured lowering, helper semantics, text and
binary encoding, loading of a specific export, and optimizer preservation.
Artifact-level claims MUST require a checked chain over the same inputs and
loaded bytes. Missing edges MUST be recorded as open or explicit assumptions;
passing identity checks MUST NOT discharge semantic obligations.

#### Scenario: Structured proof with the loader edge still open

- **GIVEN** a passing structured lowering proof and an open encoding or loader edge
- **WHEN** that scoped result is recorded in the verification ledger
- **THEN** no executed-artifact proof or full PO-17/PO-18 closure is inferred
