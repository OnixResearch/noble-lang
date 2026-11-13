## Phase 1: Portable belt contract and implementation

- [x] Define the bounded I64/Bool graph fragment, artifact header and binding,
  explicit save/load semantics, and refusal behavior.
- [x] Add the symbolic Lean belt machine, checker relation, and
  `validate_sound` theorem; register the module and corpus.
- [x] Add the opt-in Rust lowerer, artifact codec, independent checker,
  checker-issued byte-borrowing token, and checked interpreter.
- [x] Exercise wraparound, asymmetric subtraction, equality, fanout,
  long-lived values, repeated/multiple outputs, capacity boundaries, malformed
  and mutated artifacts, and unsupported graph refusals in existing targets.
- [x] Compare actual lowered artifacts with the graph reference, packed
  `Compiled`, and exact I64 expectations.
- [x] Add the extracted-source B2 belt comparison against BigInt and the
  configured ordinary-Wasm path.
- [x] Add generated Lean corpus freshness coverage and ignored cold/warm cost
  reporting with storage/preservation counts.
- [x] Record target-neutral scope, evidence boundaries, and non-claims.

## Phase 2: Independent repository integration

- [ ] Integrator regenerates and reviews shared source-bound evidence, then
  runs the clean-tree documentation, Cairn, Rust, Octet, and Lean gate set.
- [ ] Integrator merges this slice and posts the issue result.

## Out of scope

- [x] No production-default backend, Mill ISA mapping, target scheduler, or
  hardware/performance claim is selected by #113.
