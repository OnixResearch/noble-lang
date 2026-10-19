# Differential evaluation and proof obligations for finite-candidate subtyping

## ADDED Requirements

### Requirement: FSUB-EVAL-01
r[FSUB-EVAL-01]

Before any integration, the `finite-bases/v0` study MUST implement a separate
explicit-family reference checker and a target-directed/backward checker over
the same declarative judgments. Neither implementation may use production
inference helpers as its expected result. The existing production checker
remains the compatibility reference for today's language.

The two implementations MUST be compared on the same declarative judgments.
Sharing, deduplication and candidate pruning MUST preserve whole-candidate
semantics. Cache keys MUST include binder identities, constraints, substitutions
and comparison-context changes; a stale or cross-context hit is a defect.
Every construction, query, allocation and materialization, including expansion
from a shared representation into owned descriptors, MUST be metered. A cap
MUST return `exhausted`, never a semantic rejection or acceptance.

The corpus MUST include deterministic bounded enumeration plus seeded cases:
effect widening and narrowing attempts, correlated stack/effect alternatives,
nested quotations/composition, shared monomorphic Programs, rigid binders,
exact nominal arguments, real `result.noble` and `structural-map.noble`
examples, and malformed/ill-scoped inputs. Resource and LiveRef inputs are
expected `unsupported`. Negative controls MUST deliberately cross-mix
candidates, reuse a stale comparison after a binder or context change, expand a
shared representation and compare it with the owned descriptor, inject an
effect narrowing, and substitute a different nominal ID with the same display
name.

The evaluation MUST record accepted, rejected, ambiguous, unsupported,
exhausted and internal outcomes; family/DAG size; materialized nodes;
subtype/query counts; charged work; peak storage; and elapsed time at pinned
revisions and budgets. It MUST provide a reproducible command and a receipt
with source revision, toolchain, corpus, seeds, bounds and every discrepancy.
No paper-derived performance guarantee MAY be claimed from these measurements.

#### Scenario: FSUB-CASE-24 stale comparison cache

- GIVEN a cached candidate comparison made before a binder, constraint,
  substitution or context change
- WHEN the checker reuses the cached result after the change
- THEN the evaluation detects the stale hit and reports a defect; the cache
  MUST be invalidated by every identity in the key

#### Scenario: FSUB-CASE-27 cross-mixing negative control

- GIVEN a deliberately constructed result that takes its input from one
  candidate and its output/effect from another
- WHEN the differential checker evaluates it
- THEN both implementations reject it, and the receipt records the control

### Requirement: FSUB-PROOF-01
r[FSUB-PROOF-01]

The extension study MUST label every claim as proved, tested, assumed or open.
It MUST establish or explicitly narrow the following obligations before any
production integration:

1. Decidable subtyping for the selected fragment with a well-founded measure.
2. Sound candidate generation for the declarative relation.
3. Complete candidate coverage of every supported target.
4. Effective finite covers for every supported scope/context change.
5. Complete compositional operations for Noble's `run`, composition,
   quotation and branch-join rules.
6. Termination of subtype checking, family construction and candidate
   operations, separate from bounded implementation exhaustion.
7. A complete backward elimination interface and sound and complete inverse
   substitutions for the actual variables.
8. Algorithm/declarative equivalence of the reference and backward
   implementations.
9. Preservation, effect safety and ownership obligations under #15.
10. No public identity, canonical encoding, persistence, replay or ABI change.

A successful prototype is not a proof. A test corpus alone cannot discharge a
universal claim. If the corpus confirms that the accepted base interface is
principal in every supported context, the go/no-go decision MUST be no-go for
production adoption and the principal-type result MUST be documented. If a
genuine incomparable family is found, adoption still requires discharged
obligations and a separately reviewed integration plan; no automatic source
acceptance, rollout or ABI change follows.

#### Scenario: FSUB-CASE-30 unproved coverage

- GIVEN the prototype accepts a target in its differential corpus but the
  coverage obligation has no proof for that context class
- WHEN the go/no-go review runs
- THEN the result remains a tested observation, the class is labeled open, and
  production adoption is blocked

#### Scenario: FSUB-CASE-33 principal-type no-go

- GIVEN an explicit-family receipt showing one principal candidate in every
  supported context and no unexplained differential disagreement
- WHEN the go/no-go review runs
- THEN the decision is no-go, the current checker and K-EFFECT-05 are retained,
  and no candidate encoding or source-acceptance change is made
