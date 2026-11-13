## Selected fragment

One configuration separates an opaque ordered *segment* that no word inspects
from an ordered *operand stack* that `dup`, `drop`, `swap` and `quote` touch. A
payload is a data reference, a must-use resource or a wrapped payload, so
eligibility is recursive. The data-constraining words `dup`, `drop` and
`quote` require a duplicable operand; `swap` only reorders its two operands and
has no eligibility check. A payload is duplicable exactly when it contains no
resource at any depth. Dropping a data reference is therefore not releasing a
host resource, and no word in this fragment can release a must-use obligation.

## Checker and judgment

`step` is the executable one-step checker; `run` composes it, admits a
two-arm branch only when both arms leave the same operand accounting, and
admits the ordinary error return only when no operand is outstanding.
`Accounts` is the declarative judgment written independently of `run`, with one
constructor per word, composition, branch and the empty error return, and
`run_sound` proves that every checker acceptance has a judgment. Theorems
`branch_accounts_both_arms` and `fail_requires_empty_operands` state the two
structural obligations directly.

## Accounting claims

`duplicable_iff` reduces eligibility to the absence of a resource at any depth;
`wrapDepth_preserves_eligibility` and `wrapDepth_resource_not_duplicable` state
that no wrapping depth changes that; `resource_operand_blocks_non_swap_word`
states that a must-use resource blocks every data-constraining word, while
`swap_permutes_two` and `swap_moves_a_resource` state that swap reorders both
data and resources without an eligibility check. `drop_consumes_one` and
`dup_duplicates_one` can only ever move data obligations;
`quote_retains_one` states that quoting retains rather than consumes.
`segment_conserved` is only the narrower extraction lemma that a configuration
result already carrying the original segment yields the operand `run` result;
`runConfiguration_segment_preserved` proves the segment equality for an
arbitrary successful configuration result. The conservation unit then proves
`step_conserves_obligations`,
`accounts_conserves_obligations` and `run_conserves_obligations` for the
filtered resource list, with concrete data-word/resource-tail and
resource-moving swap fixtures.

This is exact for resource-moving behavior: this grammar has no admitted
acquire, release or resource-consuming word, but swap may move a resource while
the data-constraining words reject one in their constrained position.

## Correspondence and its open edge

The comparison route is handwritten. `verification/ownership/cases.mjs` is a
JavaScript mirror of the Lean model, never of the acceptance machine; it proposes
expected rows but does not execute Lean to print them. `OwnershipCorpus.lean`
kernel-checks each proposed `run` and `runTrace` result, so a mirror disagreement
fails the foundation build. Its 116 cases
cover operand shapes (including recursively wrapped payloads), nine programmes,
branch/resource controls and the error return. `OwnershipCorpus.lean` makes the
kernel decide each row; the Rust lane checks the emitted row and drives 28
accepted and 17 rejected cases through `noble_kernel::acceptance::check` one word
prefix at a time. The test-local shadow checker agrees with every row, and its
six declared rule mutations are caught. A separate production mutation lane
catches five real rule changes and records the surviving quotation mutation,
which has no production analogue. Branch/resource controls cover loss,
duplication, wrong identity and movement; owner identity remains model-only.

Model-only rows name their reason: branch accounting, error return, the
quotation constructor's program value, wrapped payloads or operand arity. The
production rejection class is the machine's own; the lane pins it per case so a
lost word rule changes the class instead of hiding behind an unchanged rejected
total.

### Per-word comparison and the kernel-decided corpus (2026-11-13)

The original lane compared each accepted case only on its final stack, so an
intermediate divergence that cancelled out by the end went unseen, and it could
not drive `swap` at all. Both are now closed.

`OwnershipTrace.lean` gives the model a total per-word trace (`traceWords`), the
flattening of a straight-line program to its word sequence (`straightLine`), the
checker over that sequence (`runWords`) and the program-level `runTrace`.
`run_eq_runWords`, `traceWords_length` (the trace has one more entry than the
word list exactly when the checker accepts), `traceWords_take` (the trace of a
word prefix is the prefix of the trace) and `runWords_prefix_accepted` carry the
claims the lane relies on; `traceWords_last` and `runTrace_getLast` state that
the last entry is the checker result. `cases.mjs` proposes rows using its
handwritten Lean-model mirror and emits
`NobleFoundation/OwnershipCorpus.lean`, where the kernel `decide`s `run` and
`runTrace` for every corpus row: 116 rows, so a corpus expectation that does not
follow from the model fails the foundation build. `cases.mjs --check` fails when
the committed corpus differs from the emission, and the Rust lane compares every
corpus row with its kernel-checked twin.

The production lane now drives one candidate per word prefix. After `k` words the
machine's derived stack must equal trace entry `k`, so the first diverging word
is reported. The bindings of each invocation are the value types the trace records
for the stack that word actually sees; that is what makes `swap` drivable (the
machine instantiates one scheme per word, so a swap prefix derives its reordered
stack) and what exposed the earlier final-stack-only lane's blind spot. 28
accepted and 17 rejected cases are driven over 60 prefix comparisons; every
rejected case refuses at the word the model refuses, in the eligibility class.

Owner identity remains unobservable: every resource payload has the same kernel
type, so the equal-count/different-identity control stays model-only. Every
model-only row states why (`branch-accounting`, `error-return`,
`quotation-program-value`, `wrapped-payload`, `operand-arity`) and the receipt
reports the counts by reason.

Two mutation lanes are now separate. The corpus mutations vary a test-local copy
of the model (all six caught, the unmutated copy agrees everywhere).
`verification/ownership/production-mutants.mjs` applies one exact byte
replacement to a production file in a scratch copy of the clean commit and
requires the lane to fail: releasing the `Drop` or `Dup` data slot, dropping an
operand in the `swap` scheme, making `swap` an identity and making `dup`
produce one copy are caught; releasing the `Quote` data slot survives because
every `quote` case is model-only, and the survivor is recorded rather than
counted as coverage. A failing comparison writes the shortest failing word
prefix and trims trailing input operands while the divergence persists to
`verification/ownership/minimised.txt`; leading operands stay because they may
be the head a word inspects. The minimiser is exercised by a synthetic probe.
When minimizing an accepted production control, it also preserves model
acceptance, so a real mismatch cannot collapse into an unrelated model-only
operand-arity refusal.

### Phase 3 decision (2026-11-13): production-checker refinement not selected

**Decision.** This change does not select an extraction or refinement route from
`crates/noble-kernel/src/acceptance` to the model. The correspondence stays the
handwritten, kernel-decided corpus comparison.

**Evidence.**

- The issue's own deliverable 5 says a handwritten translation or finite
  differential agreement leaves production-checker refinement open and MUST NOT be
  promoted to an implementation theorem; the body asks for the open edge to be
  recorded, not discharged.
- The comparison is now at every word prefix and every corpus row is decided by
  the kernel, so the remaining gap is the route, not the expectations: a refinement
  proof needs an extraction subject for the acceptance machine's candidate
  derivation, a re-audited axiom set and a clean-source receipt for that route.
  None of that exists in this change, and manufacturing it would be a new change
  with its own reviewed subject list.
- Nothing in the lane needs the refinement to be useful: the differential fails
  when a production rule changes (the production-mutants receipt) and when the
  model changes (the kernel build and the freshness check).

**Criteria a future refinement route would have to meet.** (1) Name the extraction
subject covering `crates/noble-kernel/src/acceptance`; (2) keep every theorem
inside the foundation axiom allow-list; (3) prove that the production decision
implies the model's `Accounts` for the selected fragment, with the untrusted
candidate parsing included or explicitly excluded; (4) bind a clean-source receipt
whose `sources_sha256` covers the production source.

**Reopen conditions.** The owner decides to fund a separate refinement change;
or the corpus and production disagree at a retained row (which would make the
current evidence insufficient, not merely incomplete); or the acceptance machine
gains a per-word interface that exposes owner identity, which would make the
identity controls executable and the type-only derivation argument obsolete.

The Rust-to-model edge stays open and labelled: no Charon/Aeneas extraction or
refinement proof relates the acceptance machine to the model, and the
differential is finite evidence on one host and target.

## Coverage limits

Future scoped-borrow syntax (OWN-PO-2) is not modelled: no fresh rigid scope
tags, hidden owners, reborrows, aggregates, loops or suspension are added
merely to match an upstream mechanism, so no runtime claim follows for an
unimplemented profile. Package retention/reclamation and the host-ledger
OWN-PO-4..7 obligations stay outside this pilot, and permission obligations are
never erased as ordinary proof facts.
