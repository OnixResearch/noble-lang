# Inspectable proof elaboration

## ADDED Requirements

### Requirement: IMPL-EXPOSE-01
r[IMPL-EXPOSE-01]

For every abbreviated proof, tooling MUST provide an inspect/copy-expanded form that shows:

- Each inserted argument, with its source span, binder name and position, and the constraints it originated from.
- The exact selected theorem or rule identity, with model and checker revisions.
- The instantiated claim and assumptions, and the transitive dependencies.
- The complete explicit proof term.

Fully explicit authoring MUST remain available unchanged.

**Determinism.** Elaboration MUST be deterministic for identical source bytes, resolved namespace snapshot, elaborator revision and limits. Any inference behavior change that can alter an expansion or a refusal MUST use a new recorded elaborator revision.

**Source mapping.** Every explicit node MUST map to the abbreviated source span that produced it.

**Bounded diagnostics.** Diagnostics MUST be bounded and MUST report truncation explicitly.

#### Scenario: IMPL-CASE-17 deterministic expansion

- GIVEN the same abbreviated module source, resolved snapshot, elaborator revision and limits, with dependency modules supplied in different orders
- WHEN elaboration is repeated across separate processes
- THEN the explicit terms and elaboration records are byte-identical and every inserted node maps to its originating span

#### Scenario: IMPL-CASE-19 bounded refusal diagnostics

- GIVEN a refused elaboration with 1,000 constraints and 50 listed candidates
- WHEN the diagnostic is rendered
- THEN it is truncated to the configured constraint, candidate and byte caps with an explicit truncation count, and the refusal category is unchanged
