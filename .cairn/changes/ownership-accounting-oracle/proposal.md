## Why

The ownership-lifetime design keeps shared immutable Data, must-use resources and
a future scoped-borrow class distinct, and leaves OWN-PO-1 (whole-configuration
accounting), OWN-PO-2 (scope escape) and OWN-PO-3 (recursive capture
eligibility) as separate obligations. The ordered-stack rules that enforce them
are covered by finite controls only: no independent model names what an
admitted word may consume, which obligations are conserved, or how a two-arm
branch and an ordinary error return are accounted. This change adds a bounded
Lean ownership-accounting oracle for the existing ordered stack/resource rules
(issue #59), adapting the executable-checker-to-declarative-typing architecture
rather than Move's ownership policy. It is not a new source borrow feature.

## What Changes

- `proofs/foundation/NobleFoundation/Ownership.lean` selects one explicit
  fragment: data and must-use resource payloads, recursive wrapping, the four
  words `dup`/`drop`/`swap`/`quote`, composition, a finite two-arm branch and
  the ordinary error-return alternative. It states an executable checker and an
  independent declarative judgment, proves checker soundness, that a payload is
  duplicable exactly when it contains no resource at any depth, that no
  wrapping depth makes a resource eligible, that `swap` moves either data or a
  resource without an eligibility check, that the opaque segment and its
  must-use obligations are conserved, that a branch accounts for the same
  obligations on both arms, and that an error return is admitted only with no
  outstanding operand. r[VC-OWN-01]
- `verification/ownership/cases.mjs` is an independent expectation generator
  written from that model, and `crates/noble-kernel/tests/acceptance/ownership.rs`
  drives the unary fragment through the actual acceptance machine, compares a
  test-local shadow checker against every expectation, and declares seven
  checker mutations each of which the retained corpus catches. r[VC-OWN-02]
- The previous receipt `verification/foundation/receipts/e7cc00993/` records
  the pre-correction model and is retained as historical evidence; the issue
  owner records a fresh clean-source receipt for this correction. The updated
  corpus has 108 cases (15 accepted and 6 rejected through the production
  machine, 87 model-only, seven mutations caught), and the differential receipt
  binds the corrected model, toolchain, case file and tested implementation.
  r[VC-OWN-03]

## Impact

- **Files**: `proofs/foundation/NobleFoundation/Ownership.lean`,
  `proofs/foundation/NobleFoundation.lean`, `proofs/foundation/inventory.json`,
  `verification/ownership/{cases.mjs,cases.txt,differential.mjs,differential.json}`,
  `crates/noble-kernel/tests/acceptance/ownership.rs`,
  `crates/noble-kernel/tests/acceptance.rs`.
- **Not claimed**: no extraction or refinement proof relates
  `crates/noble-kernel/src/acceptance` to the model. Wrapped payloads, the
  two-operand swap slot ordering, the production quotation constructor, two-arm
  accounting and the ordinary error return are model-only and are never counted
  as executable production controls. No release of a native object and no
  whole-compiler safety claim follows.
