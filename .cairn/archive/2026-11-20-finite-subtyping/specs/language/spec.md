# Opt-in finite-candidate subtyping study

## ADDED Requirements

### Requirement: FSUB-FRAG-01
r[FSUB-FRAG-01]

An opt-in, revisioned `finite-bases/v0` fragment MAY study richer subtyping
without changing current source acceptance. Its type grammar is `Unit`, `Bool`,
`I64` (wrapping), `Text`, `Pair`, `Sum`, `List`, `Program` with complete
ordered input/output stacks and a finite latent effect bound, and exact
`Nominal`/`GenericNominal` identities whose checked shapes match.

The fragment MUST exclude `Top`, bounded universals, higher-rank and
impredicative types, recursive types, `Syntax`, `Contract`, `Evidence`,
`Certified`, `Resource`, `LiveRef`, user-declared subtyping, coercions,
row/width subtyping and any runtime representation change. Nominal identity
MUST be exact; no subtyping across different nominal IDs or opaque
representations is admitted. Resource ownership, capture eligibility and
borrowed-reference restrictions MUST remain unchanged.

The fragment MUST be checked only against an explicit expected signature or a
complete accepted base interface. It MUST NOT add implicit instantiation beyond
the existing rank-1 scheme instantiation. Until a separate integration decision,
current source acceptance, expected-signature checking and the rule that this
revision introduces no general subtyping MUST remain in force.

#### Scenario: FSUB-CASE-01 effect widening boundary

- GIVEN a pure `Program<S,S,pure>` quotation and a declared consumer expecting
  `Program<S,S,{test.emit}>`
- WHEN the opt-in fragment checks the explicit expected interface
- THEN the widening `pure ⊆ {test.emit}` MAY be accepted only through a checked
  adaptation that retains the recipe and records the destination witness,
  while the source interface and current-language acceptance are unchanged

#### Scenario: FSUB-CASE-04 narrowing is not subsumption

- GIVEN a Program whose body requires `{test.emit}` and an expected interface
  with `pure`
- WHEN subsumption is attempted
- THEN it MUST refuse; a narrowing requires a separate inclusion derivation
  under K-EFFECT-02 and is outside this fragment

### Requirement: FSUB-REL-01
r[FSUB-REL-01]

The declarative relation MUST be the least reflexive, transitive relation closed
under:

- data reflexivity only;
- exact nominal identity, including resolved ID, arguments and checked shape;
- Pair/Sum/List covariance, recorded for a future base relation but reflexive
  in v0;
- `Program<I,O,E> <: Program<I',O',E'>` iff the input stacks have equal length,
  an identical tail and `I'[i] <: I[i]`; the output stacks have equal length and
  `O[i] <: O'[i]`; `E ⊆ E'`; and every ownership, capture, eligibility and
  borrowed-reference obligation is preserved.

Subsumption MUST occur only at an explicit expected-interface boundary
(declaration, complete base interface, run/composition boundary or branch
join), not as a mid-expression coercion. Data subsumption MUST be
representation-preserving. An effect widening MUST create a new Program value,
retain the recipe and original value, and record the destination interface
witness. Effect equality MUST NOT be used as a proxy for program identity.
Multiple maximal candidates with no subsuming candidate MUST be reported as
ambiguity; one candidate's input, output and effect MUST NOT be combined with
another's.

#### Scenario: FSUB-CASE-07 data covariance without representation preservation

- GIVEN `List<I64>` and an expected `List<Sum<I64,Unit>>`
- WHEN subsumption is attempted
- THEN it MUST refuse, because the element representation differs and no
  coercion or representation change is part of the fragment

#### Scenario: FSUB-CASE-09 candidate cross-mixing

- GIVEN two candidates `(I1,O1,E1)` and `(I2,O2,E2)` with no subsumption between
  them
- WHEN a checker constructs a result using `I1`, `O2` and `E1`
- THEN the result MUST be rejected as an invalid whole-candidate composition,
  not accepted as a new candidate

### Requirement: FSUB-CAND-01
r[FSUB-CAND-01]

A candidate MUST be a complete record `(I,O,E,θ,Ω)` carrying shared variable
identities, scope and substitutions `θ`, and ownership/eligibility obligations
and the destination witness `Ω`. A family MUST be a finite set of whole
candidates for one source value and checking context; it is not a union type, a
runtime tagged union, or permission to instantiate one monomorphic Program
independently at each use.

Candidate generation MUST be sound for the declarative relation; coverage MUST
include every supported target; finite covers MUST exist for every supported
scope/context change; and `run`, `compose`, quotation and branch joins MUST be
total on whole candidates and preserve witness correlations. Construction and
operations MUST terminate under a well-founded measure, while the implementation
MUST return a distinct `exhausted` outcome at its work/node/bytes caps and MUST
NOT interpret truncation as rejection or acceptance.

If one candidate subsumes every other candidate, the checker MUST select and
report it as principal. Otherwise it MUST retain/report the incomparable maximal
candidates and MUST NOT choose arbitrarily. Deduplication MUST use the complete
interface, witnesses, obligations and identities; candidate order and internal
node IDs MUST NOT be semantic.

Target-directed checking MUST define a complete elimination interface for `run`
and composition and sound and complete inverse substitutions for the actual
effect and stack variables. It MUST NOT assume that one canonical input
preserves all result/effect choices. If implicit instantiation is later
included, it requires a finite complete representation/query or explicit
annotations and an explicit boundary; v0 adds none.

#### Scenario: FSUB-CASE-12 incomparable maximal candidates

- GIVEN a family with two maximal candidates and no subsumption between them
- WHEN the checker is asked for one type
- THEN it reports ambiguity with both complete candidates and origins, selects
  neither, and does not combine their fields

#### Scenario: FSUB-CASE-15 bounded exhaustion

- GIVEN a family construction or backward query that exceeds its work, node or
  bytes cap
- WHEN the checker finishes
- THEN it reports `exhausted`, not `rejected`, `ambiguous` or `accepted`, and no
  truncated family is used for a subsequent decision

### Requirement: FSUB-ID-01
r[FSUB-ID-01]

Candidate ordering, family cardinality and internal node IDs MUST NOT affect
canonical encoding, hashes, definition/program-value identity, persistence,
replay or the host/Wasm ABI. An effect widening MUST retain the original
Program value and recipe while creating an adapted value whose identity
includes the destination interface witness; the original evidence MUST remain
bound to the original value.

The fragment MUST be opt-in and revisioned (`finite-bases/v0`). It MUST NOT
change current source acceptance, kernel `Ty` encoding or the Wasm boundary
without a separately reviewed integration plan that versions any changed
external representation. A source-model result MUST NOT be described as
installed Wasm correspondence, ownership preservation or kernel admission.

If the evaluation confirms that a principal candidate exists in every supported
context, the decision MUST be no-go for production adoption: retain the current
checker and K-EFFECT-05 and record the principal-type result rather than
integrating the extension.

#### Scenario: FSUB-CASE-18 candidate order is not identity

- GIVEN the same accepted base Program and the same widening, with candidates
  enumerated in a different order or with different internal node IDs
- WHEN canonical identity is compared
- THEN the public identity and interface witness are unchanged, and evidence
  for the original value still applies only to the original value

#### Scenario: FSUB-CASE-21 principal-type result selects no-go

- GIVEN the completed corpus shows one candidate subsumes every other candidate
  in every supported context
- WHEN the go/no-go review runs
- THEN it records no-go, retains current system behavior and does not create a
  source-acceptance or ABI change


