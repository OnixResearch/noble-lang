## Context

This change specifies a bounded four-version semantic merge relation and an
evaluated checking path. It implements a test-only model of the relation and
its fixtures, writes no Lean definition, checks no proof and authorizes no
merge. The relation is not a textual merge, not a recipe comparison and not a
claim that a candidate is the unique merge.

**Baseline.** All source references are at `de1044b` (the change's base). The
issue cites `66eb7cd`; the cited contract and README lines have moved only by
line numbers. The model is a test-only integration lane; no production code
changed.

### Existing contract boundary

- `PC(p,P,Q)` means: for every well-formed initial configuration satisfying
  `P`, every modeled **normal** execution ends in a configuration satisfying
  `Q` (`.cairn/specs/program-contracts/spec.md:110`).
- Prefix safety constrains every finite reachable prefix; total correctness
  establishes normal termination and its postcondition under stated
  assumptions (`spec.md:167-174`). VC-LOGIC-03 requires the report to
  distinguish `partial-correctness`, `prefix-safety` and `total-correctness`;
  a termination argument must cover recursion, loops, called programs and host
  responses, and a pure effect set is not that argument.
- VC-REWRITE-01 says rechecking transformed syntax does not transfer behavioral
  evidence; VC-REWRITE-02 requires a transformation claim to name its
  equivalence or preservation relation. Recipe equality, normal-result
  equivalence, trace preservation and full contextual equivalence are
  distinct.
- VC-ID-02 binds a closed-program claim to captured values, instantiated
  interfaces, resolved dependencies, builtin semantics and host contracts; a
  function-template identity alone does not identify a captured program value.
  VC-ID-03 says a digest is an index or integrity check, not proof validity.
- VC-EVIDENCE-01 requires the consumer to select the expected claim, allowed
  assumptions, semantic context, evidence class and limits independently of
  untrusted metadata. VC-EVIDENCE-02 requires the theorem to match the intended
  claim and all relevant definitions to be checked transitively. VC-EVIDENCE-04
  requires isolated build and independent recheck; verification is not an
  undocumented step of ordinary `prepare`.
- The contract API exposes bounded limits (`crates/noble-contracts/src/lib.rs:50`),
  distinguishable diagnostic kinds (`lib.rs:75`: `Invalid`, `Unsupported`,
  `Exhausted`, `Internal`), prepared subjects (`lib.rs:203`) and `prepare`
  (`lib.rs:253`). `export_lean` (`lib.rs:47`) is the existing claim-export
  surface. The embedded proof workflow uses a separate, pinned, resource-bounded
  Lean consumer (`crates/noble-cli/src/workflow/intrinsic.rs:591`).
- The README's verification section separates application claims from
  implementation verification and describes checking supplied evidence, not
  automatic proof search (`README.md:1021-1046`).
- PO-15/16/19/20/21 and the broader PO-17/18 lowering/loading obligations remain
  open. This change adds a merge-specific obligation; it does not discharge any
  existing PO by passing fixtures.

## Decision: `SMERGE/1` relation

### Versions, interfaces and observables

Four versions are exact closed subjects:

```text
O : base program
A : first variant
B : second variant
M : supplied candidate
```

Each subject is identified by its exact `ProgramValueId`, canonical recipe,
instantiated interface, resolved dependency closure, captured values, schema
and builtin identities, and semantic revision. A name, source location,
template ID or equal effect set is not a subject identity (VC-ID-02).

All four versions MUST have:

- the same complete ordered **input** stack, with exact `Ty` equality;
- the same complete ordered **output** stack, with exact `Ty` equality;
- no polymorphic variables (all schemes instantiated), no resources, no
  `LiveRef`, and the empty effect bound.

**Observable types** are first-order data only:

```text
Observable ::= Unit | Bool | I64 | Text | Pair Observable Observable
             | Sum Observable Observable | List Observable
```

`Program`, `Syntax`, `Contract`, `Evidence`, `Certified`, `Resource`, `LiveRef`
and opaque nominal values are not observable in this experiment. A version
whose output stack contains one of them is `unsupported-observable`. If a
version's input or output stack shape differs from another's, the relation is
`incompatible-interface`; there is no width, arity or representation
subtyping.

Value equality is structural and exact:

- `I64` is a 64-bit bit pattern; arithmetic is wrapping `wrap64`; equality is
  bit equality. A mathematical-integer reading is forbidden.
- `Bool` and `Unit` have their ordinary values; `Text` is compared as its exact
  bytes; `Pair`, `Sum` and `List` are compared structurally.

### Execution and normal return

For an input vector `xs` of the common input types, `Obs_X(xs)` is the complete
ordered output vector produced by a **normal** return of version `X`. The
relation quantifies over every well-typed `xs`.

The selected fragment is total for every `xs` (section "Terminating
fragment"). Therefore no version aborts or diverges inside the claim, and the
four normal returns are established, not assumed. An input outside the declared
types, an unsupported construct, or an execution that aborts under an
implementation quota is **not** a vacuous success: it is `unsupported`,
`abnormal-outcome` or `exhausted`, and the claim is not verified.

### Per-observable three-way merge

For one input vector `xs` and one output position `i`, let `o`, `a`, `b`, `m`
be the corresponding observable values of O, A, B, M. Define:

```text
merge(o,a,b) =
  if a = o and b = o then o
  else if a ≠ o and b = o then a
  else if a = o and b ≠ o then b
  else if a = b then a
  else CONFLICT
```

`Merge(O,A,B,M)` holds iff, for every `xs` and every position `i`,
`merge(o,a,b) ≠ CONFLICT` and `m = merge(o,a,b)`.

Equivalently:

- if A changes an observable from O, M preserves A's result;
- if B changes it, M preserves B's result;
- if neither changes it, M preserves O;
- if both change it to the same value, M preserves that value;
- if both change it incompatibly, the relation is `CONFLICT`, not a predicate
  that M can satisfy by choosing one side.

The comparison covers the **full output stack**, including positions that look
unchanged. It is per input vector, not per source location or recipe event.

### Conflicts, absent outputs and abnormal outcomes

- **Conflict.** If any `(xs,i)` yields `CONFLICT`, the checked outcome is
  `conflict` with a validated input witness and both variant observations. A
  candidate M is rejected regardless of its value; there is no merge value to
  verify. A conflict is not `invalid-candidate`: M may be well-typed and may
  equal one variant, but the merge relation is undefined.
- **Absent output.** A version that does not produce the complete declared
  output stack on a normal return, or whose output stack type differs, is
  `incompatible-interface`/`abnormal-outcome`; it is never treated as a
  missing value that M may ignore.
- **Abnormal outcome.** In `SMERGE/1` the terminating fragment rules this out
  for admitted subjects. The checker MUST still detect a subject outside the
  fragment and MUST NOT verify a claim conditional on a normal return that did
  not occur.
- **Simultaneously incompatible changes.** Two variants that change the same
  observable to different values are exactly the fourth row of `merge`; they
  are a conflict, not a priority rule. Variant order A-then-B or B-then-A MUST
  NOT silently decide.

### Preconditions and non-vacuity

`SMERGE/1` requires `preconditions = true` and a nonempty input type for every
input position (`Unit`, `Bool`, `I64`, `Text` and every finite `List` are
nonempty; an empty sum or uninhabited recursive type is outside the fragment).
A later revision MAY add a decidable input precondition `P(xs)`, but then the
claim is conditional and the evidence MUST carry either a witness `xs` with
`P(xs)` or a proved non-vacuity obligation. A `requires false` candidate, or a
precondition that excludes the entire input domain, is rejected as
`vacuous-precondition`; it MUST NOT produce a verified merge.

A partial-correctness reading is **not** selected. Assuming four normal returns
without a termination argument would make a merge verified vacuously when one
version aborts or diverges. VC-LOGIC-01 and VC-LOGIC-03 already forbid
describing such a result as termination or total correctness.

## Terminating fragment

`SMERGE/1` admits only:

- closed, monomorphic, pure programs with empty effect bounds;
- first-order data values (`Unit`, `Bool`, wrapping `I64`, `Text`, `Pair`,
  `Sum`, `List`) and the corresponding constructors/eliminators;
- stack operations, wrapping arithmetic, Boolean equality/`if`, and data
  matches;
- nonrecursive named definitions whose static dependency graph is acyclic;
- statically constructed closed quotations over first-order data whose
  invocation (`quote`/`run`) has an acyclic call graph and whose captured value
  is immutable first-order data.

**Termination argument (obligation, not a claim of this change).** Every
program is a finite straight-line expression; branches are finite. A named or
quoted call descends in the static definition/quotation DAG, which is acyclic,
so the call depth is bounded by the DAG height. Every builtin operation is
total on well-typed data; list and sum eliminators consume one constructor and
do not iterate unless they make a call, which descends. Wrapping `I64`
arithmetic is total. Therefore every execution reaches a normal return for
every well-typed input. Purity alone is not the argument; a pure program with
recursion could diverge, which is why recursion is excluded.

**Excluded:** recursion, loops, `self`, runtime-selected or
runtime-generated Programs, `compose`, Program inputs/outputs, host effects,
resources, live dispatch/reload, async, external calls, and any construct that
depends on a host response or an implementation quota. An admitted subject that
uses one of these is `unsupported-fragment`. Installed execution under finite
fuel/memory may abort; that is `abnormal-outcome`/`exhausted` and is outside
the ideal-model proof (see #10).

## Claim elaboration and Lean checking

### `noble-merge-claim/v1`

For consumer-selected subjects `O,A,B,M`, common interface and observable
list, the claim is the Lean proposition:

```text
MergeRel O A B M  :=
  ∀ xs,  no conflict in merge(Obs_O(xs), Obs_A(xs), Obs_B(xs))
      ∧  Obs_M(xs) = merge(Obs_O(xs), Obs_A(xs), Obs_B(xs))
```

`MergeRel` is defined over the existing pure reference semantics, not over
Wasm bytes, source text or a new unverified interpreter. `Obs_X` is the
normal-return output vector, `merge` is the function above, and value equality
is structural with wrapping `I64`. The termination theorem for the fragment is
a separate Lean theorem; it is not derived from purity and is not assumed by
`MergeRel`.

The claim generator is an **untrusted producer**. The consumer:

1. selects O/A/B/M, the common interface and the observable list itself;
2. regenerates the exact expected claim from the retained preparations and
   the semantic model (VC-EVIDENCE-01/02);
3. checks the supplied Lean proof against that claim with the pinned isolated
   consumer;
4. rejects any producer-supplied subject, claim, assumption, hash or trust
   flag that differs.

The existing bounded limits, diagnostic kinds and `export_lean` surface are
extended only by a new versioned merge claim; ordinary `prepare` does not run a
prover.

### Optional bounded automation and counterexamples

A bounded counterexample search MAY execute O/A/B/M on generated input vectors
and compare full output stacks. A mismatch becomes
`validated-counterexample` only after:

- independent replay by the consumer;
- confirmation that the inputs and outputs satisfy the declared types;
- confirmation that all four subjects are the selected exact identities;
- retention of the raw observations and a reproducible seed/revision.

A counterexample refutes the universal claim. A passing test corpus does not
prove it. The search is not a second semantics and cannot certify a merge.

### Outcomes

The checker reports exactly:

| Outcome | Meaning |
|---|---|
| `verified` | Lean accepted the exact regenerated `MergeRel` claim for the selected subjects |
| `conflict` | a validated input/position where A and B change O incompatibly; no merge exists |
| `validated-counterexample` | a validated input where M differs from `merge(O,A,B)` |
| `unknown` | neither a proof nor a counterexample within the bounds |
| `unsupported` | fragment, interface, observable, capture or feature outside `SMERGE/1` |
| `exhausted` | a stated work/time/memory bound was reached |
| `internal-failure` | checker or tool failure |

All non-`verified` states remain visibly nonverified. `conflict`,
`validated-counterexample`, `unknown`, `unsupported`, `exhausted` and
`internal-failure` MUST NOT be rendered as success, and a truncated search MUST
NOT be reported as `verified` or as a refutation.

### Evidence binding `noble-merge-evidence/v1`

The evidence object binds at least:

- claim/relation revision (`noble-merge-claim/v1`), semantic and IR revisions;
- exact O/A/B/M subjects: `ProgramValueId`, canonical recipe hash, instantiated
  input/output interfaces, dependency closure identities, schema/builtin
  identities and capture values;
- the selected observable list and output stack types;
- the precondition (`true` in v1) and non-vacuity evidence;
- the termination theorem/obligation status for the fragment;
- the full regenerated claim (not only a digest), proof bytes and evidence
  class;
- Lean binary/library/source-tree identities, checker limits and policy
  revision;
- the outcome (`verified`, `conflict`, counterexample, `unknown`, ...);
- exact scope: pure/resource-free/static fragment, input domain, excluded host,
  resource, live, async and runtime-generated Program features;
- the separate status of source-model proof and installed Wasm correspondence.

A digest is an index, not proof. Evidence for a different subject, capture,
dependency closure, interface, observable list or semantic revision is stale
and MUST be rejected, even when the displayed names or effect sets coincide.

## Fixture plan

Common interface for the I64 pairs is input `[I64 I64]` (bottom-to-top `x y`)
and output `[I64 I64]`. All cases are design-local and `absent / not-run /
open / unassessed`.

### Positive

| Case | O | A | B | M | Expected |
|---|---|---|---|---|---|
| SMERGE-CASE-01 independent changes | `[ ]` | `[ 1 + ]` (changes top `y`) | `[ swap 1 + swap ]` (changes bottom `x`) | `[ swap 1 + swap 1 + ]` | `verified`; both independent changes preserved |
| SMERGE-CASE-02 equivalent edits | `[ ]` | `[ 1 + ]` | `[ 2 + 1 - ]` | `[ 1 + ]` | `verified`; wrapping equality, not recipe equality |
| SMERGE-CASE-03 one-sided change | `[ ]` | `[ 1 + ]` | `[ ]` | `[ 1 + ]` | `verified` |
| SMERGE-CASE-04 both change identically | `[ ]` | `[ 1 + ]` | `[ 1 + ]` | `[ 1 + ]` | `verified` |
| SMERGE-CASE-05 captured value | `[ ]` | `[ 5 quote run + ]` | `[ ]` | `[ 5 quote run + ]` | `verified`; the capture identity/value is bound |

### Negative

| Case | Prepared input | Expected |
|---|---|---|
| SMERGE-CASE-06 lost change | O `[ ]`, A `[ 1 + ]`, B `[ ]`, M `[ ]` | `validated-counterexample` at any `y`; `M` does not preserve A |
| SMERGE-CASE-07 incompatible edits | O `[ ]`, A `[ 1 + ]`, B `[ 2 + ]`, M `[ 1 + ]` | `conflict`; neither M value verifies |
| SMERGE-CASE-08 wrapping boundary | O `[ ]`, A `[ 1 + ]`, B `[ ]`, M `[ 2 + ]` | `validated-counterexample` at `y = MAX` (`A = 0`, `M = 1`) |
| SMERGE-CASE-09 missing observation | M has output stack `[I64]` while O/A/B have `[I64 I64]` | `incompatible-interface`; not a merge |
| SMERGE-CASE-10 changed callee | O/A/B call `def inc [ 1 + ]`; M calls a different definition `inc2` with the same body | stale evidence for M is refused; a fresh claim may be checked separately, but M is not the same subject |
| SMERGE-CASE-11 changed capture | A captures `5`, M captures `6` | `validated-counterexample` at an input where the captured value changes the result; capture identity/value is bound |
| SMERGE-CASE-12 wrong proof | a valid proof of `MergeRel O A B M'` or of a different observable list | rejected: the theorem does not match the intended claim |
| SMERGE-CASE-13 stale identity | proof receipt names the old source/revision after an edit | rejected; no subject reuse |
| SMERGE-CASE-14 abnormal/unsupported | a version uses a host effect, resource, `compose` or recursion | `unsupported-fragment`, not vacuous verification |
| SMERGE-CASE-15 vacuous precondition | candidate declares `requires false` | `vacuous-precondition`; no verified merge |
| SMERGE-CASE-16 outcome visibility | `unknown`, `exhausted`, `internal-failure` receipts | rendered as nonverified; no success flag |

A test corpus alone cannot yield a universal proof. The positive cases are
regression designs for the claim generator, the relation and the evidence
binding; the negatives test refusal and stale-identity behavior.

## Model and proof outcome, go/no-go

### Phase 2 model (retained)

`crates/noble-contracts/tests/models/semantic-merge-model.rs` implements a bounded
stack model of the relation over 25 two-I64 input vectors:

```text
fixtures 10
verified 4
conflicts 1
counterexamples 3
incompatible interfaces 1
abnormal outcomes 1
```

The fixtures cover independent changes, equivalent edits, one-sided and
identical changes, lost changes, incompatible edits, wrapping `I64`, missing
observations, changed captures, abnormal programs and evidence refusals. The
model also checks that subject identity binds captures and callees, that a
wrong proof or stale revision is refused, and that an abnormal or vacuous case
is never verified. `verification/semantic-merge-model/run.mjs` records the
source-bound receipt; that receipt is retained unchanged. Its recorded decision
was no-go at that revision, because the Lean relation, termination theorem,
claim generator and isolated checker did not exist yet.

### Phase 3: selected by the owner

OWNER DECISION 2026-11-20: Phase 3 is selected for implementation (it was
previously "not selected by the no-go decision"). The recorded scope is the
Phase 3 tasks: the Lean `MergeRel`/termination obligations, the untrusted claim
generator with consumer regeneration and the isolated pinned Lean check, the
SM-3..SM-8 dispositions, and a separately reviewable integration plan. It
authorizes no integration, merge, publication, live-slot admission,
source-control migration or release.

### Pinned Lean model (commit `79be46d4`)

`proofs/mc1/NobleContracts/Merge.lean` (Mathlib-free, `Std`/`NobleContracts`
imports only) encodes the relation over the existing `Exec` reference
semantics:

- `ObservableTy`/`FirstOrderValue` restrict observables to `Unit`, `Bool`,
  wrapping `I64`, `Text`, `Pair`, `Sum` and `List`; `ObservableTy` admits no
  `Program`/`Syntax` value, and `firstOrder_of_observable` proves an observable
  stack holds no program value.
- `MergeVal o a b m` has exactly the four non-conflicting rows of the design's
  `merge`; `Conflict o a b` is "no value satisfies the merge";
  `MergeStack` is the pointwise lift over the complete ordered output stack.
- `MergeRel inputTys outputTys O A B M` quantifies over every well-typed input
  vector and every normal return: the four subjects must return normally and
  every candidate return must be the merge. Quantifying over all normal returns
  makes the encoding independent of evaluator determinism; the design's
  `Obs_X(xs)` is recovered per concrete subject, and the fixture theorems below
  prove those returns unique by chaining the `exec_*_iff` rules.
- `OuterCode`/`LeafCode`/`LeafValue` (a mutual definition) express the proved
  terminating fragment; `outer_exec` is its termination theorem and
  `mergeRel_terminating` states that a fragment subject's four normal returns
  always exist.

SM-1 is discharged by `mergeVal_functional`, `mergeVal_total`,
`mergeVal_comm`, `mergeStack_functional`, `mergeStack_comm`,
`mergeStack_lengths` ("full ordered stack, never a per-position skip"),
`mergeStack_position` (per-position extraction), `mergeStack_append`,
`i64_value_eq` and `wrap_max_add_one`. The wrapping reading is fixed by
`BitVec 64` equality; `mergeRel_identical_increment` and
`mergeRel_independent_changes` prove the case-04 and case-01 instances, and
`wrapping_boundary_is_not_merge` proves that a candidate reading a
mathematical integer is not the merge at the wrapping boundary.

**Narrowing of SM-2.** The theorem proves a strict sub-fragment of the
design's. Leaf code is straight-line dataflow (`CoreOp`/`LeafCode`) whose
literal values may themselves contain only leaf values (`LeafValue`); outer
code adds statically embedded leaf blocks, quotation of a stack value that the
invariant proves is a leaf, and the eliminators `dip`/`run`/`case`/`if`/
`listCase`, all of which execute stack values that `AllLeaf` proves are leaf
code. The written reasons for the remaining narrowing:

- the MC1 model has no named definitions, so the design's "nonrecursive named
  definitions with an acyclic dependency graph" is not formalized here; a
  subject using named definitions is outside the proved fragment and must be
  refused as `unsupported-fragment` until that obligation is proved;
- the model has no static definition store, so "the call graph descends in the
  static DAG" is replaced by the stronger static condition that every embedded
  program is a leaf and every invoked stack value is a leaf; this is what makes
  the termination argument a finite induction rather than a creation-ancestry
  invariant;
- `compose`, `reflect`, `self`, host effects, resources, live dispatch, async,
  external calls, runtime-generated Programs and quota-dependent execution are
  excluded from both definitions and therefore from the theorem, matching the
  design's excluded set. A subject using one of them is `unsupported-fragment`.

The axiom audit of every public theorem in the module reports only `propext`,
`Classical.choice` and `Quot.sound` (several depend on no axiom at all), inside
the mc1 allowlist. No `sorry`, `admit`, custom `axiom`, `native_decide`,
`opaque`, `partial`, `#eval` or `run_cmd` occurs in the module.

### Claim lane (commits `5c2d19d3`, `0a387027`)

The Phase 3 lane lives beside the model, as `#[path]` modules of the existing
`source` test target: `merge/claim.rs`, `merge/consumer.rs`, `merge/lean.rs`
and `merge/verification.rs`.

- **Regeneration.** `Preparation` is the consumer's retained record of one
  subject (program, input/output observables). `regenerate` selects the four
  preparations, the common interface and the observable list itself, requires
  exact interface equality, `preconditions = true` and a nonempty input domain,
  lowers each program to MC1 code and renders the exact
  `MergeRel <in> <out> O A B M` proposition. The lowering maps `Push`/capture
  to `Op.lit (Value.i64 (BitVec.ofNat 64 n))`, `Add`/`Sub`/`Swap`/`Drop` to
  `Op.word 4/5/2/1`; captures are inlined as literals and remain bound by the
  subject digest, so the live `quote`/`run` form is covered by the Lean
  fragment theorem, not by this lane's lowering.
- **Subject identity.** `subject_digest` is SHA-256 over length-delimited
  fields: semantic revision, ordered input and output types, every op with its
  code and payload, the capture list and the callee. A changed callee or
  capture is a different subject even when the code coincides.
- **Admission.** `admit` regenerates and then refuses every differing producer
  field: schema, relation/revision, assumptions (must be empty), trust flag
  (must be false), the four subject digests, the claim text, the declared claim
  hash and a premarked `verified` outcome.
- **Outcomes.** `Outcome` has exactly the seven design states; `name` renders
  them and only `Verified` is success. `checked` is the single constructor of
  `Verified` and requires the admitted proof bytes plus an axiom set inside
  `propext`/`Classical.choice`/`Quot.sound`. `evaluate` orders the steps:
  refusal, then a validated refutation (which dominates and does not consult
  the checker), then the isolated check.
- **Search.** `replay` re-checks the input types, re-runs the four exact
  subjects on a fresh stack and compares the full output stack. `search` runs
  at most 25 vectors from a retained seed; a mismatch is replayed before it
  becomes `validated-counterexample`, a conflict is `conflict`, and a passing
  or truncated corpus stays `unknown`.
- **Isolated check.** `lean::check` refuses an over-bound claim/proof, a proof
  line that could smuggle a command, a library that does not select
  `leanprover/lean4:v4.35.0`, or a missing pinned `NobleContracts.Merge`
  module. It then writes a consumer that imports only `Lean` and
  `NobleContracts.Merge`, kernel-checks `merge_claim : <regenerated claim>`,
  walks the theorem's axioms against the allowlist and prints
  `NOBLE-MERGE-ACCEPT`; the child runs with a fixed `LEAN_PATH` and a wall
  deadline, and a timeout is `exhausted`. The check also records a digest of
  the pinned `Merge`/`Rules`/`Model` sources and the `lean --version` identity.

Phase 3 tests: `claim_regeneration_is_exact_and_canonical`,
`consumer_refuses_every_differing_producer_field`,
`interface_observable_and_precondition_refusals`,
`bounded_search_validates_witnesses_and_never_certifies`,
`outcome_discipline_is_total_and_visible`,
`search_receipts_retain_the_witness`, and the `#[ignore]`d
`semantic_merge_lean_check` (run by the lane).

### Obligation dispositions

| Id | Disposition | Evidence / written reason |
|---|---|---|
| SM-1 | **Proved** | `MergeRel` and the `mergeVal_*`/`mergeStack_*` theorems in `proofs/mc1/NobleContracts/Merge.lean`; case-01/04 instances; wrapping-boundary theorem |
| SM-2 | **Proved for the narrowed fragment** | `outer_exec`, `mergeRel_terminating`, `leaf_exec`; narrowing and reasons above |
| SM-3 | **Narrowed and tested** | The claim is regenerated from consumer-selected preparations and lowered to exact MC1 subjects; there is no Lean formalization of the preparation pipeline, so elaboration is enforced by exact-text regeneration plus the subject digests, and is tested rather than proved (`claim_regeneration_is_exact_and_canonical`, `consumer_refuses_every_differing_producer_field`). The `Push`/capture lowering is documented above as this lane's exact subject form. |
| SM-4 | **Narrowed** | The isolated check is a fresh pinned Lean process over a pinned source digest with the mc1 axiom allowlist; kernel soundness is Lean's, and refinement of the MC1 reference semantics against the design's model is not re-proved here. The repository's production bubblewrap consumer is not wired and no production claim path exists; that integration is unselected. |
| SM-5 | **Tested** | `admit` refuses every differing producer field; `subject_digest` binds code, captures, callee, interface and revision; the digest is recorded as an integrity index, not proof validity |
| SM-6 | **Tested** | The seven-state `Outcome`, the single `checked` constructor of `Verified`, refutation dominance and the never-verified truncated/passing search are exercised by `outcome_discipline_is_total_and_visible` |
| SM-7 | **Documented separate** | The lane proves source-model facts only; no installed Wasm execution claim, compiler refinement or artifact correspondence is made (owned by #10/PO-18) |
| SM-8 | **Documented** | The lane performs no merge, publication, live-slot admission, migration or release; the receipt records the no-go decision |

### Separately reviewable integration plan

This plan is written for the independent reviewer the integrator schedules
(OWNER DECISION 2026-11-20 item 1). Each step names its blocker; no step is
implemented by the Phase 3 lane.

1. **Production claim module.** Move `noble-merge-claim/v1` and
   `noble-merge-evidence/v1` into a `noble-contracts` module with the crate's
   `Limits`, diagnostics and `export_lean` surface; `prepare` must not run a
   prover. Blocker: an owner decision to integrate plus the architecture
   capacity review (Octet's fixed unit budget).
2. **Consumer regeneration from prepared subjects.** Replace the test lane's
   lowered `Program` records with `Prepared` subject metadata (`ProgramValueId`,
   canonical recipe, instantiated interfaces, dependency closure, captures,
   schema/builtin identities, semantic revision) and regenerate the claim from
   those. Blocker: #15's identity/linkage rules for captured subjects.
3. **Production isolated consumer.** Extend the sandboxed consumer with the
   `NobleContracts.Merge` import closure and the merge axiom policy, replacing
   the lane's process-level check. Blocker: a pinned-model extension of
   `workflow::intrinsic` (the exact MC1 model source pins) and its review.
4. **Evidence envelope.** Emit the full `noble-merge-evidence/v1` field list
   (toolchain/library/source-tree identities, checker limits, policy revision,
   precondition/non-vacuity evidence, termination-obligation status, exact
   scope). Blocker: the production consumer from step 3.
5. **Fragment admission.** Encode the narrowed fragment as a subject check that
   refuses named definitions, deeper quotations, `compose`/`reflect`, host
   effects, resources, live dispatch, async and runtime-generated Programs as
   `unsupported-fragment`. Blocker: extending the Lean theorem to named
   definitions or restricting the admitted subjects in the checker.
6. **Frozen fixture matrix.** Run the full SMERGE-CASE list against frozen
   subjects with retained raw observations, claims and proofs, including the
   outcomes that need a real bound (`exhausted` after the production deadline)
   and the abnormal/unsupported refusals the lane can only model.
7. **Independent review.** Retain the reviewer's record with the reviewed
   commit, findings and verdict in this change. The owner keeps a veto.
8. **Opt-in selection only.** If steps 1-7 pass, select the check behind an
   explicit opt-in flag; still no automatic merge, publication, live-slot
   admission or release permission.

### Re-evaluated go/no-go

The frozen criteria ("reopen only after those obligations are discharged and a
separately reviewed integration plan exists") are **partly** met: SM-1 and
narrowed SM-2 are proved in the pinned model, SM-3/SM-4 are narrowed with
written reasons, and SM-5/SM-6 are tested; the integration plan is written.
The decision is **NO-GO for integration at this revision**, with reasons:

- SM-4 remains narrowed: the isolated check is process-level and the
  production sandboxed consumer is not wired.
- step 7 of the plan (independent review) is pending; the plan is reviewable
  but not reviewed.
- SM-7 stays a separate obligation, so a source-model proof does not transfer
  to an installed artifact.

The evidence is `verification/semantic-merge-verification/receipt.json`
(sha256 `d059a8153800b8982d080830618ab49465980d36860d049e4d64584a6bdef21b`,
clean tree at `9338c66d`) and its successor
`verification/semantic-merge-verification/receipt-3beddcd9.json`
(sha256 `e1382407a04bafec3f8e096b3b041d37d8f4976df4fe5f8c8904ff0947af367c`,
clean tree at `3beddcd9`, after `e471a9e7` pinned the 34 Merge theorems in
`proofs/mc1/ContractGate.lean`), and
`verification/semantic-merge-verification/receipt-4a2bb986.json`
(sha256 `6d2645d847415d8a54c6d89b5d31bdcbc4ae0e1ac35ef444ef15b777368f6f87`,
clean tree at `4a2bb986`), and its final successor
`verification/semantic-merge-verification/receipt-d515869b.json`
(sha256 `ad17ee14a7d4e675a5157b6b67cdfd2da150a86e630f811424e0f3c07e3dba81`,
clean tree at `d515869b`). Both regenerated claims (`SMERGE-CASE-04`, `SMERGE-CASE-01`) were
kernel-checked by the pinned model with axioms `propext`/`Quot.sound`, the
Phase 2 lane was re-run unchanged, and the wrong-proof and command-submission
controls stayed `unknown`.

No claim generator is wired into `prepare`, no admission path exists, no merge,
branch, live slot, migration or release action follows, and the production
surfaces are unchanged. The GO condition is the completion of plan steps 1-7
with the reviewer's record.

## Proof obligations

| Id | Obligation | Relation |
|---|---|---|
| SM-1 | `MergeRel` is the per-observable three-way relation over the full ordered output stack, with structural wrapping-`I64` equality | VC-REWRITE-02, issue #30 |
| SM-2 | Every `SMERGE/1` subject terminates normally on every well-typed input; purity is not the premise | VC-LOGIC-03 slice |
| SM-3 | Claim elaboration from the four exact preparations yields exactly `MergeRel` with the selected observables and capture/dependency identities | VC-ID-02, VC-EVIDENCE-02 |
| SM-4 | The Lean checker is sound for the reference semantics and the isolated consumer accepts only the regenerated claim under the selected policy | PO-09/PO-16 slices |
| SM-5 | Evidence binding rejects any changed subject, capture, dependency, interface, observable or revision | VC-EVIDENCE-01/02 |
| SM-6 | Outcome discipline is total: every run produces one of the listed states, and only Lean acceptance yields `verified` | V-CHECK-04 |
| SM-7 | Source-model proof and installed Wasm correspondence remain separate | #10, PO-18 slice |
| SM-8 | No automatic merge, publication, live-slot admission, source-control migration or release permission follows | issue #30 |

SM-1 and narrowed SM-2 are proved in the pinned model; SM-3 and SM-4 are
narrowed with written reasons; SM-5 and SM-6 are tested by the Phase 3 lane;
SM-7 and SM-8 are documented. No disposition claims more than its evidence:
the termination theorem is a new obligation and is not inherited from an
effect set, purity or the existing partial-correctness claim, and the
source-model proof does not transfer to an installed artifact.

## Evaluation, go/no-go and coordination

**Experimental plan.**

1. Encode `MergeRel` and the fragment's termination theorem in the pinned Lean
   model; review the statements before any proof attempt.
2. Implement the untrusted claim generator and the consumer's independent
   regeneration; refuse any field not regenerated by the consumer.
3. Implement the isolated Lean check and the optional bounded counterexample
   search with reproducible seeds and raw receipts.
4. Execute the fixture plan against frozen subjects; retain every raw
   observation, claim, proof and tool identity.
5. Record positive, conflict, counterexample, unknown, unsupported, exhausted
   and internal outcomes exactly.

**Go/no-go.** Steps 1-6 of the experimental plan are executed by the Phase 3
lane: the statement and proofs are in the pinned model, the claim generator and
consumer regeneration refuse producer-supplied fields, the isolated check and
the bounded search retain reproducible evidence, and the observed outcomes are
recorded in `verification/semantic-merge-verification/receipt.json`. The
re-evaluated decision is **no-go for integration at this revision**, for the
reasons in "Re-evaluated go/no-go": SM-4 is narrowed rather than discharged,
and the integration plan awaits its independent review. A successful check
would still not be an automatic merge: no merge is performed, no branch is
published, no live slot is admitted and no release permission is granted.

**Coordination.** #15 owns preservation, effect safety and the broader proof
foundation; #10 owns proof-to-executed-artifact correspondence, so an installed
Wasm execution claim needs its own envelope; #6 owns proof automation and
diagnostics; #9 owns broader behavioral-effect protocols. Valence validates
identity/linkage, Kiln interprets CI policy, and Octet owns a separate Rust
implementation rail. This change owns only the Noble semantic relation and its
claim/evidence design.

## Acceptance mapping (issue #30)

| Issue criterion | Section |
|---|---|
| Same-input execution; per-observable comparison over the full output stack; conflict/absent/incompatible rules | "Decision: `SMERGE/1` relation" |
| Wrapping I64, exact dependencies, interfaces and captures; no name/location/template identity | "Versions, interfaces and observables"; "Evidence binding" |
| Normal-return versus termination; no vacuous acceptance | "Preconditions and non-vacuity"; "Terminating fragment" |
| Exclude host effects, resources, live dispatch, concurrency, runtime-generated Programs | "Terminating fragment" |
| Claim elaboration and isolated Lean checking; separate supplied-proof checking, automation and counterexample search | "Claim elaboration and Lean checking" |
| Outcome preservation, including unknown/exhausted/internal | "Outcomes" |
| Versioned evidence binding for O/A/B/M, claim, captures, model, toolchain and scope | "Evidence binding `noble-merge-evidence/v1`" |
| Positive and negative fixtures | "Fixture plan" |
| Reviewed design, proof obligations, experimental plan and go/no-go | "Proof obligations"; "Evaluation, go/no-go and coordination" |

## Risks and non-claims

- The per-observable relation is intentionally not full contextual equivalence.
  It compares normal-return output values for the selected interface; it says
  nothing about traces, resources, effects, recipe equality or behavior under
  other inputs/interfaces.
- Structural covariance or a `Top` type would enlarge the candidate space but
  is not part of this relation and would need its own proofs. Nothing here
  imports F-sub-top.
- The terminating fragment is narrow. That is the price of avoiding vacuous
  acceptance; extending it to partial correctness requires an explicit
  termination/abnormal premise, not a relabel.
- Lean acceptance is evidence for the encoded claim under the pinned model and
  toolchain. It is not a verified compiler, Wasm engine, host or merge
  implementation, and it does not make a candidate publishable.
- A four-version relation does not define a unique merged source, resolve
  textual conflicts or replace review. It checks a supplied candidate against
  an exact semantic relation.
- The Phase 3 lane's claim is over the MC1 code its consumer lowering produces
  from the retained preparations. It is not a claim that the lowered code is
  the live quotation form (the Lean fragment admits `quote`/`run`, and the
  capture stays bound by the subject digest), and not a claim about the
  repository's production claim path.
- The Phase 3 check isolates a fresh pinned Lean process with a fixed
  `LEAN_PATH`, a pinned source digest and the mc1 axiom allowlist. It is not
  the production bubblewrap sandbox, and its acceptance is not a policy
  decision, an admission or a release.
