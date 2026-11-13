# Evidence obligations for issue #112

These evidence classes are deliberately separate. A finite table is not a proof; a model proof is
not a production checker theorem; neither is a go/no-go decision by itself.

## Kernel-checked model

`proofs/foundation/NobleFoundation/UsePermissions.lean` defines the two-bit algebra, recursive
Pair/Sum/List/nominal eligibility, Program invariants, ordered-stack word rules, two-arm join, and
modeled normal Result-error return. The registered theorem set includes:

* algebra and capture: `combine_commutative`, `combine_associative`, `cases_of_permission`,
  `split_adds_no_permission`, `capturable_iff_unrestricted`, and `capturable_iff_bits`;
* recursion: `pair_of_must_use_is_must_use`, `sum_of_must_use_is_must_use`,
  `list_of_must_use_is_must_use`, `nested_pair_sum_list_must_use`, and wrapping-depth controls;
* Program and stack rules: `program_is_unrestricted`,
  `captured_environment_is_capturable`, `step_program_positions_are_unrestricted`,
  and the theorems for `dup`, `drop`, `quote`, `swap`, `dip`, `compose`, and `run`;
* branches and errors: the exact-stack branch rule, missing/duplicated-position controls,
  `fail_requires_discardable`, and controls distinguishing this hypothetical rule from
  `Ownership.fail`;
* seeded errors: the wrong always-allow and one-bit capture rules, one-arm branch, obligation-
  ignoring error return, Program-permission inheritance, and capture-then-Program-duplication
  chain are refuted by named kernel-checked controls.

The kernel establishes only these model statements. It does not establish that the production
checker implements the model, that the model is Noble source semantics, or that the additional
categories are desirable. The proof's `fail` behavior is intentionally not substituted for #59's
different, stricter ownership oracle.

## Finite executable expectations and selected-profile tests

`cases.mjs` contains a small independent evaluator and separately declared expected decisions.
The script refuses to emit the corpus if any computed decision differs from its frozen expected
row. `cases.txt` preserves those rows, including the four classes crossed with the relevant words,
compound permissions, Programs, error returns, and join controls. This is finite executable
evidence for the stated model, not exhaustive proof over arbitrary concrete implementations.

`forms/*.noble` are the only selected-profile cases. The lane runner invokes the real `noble
compile --opt off` for each one, then `noble live repl`, and checks both normal status and an
independently stated final stack. It measures only current-profile compilation and execution.
The proposed affine, must-handle, must-use, capture-failure, join, and error-return examples have
no selected-profile syntax/values and remain model-only.

## Production checker correspondence: not implemented

There is no new Rust checker or elaborator in this go/no-go slice, and no correspondence claim is
made. The planned hook is the existing
`crates/noble-kernel/tests/acceptance/ownership.rs` test target, extended by a `#[path]` module (no
new compilation unit) only after a separate profile is approved. A future correspondence test
must compare selected Rust source-to-acceptance decisions with independently generated cases and
must distinguish `Data`, `Resource`, and `LiveRef` behavior; it must not copy expectations from
the Rust checker itself. Because the current checker exposes only `is_data`, it cannot currently
realize the four combinations. Any implementation would be a separate Rust change requiring the
repository's source-inventory renewal, Octet, workspace tests, and review of the generic-argument
asymmetry.

The retained #59 model and receipts remain bound to the existing two-class policy. This lane does
not edit, relabel, or claim new coverage for them.
