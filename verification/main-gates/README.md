# Main gate restoration (issue #22)

The current [`receipt.json`](receipt.json) records the complete local run of
all 14 pinned flake checks on Numeric-Graph-v1 integration commit
`bc10df848ed81d53e849c440c1f639979e40d38d`: **10 pass, 4 fail**.
Passing: tool-selection, tool-versions, tool-overrides, format, rust-tests,
clippy, documents, cairn, policy and source-inventory-controls.
Failing: boundary-controls ([#26](https://github.com/OnixResearch/noble-lang/issues/26),
[#27](https://github.com/OnixResearch/noble-lang/issues/27)), octet (#26),
source-coverage and native-assurance
([#25](https://github.com/OnixResearch/noble-lang/issues/25)).
`rust-tests` reports 529 passed, 0 failed, 6 ignored in 77 test binaries;
its retained log SHA-256 is
`095c328b3c1dc2538508705b95d1c59ace56cc446224f8c6b4894c161e235071`.
Every row binds its original external log; recovery does not rerun or
reinterpret a failing check. This is not global gate acceptance.

The publication merges the acceptance runner and its inputs plus docs and
evidence only after that tested commit; Rust, Cargo, policy and Nix inputs
remain byte-identical to `bc10df8`. Final source-bound acceptance and the
independent claims review remain pending. No case is promoted, no Cairn
sync or archive is performed, and CASE-02 capability facts stay OPEN.
NG-PO-A..H remain OPEN; no performance, GPU or cross-platform claim is added.

## Historical baseline

The preserved [`receipt-f2b398d.json`](receipt-f2b398d.json) records one run of every pinned flake check on `f2b398d`
(`integration/issue-14-ngraph`). That branch merges issue #14's numeric
graph Phase B/C crate onto `main` `407dfd6`. The receipt also keeps the
earlier runs on integration commit `2edb977` (`integration/issues-20261015`)
and on issue #22 branch commit `8df6c43`. That branch starts at
Calculator-v1 `44329dc`.
That historical receipt holds the tested commit, each check's result,
log path, log SHA-256 and duration, plus the selected tool paths. The logs
stay outside the repository; their hashes bind them.

## Results at `f2b398d` (issue #14 numeric graph integration)

| check | result | integration `2edb977` | change from `2edb977` | follow-up |
|---|---|---|---|---|
| `tool-selection` | pass | pass | 144 pinned file digests (was 143) |  |
| `tool-versions` | pass | pass |  |  |
| `tool-overrides` | pass | pass |  |  |
| `format` | pass | pass | includes the crate |  |
| `rust-tests` | pass | pass | 471 passed (+48 from the crate), 5 ignored (+1) |  |
| `clippy` | pass | pass | includes the crate's lib, bins and tests |  |
| `documents` | pass | pass |  |  |
| `cairn` | pass | pass | `numeric-graph-profile` gates pass |  |
| `policy` | pass | pass | new exports and provider controls |  |
| `source-inventory-controls` | pass | pass |  |  |
| `boundary-controls` | fail | fail | same first failure; refusal now names limit 78 | [#26](https://github.com/OnixResearch/noble-lang/issues/26), [#27](https://github.com/OnixResearch/noble-lang/issues/27) |
| `octet` | fail | fail | none: same 236 findings (232 errors) | [#26](https://github.com/OnixResearch/noble-lang/issues/26) |
| `source-coverage` | fail | fail | +842 architecture unknowns (3,545) | [#25](https://github.com/OnixResearch/noble-lang/issues/25) |
| `native-assurance` | fail | fail | +842 architecture unknowns (3,545) | [#25](https://github.com/OnixResearch/noble-lang/issues/25) |

All 14 checks were built locally (`--builders ""`). A first remote attempt
stalled while copying Rust toolchains to the builder and was stopped; its
partial logs are listed in `receipt-f2b398d.json` (`flake_note`).

`rust-tests` ran 471 tests in 68 test binaries: 471 passed, 0 failed and 5
ignored. The crate adds 15 binaries with 48 passing tests and 1 ignored
test. The ignored test is `extract_b2`'s L2 BigInt oracle, which needs
`NOBLE_NODE`. No default crate test needs Node or the CLI. The other 4
ignored tests are the same ones as at `2edb977`.

What the new crate changes in the failing checks:

- **`source-coverage`, `native-assurance`.** The architecture phase now
  reports 3,545 findings, up from 2,703. All 842 new findings are
  `required-compiler-unknown` in `noble-ngraph-experiment`:
  - 266 `?` desugarings;
  - 220 `for` desugarings;
  - 6 range desugarings;
  - 343 unresolved calls;
  - 7 `MissingBody` (a required trait method with no body in the unit).

  The 2,703 findings outside the crate have the same code, package, path and
  fact set as an unchanged-policy collection of the merged source. The
  warning-only Tigerstyle phase in the same logs rises from 2,271 to 4,464
  warnings. All 2,193 new warnings are in the crate, and they appear whether
  or not the crate is a policy package.
- **`octet`.** No change: 236 findings (232 errors). Cargo stops in
  `noble-kernel` before it compiles the crate, so the crate's deny-level
  findings are not reached.
- **`boundary-controls`.** The first failing case is still `acyclic-shell`.
  Its deny-all run refuses before collection with "selected compiler
  architecture units exceed policy limit 78". At `2edb977` the same case
  reported "limit 58". The fixture adds two `noble-cli` lib units to an
  exact capacity, so this is #27's capacity reservation. The `2edb977` note
  below attributes this failure to the deny-all abort; that is corrected
  here. The `workspace` case, built separately at both commits, aborts in
  `noble-kernel` with the same 118 findings (116 errors). None of them is
  in the crate (#26).

### What the merge needed

`f531623` merges `feat/issue-14-ngraph-core` `3e471ea`. It adds the
workspace member `crates/noble-ngraph-experiment`, receipts under
`verification/ngraph/` and updates to `.cairn/changes/numeric-graph-profile/`.
It changes no `noble-kernel`, `noble-contracts`, `noble-wasm` or `noble-cli`
source. Three integration commits follow:

- **Vendor re-pin** (`2d03e99`). The crate adds one registry package,
  `rustc_apfloat` `0.2.3+llvm-462a31f5a5ab`; `blake3`, `serde_json`,
  `bitflags` and `smallvec` were already locked. The eight-lock reviewed
  vendor therefore has 238 archives, and the `nix/reviewed-vendor.nix` count
  asserts move from 237 to 238. The vendor is now
  `/nix/store/b0wvglmmv85jgi47rsf8j1nf4ygm7kbb-noble-reviewed-offline-vendor`,
  with measured NAR `sha256-ykvOtE9J6E2pgRlkq0nUdb6xjF2iapE7lCMcT1bFrNk=`.
  Until the re-pin, the `flake.nix` vendor assert stopped evaluation of
  every output. The M5/M6 pins carry the new vendor. The crate manifest is
  pinned alongside the other workspace manifests, so 144 file digests are
  pinned. Changed hashes are re-pinned and `policy/tool-selection.json` is
  re-exported.
- **Architecture scope** (`20067cb`, `ba240bc`). Octet collects only the
  policy's production packages. With the merge alone, a scratch collection
  selected the same five packages and 58 units and reported the same 2,703
  findings, so the crate was outside the architecture, source-inventory and
  native-assurance gates. The policy has no non-production package role:
  `test` and `generated` apply only to non-production scopes, and Octet's
  closed schema has no exclusion or experiment concept. The crate therefore
  joins production as a `composition-root`, like `noble-cli` and
  `noble-syndicate`. It is a std harness that depends inward on
  `noble-kernel` and `noble-contracts` and runs host effects: files,
  subprocesses, clocks and threads. The owner kept this classification:
  the approved P3 placement is a workspace crate, and composition-root keeps
  it under every gate without inventing an exclusion concept; moving it out
  of the workspace remains an owner option. The commits add:
  - package, `src` and `tests` roles;
  - a pure-value `rustc_apfloat` provider classification, also added to
    the reviewed provider list in `nix/octet-controls.nix`;
  - native-assurance records for the package, its three external edges and
    a safety argument;
  - the crate's five source-inventory dependency edges.

  No waiver, dependency rule or finding classification changed.
- **Capacity** (`20067cb`). A scratch collection with that policy observed
  exactly 78 complete units: the crate's lib and lib test, four bins, their
  four test units and ten integration tests. Its largest shard (lib+test:
  7,300 calls, 12,441,948 bytes) and the 163,306,150-byte IR fit the
  existing caps, so only `max_units` and `max_shards` move from 58 to 78.
- **Source inventory** (`f2b398d`). `tools/source-inventory-renew.mjs`
  renewed the inventory from a complete 78/78-unit collection of `20067cb`.
  It kept all 9,063 reviewed paths, removed none, and added 2,432 crate
  paths: 897 OPEN bodies, 779 generated and 756 structural. See
  [the inventory record](../source-inventory.md). The pure source and
  native-assurance comparators over that collection both return
  `valid=true`.

### Docs-only commits after the tested commit `f2b398d`

None of these commits changes Rust, WAT, `.noble`, Nix, Cargo, proof or
`policy/` sources. They touch only Markdown and receipt JSON, so the heavy
results above still apply.

- `2c13735`: this receipt.
- `2002db3`: records the owner's decision to keep the composition-root
  classification (one README sentence).
- `33d4d87`: merge of `feat/issue-14-ngraph-core` `233429d`, the Phase C1
  Wasm/V8 receipt. It changes only
  `.cairn/changes/numeric-graph-profile/{design,tasks}.md`,
  `verification/ngraph/README.md` and the new
  `verification/ngraph/phase-c-l1.json`.
- The commit that adds this list. The doc-sensitive gates (`documents`,
  `cairn`, `tool-selection`, `policy`, `format`) were re-run on it; all five
  pass.

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
collector evidence in `receipt-f2b398d.json`, and renews the source inventory
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
with the same runner, and its logs are in `receipt-f2b398d.json` under
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
   `inventoryArtifacts`. Set `CARGO_CONFIG_SRC` to a Cargo config whose
   `reviewed-vendor` directory is the currently pinned vendor.
4. `derive-inventory.sh <worktree> /var/tmp/<dir>/out <outdir>`: run the
   repository's derive and comparator.
5. `node tools/source-inventory-renew.mjs <inventory.json> <label> <report>`,
   then `export-policy.sh`: renew the inventory.
6. `run-gates.sh <worktree> [checks…]`: build each flake check for `HEAD` (or
   `REV`), writing one log per check.
7. `bun tools/cairn.mjs gate {proposal,design,tasks} <change> --root .` for
   `calculator-v1` and `numeric-graph-profile`, then
   `bun tools/cairn.mjs validate --root .`.
