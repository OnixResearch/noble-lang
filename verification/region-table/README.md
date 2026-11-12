# Region table refinement (`behavioral-effect-protocol` T4)

Lean-kernel check for obligation T4 of the change now archived at
`.cairn/archive/2026-11-08-behavioral-effect-protocol`: the
`ownership::Table` transitions the bounded-region adapter uses refine `RegionProtocol-v1`.

The lane builds `proofs/m5/RegionTable.lean` in an isolated project with the pinned Lean toolchain,
on top of two inputs:

- the pinned M5 extraction of `noble_kernel::resources`, rebuilt from
  `verification/m5/discovery.tar.gz`;
- the T1–T3 model `proofs/foundation/NobleFoundation/RegionProtocol.lean`.

The check then audits every inventoried theorem's compiled statement and transitive axioms. It also
audits four reused theorems, so a finite instance, an extra premise or a weaker statement cannot
satisfy the inventory by mentioning a dependency:

- `M5Resources.transition_refines` and `M5Resources.table_decide_refines`;
- the model's `exactly_one_release_commit` and `run_final_owner`.

## Command

```sh
env -i HOME="$HOME" USER="$USER" LOGNAME="$USER" PATH=/run/current-system/sw/bin:/usr/bin:/bin \
  "$(selected node)" verification/region-table/check.mjs --output /tmp/region-table-check
```

The harness makes four refusals:

- a caller environment carrying `LEAN*`, `LAKE*`, `NODE*`, `BASH*`, `ENV`, `LD_*`, `DYLD_*`,
  `GIT_*`, `NIX_*` or `ELAN*` variables;
- an output directory inside the source tree;
- a dirty worktree, unless `--allow-dirty` is passed explicitly (a dirty receipt records it rather
  than hiding it);
- any of the six `crates/noble-kernel/src/resources` files differing from the bytes the retained
  extraction recorded.

Because of the last refusal, the extracted table methods are the current ones.

## Subject

`crates/noble-cli/src/component/bounded_region.rs` imports `noble_kernel::resources` as `ownership`.
It builds a one-slot table: `Table::new` with one slot, one pin, one owner per context, one
generation and 64 scopes, then `register`. It calls four table methods:

- a borrowed read is `begin`, then `native_access`, then `complete` with the outcome
  `native_access` selects;
- a guest release, a destructor and the host's cleanup are all `release`.

`Represents` relates that table to a model state:

- **records:** the single record carries the owner's handle and stands for the owner (`live` for
  `Live`, `released` for `Retired`);
- **scope counter:** it is the model's `granted`, with the model's scope limit as its ceiling;
- **pins:** none is outstanding.

The model host the table induces, `tableHost`, answers `true` for `access` and for `complete`. The
region, the owner identity and the `lower` oracle stay the host's.

## What is proved

The statements name the retained generated functions (`Table.new`, `Table.register`, `Table.commit`
and the `access` methods), not a handwritten table.

| Theorem | Claim |
|---|---|
| `registration_represents` | the adapter's `Table::new` and `register` yield the owner and a table that represents the model's initial state; the adapter's requirement is `requirementFor` of the owner's handle |
| `begin_admits`, `begin_exhausted` | below the scope ceiling, `begin` allocates the serial `granted + 1`, admits the borrow, stores the opened record and holds one pin; at the ceiling it refuses with `ScopeExhausted` and returns the table unchanged |
| `open_borrow_returns` | A4 for the table: `native_access` authorizes the open borrow, and `complete` returns the owner for either outcome, stores the returned record and releases the pin, so the model's `lost` state is unreachable |
| `release_table`, `begin_retired_table` | `release` retires a live owner as one local release; on a retired record `release` and `begin` are refused and return the table unchanged |
| `commit_admitted`, `commit_returned`, `commit_released`, `selected_of_records` | on the one-slot table, `commit` stores the decided record and moves the pin count by the action |
| `begin_fresh`, `access_opened`, `complete_opened`, `release_live`, `release_retired`, `begin_retired`, `*_valid`, `*_handle`, `released_state` | the `M5Resources.step` decisions those methods take, and the records they leave well formed |
| `accounting_local_releases` | the release count `actionReleases` is the `local_releases` field of the extracted `Decision.accounting` |
| `read_step` | under `tableHost`, a read on a live owner keeps it live, grants a borrow exactly when the claim is the owner's, the range is in bounds and the scope limit is not reached, and commits no release |
| `calls_refine`, `cleanup_refines` | for every guest request, and for the host's cleanup, the adapter's table calls take a table representing the model state to one representing the model's next state, accounting exactly the model's release commits |
| `exec_refines`, `run_refines` | the same for every execution and every run, by induction over the program |
| `table_retires_exactly_once` | P1 and the cleanup claim for the table itself: from a live owner, every run ends with the record retired, no pin outstanding and exactly one local release accounted |

`open_borrow_returns` is the method-level counterpart of `M5Resources.normal_completion_owner_once`,
which states the same completion for a single record's decision. The method-level form reaches it
through `table_decide_refines` and adds the `commit` that stores the returned record.

A mutation control is not retained, but was run while developing the module. Counting a local release
as two breaks `accounting_local_releases`, the release and destructor cases of `calls_refine`, and
`cleanup_refines`.

## Premises that stay premises

- **T5.** The per-request call sequence `tableCalls` is read off `bounded_region.rs`, and the
  receipt binds that file's bytes. That the Rust host follows the sequence, and performs the identity
  and bounds checks the model states before `begin`, is T5 and stays open.
- **T6.** The model's `lower` oracle is the guest lowering. It belongs to T6 and stays free.
- **Not modeled.** Wasmtime, the Canonical ABI and the CLI wrapper.
- **Trusted translation.** The retained Charon/Aeneas translation is trusted to model the bound Rust
  sources. The lane compiles the generated sources anew but does not re-run the translation.

## Receipt

[`receipt.json`](receipt.json) is the retained receipt of one run, on the clean commit `0985ee06`
with Lean 4.35.0: **30** theorems, plus the four reused ones. The reported axioms are `propext`,
`Classical.choice` and `Quot.sound`, all inside each theorem's allow-list. The four record-shape
lemmas use none.
