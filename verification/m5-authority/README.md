# M5 authority ledger: implementation theorems (issue #66, tasks 2 and 3)

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
2. **Statements, subjects and axioms.** Every theorem in
   [`M5Authority.lean`](../../proofs/m5/M5Authority.lean) must appear in
   [the inventory](../../proofs/m5/authority-inventory.json), and the reverse. The inventory pins
   three things, and each must equal what the compiled module produces:
   - each theorem's compiled statement;
   - its *subject functions*: the generated functions its statement names, followed through this
     module's definitions and constructors but neither into proofs nor into generated bodies. For
     example, `reachable_ledger_consistent` is about `Authority.new` and the nine public
     transitions, through `Reachable` and `Step`;
   - the printed form of every specification definition (`Step`, `Reachable`, `LedgerMove`,
     `LedgerConsistent`, `QuotaWithin` and the rest).

   A weakened statement therefore fails, and so does a definition such as `LedgerConsistent`
   changed under an unchanged statement, or a generated function swapped out of `Step`. Each
   transitive axiom set must lie within `propext`, `Classical.choice` and `Quot.sound`.
   Renewal is explicit: a failing run writes `observed-pins.json` for review. Nothing updates the
   inventory automatically.
3. **Source binding.** A Lean pass collects every `noble_kernel` constant reachable from the
   inventoried theorems, through types, proof terms and constructors. Each one maps to the generated
   declaration that owns it and that declaration's `Source:` Rust file. Every such file must still
   have exactly the bytes the extraction recorded (`exact-file`). If a file has changed elsewhere,
   the archived text of every reachable item must occur verbatim in today's file (`item-text`).
   Elaborator-shared matchers and side proofs are traversed but bind no Rust item. Reachable
   handwritten external models are listed as a trust boundary.
4. **Mutation controls.** Seven structure-preserving edits each remove one check from the generated
   code: the quota charge, witness consumption, the exact-plan comparison, checkpoint validation, the
   already-started refusal, the protected-operation increment, and the live-state guard of witness
   retirement. Each mutated kernel must still compile. Elaborating the unchanged theorems against it
   must fail. The targeted theorems must then depend on `sorryAx`, while the other theorems keep
   strict axioms.

## What is proved

| Group | Theorems | Claim |
|---|---|---|
| Committed admission | `committed_admission_presents_the_claim`, `…_binds_the_exact_plan`, `…_requires_the_current_checkpoint`, `…_requires_current_authority`, `…_consumes_one_live_witness`, `…_registers_one_unstarted_attempt`, `…_charges_within_the_supplied_quota` | A committed `admit` required the presented, authority-owned claim; a live retained witness; and a current grant, both carrying exactly the requested plan. It also required current facts: checkpoint, source, credentials, revocation, policy and validity window. It consumes exactly that witness, registers exactly one unstarted attempt, and charges exactly the plan units to this checkpoint's record for the plan quota, within the supplied snapshot. |
| Refusal | `refused_admission_preserves_ledger`, `commit_refusal_returns_input`, `charge_quota_refusal_returns_input` | Denial and preflight leave witnesses, attempts, quota records and every ledger counter unchanged. They still move their request counter and requested effect. |
| Start | `start_execution_marks_one_unstarted_attempt`, `started_attempt_cannot_start_again`, `start_execution_refuses_a_started_attempt`, `start_execution_refusal_returns_input` | A start marks exactly one unstarted attempt and counts one protected operation. Replaying the same permit is refused without changing anything. |
| Quota invariant | `new_starts_within_quota`, `step_preserves_quota_within`, `reachable_quota_within` | From `Authority.new`, over every finite serialized prefix of the public transitions (authorize, advance, admit, start, observe, retire, cancel, fail, deliver), every checkpoint-local quota record stays within the snapshot the host supplied for it. |
| Ledger moves | `step_moves_ledger` and one `…_moves_ledger` theorem per transition | Every public transition is one `LedgerMove`. Authorization appends one live witness. Admission consumes one live witness and registers one unstarted attempt. A start marks one unstarted attempt. An observation rewrites one attempt record, keeping its key, plan and start flag. Retirement retires one live witness, or every live witness on cancel, fail and delivery. Otherwise the tables are unchanged. The four counters that count the tables move exactly with them. |
| Ledger invariant | `new_starts_ledger_consistent`, `step_preserves_ledger_consistent`, `reachable_ledger_consistent` | From `Authority.new`, over every finite serialized prefix: consumed witnesses equal registered attempts in number. `protected_operations` counts exactly the started attempts. `witnesses_created`, `attempts_admitted` and `witness_consumptions` count their tables. |
| Traces | `settled_witness_is_never_rewritten`, `consumed_witness_never_returns_to_live`, `started_attempt_stays_started`, `started_attempt_never_starts_again` | Over every finite sequence of public transitions (`Relation.ReflTransGen Step`), a witness that has left `Live` is never rewritten, so consumed authority is never resurrected. A started attempt stays started with its key and plan, so its execution permit can never start it again. |

The remaining theorems are the lemmas these rest on: total comparison and clone models, lookups,
the terminating quota scan and witness-retirement loop, the per-check validator steps, the
commit steps, the quota frames, and three `countP` list lemmas.

## Retained runs

### `af46d978d`: [receipt-af46d978d.json](receipt-af46d978d.json)

A clean run at source `af46d978d`. All 93 theorems compile: 83 depend on `propext, Classical.choice, Quot.sound`, 6 depend on `propext, Quot.sound`, 4 depend on `propext`. Every
statement, subject set and the 10 specification definitions match their inventory pins.

The reachability pass found 415 project constants, including
112 generated declarations and 38 shared auxiliaries. The seven
`authority/` files are bound at `exact-file`. `types.rs` has changed since the extraction and is
bound at `item-text`: its six reachable `EffId`/`ResourceKind` declarations are byte-identical to
the extracted text. The reachable external models are `core.option.Option.as_ref` and the `u32` to
`usize` conversion.

Each mutated kernel compiled, and elaborating the unchanged theorems against it failed:

| Control | Failing declarations | Theorems on `sorryAx` | Theorems kept strict |
|---|---|---|---|
| `omitted-quota-charge` | `charge_quota_charges_the_plan_units` | 6 | 87 |
| `omitted-witness-consumption` | `commit_consumes_the_claimed_witness` | 10 | 83 |
| `omitted-exact-plan-comparison` | `validate_entry_requires_the_exact_plan` | 2 | 91 |
| `omitted-checkpoint-validation` | `validate_facts_requires_the_current_checkpoint` | 2 | 91 |
| `omitted-started-refusal` | `start_execution_marks_one_unstarted_attempt`, `start_execution_refuses_a_started_attempt` | 11 | 82 |
| `omitted-protected-operation-count` | `start_execution_marks_one_unstarted_attempt` | 10 | 83 |
| `retire-consumed-witness` | `retire_moves_ledger` | 8 | 85 |

A mutation shows that a proof depends on the removed check; it does not by itself refute a
statement. The last two controls also make statements false. Without the increment,
`protected_operations` stops counting started attempts. With the guard swapped, a consumed witness
can be retired. Each `LedgerMove` carries its counter effects, and every invariant and trace proof
goes through `step_moves_ledger`. So a counter-only mutation also stops the trace proofs, even
though those statements do not mention counters.

A pin negative control used an inventory with three changes: `reachable_ledger_consistent`
restated as `True`, the subject set of `committed_admission_binds_the_exact_plan` emptied, and
`LedgerConsistent` weakened to `True`. The checker refused it, naming exactly those three pins.

### `e71fd9c97`: [receipt.json](receipt.json)

The first run covered 66 theorems and five mutation controls, and established the committed-admission
guarantees and the checkpoint-local quota invariant. Its receipt is unchanged.

## Boundaries

- The Charon/Aeneas translation of the bound sources is trusted; this lane does not re-run it.
- Checkpoint renewal is a host premise, as in phase 1's `renewed_snapshot_keeps_quota`. A new
  checkpoint opens new quota records from new host snapshots. No cross-checkpoint conservation is
  claimed, and replenishment that ignores prior reservations is outside the premise.
- Consumed witnesses and registered attempts agree in number over reachable states. Each committed
  admission pairs one witness with one attempt; no trace-level pairing of a particular witness with a
  particular attempt is stated beyond that.
- No liveness: nothing forces an admitted attempt to start or an invocation to finish.
- No concurrency, crash-recovery, remote-revocation atomicity, exactly-once delivery, or Rust
  compiler, Charon or Aeneas correctness claim.
