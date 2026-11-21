# Four-version semantic merge relation

## ADDED Requirements

### Requirement: SMERGE-REL-01
r[SMERGE-REL-01]

An opt-in `SMERGE/1` relation SHALL compare four exact closed subjects `O`
(base), `A` and `B` (variants) and `M` (candidate) over the same complete
ordered input stack and the same complete ordered output stack. Interfaces MUST
be exact `Ty` equality; there is no width, arity or representation subtyping.

Observables MUST be the full ordered output stack over first-order data
(`Unit`, `Bool`, wrapping `I64`, `Text`, `Pair`, `Sum`, `List`). `Program`,
`Syntax`, `Contract`, `Evidence`, `Certified`, `Resource`, `LiveRef` and opaque
nominal values MUST NOT be observable in this experiment. `I64` MUST be
compared as a 64-bit bit pattern with wrapping arithmetic; a mathematical-integer
reading is forbidden. `Text` is exact bytes; aggregate equality is structural.

For every well-typed input vector `xs` and every output position `i`, the
relation MUST use:

```text
merge(o,a,b) =
  if a = o and b = o then o
  else if a ≠ o and b = o then a
  else if a = o and b ≠ o then b
  else if a = b then a
  else CONFLICT
```

`Merge(O,A,B,M)` MUST hold exactly when, for every `xs` and `i`, no conflict
occurs and `Obs_M(xs)[i] = merge(Obs_O(xs)[i], Obs_A(xs)[i], Obs_B(xs)[i])`.
The relation MUST cover the full output stack, including unchanged positions.

A conflict MUST be reported as `conflict` with a validated input witness and
both variant observations; a candidate MUST NOT be verified against a
conflicting base merely by choosing one side. An absent or differently shaped
output MUST be `incompatible-interface`, not a missing value to ignore. An
abnormal outcome or a subject outside the fragment MUST NOT be treated as a
vacuous success.

`SMERGE/1` MUST require `preconditions = true` and a nonempty input domain.
A later conditional revision MUST carry a witness or proved non-vacuity; a
`requires false` candidate MUST be `vacuous-precondition`, never verified.

#### Scenario: SMERGE-CASE-01 independent changes

- GIVEN O `[ ]`, A `[ 1 + ]`, B `[ swap 1 + swap ]` and M
  `[ swap 1 + swap 1 + ]` over input `[I64 I64]`
- WHEN the relation is checked
- THEN M preserves A's change to the top and B's change to the bottom, and the
  result is `verified`

#### Scenario: SMERGE-CASE-07 incompatible edits

- GIVEN O `[ ]`, A `[ 1 + ]`, B `[ 2 + ]` and M `[ 1 + ]`
- WHEN the relation is checked
- THEN it reports `conflict` with an input witness, and neither M nor B may be
  verified by a variant-order rule

### Requirement: SMERGE-SCOPE-01
r[SMERGE-SCOPE-01]

`SMERGE/1` MUST admit only closed, monomorphic, pure programs with the empty
effect bound, first-order data values, stack operations, wrapping arithmetic,
Boolean control and data elimination, nonrecursive acyclic named definitions,
and statically constructed closed quotations with immutable first-order data
captures and an acyclic call graph.

The fragment MUST exclude recursion, loops, `self`, `compose`, runtime-selected
or runtime-generated Programs, Program inputs/outputs, host effects, resources,
live dispatch/reload, async, external calls, and any dependence on a host
response or an implementation quota.

Normal termination for every well-typed input MUST be established from the
finite straight-line program, the acyclic definition/quotation DAG and total
builtins. Purity, an empty effect set or the existing partial-correctness claim
MUST NOT be used as the termination argument. A subject outside the fragment
MUST be `unsupported-fragment`; an implementation-quota abort MUST NOT become
a vacuous verified merge.

Subject identity MUST bind the exact `ProgramValueId`, canonical recipe,
instantiated interface, resolved dependency closure, captured values, schema
and builtin identities and semantic revision. It MUST NOT be reduced to names,
source locations, template IDs or equal effect sets. A changed callee or
capture is a different subject even when its outputs coincide.

#### Scenario: SMERGE-CASE-08 wrapping boundary

- GIVEN O `[ ]`, A `[ 1 + ]`, B `[ ]` and M `[ 2 + ]` over an `I64`
- WHEN the relation reaches `y = 2^64 - 1`
- THEN it reports a counterexample because A produces `0` and M produces `1`;
  the wrapping semantics MUST NOT be read as mathematical integers

#### Scenario: SMERGE-CASE-14 unsupported fragment

- GIVEN a version that uses a host effect, resource, `compose` or recursion
- WHEN the subject is checked
- THEN the outcome is `unsupported-fragment`, not a vacuous verification

### Requirement: SMERGE-EVIDENCE-01
r[SMERGE-EVIDENCE-01]

A `noble-merge-evidence/v1` object MUST bind the relation/claim revision,
semantic and IR revisions, and the exact O/A/B/M subjects: program-value
identity, canonical recipe, instantiated interfaces, dependency closure,
schema/builtin identities, capture values and semantic revision. It MUST bind
the selected observable list and output stack types, the precondition and
non-vacuity evidence, the termination obligation status, the full regenerated
claim (not only a digest), proof bytes and evidence class, Lean
binary/library/source-tree identities, checker limits and policy revision, the
outcome, and the exact fragment/input scope.

A consumer MUST select the subjects, observables, claim, assumptions, semantic
context, evidence class and limits independently of untrusted producer metadata.
Producer-supplied subjects, claims, assumptions, hashes or trust flags that
differ from the consumer's regeneration MUST be rejected. Evidence for a
different subject, capture, dependency, interface, observable list or revision
MUST be stale and refused.

The checker MUST report exactly `verified`, `conflict`,
`validated-counterexample`, `unknown`, `unsupported`, `exhausted` or
`internal-failure`. Only Lean acceptance of the exact regenerated claim MAY
yield `verified`; every other state MUST remain visibly nonverified. `conflict`,
`validated-counterexample`, `unknown`, `unsupported`, `exhausted` and
`internal-failure` MUST NOT be rendered as success.

A source-model proof and installed Wasm correspondence MUST remain separate.
A digest is an index or integrity check, not proof validity.

#### Scenario: SMERGE-CASE-10 changed callee

- GIVEN O/A/B call `inc` while M calls a different definition `inc2` with the
  same body and outputs
- WHEN evidence for the old subjects is offered for M
- THEN the consumer refuses it as a stale/different subject; a fresh claim for
  M may be checked separately, but old evidence does not transfer

#### Scenario: SMERGE-CASE-16 outcome visibility

- GIVEN a receipt whose outcome is `unknown`, `exhausted` or
  `internal-failure`
- WHEN it is displayed or consumed
- THEN no success, verified or merged flag is set, and the exact nonverified
  state is retained


