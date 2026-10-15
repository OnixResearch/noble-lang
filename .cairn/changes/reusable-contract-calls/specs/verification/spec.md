# Reusable-call correspondence obligations

## ADDED Requirements

### Requirement: PREUSE-EVID-01
r[PREUSE-EVID-01]

Revision-3 reusable-call and law-bundle support MUST record each of the following as an open obligation, separately from finite scenario results. Each obligation is discharged only by its own proof or refinement evidence:

- **Source-to-logical call translation:** each authenticated occurrence maps to exactly one modeled call with exact slot, provenance and instantiation.
- **Claim regeneration:** regenerated claims match the elaborated source contracts.
- **Checked rule application:** the Rust symbolic and bridge checker establishes exactly the reviewed chain-rule premises.
- **Replay soundness:** an accepted envelope yields the exact theorem.
- **Model lemmas:** environment embedding and lifting from MC1 to the named model.
- **Erasure:** recipe, DefinitionId and Wasm are unchanged.
- **Model adequacy.**
- **Executed-artifact correspondence.**

Passing scenarios, a successful Lean compilation, document validation or a bundle record MUST NOT be reported as discharging any of these obligations.
