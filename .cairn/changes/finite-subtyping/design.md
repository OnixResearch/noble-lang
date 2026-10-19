## Context

This change designs the bounded language extension for issue #29, specifies a
small experimental prototype and a differential evaluation, and records the
go/no-go decision. The test-only prototype confirms a principal candidate in
every supported family; the selected decision is **no-go** for production
adoption. It adopts no type system, changes no source acceptance, and claims no
soundness or completeness result.

**Baseline.** All source references are at `de1044b` (the change's base). The
issue cites `66eb7cd`; the cited inference, source and type files have moved
only by line numbers. The prototype is a test-only integration lane; no
production code changed.

### What exists

- `noble-kernel::types::Ty` has `Unit`, `Bool`, `I64`, `Text`, `Syntax`,
  `Contract`, `Evidence`, `Certified`, `Pair`, `Sum`, `List`,
  `Program(Vec<Ty>, Vec<Ty>, EffSet)`, `Resource(ResourceKind)`,
  `Nominal(NominalTypeId, NominalShape)`, `GenericNominal(...)` and
  `LiveRef(Vec<Ty>, Vec<Ty>, EffSet)` (`crates/noble-kernel/src/types.rs:197`).
  There is no `Top` and no bounded-universal constructor.
- `Ty` equality is structural, with exact effect equality for `Program` and
  `LiveRef` (`crates/noble-kernel/src/types/impls.rs:180-210`). There is no
  variance.
- The frontend inference arena has `Hole`, `RigidValue`, `RigidStack`,
  `Link`, data constructors, `Program(input, output)` and stack `Push`
  terms (`crates/noble-contracts/src/inference/mod.rs:22`); a
  `Program` interface carries a separate effect node (`inference/mod.rs:54`).
- `unify` is structural (`inference/unify.rs:100`). Two `Program` terms add
  an effect equation and unify input/output; every constructor mismatch is
  `"program type or stack witness cannot be resolved"` (`unify.rs:130-220`).
- A declared signature is applied with rigid variables: the input is unified,
  the expected effect pattern is produced, and completion either equates the
  body effect (non-rigid) or records a rigid goal (`source/inference/mod.rs:233`;
  `source/inference/completion.rs:5-24`). Expected-signature checking already
  exists; this issue must not describe it as missing.
- `K-EFFECT-05` already permits an explicit checked adaptation that widens a
  latent effect bound, retains the original recipe and records the destination
  witness in identity. `K-EFFECT-02` forbids narrowing without establishing
  inclusion. This is the only existing subtyping-like relation, and it is
  deliberately not called general subtyping.
- The canonical definition identity ignores names and formatting
  (`crates/noble-contracts/tests/source.rs:76`); specialization identity folds
  selected adapter identities (`source/emission/identity.rs`). Source
  regressions cover fresh named uses versus monomorphic duplicated Programs
  (`tests/source.rs:32`), incompatible branch outputs (`:168`), resource
  duplication (`:226`) and narrowed universal effects
  (`tests/declared/cases.rs:256`). All are retained.

The language specification states that this revision does not introduce
general subtyping (`.cairn/specs/language/spec.md:487`). `K-SAFE-05` and
`S-GATE-03` permit a future principled extension only with rules that preserve
or strengthen the safety invariants and with corresponding proof obligations.
The extension below is therefore opt-in, versioned and outside current
acceptance until a separate integration decision.

## Motivating examples and the honest principal-type question

### Effect widening is the only non-reflexive relation available

Consider a module that requires a host effect and a consumer whose declared
interface expects a program with that effect:

```text
module widen@1 [
  require emit Text
  signature consume forall<S:stack>
      [ S Program<S,S,test.emit> -- S ! test.emit ]
  export consume
  def consume [ run ]
]
```

Passing the quotation `[ ]` (pure, interface `Program<S,S,pure>`) currently
rejects: the Program effect equation `pure = {test.emit}` is inconsistent, and
the concrete solver refuses it. A widened interface `pure ⊆ {test.emit}` is
safe: the program performs no host effect, and the consumer's bound is a
conservative super-bound. Under the proposed relation this is accepted through
an explicit adaptation that retains the recipe and records the destination
witness, exactly as `K-EFFECT-05` requires. The reverse direction is refused:
a program whose body requires `{test.emit}` MUST NOT be used where `pure` is
expected without a separate inclusion derivation under `K-EFFECT-02`.

### Data subtyping would change representation

`List<I64>` is not a subtype of `List<Sum<I64,Unit>>` under Noble's structural
representation: the element representation differs (a raw `I64` versus a tagged
sum), and kernel `Ty` equality is exact. Accepting it without an explicit
coercion would either change the runtime representation or silently interpret
bytes under another type. The design therefore rejects structural covariance
for data unless a future relation proves representation preservation. The same
argument applies to `Pair` and `Sum`, to generic nominal arguments, and to
nominal opacity.

### Program variance is mostly vacuous in v0

A `Program<I,O,E>` value consumes exactly `I` and produces exactly `O`. The
standard safe variance is contravariant inputs and covariant outputs:

```text
Program<I,O,E> <: Program<I',O',E'>  if  I' <: I,  O <: O',  E ⊆ E'
```

In v0 the only base subtyping is reflexivity for data and exact identity for
nominals, so `I' <: I` and `O <: O'` reduce to equality. Program variance is
recorded for a future base relation but is vacuous today. Stack tails are
invariant: a generic scheme's tail variable is instantiated, not subtyped, and
a closed Program preserves the exact tail it receives. There is no width
subtyping: a Program that requires extra inputs cannot be used where a shorter
input stack is expected, and a Program that produces extra outputs cannot be
silently truncated without an explicit continuation.

### Consequently, the expected result may be no-go

Effect inclusion is a directed finite lattice: `pure` is the least bound and
set union is the join. If the base checker's accepted interface is one
candidate and subsumption only widens its effect bound, every candidate is above
that base candidate, so the base candidate is principal. The issue explicitly
allows documenting that result. The prototype must confirm it on real Noble
examples and generated contexts; it must not assume it. Only a non-directed
base relation (for example, a future representation-preserving refinement or
nominal relation with incomparable supertypes) could create a genuine
incomparable family, and that relation would need its own soundness and
coverage proofs before this machinery is justified.

## Decision: fragment `finite-bases/v0`

**Choice:** An opt-in experimental fragment with this grammar:

```text
τ ::= Unit | Bool | I64 | Text
    | Pair τ τ | Sum τ τ | List τ
    | Program ⟨τ…⟩ ⟨τ…⟩ E
    | Nominal(id) | GenericNominal(id, τ, τ)
E ::= finite set of host effect identities
```

- `I64` is the wrapping 64-bit type; subtyping never reinterprets its bits.
- `Program` carries complete ordered input/output stacks and a finite latent
  upper bound.
- `Nominal(id)` and `GenericNominal(id,a,b)` denote exact resolved identities;
  their shapes must match the registered representation. No subtyping across
  different IDs and no widening of opaque representations.
- **Unsupported and outside the fragment:** `Top`, bounded universals,
  higher-rank or impredicative types, recursive types, `Syntax`, `Contract`,
  `Evidence`, `Certified`, `Resource`, `LiveRef`, user-declared subtyping,
  coercions, row/width subtyping, and any runtime representation change.
  Nominal declarations remain exactly as they are; resource ownership,
  capture eligibility and borrowed-reference restrictions are not altered.
- Annotations: the fragment is checked against an explicit expected signature
  or a complete accepted base interface. It does not add implicit
  instantiation beyond the existing rank-1 scheme instantiation. Any future
  implicit instantiation requires a finite complete query and is a separate
  decision.

**Rationale:** This is the smallest fragment that tests the paper's central
question without importing F-sub-top. It preserves every existing kernel type
and identity rule, and it makes the expected principal-type result testable
rather than assumed.

## Declarative subtyping

The relation is the least reflexive, transitive relation closed under:

1. **Data reflexivity.** `τ <: τ` for every well-formed data type. No other
   relation between distinct data constructors is admitted.
2. **Nominal identity.** `Nominal(id) <: Nominal(id)` only when the resolved
   ID and checked shape are identical; `GenericNominal(id,a,b)` requires the
   same ID and identical arguments and shape. No nominal subtyping is
   user-declarable.
3. **Pair/Sum/List.** Covariance is stated for a future base relation
   (`Pair<A,B> <: Pair<A',B'>` if `A <: A'` and `B <: B'`, and similarly for
   `Sum` and `List`), but in v0 it is only reflexive because no non-reflexive
   data relation exists. This keeps the rule ready without changing
   representation.
4. **Program.** `Program<I,O,E> <: Program<I',O',E'>` iff:
   - `I` and `I'` are closed stacks of the same length, the tail is identical,
     and `I'[i] <: I[i]` for every position (contravariant inputs);
   - `O` and `O'` are closed stacks of the same length and `O[i] <: O'[i]`
     (covariant outputs);
   - `E ⊆ E'` (effect-bound inclusion);
   - every ownership, capture, eligibility and borrowed-reference obligation
     of the source interface is preserved in the target interface.
5. **Resource and LiveRef.** Exact identity only, and outside the v0 fragment.
   A widening MUST NOT make a resource duplicable, discardable, capturable or
   storable, MUST NOT hide a required effect, and MUST NOT lose a
   borrowed-reference obligation. An immutable Program does not become
   move-only merely because an input expects a resource; resources are not in
   the fragment at all.

**Subsumption placement.** Subsumption is checked only at an explicit expected
interface boundary: a declared signature, a complete base interface, a `run`
or composition boundary, or a branch join. It is not a coercion inserted in the
middle of an expression. An effect widening is a checked interface adaptation
that creates a new Program value, retains the recipe and records the destination
witness; the source value is unchanged. Data subsumption in v0 is reflexivity
and therefore representation-preserving and identity-preserving. No implicit
coercions, no representation changes and no new runtime tags are introduced.

**Effects: inclusion versus equality.** A latent bound is an upper bound. The
relation permits `E ⊆ E'` only through the explicit adaptation. It never
permits narrowing. Effect-set equality is not used as a proxy for program
identity; identity remains the canonical recipe plus resolved dependencies,
captures, instantiated interface witnesses and adapter identities.

**Ambiguity.** If more than one maximal target candidate exists and none
subsumes the others, the declarative relation does not choose. The checker
reports ambiguity with the candidate list, their complete interfaces and their
origins. It MUST NOT combine one candidate's input with another's output or
effect.

## Candidate families

A candidate is a complete record:

```text
Candidate = (I, O, E, θ, Ω)
```

where `θ` records shared variable identities, scope and substitutions, and `Ω`
records ownership/eligibility obligations and the destination interface
witness. A family is a finite set of candidates for one source value and one
checking context. It is a checker representation of alternative valid typings,
**not** a union type, a runtime tagged union, or permission to instantiate one
monomorphic Program independently at each use.

Required properties, recorded as proof obligations rather than claimed:

- **Whole-candidate semantics.** Every operation consumes a candidate as a
  unit. Cross-mixing is forbidden: no input from candidate A with output from
  candidate B, no shared variable from one candidate with a constraint from
  another.
- **Shared identities.** Generalization, named-use instantiation and stack-tail
  variables retain the same identities across all candidates of one family
  where they denote the same binder.
- **Sound generation.** Every generated candidate is derivable by the
  declarative relation from the source value's accepted base interface.
- **Coverage.** Every target accepted by the declarative relation has at least
  one candidate whose interface is a subtype of the target.
- **Finite covers.** For each supported context change (a new expected
  signature, branch, run or composition boundary), construction terminates
  with a finite family or a distinct exhaustion result. It MUST NOT silently
  truncate.
- **Compositional operations.** `run`, `compose`, quotation and branch joins
  are total on whole candidates, preserve the witness correlations and return a
  candidate family for the result.
- **Termination versus exhaustion.** Type and effect universes are finite, so
  the mathematical construction is terminating. The implementation has
  work/node/bytes caps; exceeding a cap returns `exhausted`, never
  `rejected`, `ambiguous` or `accepted`.
- **Principal preference.** If one candidate subsumes every other candidate in
  the family, the checker MUST use it and report it as principal. If the family
  has incomparable maximal candidates, it MUST report ambiguity or retain the
  family; it MUST NOT arbitrarily select one.
- **Deduplication.** Candidates with identical complete interfaces, witnesses,
  obligations and identities are one candidate. Candidate order and internal
  node IDs are not semantic and MUST NOT affect public identity.

For v0, the expected family is the upward closure of the accepted base
interface under effect inclusion. It is a chain with a least element, so the
base interface should be principal. The prototype measures whether any real
example produces an incomparable maximal pair.

## Backward checking

Target-directed checking starts from the expected interface `(I_exp, O_exp,
E_exp)` and computes what a callee must provide:

- **Effects.** For a tail `run`/composition boundary, the callee's latent bound
  must be included in the continuation's bound. The inverse operation is a
  subset query and, when the expected bound contains variables, a finite
  substitution of the variable to the empty/required set. A required bound that
  is not included is refused; it is never widened silently.
- **Stacks.** The expected output fixes the callee's output positions; the
  expected input fixes the callee's input positions through contravariance. In
  v0, stacks are exact, so the inverse is identity plus an arity/tail check.
  A future base relation requires a proved inverse substitution.
- **Elimination interface.** For `run` and composition, the design requires a
  complete interface: every accepting derivation of the composed program has a
  corresponding accepting callee derivation and expected continuation.
  Assuming a single canonical input preserves all result/effect choices is not
  allowed; that requires a Noble-specific theorem.
- **Implicit instantiation.** v0 does not add it. Existing rank-1 scheme
  instantiation remains. If a later revision includes implicit instantiation,
  it must define a finite complete representation/query for its choices or
  require explicit annotations and report the boundary.
- **Equivalence.** The backward checker and the explicit-family checker MUST
  agree on every corpus case in the declarative relation. A disagreement is a
  defect in the prototype, not evidence for a new rule.

## Identity, versioning and compatibility

- An effect widening creates an adapted Program value: the recipe and
  dependencies are retained, the destination interface witness is recorded, and
  the new value's identity includes that witness (K-EFFECT-05). The original
  value and its evidence are unchanged.
- Candidate ordering, family cardinality and internal node IDs MUST NOT affect
  canonical encoding, hashes, definition/program-value identity, persistence,
  replay or the host/Wasm ABI.
- The fragment is opt-in and revisioned (`finite-bases/v0`). Current source
  acceptance, expected-signature checking, kernel `Ty` encoding and the Wasm
  boundary are unchanged. No migration is required unless a later integration
  plan selects a public representation; that plan must version any changed
  external encoding.
- A source-model result does not establish installed Wasm correspondence,
  ownership preservation or kernel admission. Those remain separate
  obligations under #15, #10 and #3.
- If the prototype confirms principal types, the go/no-go decision is
  **no-go**: do not adopt the extension, retain the current checker and
  K-EFFECT-05, and document the principal-type result. If a genuine
  incomparable family appears, adoption still requires discharged proof
  obligations and a separate integration plan; it is not automatic.

## Executed prototype and evaluation

`crates/noble-contracts/tests/models/finite-bases.rs` implements the selected
`finite-bases/v0` relation over public kernel types:

- an explicit-family checker that enumerates the upward effect closure, retains
  whole `(inputs, outputs, effects)` candidates, and identifies a principal
  candidate;
- a target-directed/backward checker that compares the base interface with the
  expected interface and applies the same stack and effect-inclusion rules;
- 277 finite differential cases over universes of size 0, 1, 2 and 4;
- real interface-shaped examples based on `result.noble` (`map_ok`/`and_then`
  style Programs), data covariance and exact nominal identity;
- negative controls for effect narrowing, representation-changing data
  covariance, nominal mismatch and cross-candidate field mixing;
- outcomes recorded as accepted, refused, principal, ambiguous, unsupported or
  internal. The executed lane has no ambiguity and no exhaustion.

`verification/finite-subtyping/run.mjs` executes the lane and writes a
source-bound receipt. The receipt records the source revision, toolchain,
bounds, counts and the decision.

## Prototype outcome and go/no-go

The executed receipt records:

```text
cases 277
accepted 94, refused 183
principal_families 277, ambiguous 0
max_family 16
real accepted 3, real refused 4
decision no-go
```

Every supported family has a principal candidate. The only non-reflexive
relation in `finite-bases/v0` is directed effect-bound inclusion, which already
has a least element in the base interface and is provided in production by the
current checker plus K-EFFECT-05. The finite-family machinery therefore adds no
accepted context in this fragment.

**Decision: no-go for production adoption.** Do not integrate the extension,
do not change source acceptance, kernel types, canonical identity or the ABI,
and retain the current checker and K-EFFECT-05. The result is a bounded
prototype result, not a universal principal-type theorem.

## Future evaluation (not executed)

Metering of construction, queries and materialized nodes; the
`structural-map.noble`, branch, named-definition, rigid-binder and malformed
corpora; stale comparison-cache controls; peak-storage and work accounting;
and a direct differential against the production checker remain future work.
No paper-derived performance guarantee is claimed.

## Proof obligations (proved, tested, assumed and open)

| Id | Obligation | Status in this change |
|---|---|---|
| O1 | Subtyping is decidable for `finite-bases/v0` with a well-founded measure | open; design argument only |
| O2 | Candidate generation is sound for the declarative relation | open |
| O3 | Candidate coverage is complete for every supported target | open |
| O4 | Finite covers exist for every supported scope/context change | open |
| O5 | Compositional operations (`run`, `compose`, quotation, joins) are complete | open |
| O6 | Family construction and candidate operations terminate; bounded exhaustion is distinct | open |
| O7 | The backward elimination interface is complete and its inverse substitutions are sound and complete | open |
| O8 | The reference and backward implementations agree with the declarative relation | open; differential tests only |
| O9 | Effect widening preserves effect safety, ownership and kernel admission under #15 | open |
| O10 | No public identity, canonical encoding, persistence or ABI change | open; design constraint |

**Go/no-go criteria.** Proceed to a separately reviewed integration plan only
if O1–O9 are discharged or the fragment is narrowed until they are, O10 is
shown, the differential corpus has no unexplained disagreement, and a genuine
need for incomparable candidates is demonstrated. If the corpus confirms that
the accepted base interface is principal in every supported context, select
**no-go** and document the principal-type result. No production rollout,
automatic source-acceptance change, proof-system change or ABI change follows
from a successful prototype.

## Acceptance mapping (issue #29)

| Issue criterion | Section |
|---|---|
| Concrete accept/reject examples; establish whether multiple incomparable candidates are needed | "Motivating examples and the honest principal-type question" |
| Smallest useful grammar and annotation boundary; decide Top/bounded quantification | "Decision: fragment `finite-bases/v0`" |
| Formation, subtyping, subsumption, instantiation, ambiguity; checking versus inference | "Declarative subtyping"; "Backward checking" |
| Variance, stack tail/arity, effect inclusion/equality, Pair/Sum/List, nominal, LiveRef | "Declarative subtyping" |
| Resource identity, ownership, capture, borrowed restrictions | "Declarative subtyping"; unsupported list |
| Representation-preserving or coercions; kernel evidence | "Declarative subtyping"; "Identity, versioning and compatibility" |
| Candidate semantics, proof obligations, termination, backward elimination, implicit instantiation, equivalence | "Candidate families"; "Backward checking"; "Proof obligations" |
| Explicit-family and backward implementations, metering, corpus, negative controls, benchmarks, diagnostics | "Evaluation plan" |
| Compatibility, versioning, reproducible evaluation and go/no-go | "Identity, versioning and compatibility"; "Evaluation plan"; "Proof obligations" |

## Risks and non-claims

- The likely no-go is a valid result, but it must be earned by the prototype,
  not asserted. The design keeps the family machinery so the experiment can
  refute the hypothesis.
- Structural covariance without representation preservation would be a
  type-safety defect disguised as convenience. The v0 fragment therefore keeps
  data subtyping reflexive.
- Effect widening can hide a host requirement only if inclusion is checked in
  the wrong direction. The relation and scenarios pin the direction.
- A future base relation with incomparable supertypes would invalidate the
  principal-type argument and require new proofs; it is not smuggled in through
  the Program variance rule.
- This change adds no source syntax, no checker, no candidate encoding, no
  kernel type, no canonical hash and no ABI. All such work requires the
  separate integration decision above.
