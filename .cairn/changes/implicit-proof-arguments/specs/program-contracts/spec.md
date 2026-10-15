# Bounded implicit proof-argument elaboration

## ADDED Requirements

### Requirement: IMPL-ELAB-01
r[IMPL-ELAB-01]

An opt-in, revision-recorded proof elaborator MAY omit arguments only in three cases:

- `Type0` or value arguments of an exactly resolved proof application.
- The `Type0` prefix of an explicit-spine application.
- Structural parameters of a reviewed template rule that are fixed by the exact prepared subject.

The arguments are reconstructed solely from these sources: the head's signature, the supplied arguments' sorts, inferable explicit premise arguments, the expected goal and the trusted subject preparation. The elaborator solves first-order equations over `PureTyCode` terms, value levels and `Pi`/`Eq` propositions by syntactic unification with an occurs check. The only definitional equality is the checker's structural identity.

An elaboration is unique only when the most general unifier assigns every metavariable a ground term whose free levels lie within the application context. If any of the following holds, the elaborator MUST refuse at the source span with distinct categories and an explicit-argument request:

- Underconstrained, conflicting or ill-scoped solutions.
- Premise-witness placeholders, motive inference or partial application.
- Exhaustion or internal failure.

The refusal MUST report the unresolved binder, the selected head identity and the originating constraints. Candidates MAY be listed but MUST NOT be chosen. The elaborator MUST NOT search the local context, imports, bundles or any global instance table, MUST NOT apply a heuristic default or tie-break, and MUST NOT let import order influence the result.

#### Scenario: IMPL-CASE-01 polymorphic reflexivity abbreviation

- GIVEN an accepted exported `poly-refl : (Pi (A Type0) (Pi (x A) (Eq A x x)))` and the goal `(Pi (x (List I64)) (Eq (List I64) x x))`
- WHEN the spine application with implicit `A` and explicit `x` is elaborated
- THEN it expands to `(intro (x (List I64)) (apply (apply (use alias.poly-refl) (List I64)) x))`, and the expanded term is checked against the same declared claim as the hand-written explicit term

#### Scenario: IMPL-CASE-06 two applicable local proofs

- GIVEN two local premises `h1` and `h2`, both of type `(Eq I64 x x)`, and a premise placeholder at a binder requiring `(Eq I64 x x)`
- WHEN elaboration runs
- THEN it refuses with `unsupported: premise inference`, lists `h1` and `h2` with their spans without choosing either, and refuses identically when only one candidate exists

### Requirement: IMPL-TRUST-01
r[IMPL-TRUST-01]

The proof elaborator MUST be an untrusted producer.

- **Acceptance.** Only its fully explicit output is accepted. That output MUST pass the existing explicit source proof checker, which independently revalidates every inserted `Type0` code, value sort and substitution, and then the independently pinned Lean consumer against the separately regenerated exact expected claim. A missing proof witness, bridge or join obligation, or an unresolved metavariable, MUST NOT become an assumption, axiom or accepted hole.
- **Budget.** Constraint generation, unification, substitution, normalization, dependency traversal, term expansion, memory and diagnostic output MUST be charged to the same bounded admission request as checking. Exhaustion MUST be reported distinctly and never as acceptance or refutation.
- **Scope.** Elaboration MUST NOT run during ordinary `quote`/`compose`/`run`, runtime evidence replay or admission.
- **Identity.** An inferred logical equality or behavioral law MUST NOT establish Program, recipe or content-hash identity, and MUST NOT authorize evidence reuse for a different subject.

#### Scenario: IMPL-CASE-10 forged inferred argument

- GIVEN an abbreviated proof whose producer-supplied expansion or elaboration record claims a different inserted code than the deterministic elaborator derives
- WHEN the submission is checked
- THEN the explicit checker refuses any ill-typed inserted argument, source-binding re-elaboration refuses a byte mismatch, and no record field is used as authority

#### Scenario: IMPL-CASE-18 elaboration budget exhaustion

- GIVEN a head with 200 nested `Type0` binders and a work, normalization or substitution budget just below the requirement
- WHEN elaboration runs
- THEN it reports exhaustion naming the stage, accepts nothing and reports no refutation
