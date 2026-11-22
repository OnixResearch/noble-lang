# Integrator review: implicit-args/v1 elaborator, records and goal-view row (2026-11-22)

This is an **integrator review** under owner decision 28 (reviewer agents unavailable), not a
fresh-context review. The owner keeps a veto before any push.

## Scope

- Reviewed (implemented by the campaign's implicit-args slice, not by the integrator):
  `e8e15502` (grammar opt-in, `intrinsic/elaborate.rs`, staging, `verify-module` record,
  `tests/declared/implicit.rs`) and `1bf47094` (loop and early-return refactor for Aeneas).
- Integrator commits on the same branch, covered by the owner veto rather than by this review:
  `3bdbfc37` and `291f7c71` (Octet deny-all clearance), `1243ccea` (shared request budget),
  `6aeadf48` and `e302f93e` (goal-view inserted-arguments row), `b13f7df4` (record completion).
- Against: `design.md` decisions "Inference fragment", "Explicit fallback", "Shared budget model"
  and "Inspect/copy-expanded form", the `IMPL-GRAMMAR-01` delta and owner decision 13.

## Method

Read the design and every diff above; ran on the branch:

- `cargo test -p noble-contracts` (every target; the `declared` target 109 passed, its
  `implicit` subset 16 passed) and `cargo test -p noble-cli --bin noble workflow::` (14 passed);
- `cargo clippy -p noble-cli -p noble-contracts --all-targets -- -D warnings` and `cargo fmt
  --all --check`: clean;
- the flake `octet` check through probes on the merged tree: 309 deny-all findings at first,
  then 6, then 2 (test-only `missing_const_fn` false positives), then a clean run at `5536bde0`
  (lint and architecture phases, status clean, 0 findings); the final batch run is the authority.

## Findings

| # | Finding | Disposition |
|---|---|---|
| F1 | The design's shared budget ("elaboration plus checking together fit the one request") was not implemented: `check_batch_with_budgets` started a fresh meter, so an abbreviated proof could spend up to twice the work limit. | Fixed in `1243ccea`: the checker resumes from `ProofBatch::elaboration_work`. `elaboration_and_checking_share_one_request_budget` failed on the old checker and pins the boundary (exact combined work fits, one unit less exhausts, checking alone still fits). |
| F2 | The goal view had no inserted-arguments row (task 2's second half). | Fixed in `6aeadf48`: every obligation-map and goal-view entry carries `inserted` from the elaboration record; an explicit proof has an empty row. |
| F3 | The `verify-module` record lacked the design's schema name, claim digest, model/checker revisions, consumer policy and node-level source map. | Fixed in `b13f7df4` (`noble-elaboration/v1`; pre-order source map, inserted nodes at their `(infer)` spans), tested on a real staged and checked module. |
| F4 | The elaborator substitutes with a metavariable-aware analogue of the checker's `replace_prop` rather than calling it; equivalence is pinned only by `instantiation_rebases_like_the_checker`. | Accepted with note: soundness does not depend on it, because the explicit checker (which uses `replace_prop`) and the pinned Lean consumer recheck every expansion, so a divergence can only refuse. A shared substitution would need `replace_prop` over metavariable terms; recorded on task 1. |
| F5 | The proof-header scanner now stays in `ProofHeader` for any token other than `:` or `for` (it used to fall back to member mode). | Accepted: the parser still refuses a malformed header (`expected proof for, : or infer opt-in`); only the lexing mode of an already-refused header changes. |
| F6 | `1bf47094` claims the elaborator is Aeneas-extractable; no M4 discovery has run on it. | Open: the batch carrying this branch must regenerate the contracts subject and run the MC1/M4 lanes; the Rust-refinement half of task 4 stays open regardless. |
| F7 | Octet clearance used owner-scoped expectations for most findings. | Accepted: the wording follows the crate's established reasons; the real fixes (two `const fn`s, three guarded subtractions made saturating, bool/quantity renames, an explicit `Some(_) \| None` arm, a reserved vector, ten unfulfilled expectations removed, a second assertion) are listed in `3bdbfc37`. |

## Verdict

**ACCEPT-WITH-NOTES** for task 1 (the `(infer)` spelling and `infer` opt-in of decision 13, and
an elaborator inside the shared request budget whose output the unchanged checker rechecks;
note F4) and task 2 (records, copy-expanded text, source maps and the goal view's
inserted-arguments row). Tasks 3 (IMPL-CASE receipts), 4 (Rust refinement; F6) and 5
(registration and sync after review) stay open.
