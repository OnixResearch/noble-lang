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

### Requirement: IMPL-GRAMMAR-01
r[IMPL-GRAMMAR-01]

OWNER DECISION 2026-11-20: implicit arguments are spelled `(infer)` (decision 13).

`Intrinsic-Proofs-Draft` source proof terms MUST admit the nullary form `(infer)` only when the declaring proof opts in, and the opt-in MUST be recorded with the declaration. The selected spellings are:

- The opt-in is the word `infer` between the proof name and its claim: `proof 1 NAME infer : [PROPOSITION] [TERM]` or `proof 1 NAME infer for CONTRACT [TERM]`. Any other word there MUST reject with a diagnostic naming the permitted forms.
- `(infer)` is admissible only as the argument of `apply`, as an argument of an `app` spine, or as the single argument of `exec-literal`. Every other position MUST reject with a diagnostic naming that position: whole proof term, `use` target, application head, `refl` operand, `intro` binder name or sort, `subst` binder, motive or evidence, and any other rule argument.
- A proof that does not opt in MUST reject any `(infer)` or `app` form with a diagnostic naming the opt-in requirement, before checking.
- The explicit proof core MUST continue to reject the abbreviations: acceptance consumes only the fully explicit elaborated term, and `check_batch` without elaboration refuses `(infer)` exactly as before. Revision-1 and revision-2 terms without abbreviations keep their meaning.

`exec-literal (infer)` takes the literal the trusted subject preparation extracts from the exact prepared subject; it never searches the module, imports or the local context.

#### Scenario: IMPL-CASE-21 selected positions are accepted

- GIVEN an opted-in proof whose `apply` arguments, `app` spine and `exec-literal` use `(infer)` at the selected positions
- WHEN the module is prepared
- THEN exactly those placeholders elaborate, and the explicit checker accepts the fully explicit term

#### Scenario: IMPL-CASE-22 other positions are refused by name

- GIVEN an opted-in proof with `(infer)` as a whole term, a `use` target, an application head, a `refl` operand, an `intro` binder name or sort, or a `subst` motive
- WHEN the module is prepared
- THEN preparation refuses at that span with a diagnostic naming the position, and no proof is accepted, assumed or refuted
