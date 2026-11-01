# Differential test plan for values, stack order, resource obligations and effect traces (issue #79)

This is the *plan* the issue asks for. Nothing in it is executed yet: the issue excludes
implementation, and no focus elaborator exists to compare against a reference. The plan exists so
that a later implementation has a decided oracle and decided vectors rather than invented ones.

## Oracles, per axis

| Axis | Reference observation | Why not the elaborator itself |
|---|---|---|
| values | the existing checker plus `noble live repl` stack reporting on a hand-authored explicit program, exactly as `receipts/2026-10-31/measure.json` does for both compiled forms | the elaborator's output cannot referee its own meaning |
| stack order | the ordered `stack_in`/`stack_out` shapes of the bootstrap contracts the elaboration claims to target (`dip`, `compose`, `run`, `pair`, `unpair`); the checker refuses a mismatched signature rather than agreeing by accident | signature IDs are complete and ordered; argument counts and suffix matches are not accepted |
| resource obligations | **#59's selected fragment**, reused rather than duplicated: the ownership-accounting oracle over ordered stack rules | a second ownership oracle would be a second authority on the same question |
| effect traces | `Program.effects` edge passthrough recorded by the elaborator, compared against the hand-authored program's traces | a pure elaborator must not make an effectful program pure, and the trace is where that shows |

## Vectors

| Vector | Explicit reference program | What the differential must show |
|---|---|---|
| `nested-product` | `forms/nested-product.explicit.noble` | equal value `Pair(2, 5)`; untouched component `5` observed exactly once |
| `type-changing-step` | `forms/type-changing-step.explicit.noble` | equal value `Bool false`; the focus boundary's input `I64` and output `Bool` both checked |
| `type-changing-nested` | not yet authored | a nested update whose inner operation changes type: the case the sugar exists for |
| `preserved-prefix` | partially covered by `nested-product` | where a resource prefix is supported; today it is not, so this vector is `not-run` with that reason rather than asserted |
| `move-threaded` | none exists in source | admitted only after a must-use resource word exists in the profile |

## Rules the differential must obey

1. **No identity promises.** Differently elaborated programs may produce different recipes or
   hashes. The comparison is on values, stack order, resource obligations and effect traces — never
   on artifact digests being equal.
2. **No evidence transfer.** Any evidence binds the exact source commit, elaborator revision,
   profile and generated Program digest. Matching signatures or outputs do not transfer proof
   evidence between Programs.
3. **Refusals are observations.** Ambiguous or unsupported focus, exhaustion and invalid ownership
   must be *distinct diagnostics*, not one generic failure, and each is recorded as a refusal the
   differential expects rather than a crash it tolerates.
4. **Recorded, not assumed.** A vector that does not run is recorded as `not-run` with its reason.
   A vector that runs on one form but not the other is a defect of the plan, not a pass.

## Status

| Vector | Status |
|---|---|
| nested-product | the explicit and library forms agree today (measured in the receipt); no elaborator output to compare |
| type-changing-step | same |
| type-changing-nested | not-run: needs an elaborator |
| preserved-prefix | not-run: the selected profile has no resource prefix to preserve |
| move-threaded | not-run: no must-use resource word in source |

The refusal/admission controls the issue names (duplicate or lost owner, closing a resource promised
back, stale pre-update binding, overlapping focus, wrong reconstruction type, branch mismatch,
forgotten ordinary-error obligation, escape through returned aggregates or saved Programs) and the
mutated elaborations (inserted forbidden `dup`/`drop`/capture, omitted reconstruction, reordered
effects) are listed here so the later lane inherits them; exercising them requires the elaborator
this design issue does not implement.
