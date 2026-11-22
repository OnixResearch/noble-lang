# Integrator self-review: implicit proof arguments, batch 5 (2026-11-22)

This record is an integrator self-review under owner decision 38 ("Push once every gate
passes; your veto is the only review"). It is not a fresh-context review: the integrator wrote
the reviewed commits. The owner accepted the IMPL-CASE amendments of decision 37 as recorded.

## Reviewed commits

- Decision 36 refactor (`33f5af62`..`3949581b`): the elaborator in Aeneas-extractable shapes.
- `de48c8db`: imported proofs resolve under their alias in `(infer)` mode (found by IMPL-CASE-01).
- `0a61b0b5`, `d91f5877`, `af6de786`: refusal content (binder, head, originating constraints),
  bounded diagnostics, shared proof budgets, extractable helper shapes, span text without
  `String::from(&String)`.
- `57d6236d`, `b6af7236`, `a82c182d`: Octet deny-all clearance of the refactor and the slice.
- `b5d772a6`: IMPL-CASE-01..20 registration (seven amendments) and the acceptance gate.

## Checks performed

| Check | Result |
|---|---|
| `cargo test -p noble-contracts` (all targets) | passed (all suites green) |
| `cargo test -p noble-cli --bin noble -- workflow::` | 14 passed |
| `cargo clippy -p noble-contracts -p noble-cli --all-targets -- -D warnings` | clean |
| Charon + Aeneas over the contracts crate | rc 0, no opaque model |
| M4 discovery Lean build of the regenerated contracts subject | passed (after `af6de786`) |
| Octet deny-all (flake check, leviathan) at `f422c272` | clean, 0 findings |
| IMPL gate (`runs/20261122-b5`, 100 determinism runs, commit `b4c63a1e`) | `all-passed`: 20 of 20 promotable, receipt sha256 `1641c70c…` |

## Findings

- F1 (fixed): IMPL-ELAB-01 required binder, head and originating constraints in every refusal;
  only `underconstrained` carried them.
- F2 (fixed): IMPL-EXPOSE-01 bounded diagnostics were not implemented.
- F3 (fixed): `--normalization-work`/`--substitution-work` did not bound elaboration.
- F4 (fixed): `String::from(&String)` in `span_text` is not modeled by the pinned Aeneas library.
- F5 (fixed): the decision-36 refactor had not been Octet-probed; 40 findings cleared.
- F6 (open, recorded): elaboration's single production call site is module registration; no
  runtime counter observes "zero elaborator invocations" (IMPL-CASE-20 amendment).

## Verdict

ACCEPT-WITH-NOTES for promotion of the cases the receipt shows as observed; proof stays open
for every case, and task 4 (unification uniqueness at Rust level) and task 5 (requirement sync)
stay open.
