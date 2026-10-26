# Independent finite-model effect-oracle design

## ADDED Requirements

### Requirement: FXORACLE-SEM-01
r[FXORACLE-SEM-01]

An opt-in `finite-effects/v1` reference semantics SHALL interpret the existing finite effect-constraint grammar without changing production acceptance. It MUST use an independently represented finite universe, exact set union, explicit equations and rigid binders.

For a concrete system (no rigid node or goal), the reference MUST define an assignment over `Constant`, `Hole` and `Union` nodes with `A(const S) = S`, `A(union a b) = A(a) ∪ A(b)` and `A(a) = A(b)` for every equation, and MUST interpret the solver's result as the unique greatest assignment. Satisfiability MUST be determined by bounded exhaustive enumeration. A satisfying assignment exists exactly when the system is satisfiable, and because the solution set is closed under pointwise union the greatest assignment is unique. The reference MUST report `inconsistent` for a system with no assignment.

For the rigid lane, rigid variables MUST denote arbitrary subsets of the universe, not narrowable holes. A rigid valuation `ρ` and a hole assignment `H` MUST be separate. The reference MUST first establish rigid consistency for every enumerated `ρ` and MUST report `rigid-inconsistent` when some `ρ` has no equation-satisfying `H`. It MUST report `goals-valid` only when every enumerated `ρ` and every equation-satisfying `H` satisfies every rigid goal, and `goal-counterexample` with the first violating witness otherwise. An inconsistent system MUST NOT be reported as a vacuously valid implication.

The supported fragment MUST require an acyclic union graph, in-range references and a declared cap on universe size, holes, nodes, equations, rigid binders and goals. Cyclic or otherwise unrepresentable systems MUST be reported `unsupported-reference` with no semantic verdict. The reference MUST NOT claim mathematical completeness for the production symbolic pass: a production refusal of a semantically valid goal is a conservative-refusal characterization unless the case is in the explicitly declared symbolically exact subfamily.

#### Scenario: FXORACLE-CASE-01 shared hole, pure and host constraints

- GIVEN a concrete system with universe `{emit}`, holes `h0,h1`, a zero constant and an equation set that forces `h0` and `h1` to the empty set plus a host constant on one branch
- WHEN the production solver and the reference enumerator both run with sufficient budget
- THEN production accepts, the complete final bound table equals the reference greatest assignment, and no host bit is lost

#### Scenario: FXORACLE-CASE-09 inconsistent rigid goal

- GIVEN a rigid system whose equations force two distinct constants to be equal and whose rigid goals would hold in every assignment of the remaining nodes
- WHEN the reference enumerates rigid valuations and hole assignments
- THEN it reports `rigid-inconsistent` and never `goals-valid`

### Requirement: FXORACLE-DIFF-01
r[FXORACLE-DIFF-01]

The oracle MUST be an independent test-only checker. It MAY construct production arenas through their public effect constructors and read the final bounds, but it MUST NOT use production narrowing, symbolic propagation, the production pass driver or `EffSet` helpers to compute an expected result. Expected assignments, satisfiability, rigid validity and counterexamples MUST be computed in the oracle's own representation.

Production outcomes `accepted`, `invalid`, `unsupported`, `exhausted` and `internal` MUST be recorded separately from reference outcomes. The oracle MUST classify at least:

- acceptance with a satisfiable reference and an equal complete assignment as `agree-accept`;
- acceptance with a different assignment as `wrong-assignment`;
- acceptance with an inconsistent or counterexample reference as `unsound-accept`;
- `invalid` with an inconsistent, counterexample, unsupported or exhausted-reference context as the matching refusal class;
- `invalid` with a semantically valid reference as `conservative-refusal`, not an automatic defect;
- `exhausted` as neither acceptance nor semantic rejection;
- an out-of-fragment or reference-exhausted case as `oracle-unavailable`/`oracle-exhausted`, not as a defect.

A production `Exhausted` run MUST never be counted as a semantic rejection, and a low-work run MUST NOT be used for the semantic comparison. Every case MUST also run at a sufficient budget and MUST compare every final effect bound, not only `effect_value` of a selected program effect. The declared symbolically exact subfamily MUST agree; a refusal inside that subfamily is `defect-missed-supported-case`.

#### Scenario: FXORACLE-CASE-06 non-neutral union does not propagate

- GIVEN an equation between two union sides that both contain distinct holes
- WHEN the solver and the reference run
- THEN neither side may be narrowed by attributing the other side's effects; production and reference agree or the discrepancy is classified, and the oracle never reports a spurious host bit

#### Scenario: FXORACLE-CASE-18 low-work exhaustion

- GIVEN a satisfiable concrete case and a work budget below its measured need
- WHEN the low-work lane runs and then the sufficient-budget lane runs
- THEN the low-work outcome is `exhausted` and is not recorded as rejection, while the sufficient run is compared against the reference assignment

### Requirement: FXORACLE-CORPUS-01
r[FXORACLE-CORPUS-01]

The oracle corpus MUST include deterministic exhaustive cases within declared caps and seeded larger cases with a replayable generator. Caps MUST cover the finite universe, holes, nodes, equations, rigid binders, goals, enumerated assignments and work; a case outside a cap MUST be skipped explicitly and MUST NOT be silently truncated.

The deterministic corpus MUST cover empty universes, single and shared holes, repeated and nested unions, pure neutral operands, two non-neutral sides, contradictory constants, multiple independent equations, constant narrowing, rigid identity, distinct rigid binders, rigid/constant and rigid/union forms, constrained and unconstrained holes in goals, inconsistency, effect extraction, and order/duplicate/renaming isomorphs.

For accepted cases with sufficient budget, the oracle MUST check equation permutation, equation duplication, node/binder renaming and union-operand swapping for acceptance and the complete resulting assignment. It MUST NOT require equal work counts or exhaustion points. A metamorphic mismatch is a defect only when both runs are inside the supported fragment with sufficient budget.

Discrepancies MUST be shrunk under a bounded attempt budget while preserving well-formed references and the original outcome class. The minimized record MUST be deterministic and MUST be retained with the unreduced original when shrinking would change the class.

#### Scenario: FXORACLE-CASE-21 equation order is invariant

- GIVEN a multi-equation accepted case
- WHEN the equations are reversed or permuted and both runs have sufficient budget
- THEN acceptance and every final bound are unchanged; work counts may differ

#### Scenario: FXORACLE-CASE-24 narrowed constant

- GIVEN a case whose constant is narrowed by an equation or union
- WHEN the production solver and the reference run
- THEN production refuses, the reference is inconsistent, and any shrink retains that class

### Requirement: FXORACLE-CTRL-01
r[FXORACLE-CTRL-01]

The oracle MUST be accompanied by test-local faulty-solver variants that are never compiled into production. The variants MUST include at least: omitted constant exactness, omitted union-child narrowing, attribution through a non-neutral union side, rigid-as-hole allocation, a single non-converged pass, and a zero-initialized hole bound. Each variant MUST be caught by a named deterministic case and classified as the expected wrong-assignment or unsound-accept discrepancy. An escape MUST fail the lane and require a new distinguishing corpus family, not a weakened variant.

The oracle MUST have a reproducible command and a receipt recording the source revision and dirty state, toolchain, oracle revision, seed schedule, every bound and work budget, counts by outcome and discrepancy class, every conservative refusal, minimized discrepancies, and the exact command. The receipt MUST state that finite testing is evidence only for the tested domain and does not close a proof obligation, alter source acceptance, promise inference completeness or replace independent kernel admission.

#### Scenario: FXORACLE-CASE-27 rigid-as-hole variant

- GIVEN the test-local variant that allocates a rigid binder as a narrowable hole
- WHEN it runs an accepted rigid-identity case
- THEN the oracle reports an unsound accept or wrong assignment for the named case and the receipt records the variant, case and class

#### Scenario: FXORACLE-CASE-30 missing receipt bound

- GIVEN an oracle receipt without a seed schedule, a bound or a conservative-refusal list
- WHEN the receipt is reviewed
- THEN it is rejected as incomplete evidence and no semantic agreement is claimed
