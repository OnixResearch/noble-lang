# Four-version merge claim checking and evaluation

## ADDED Requirements

### Requirement: SMERGE-CHECK-01
r[SMERGE-CHECK-01]

The `SMERGE/1` claim MUST be encoded as `noble-merge-claim/v1` over the existing
pure reference semantics: for the four exact subjects and selected observables,
universally over every well-typed input vector, the merge has no conflict and
the candidate's full output stack equals the per-observable merge result. The
claim MUST use structural value equality and wrapping `I64`; it MUST NOT be
defined over source text, Wasm bytes or a second unverified interpreter.

Claim elaboration MUST be an untrusted producer. The consumer MUST select the
subjects, observables and policy, regenerate the exact expected claim from the
retained preparations, and check the supplied proof against that claim with the
pinned isolated Lean consumer. Producer-supplied subjects, claims, assumptions,
hashes and trust flags MUST be discarded or refused when they differ. Ordinary
`prepare` MUST NOT run a prover.

Termination for the selected fragment MUST be a separately stated and checked
Lean theorem. It MUST be derived from the finite program, the acyclic
definition/quotation DAG and total builtins; purity, an empty effect set or the
existing partial-correctness claim MUST NOT be used as the termination
argument.

Optional bounded automation MAY execute the four versions on generated inputs.
A mismatch MUST become `validated-counterexample` only after independent
consumer replay, type checking, exact-subject confirmation and retention of raw
observations with a reproducible seed and revision. A passing corpus MUST NOT
be reported as a proof. A truncated search MUST NOT yield `verified` or a
refutation.

#### Scenario: SMERGE-CASE-12 wrong proof

- GIVEN a valid Lean proof of `MergeRel O A B M'` or of a different observable
  list
- WHEN the consumer regenerates the exact claim for `O A B M`
- THEN the proof is rejected as not matching the intended claim

#### Scenario: SMERGE-CASE-15 vacuous precondition

- GIVEN a candidate whose declared precondition is `requires false`
- WHEN the claim would quantify over an empty input domain
- THEN the checker reports `vacuous-precondition` and never `verified`

### Requirement: SMERGE-GATE-01
r[SMERGE-GATE-01]

Before any integration, the `SMERGE/1` design MUST be evaluated with a
reproducible plan that includes:

- independent output changes, equivalent edits, one-sided changes and identical
  changes as positive fixtures;
- lost changes, incompatible edits, wrapping boundaries, missing observations,
  changed callees/captures, wrong proofs, stale identities, abnormal outcomes
  and vacuous preconditions as negative fixtures;
- a frozen consumer, pinned semantic model and toolchain, source-bound receipts
  and raw observations;
- an explicit record of `verified`, `conflict`, `validated-counterexample`,
  `unknown`, `unsupported`, `exhausted` and `internal-failure` outcomes.

The study MUST label the merge obligations as proved, tested, assumed or open.
It MUST establish the relation/claim correspondence, the terminating-fragment
theorem, claim elaboration and consumer regeneration, checker soundness,
evidence binding and outcome discipline before a separate integration plan may
proceed. Source-model proof and installed Wasm correspondence MUST remain
separate obligations.

A successful check MUST NOT perform an automatic merge, publish a branch,
admit a live slot, migrate source control or grant release permission. The
go/no-go decision MUST stop or narrow the fragment when an obligation remains
open; a test corpus alone MUST NOT close a universal obligation.

#### Scenario: SMERGE-CASE-17 reproducible positive receipt

- GIVEN SMERGE-CASE-01 through SMERGE-CASE-05 executed against frozen subjects
- WHEN the receipt is reviewed
- THEN it records exact subjects, claims, proofs, observations, toolchain,
  revisions, bounds and outcomes, and only Lean acceptance marks a case
  `verified`

#### Scenario: SMERGE-CASE-20 no automatic merge

- GIVEN a `verified` merge claim for one candidate
- WHEN a tool consumes the receipt
- THEN no branch, publication, live slot, migration or release action follows;
  those remain separate authorized operations
