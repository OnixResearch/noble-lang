# M5 authority ledger: implementation theorems (issue #66, task 3)

This lane proves statements about the generated Lean functions in the retained M5 Charon/Aeneas
translation of `crates/noble-kernel/src/authority`. The statements are not about a handwritten
machine. The phase 1 model,
[`NobleFoundation.AuthorityLedger`](../../proofs/foundation/NobleFoundation/AuthorityLedger.lean),
and the finite Rust controls in
[`crates/noble-kernel/tests/m5-authority`](../../crates/noble-kernel/tests/m5-authority) stay separate
evidence.

## Command

```sh
env -i HOME="$HOME" USER="$USER" LOGNAME="$USER" PATH=/run/current-system/sw/bin:/usr/bin:/bin \
  "$(selected node)" verification/m5-authority/check.mjs --output /tmp/m5-authority-check
```

The harness refuses a caller environment carrying `LEAN*`, `LAKE*`, `NODE*`, `BASH*`, `GIT_*`,
`NIX_*` or `ELAN*` variables. It refuses an output directory inside the source tree and a dirty
worktree. `--allow-dirty` records a dirty run instead of refusing it, but does not hide it.

## What one run checks

1. **The generated input.** It unpacks `verification/m5/discovery.tar.gz` and requires the five
   generated `NobleKernel` sources to match both [the profile](model-inputs.json) and the
   historical extraction receipt `verification/m5/extraction.json`. It compiles them anew; archived
   `.olean` files are never imported. The pinned Lean 4.31 executable and the Aeneas/Mathlib
   checkouts must match the M5 lake manifest.
2. **Statements and axioms.** Every theorem in [`M5Authority.lean`](../../proofs/m5/M5Authority.lean)
   must appear in [the inventory](../../proofs/m5/authority-inventory.json), and the reverse. Each
   compiled statement is recorded. Each transitive axiom set must lie within
   `propext`, `Classical.choice` and `Quot.sound`.
3. **Source binding.** A Lean pass collects every `noble_kernel` constant reachable from the
   inventoried theorems, through types, proof terms and constructors. Each one maps to the generated
   declaration that owns it and that declaration's `Source:` Rust file. Every such file must still
   have exactly the bytes the extraction recorded. Reachable handwritten external models are listed
   as a trust boundary.
4. **Mutation controls.** Five structure-preserving edits each remove one check from the generated
   code: the quota charge, witness consumption, the exact-plan comparison, checkpoint validation, and
   the already-started refusal. Each mutated kernel must still compile. Elaborating the unchanged
   theorems against it must fail. The targeted theorems must then depend on `sorryAx`, while the
   other theorems keep strict axioms.

## What is proved

| Group | Theorems | Claim |
|---|---|---|
| Committed admission | `committed_admission_presents_the_claim`, `…_binds_the_exact_plan`, `…_requires_the_current_checkpoint`, `…_requires_current_authority`, `…_consumes_one_live_witness`, `…_registers_one_unstarted_attempt`, `…_charges_within_the_supplied_quota` | A committed `admit` required the presented, authority-owned claim; a live retained witness; and a current grant, both carrying exactly the requested plan. It also required current facts: checkpoint, source, credentials, revocation, policy and validity window. It consumes exactly that witness, registers exactly one unstarted attempt, and charges exactly the plan units to this checkpoint's record for the plan quota, within the supplied snapshot. |
| Refusal | `refused_admission_preserves_ledger`, `commit_refusal_returns_input`, `charge_quota_refusal_returns_input` | Denial and preflight leave witnesses, attempts and quota records unchanged. They still move their request counter and requested effect. |
| Start | `start_execution_marks_one_unstarted_attempt`, `started_attempt_cannot_start_again`, `start_execution_refuses_a_started_attempt`, `start_execution_refusal_returns_input` | A start marks exactly one unstarted attempt and counts one protected operation. Replaying the same permit is refused without changing anything. |
| Quota invariant | `new_starts_within_quota`, `step_preserves_quota_within`, `reachable_quota_within` | From `Authority.new`, over every finite serialized prefix of the public transitions (authorize, advance, admit, start, observe, retire, cancel, fail, deliver), every checkpoint-local quota record stays within the snapshot the host supplied for it. |

The remaining theorems are the lemmas these rest on: total comparison and clone models, lookups,
the terminating quota scan and witness-retirement loop, the per-check validator steps, the
commit steps and the quota frames.

## Retained run

The [receipt](receipt.json) records a clean run at source `e71fd9c97`. All
66 theorems compile. 63 depend on `propext, Classical.choice, Quot.sound` and 3 depend on `propext, Quot.sound`.

The reachability pass found 412 project constants. Of these,
112 are generated declarations, 38
are shared auxiliaries, and the rest are their projections, constructors and module constants.
The seven `authority/` files are bound at `exact-file`. `types.rs` has changed since the
extraction, so it is bound at `item-text` through 6 declarations. The `EffId` and
`ResourceKind` items and their derives are byte-identical to the extracted text. The reachable
external models are `core.option.Option.as_ref` and the `u32` to `usize` conversion.

Each mutated kernel compiled, and elaborating the unchanged theorems against it failed:

| Control | Failing declarations | Theorems on `sorryAx` | Theorems kept strict |
|---|---|---|---|
| `omitted-quota-charge` | `charge_quota_charges_the_plan_units` | 6 | 60 |
| `omitted-witness-consumption` | `commit_consumes_the_claimed_witness` | 2 | 64 |
| `omitted-exact-plan-comparison` | `validate_entry_requires_the_exact_plan` | 2 | 64 |
| `omitted-checkpoint-validation` | `validate_facts_requires_the_current_checkpoint` | 2 | 64 |
| `omitted-started-refusal` | `start_execution_marks_one_unstarted_attempt`, `start_execution_refuses_a_started_attempt` | 3 | 63 |

A mutation shows that a proof depends on the removed check; it does not refute a statement. The quota
charge control, for example, also stops the quota-invariant proofs, because they go through the exact
charge theorem.

## Boundaries

- The Charon/Aeneas translation of the bound sources is trusted; this lane does not re-run it.
- Checkpoint renewal is a host premise, as in phase 1's `renewed_snapshot_keeps_quota`. A new
  checkpoint opens new quota records from new host snapshots. No cross-checkpoint conservation is
  claimed, and replenishment that ignores prior reservations is outside the premise.
- Witness consumption and attempt registration are proved for each committed admission. No counting
  invariant over reachable states relates consumed witnesses to registered attempts.
- No concurrency, crash-recovery, remote-revocation atomicity, exactly-once delivery, or Rust
  compiler, Charon or Aeneas correctness claim.
