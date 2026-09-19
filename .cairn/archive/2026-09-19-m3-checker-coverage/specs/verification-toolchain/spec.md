# M3 checker coverage acceptance delta

## ADDED Requirements

### Requirement: VT-M3-01
r[VT-M3-01]

M3 MUST extend the M2 checking fragment to a named `bootstrap fragment v1` by auditing every B-CHECK-01 through B-CHECK-07 requirement of the core-bootstrap specification against the M2 verification records, and MUST scope in every partially covered family with its own named controls.
The audit MUST be recorded with its evidence citations before implementation, and every scoped-in family MUST retain its fragment label.
Recursive definition dependencies and user-declared recursive schemas in environment data MUST produce an explicit unsupported or rejection outcome with a diagnostic naming the identity, never silent acceptance or divergence (B-CHECK-02).
Cyclic type equations and self- or mutually-referential substitution witnesses MUST be rejected (B-CHECK-05), and every new checking path MUST charge work before an allocation or traversal exceeds its declared limit (B-CHECK-07).
Branch-local type information MUST come from checked eliminator derivations for all three bootstrap eliminators (`case`, `if`, `list.case`): joins MUST preserve the complete common output stack and the conservative effect bound, eliminating `Sum<Resource<R>,I64>` MUST expose the selected payload at its own type while the whole sum remains non-`Data`, and advertised witnesses MUST NOT introduce trusted refinements or implicit union joins (B-CHECK-06).
A rejected submission MUST NOT execute a candidate body or issue a candidate-body host request (B-RESULT-02).

#### Scenario: Recursive definition and schema rejection

- GIVEN environment data containing a recursive definition dependency and, separately, a user-declared recursive schema
- WHEN the acceptance checker evaluates each against the fragment
- THEN each yields its explicit non-accepted outcome with a diagnostic naming the offending identity and executes no body

#### Scenario: Cyclic substitution rejection

- GIVEN a self-referential substitution witness and a mutually referential substitution chain as separate candidates
- WHEN the checker evaluates each
- THEN each is rejected before its work budget is exceeded, with the cycle identified in the diagnostic

#### Scenario: Eliminator depth

- GIVEN a `case` elimination over `Sum<Resource<R>,I64>` whose branches check at the same complete output stack, and mutations that advertise a branch refinement or join unequal output stacks implicitly
- WHEN the checker evaluates each
- THEN the payload-exposing elimination is accepted with the resource branch accounting for its owner and the sum remaining non-`Data`, while each mutation is rejected without a trusted refinement or implicit union join

### Requirement: VT-M3-02
r[VT-M3-02]

M3 MUST complete the environment word table to every bootstrap word: the stack, data, control, and collection words, the wrapping arithmetic including `=`, `quote`, `compose`, `run`, `reflect`, and the supplied `test.emit` (B-SCOPE-01 items 4-5, language specification sections 4.4, 6.3-6.5, 7.1-7.4, 11.3).
Each table entry MUST carry a per-word contract, at least one positive and one rejection control, a property-generation entry, and an independent oracle arm derived from the documented contract rather than from kernel decision code.
The extended kernel MUST stay within the confirmed extraction subset, and the regenerated module MUST remain bound to the actual source, inventory, selection, and tool identities through the mandatory Charon → Aeneas → Lean route (VT-AENEAS-01, VT-AENEAS-03, B-IMPL-02).
A word missing from the table, the pool, the oracle, the controls, or the documentation examples MUST be a visible gap, not a silent omission.

#### Scenario: Complete word table

- GIVEN the regenerated extraction inventory and the recorded word table
- WHEN the coverage comparison runs
- THEN every bootstrap word appears in the kernel table, the generator pool, the oracle, the control matrix, and the executed documentation examples

#### Scenario: Oracle independence

- GIVEN the eliminator oracle arms and any kernel decision code
- WHEN the arms are inspected and the agreement lane runs
- THEN the arms derive from the documented contracts only and the recorded seeds show zero disagreements between the kernel checker and the oracle

#### Scenario: New word extraction

- GIVEN the `=` contract added to the kernel table
- WHEN the pinned extraction regenerates the Lean module
- THEN the new contract translates without an extraction refusal and the module compiles under the pinned backend

### Requirement: VT-M3-03
r[VT-M3-03]

M3 MUST deepen proof coverage per rule and per word beyond the single end-to-end M2 refinement.
For every word table entry, a scheme refinement MUST relate the extracted instantiation path to the reference instantiation.
For every fragment rule family — empty, literal, word, sequence, quotation, and the three eliminator rules — acceptance through that checker path MUST imply the declarative derivation at that node in the acceptance-theorem shape (V-CHECK-03).
The eligibility and effect-inclusion decision paths MUST each be proved equivalent to their declarative side conditions, duplicated first-class program values MUST share one instantiated interface at both levels, termination MUST be re-established over fragment v1 including its rejection walks, and positive acceptance coverage MUST exhibit an accepted, derivable fixture for every word (V-CHECK-05, PO-09, PO-10).
The refinement family MUST relate the extracted checker's outcomes to the reference checker's outcomes per word and per rule, including the new rejection families, and MUST bind the generated constants (PO-11, VT-AENEAS-04).
Acceptance-path decision functions MUST NOT remain silently open in the coverage classification (VT-SCOPE-05).
Every theorem MUST retain its fragment label; none closes whole-language, whole-kernel, or Wasm obligations (V-MODEL-01, V-GATE-05).

#### Scenario: Per-rule soundness

- GIVEN the fragment v1 reference judgment with its rule families and the extracted checker paths
- WHEN the per-rule soundness targets build under the pinned toolchain
- THEN each family's acceptance implies a derivation containing that rule instance, with no proof holes

#### Scenario: Per-word coverage and refinement

- GIVEN the word table and the representative per-word fixtures
- WHEN the coverage and refinement targets build
- THEN every word has an accepted, derivable fixture and an extracted-versus-reference agreement theorem binding the generated constant

#### Scenario: Decision-path correspondence

- GIVEN the extracted eligibility and inclusion decisions and the declarative side conditions
- WHEN the correspondence targets build
- THEN each decision is equivalent to its side condition, and a duplicated program value shares one instantiated interface at both levels

### Requirement: VT-M3-04
r[VT-M3-04]

M3 MUST extend the M2 proof, coverage, property, documentation, and refusal gates in place rather than duplicate them, and MUST NOT weaken any M2 control.
The extended proof gate MUST require the fragment v1 theorem set with its axiom policy and generated-subject bindings.
The extended coverage gate MUST re-derive the per-function classification from the regenerated inventory with separate extracted, proved, modeled, excepted, and open statuses.
The refusal matrix MUST additionally reject a missing per-word theorem, a missing per-word coverage case, a removed eliminator oracle arm, and a schema-revision regression, alongside the M2 mutations (VT-CI-05).
Obligation-ledger entries and status flags MUST move only with the fragment v1 evidence, and the M2 disclosures MUST be closed or re-disclosed with their evidence (EV-BIND-01, VT-AENEAS-05).
Tests, extraction success, and document checks MUST NOT substitute for a required refinement proof.

#### Scenario: Extended gates pass

- GIVEN the implemented fragment v1 with its regenerated module and theorem set
- WHEN the extended proof and coverage gates run on the tree
- THEN both pass with their outputs retained and every M2 gate still passing on the same tree

#### Scenario: New refusals reject

- GIVEN a missing per-word theorem, a missing per-word coverage case, a removed eliminator oracle arm, and a schema-revision regression as separate mutations of a scratch copy
- WHEN the extended refusal matrix evaluates each mutation
- THEN each mutation is rejected while the positive baseline stays green

#### Scenario: Honest evidence movement

- GIVEN the built fragment v1 evidence and the obligation ledger
- WHEN the ledger entries for PO-09, PO-10, and PO-11 are updated
- THEN they carry exactly the fragment v1 claims with bound evidence, every other obligation stays open, and any remaining M2 disclosure is re-disclosed rather than dropped
