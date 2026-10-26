## Selected fragment

One configuration separates an opaque ordered *segment* that no word inspects
from an ordered *operand stack* that `dup`, `drop`, `swap` and `quote` touch. A
payload is a data reference, a must-use resource or a wrapped payload, so
eligibility is recursive. A word that constrains an operand requires that
operand to be duplicable, and a payload is duplicable exactly when it contains
no resource at any depth. Dropping a data reference is therefore not releasing
a host resource, and no word in this fragment can release a must-use
obligation.

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
that no wrapping depth changes that; `resource_operand_blocks_every_word` states
that a must-use resource as the constrained operand blocks every word, which is
why `drop_consumes_one` and `dup_duplicates_one` can only ever move data
obligations; `quote_retains_one` states that quoting retains rather than
consumes; `segment_conserved` states that the must-use obligations the opaque
segment carries survive every admitted program.

## Correspondence and its open edge

The comparison route is handwritten. `verification/ownership/cases.mjs` is
written from the Lean model, never from the acceptance machine, and generates
104 cases: operand shapes including empty, one data payload of each data type, a
data pair, a must-use resource at each eligible position, and wrapped payloads at
two depths, over nine programmes including the two-arm branch and the error
return. The Rust lane drives the unary fragment (`dup`/`drop` sequences) through
`noble_kernel::acceptance::check`, compares a test-local shadow checker against
every expectation, and declares seven mutations (resource duplication, resource
release, eligibility-free swap, consuming quote, branch ignoring accounting,
error return permitting an outstanding operand) each of which the corpus
catches.

Only an eligibility rejection of nameable payloads by a programme with a
retain-in-place analogue is an executable production control: a production arity
mismatch, the quotation constructor's program value, and the two-operand swap
slot ordering have no retain-in-place analogue and stay model-only. The
production rejection category can also be a stack-join failure rather than
eligibility; the lane asserts the agreement of the accounting decision and
reports the category split rather than pinning one category.

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
