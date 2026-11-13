## Why

Agent-proposed training changes currently arrive as free-form code. OPAL (pinned
`7a0642b`) demonstrates the workflow but executes arbitrary Python inside the
training process, so a proposal is trusted by construction. Noble already has
the safer ingredients: `adam/0` with exact hyperparameter bits and an immutable
binary64 optimizer state, `ngraph-ir/0` admission for `F32` graphs, and a
host-side evidence boundary in `noble-ngraph-experiment`. What is missing is a
small, explicitly typed candidate transition that the host can admit, replay and
reject without executing foreign code.

r[NGRAPH-OPT-01] r[NGRAPH-OPT-02] r[NGRAPH-OPT-03] r[NGRAPH-OPT-04]
r[NGRAPH-OPT-05]

## What Changes

- **ADDED** `NGRAPH-OPT-01`: the minimal typed candidate IR, shape/state
  interface, operation whitelist and declared budgets, with explicit unsupported
  diagnostics rather than silent acceptance.
- **ADDED** `NGRAPH-OPT-02`: the numeric-semantics decision. `adam/0` uses
  binary64 moments, so a binary32-only candidate is a distinct algorithm; the
  design selects a deliberately `F32` reference update first and keeps binary64
  candidates as a separately specified option.
- **ADDED** `NGRAPH-OPT-03`: admission and rejection outcomes, including
  malformed, effectful, over-budget, shape-mismatched, aliasing, nonfinite and
  invalid-state-transition candidates.
- **ADDED** `NGRAPH-OPT-04`: immutable snapshot publication and whole-trial
  rollback custody, with the host owning RNG/data-cursor state and fail-closed
  checkpoints.
- **ADDED** `NGRAPH-OPT-05`: evaluation isolation and preregistration discipline,
  keeping proposer authority away from the evaluator and separating the future
  performance experiment from semantic validation.

This is a design change. It adds no opcode to the Rust optimizer, admits no
candidate in production, and does not touch `NumericGraph-Design`,
`adam/0` or the numeric-graph admission path.

## Impact

- Nouveau source subjects: none. The design names a future bounded module; no
  Rust unit is added by this package.
- Affected specs: `.cairn/specs/backend-experiments/spec.md` (delta only).
- Extracted interface: none. Lean semantics, checker obligations and finite
  Rust comparisons are named as future, separately admitted work under
  #38/#44/#45.
- Adoption: opt-in experiment only. Ordinary Noble Programs, the `I64` slice and
  `F32` graph admission are unchanged.
