# Main gate restoration (issue #22)

This receipt records one run of every pinned flake check on the integration
commit `2edb977` (`integration/issues-20261015`). It also keeps the earlier run on
the issue #22 branch commit `8df6c43`. That branch starts at Calculator-v1
`44329dc`.
[`receipt.json`](receipt.json) holds the tested commit, each check's result,
log path, log SHA-256 and duration, plus the selected tool paths. The logs
stay outside the repository; their hashes bind them.

## Results at integration `2edb977`

| check | result | branch `8df6c43` | parent `0ea10d3f` | follow-up |
|---|---|---|---|---|
| `tool-selection` | pass | pass | not run |  |
| `tool-versions` | pass | pass | not run |  |
| `tool-overrides` | pass | pass | not run |  |
| `format` | pass | pass | fail |  |
| `rust-tests` | pass | pass | fail |  |
| `clippy` | pass | pass | fail |  |
| `documents` | pass | pass | not run |  |
| `cairn` | pass | pass | not run |  |
| `policy` | pass | pass | not run |  |
| `source-inventory-controls` | pass | pass | not run |  |
| `boundary-controls` | fail | fail | fail | [#26](https://github.com/OnixResearch/noble-lang/issues/26), [#27](https://github.com/OnixResearch/noble-lang/issues/27) |
| `octet` | fail | fail | fail | [#26](https://github.com/OnixResearch/noble-lang/issues/26) |
| `source-coverage` | fail | fail | fail | [#25](https://github.com/OnixResearch/noble-lang/issues/25) |
| `native-assurance` | fail | fail | fail | [#25](https://github.com/OnixResearch/noble-lang/issues/25) |

On the integration commit, `rust-tests` ran 423 tests: 423 passed, 0
failed and 4 ignored by design. The ignored tests are the hour-long
`calc_state_02` test and the slow release tier of `calculator_numeric_paths`.

Two failures differ from the `8df6c43` run:

- **`octet`** has the same 236 findings (232 errors).
- **Architecture phase** (`source-coverage`, `native-assurance`) reports
  2,703 required compiler unknowns: 2,165 `?`, 379 `for`, 25 range and 134
  unresolved.
- **`boundary-controls`** no longer hits the duplicate-key fixture error
  (fixed in `b7927b0`). The `acyclic-shell` case now fails like `workspace`:
  deny-all aborts before writing its `deny` artifact. The remote builder
  attempt failed earlier on an invalid store path on that host, so the
  recorded log is a local re-run of the same commit.

### Docs-only commits after the tested commit

These commits follow `2edb977`. None of them changes Rust, WAT, `.noble`,
Nix, Cargo, proof or `policy/` sources; `git diff --stat` shows only
Markdown, receipt JSON and receipt archives. The heavy-check results above
therefore still apply to these commits. The doc-sensitive gates
(`documents`, `cairn`, `tool-selection`, `policy`, `format`) were re-run on
the commit that adds this note.

- `b03ed86`: this receipt.
- `5bedea4`: merge of `perf/issue-16-calculator-latency` `4437ebc`
  (README and tasks.md: reuse on/off on the merged build).
- `1164d18`: merge of `verification/calc-integration-eccbc02` `ee5eef7`
  (integrated CALC receipt JSON and tar.gz, plus README and tasks.md
  references).
- `0605034`: this list.
- `ba0d7c7`: merge of `feat/issue-21-calculator-oracle` `8260310`, which
  adds files only under `verification/calculator-v1/`:
  - Node oracle and comparison scripts;
  - seeded-fault patches applied only to scratch mutant builds;
  - evidence controls;
  - the CALC-12 receipts and the CALC-10–13 records.

  Its Node self-checks were run on the merged tree with the pinned Node 24.13.0:
  - `compare.mjs cases` agrees with all 38 cases-file expectations;
  - `compare.mjs generate --seed 2112` produced 14 vectors;
  - `compare.mjs harness` was run on those vectors once per fault model, and
    each of the 7 fault models disagreed on at least one vector;
  - `evidence-controls/controls.mjs` was run against the integrated release
    binary (`f7f02dfb…85ec`) and reported all 6 controls ok (`all_ok`).
- The commit that adds this entry fixes two directory links in #21's Markdown (`calc-12-results.md`,
  `seeded-faults/README.md`) that the `documents` local-link checker rejects. They
  are now inline code paths; no linked content changed.
  That commit is `6323c1d`.
- `64df5e9`: adds the #21 merge to this list.
- `05a50b0`: links the follow-ups #25, #26 and #27. This commit was pushed to
  `main`.
- Post-push accuracy fixes:
  - `f15a2d5`: the calculator-v1 `design.md` and `proposal.md` replay wording
    now describes #17's committed session.
  - `49a3fc3`: the #18 record cites the integrated CALC-01 15/15 receipt.
  - `d738935`: the Node engine test count in this README is corrected.
  - `3394e25`: commits #16's latency record (`verification/calculator-v1/latency/`).
  - `d4874ee`: records #20's slow numeric-path tier, which passed 3/3 on
    `05a50b0` in 528.8 s.
  - `6f8193c`: lists these post-push commits. This commit was pushed to `main`.
- `6d8b9e5`: merges `feat/issue-24-wolfram-oracle` `adc0351`. It adds
  `verification/calculator-v1/wolfram/` (lane, bridge, fixtures and receipts)
  and pointer edits in `oracle/README.md` and `calc-12-results.md`. The lane's
  offline tests pass:
  - `node --test lane.test.mjs` with the pinned Node 24.13.0: 11/11, 0 skipped;
  - `python3 -m unittest` with nixpkgs Python 3.13.12 from Octet's pinned
    nixpkgs: 14/14. Noble's flake pins no Python, and the bridge tests use only
    the standard library.
- `36c9193`: the README latency load figure now matches the recorded 45.74–60.54,
  and tasks.md cites the in-tree CALC receipt path.
- The commit that adds these entries.

Integration also renews the collection capacity to 58 units (`1d074ea`), with
collector evidence in `receipt.json`, and renews the source inventory
(`2edb977`). The pure comparator returns `valid=true`.

The rest of this document describes the issue #22 branch run on `8df6c43`.
That run's per-check logs remain listed under `parent_comparison` and in
`/home/brittonr/.cache/noble-issues/logs/gates/`.

## Results at `8df6c43`

| check | result | parent `0ea10d3f` | follow-up |
|---|---|---|---|
| `tool-selection` | pass | not run |  |
| `tool-versions` | pass | not run |  |
| `tool-overrides` | pass | not run |  |
| `format` | pass | fail |  |
| `rust-tests` | pass | fail |  |
| `clippy` | pass | fail |  |
| `documents` | pass | not run |  |
| `cairn` | pass | not run |  |
| `policy` | pass | not run |  |
| `source-inventory-controls` | pass | not run |  |
| `boundary-controls` | fail | fail | [#26](https://github.com/OnixResearch/noble-lang/issues/26), [#27](https://github.com/OnixResearch/noble-lang/issues/27) |
| `octet` | fail | fail | [#26](https://github.com/OnixResearch/noble-lang/issues/26) |
| `source-coverage` | fail | fail | [#25](https://github.com/OnixResearch/noble-lang/issues/25) |
| `native-assurance` | fail | fail | [#25](https://github.com/OnixResearch/noble-lang/issues/25) |

The follow-ups are filed by root cause:

- [#25](https://github.com/OnixResearch/noble-lang/issues/25): Octet architecture unknowns, which block `source-coverage` and `native-assurance`.
- [#26](https://github.com/OnixResearch/noble-lang/issues/26): noble-kernel deny-all errors, which block `octet` and the `boundary-controls` deny bundles.
- [#27](https://github.com/OnixResearch/noble-lang/issues/27): the `boundary-controls` unit roster and capacity reservation.

## What changed to restore the gates

- **Formatting.** `cargo fmt --all` was run with the pinned nightly-2026-03-21
  rustfmt.
- **Flake evaluation.** The `44329dc` `Cargo.lock` change moved the reviewed
  offline vendor to
  `/nix/store/vnxgnkzna9csg9m7rd31pazj945ibf38-noble-reviewed-offline-vendor`.
  Its measured NAR is `sha256-Yhlrb3Idkjir0HiUIb4RpH/ulhgX6/aEAFK8S84fwBo=`.
  Until it was re-pinned, the `flake.nix` vendor assert stopped evaluation of
  every output. The vendor is re-pinned in the M5/M6 pins, changed file
  hashes are re-pinned in `policy/tool-selection-files.ncl`, and
  `policy/tool-selection.json` is the pinned Nickel export.
- **Clippy `-D warnings`.** Fourteen warnings were fixed without changing
  behavior. The JSON origin checks keep string comparison, because clippy's
  suggested fix would compare a JSON string with a number. Two reviewed
  exceptions remain:
  - `#[expect(clippy::large_enum_variant)]` on the kernel `Completion`.
    `Step` is produced for every nested body, so the size gap is not wasted
    storage. Boxing `Frame` would add a heap allocation per frame and change
    the extracted kernel subject.
  - `#[expect(clippy::too_many_arguments)]` on
    `ModuleSession::correspond_checked_target`. Bundling its four
    independent selectors would change the public API and the reviewed
    inventory.
- **Collection capacity** (`policy/architecture.ncl`). The collector observes
  exactly 56 units. The two new units are Calculator-v1's
  `calculator_collect` and `calculator_profile` `noble-wasm` integration
  tests. At 56 units, the noble CLI bin and bin+test shards stopped at
  exactly 16,384 calls, and the bin shard also at 16,384 constructors. Those
  shards were marked incomplete, and their truncated JSON was already 26–28
  MB of the 32 MiB shard cap. The unit, call, constructor and byte caps were
  raised so that collection can complete; truncation yields no inventory at
  all. No finding, unknown, waiver or classification rule changed.
- **Source inventory.** `tools/source-inventory-renew.mjs` renewed the
  reviewed policy from a complete collection; see
  [the inventory record](../source-inventory.md). The pure comparator returns
  `valid=true`.
- **Node engine tests.** `engine-assets.test.mjs` and
  `engine-calculator.test.mjs` now run inside `rust-tests`, through
  `smoke.rs` with the built CLI as assembler. The test fails unless the TAP
  output reports every test passed and none skipped. On `8df6c43` that was
  2 tests. On the integration commit it is 6 tests, 6 passed, 0 failed and
  0 skipped: 2 in `engine-assets` and 4 in `engine-calculator`, after #16 and
  #17. #17's `calculator_engine.rs` also runs `engine-calculator` separately.
- **Sandbox shell.** Six live-slot Node test scripts spawned `/bin/bash`.
  The Nix sandbox has no `/bin/bash`, so `rust-tests` failed with
  `spawn /bin/bash ENOENT`; the parent `0ea10d3f` failed the same way. Their
  wrapper is POSIX `sh`, so the scripts now spawn `/bin/sh`.
- **Cairn.** The `calculator-v1` tasks gate and `validate` failed on a task
  without a requirement reference; the task now cites
  `r[CALC-BOUND-01] r[CALC-LIMIT-01]`. The proposal, design and tasks gates
  now pass, and validation returns `valid=true`. No CALC case is promoted.

## Remaining failures

Every remaining failure also fails at the parent `0ea10d3f`. The parent was run
with the same runner, and its logs are in `receipt.json` under
`parent_comparison`. None of them is weakened here. Each failure has a
follow-up issue: #25, #26 or #27.

**`source-coverage` and `native-assurance`.** Both checks build
`inventoryArtifacts`, which runs `cargo-octet check`. That command exits 1
because Octet's project-architecture phase is `blocked (policy-check)`. The
pure source comparator never runs. At the tested commit, the phase reports
2,649 `required-compiler-unknown` findings; the parent reported 1,494.

By cause:
- 2,141 `?` desugarings;
- 359 `for` desugarings;
- 24 range desugarings;
- 125 calls without a resolved definition.

The first three causes are `UnsupportedExpansion`: a compiler desugaring that
the collector does not treat as compiler-owned.

By package:
- noble-cli: 1,968;
- noble-contracts: 451;
- noble-wasm: 138;
- noble-kernel: 92.

Findings are counted once per observing unit, so CLI files count twice: once
for the bin and once for bin+test. Calculator-v1 accounts for most of the
increase:
- `calculator.rs`: 984;
- the new kernel `clone_budget.rs`: 77;
- the new noble-wasm preflight and weight files: 20.

A same-collector reproduction at the parent found 1,494 findings: 1,225 `?`,
210 `for`, 10 range and 49 unresolved. Every one of the 1,155 additional findings
is in one of the 13 source files that Calculator-v1 changed, and no finding was
removed. The unknowns come from ordinary Rust syntax in source code. No stale baseline
or allowlist produces them. The policy's only suppression mechanism is a
per-finding `waivers` list, and it is empty. Waiving 2,649 findings, or
patching Octet to treat these desugarings as compiler-owned, would redefine
the gate. The renewed source inventory is valid on the same complete
collection.

**`octet` and `boundary-controls`.** The deny-all gate stops while compiling
`noble-kernel` (lib test), on 232 deny-level Tigerstyle errors; the parent
had 146 in the same lint families. Calculator-v1's `clone_budget.rs` adds 35
of them. `boundary-controls` fails for two independent reasons, and both are also
present at the parent:

- Its positive `workspace` case runs the same deny-all gate, which aborts
  before writing `deny/compiler-architecture-ir.json`, so
  `boundary-read-observation.nix` cannot open the file. The missing IR comes
  from the deny-all lint abort, not from the architecture-phase block.
- The `acyclic-shell` fixture (`nix/boundary-fixture.nix`) renames the shell
  manifest's `[dependencies]` to `[dev-dependencies]`. `crates/noble-cli`
  already has a `[dev-dependencies]` table, so `cargo generate-lockfile`
  fails with "duplicate key".

## Non-claims

This is not M1 or CALC acceptance, and no CALC case is promoted. The
inventory classification is accounting, not extraction, refinement or proof.
The pure comparator result is not a `source-coverage` pass. Each pass binds
only the tested commit.

## Re-running

The helper scripts live in the issue campaign directory
(`/home/brittonr/.cache/noble-issues/bin`). Each one wraps the repository's
own pinned commands:

1. `repin-files.sh <worktree>`: re-hash edited files listed in
   `nix/tool-selection-files.nix`, then re-export `tool-selection.json`.
2. `export-policy.sh <worktree>`: run the `policy` check's Nickel and Octet
   standards exports, then re-pin.
3. `collect-local.sh <worktree> <rev> /var/tmp/<dir>`: reproduce
   `inventoryArtifacts`.
4. `derive-inventory.sh <worktree> /var/tmp/<dir>/out <outdir>`: run the
   repository's derive and comparator.
5. `node tools/source-inventory-renew.mjs <inventory.json> <label> <report>`,
   then `export-policy.sh`: renew the inventory.
6. `run-gates.sh <worktree> [checks…]`: build each flake check for `HEAD` (or
   `REV`), writing one log per check.
7. `bun tools/cairn.mjs gate {proposal,design,tasks} calculator-v1 --root .`,
   then `bun tools/cairn.mjs validate --root .`.
