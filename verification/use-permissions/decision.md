# Decision: do not select a separate duplication/discard profile (issue #112)

## Decision

**NO-GO for a source-profile or kernel change.** Keep the current `Data`/`Resource` policy. Do not
add affine or must-handle annotations, infer them, or change the kernel's `is_data` walk. The
four-cell alternative is specified and checked as an evaluation model only. It has not earned a
new language category on any located workload.

## Worked comparisons

The CLI evidence is in [`receipts/2026-11-13/measure.json`](receipts/2026-11-13/measure.json),
bound to clean source commit `74b1748153fb778e53c4dd8830d6912889f00524`. All source examples run
under the selected Core-Bootstrap profile and use current Data.

1. **Reuse:** `3 dup 4 + swap -` leaves `I64 4`. The value is required twice, so explicit `dup`
   expresses the intent. `Data` admits it today. The existing Resource policy refuses the same
   duplication, as it should for an exactly-once owner. A new unrestricted cell would merely
   rename Data.
2. **Explicit discard:** `1 2 drop 3 +` leaves `I64 4`. `drop` removes the unused literal `2`
   before the remaining calculation. Current Data already permits this. It is structural discard
   only; it cannot release a host resource, and it must not be used to model an effectful release.
3. **Explicit Resource release:** the bounded-region contract's `region.read` returns the same
   borrowed owner on either normal Result branch, after which `regions.release(own<region>)`
   consumes it once (`specs/RESOURCE-ADAPTERS.md:23`). This spec-anchored case is model-only in
   this Core-Bootstrap lane. The explicit release is effectful and ownership-aware; `drop` is
   rejected for the Resource and cannot substitute for the operation. The four-way data algebra
   adds no permission or release effect here.
4. **No-help control:** `7 dup +` leaves `I64 14`. Duplication is the only relevant operation;
   introducing an independent discard bit does not improve the program or its checking.
5. **Unresolved must-handle candidate:** `5 1 + drop` currently leaves an empty stack. The model's
   at-least-once class would refuse a direct drop, but there is no concrete value in the selected
   source profile whose contract says it must be consumed and whose explicit drop must be rejected.
   The example is a counterfactual control, not a discovered need and not a go signal.

The affine candidate is equally unmotivated for guest authority: H-AUTH-04 requires live
authorization witnesses to remain outside Data and Capture and prohibits discard. The only
current Resource class is exactly-once, not an affine payload for which silently dropping a live
owner is safe. The #79 focus-elaboration decision also records that the selected source profile
has no must-use resource word; those cases remain model-only rather than being passed off as
executable examples.

## Bounded algebra and behavior assessed

The model has four pairs `(dup, drop)`: Data `(1,1)`, must-use `(0,0)`, affine `(0,1)`, and
must-handle `(1,0)`. Pair and Sum combine each bit with conjunction; List and opaque wrappers
inherit the element/representation; variant wrappers combine both arms. Capture requires both
bits. Thus the split can only withhold one of Data's current operations and gives no new operation;
`split_adds_no_permission` and `capturable_iff_unrestricted` state those limits.

Programs remain immutable and unrestricted values, independent of their input-interface names.
Their captured environment is constrained separately: every payload must be capturable, or
`quote` refuses. No one-shot closure is added. For control flow the bounded model requires exact
ordered-stack agreement across both finite branch arms. A missing or duplicated tagged position
fails the join. Its normal Result-error edge discards only positions with the drop bit; that rule
is only part of the hypothetical permission model, not a change to #59's current error-return
oracle or to abnormal host cleanup. The model has no Result payload or effect trace. An explicit
operation such as `regions.release` stays in the program's ordered effect sequence; `drop` and
error unwinding cannot create, erase, or reorder it. Trap, cancellation and native cleanup remain
host obligations, not guest discard permissions. `drop` is never a resource destructor. These
explicit boundaries prevent a structural permission from becoming host authority.

## Authoring, diagnostics, and checking cost

The measured examples remain concise explicit stack code: the repeated-use case uses `dup`, and
the disposal case uses `drop`. Across seven warm-after-one compile runs, the existing-profile
medians are 10.744 ms for harmless duplication, 8.994 ms for the discarded intermediate, 9.038 ms
for the must-handle counterfactual, and 10.375 ms for the shared value; emitted modules are 56,121
to 58,218 bytes. No source annotation currently names the two bits, so authoring cost for a
hypothetical profile cannot be measured honestly. These are existing-profile CLI measurements,
not a price for the proposed checker.

The production `Ty::is_data` is one bounded iterative walk with one Boolean result. A four-way
algebra would carry two Boolean bits and combine both at each visited node; it could retain one
structural walk, but it would also need operation-specific diagnostics and branch-join accounting
with owner identity. Diagnostics would have to distinguish “not duplicable,” “not discardable,”
“not capturable,” shape/identity mismatch at a join, and bound exhaustion. No implemented
four-way checker exists, so there are no defensible numbers for additional memory, compile time,
or diagnostic overhead. The model's execution cost is not a proxy for those implementation costs.

If this design is reconsidered, the bounded policy must keep a fixed eligibility algebra compatible
with rank-1 value/stack/effect schemes. A traversal or join-budget exhaustion must be an explicit
inconclusive/refusal diagnostic, never silently treated as a successful proof or conflated with a
semantic permission refusal. No open-ended trait search, general subtyping, higher-rank Program
polymorphism, user-defined destructors, or authority weakening is within this evaluation.

## Go/no-go criteria and reopen conditions

GO would require all of the following before implementation selection:

1. A concrete, useful source-profile value that is duplicable but must not be discarded, for which
   the existing explicit `drop` is contractually wrong and cannot be replaced by an explicit
   consume/release operation. The value must be inert data, not a resource or live authorization
   witness.
2. A worked normal/error and two-arm control-flow account showing the obligation is preserved,
   including nested Pair/Sum/List/nominal wrappers and a Program capture/duplication attempt.
3. A bounded fixed-rule design with a distinct exhaustion result, stable diagnostics, and proof,
   finite-test, and checker-correspondence obligations separated.
4. An evidence-based cost case showing the authoring/checking benefit justifies the additional
   bit, joins, diagnostics, and checker surface while preserving H-AUTH-04 and #59's exact-once
   resource behavior.

None is established by the current cases. Reopen on the first genuine must-handle-data witness
that meets criterion 1, then rerun the full gate in this order; do not treat the synthetic
`must-handle-candidate.noble` as that witness. Until then, keep the two-class model and do not widen
the language.
