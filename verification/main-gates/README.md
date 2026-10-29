# Main gate restoration (issue #22)

## Checking change log — 2026-10-29 (all fourteen checks pass at `91a473c`)

The [receipt](receipt-91a473c.json) records **14 PASS / 0 FAIL** at
`91a473c816619a0ef0ba60ced54b4d761e02bc20`, completing the gate repair in
[#72](https://github.com/OnixResearch/noble-lang/issues/72). Rust reports
**867 passed, 0 failed and 7 ignored across 98 result groups**. Strict Octet's
lint and architecture phases are clean. The sandboxed boundary matrix passes
both positive baselines and all 16 negative fixtures; each native comparator
verdict and case-output NAR identity is retained.

The exact pinned extraction Rust, Charon, Aeneas and upstream pin-check
outputs were realized on the verified builder. `tool-versions` and
`tool-overrides` both pass. This establishes executable availability and
override refusal, not a fresh extraction or compatibility replay.

Source coverage and native assurance return valid with empty diagnostics.
The inventory covers **109 units and 15,385 reviewed production paths**;
all **7,981 authored bodies remain open refinement obligations**. Their first
remote-store invocations failed before comparison because the local evaluator
refused an unsigned remote IFD artifact. The successful recovery evaluates
the byte-identical archived flake natively on that builder. No trusted key,
signature check, selected tool or source policy was relaxed; both failed
attempts remain in the receipt.

This pass binds `91a473c` only. The receipt and current-source declarations
are later documentation, not another full-roster execution. The earlier
`13b5a35` failures and interrupted tool run remain unchanged. No milestone,
case, proof obligation or Cairn archive is promoted by this gate restoration.

## Source repair — issue #72

The strict-source repair keeps request work as an explicit returned value,
separates signed-owner instantiation/charging from exact binding comparison,
and keeps live-reference and resource-catalog refusals typed. Moving
`capacity.rs` to `capacity/mod.rs` preserves `capacity::at_least` and its callers.
The finite authority/checkpoint and async-interleaving controls remain tests,
not extraction or refinement evidence. Historical translated files and
receipts are unchanged.

A native kernel API consumer accepted the live dispatch and refused a wrong
forwarded type (`StackJoin`), missing/out-of-range forwarding
(`BorrowProvenance`), a forged resource catalog (`InvalidContract`), and zero
work (`Exhausted(Work)`). Signed-owner mismatches, an absent owner and a foreign
caller returned `Unsupported(NodeForm)`. The two-site signed fixture accepted
at 47 work units and exhausted at 46; that is an observed fixture boundary,
not a universal cost claim. The kernel suite passed 174 tests in 17 result
groups, with no failures or ignored tests.

A separate real API consumer used BLAKE3 identities, emitted and checked the
`40 + 2` certificate, and refused a forged claim. It validated an actual
compiled packed layout and refused aliased registers and altered constant
bits. The batch consumer retained ordered wrapping results, cleared old
output for a shorter/empty batch, and retained buffer capacity. The graph
validators reserve from graph size, never from an untrusted declared register
count; this does not establish correspondence with the Lean models.

The graph suite passed 77 tests with one ignored test across 15 result groups.
A deterministic per-step cost regression first returned `0.01` instead of
`5.01` microseconds: the reporting helper converted microseconds a second
time. This predates the split: the `8ce33f961` receipt already contains the
mis-scaled column; [the cost-lane erratum](../ngraph/README.md#cost-accounting-issue-64-items-5-7)
derives corrected historical values without rewriting that receipt. The
corrected helper preserves the unit and the zero-step `None` case.
All 28 emitted cost rows agree with their reported per-call cost and step
count, including seven zero-step rows; the corpus keeps its one refused
derivation among 28 attempts. Timing remains measured, not a speed claim.

The retained boundary baseline records 25 errors, four warnings and Cargo exit
101. The pinned collector normalizes architecture shards only after Cargo
succeeds, so the old bundle contains static artifacts but no compiler IR. The
boundary adapter's missing-file refusal was therefore correct. The repaired
pinned-hook run produces complete 109-unit IR with both strict phases clean.
Artifact replay succeeds, but the unmodified boundary comparator rejects that
real bundle with only `compiler-unit-roster`: its reviewed list still held 96
units. The roster now includes the 13 already-existing test targets, with no
removals; exact equality, both positive baselines, all negative fixtures and
the 109-unit capacity remain unchanged.

Source accounting is renewed from the complete selected collection: 15,385
reviewed production paths (7,981 body/open, 4,239 generated, 3,165 structural),
5,985 test-only paths and 88 macro origins. The measured delta adds 32 paths and
removes 63 stale closure/import/test names; no retained classification or
package owner is changed. Accounting does not discharge refinement.

These API and host-collector observations alone did **not** clear the
fourteen-gate roster. The independent committed-source execution is recorded
above and in the linked receipt; no milestone or Cairn archive follows from
lint clearance.

## Checking change log — 2026-10-28 (native source binding at `13b5a35`; not all gates pass)

The [receipt](receipt-13b5a35.json) binds the #65 native-contract reporting
cutover and renewed source inventory to
`13b5a352e452a9b5f14720d1007289883cde7079`: **10 PASS / 2 FAIL / 1 INTERRUPTED /
1 NOT RUN**. The checks ran from a clean detached worktree. The Rust gate
reports 866 passed, 0 failed and 7 ignored across 98 test-result groups;
Clippy, format, documents, Cairn, policy, tool-selection, source-inventory
controls, source-coverage and native-assurance pass. The source comparator is
valid with empty diagnostics over 109 units and 15,416 reviewed production
paths. All 7,977 authored bodies remain open refinement obligations.

`octet` stops in `noble-kernel` with 58 findings (50 errors, 8 warnings).
`boundary-controls` fails opening the workspace deny
`compiler-architecture-ir.json`; the retained log does not establish why it
is absent. Neither the strict CLI/architecture phase nor the boundary matrix
is claimed to pass. The original failures are retained without waivers.

The tool-gate group reaches its outer 3,600-second timeout while
`tool-versions` is realizing dependencies; `tool-overrides` never starts.
A cache-assisted recovery dry run still requires 135 builds and 2.1 GiB of
fetched outputs. Twenty-five Rust archives alone expand to 9.37 GiB before
build scratch, with roughly 14 GiB free, so the larger rebuild was not
admitted. This is prerequisite evidence, not a gate pass. The remaining
strict, boundary and tool-gate work is tracked in
[#72](https://github.com/OnixResearch/noble-lang/issues/72).

The same receipt retains four independent native CLI probes for matching
source, stale source, a rebound false predicate with a stale positive claim,
and the matching finite refutation, including calibrated syscall observation
and exact input/output hashes. These are finite implementation observations,
not a Lean/Rust correspondence theorem or a universal noninterference proof.
No milestone, scenario or Cairn archive is promoted; previous green receipts
remain historical evidence for their own commits.

## Checking change log — 2026-10-23 (all fourteen checks pass at `e4cfb08`)

The [receipt](receipt-e4cfb08.json) records **14 PASS / 0 FAIL** at
`e4cfb086a3be5e2d6c19735c76c1d23145871fd9`, after WORKER-04 was completed: host
reflect-prepare over installed cells, host-selected dependency modules in the
invocation closure, and a retained declared-module session with alias
rebinding. `rust-tests` reports 85 binaries, 793 passed, 0 failed and 7
ignored. `octet` is clean in both phases. The renewed inventory accounts for
14840 production paths. `boundary-controls` passes all 18 cases on its second
invocation under a 3500 s client timeout.

Reaching a clean `octet` took one repair (`21340da3`): the new closure
helpers in `noble-contracts` take and return the meter by value rather than
`&mut`, and the retained-session stage share uses `checked_div`. Replays of
the WORKER-02, 03, 04, 05, 06, 07, 09, 10 and 12 gates at `e4cfb086` pass. The
pass binds `e4cfb08` only and is not milestone acceptance.

## Checking change log — 2026-10-23 (all fourteen checks pass at `4e6b2b0`)

The [receipt](receipt-4e6b2b0.json) records **14 PASS / 0 FAIL** at
`4e6b2b09c5734fe3f6f5398d93c6b49703b3fd58`, after the MW1 case work:
WORKER-03, WORKER-07 and WORKER-10 accepted, WORKER-09's MW1 rows executed,
and WORKER-04's export-import and optimizer variants run (the case stays
partial). `rust-tests` reports 85 binaries, 789 passed, 0 failed and 7
ignored. `octet` is clean in both phases. The renewed inventory accounts for
14729 production paths. `boundary-controls` passes all 18 cases; under a
3500 s client timeout it needed three invocations, the last building only the
aggregate.

Reaching a clean `octet` took one repair: the new `test.send` branch pushed
the kernel's `kind_matches_definition` over the assertion-density
threshold, so a sealed `bound_operation` map now gives each bound
operation's slot and effect (`44bba57e`). Replays of the WORKER-02, 03, 04,
05, 06, 07, 09, 10 and 12 gates at `4e6b2b09` pass. The pass binds
`4e6b2b0` only and is not milestone acceptance.

## Checking change log — 2026-10-22 (all fourteen checks pass at `0dd2fe4`)

The [receipt](receipt-0dd2fe4.json) records **14 PASS / 0 FAIL** at
`0dd2fe41c3f32de490e2502346c58a50085a1b24`, after the WORKER-12, T1,
WORKER-05 and WORKER-06 work and the three remaining MW1 entry gates: the
Worker-Typed preparation service, declared `WorkerData@1` schema modules for
Worker Program v2, and the typed worker's bounded execution profile.
`rust-tests` reports 85 binaries, 788 passed, 0 failed and 7 ignored.
`octet` is clean in both phases. The renewed inventory accounts for 14520
production paths. `boundary-controls` passes all 18 cases in one 3499 s
invocation beside the other thirteen checks.

Reaching a clean `octet` took four lint repairs and one capacity change. The
K-CHECK-07 request budget from `2d263eaf` is an `Rc<Cell<u32>>` in
`noble-contracts`; its fields, impls and parameters now carry reviewed
expectations (`71e6f430`) rather than an explicit-state redesign. The
emitted import writer and the preparation CLI test were split (`8dc625a0`,
`d3fa394d`, `174310c4`). The CLI `noble` bin+test shard grew to 32,926
constructors, so only `max_constructors` doubles (`f99b1d4a`). Replays of
the WORKER-02, 05, 06 and 12 gates at `0dd2fe41` pass. The pass binds
`0dd2fe4` only and is not milestone acceptance.

## Checking change log — 2026-10-22 (all fourteen checks pass at `6e6f6ed`)

The [receipt](receipt-6e6f6ed.json) records **14 PASS / 0 FAIL** at
`6e6f6ed94d99d9275cb0dac88d9b555c1a01f6e2`, after the WORKER-12 adapter
closeout, the WORKER-02 acceptance, the Worker Program v2 package-encoding
work and the boundary aggregate change. `rust-tests` reports 85 binaries, 774
passed, 0 failed and 7 ignored. `octet` is clean in both phases. The renewed
inventory accounts for 14350 production paths. `boundary-controls` passes all
18 cases.

The boundary aggregate now links each case bundle instead of copying it. Its
NAR fell from 7.6 GiB to 7.9 KiB, and the case bundles stay in its closure.
The verification run at `b1ef5886` took 3232 s; the passing `9a15fc5` run
had needed a further 2799 s just to copy the old aggregate back. Of three
invocations at `6e6f6ed`, the first ran beside the other thirteen checks and
stopped at the 3600 s tool limit. The second failed five cases with exit 127
because the builder could not find `gnutar-1.35/bin/tar`. The third rebuilt
those cases without that error and passed. A replay of the WORKER-02 gate at
`6e6f6ed` gives the same nine outcomes as its accepted run.

Commits `3e5f4a51` through `adb26c43` edited tool-selection-bound files without
re-pinning, so they do not evaluate; `8b25bcb6` restores evaluation. The pass
binds `6e6f6ed` only and is not milestone acceptance.

## Checking change log — 2026-10-21 (all fourteen checks pass at `9a15fc5`)

The [receipt](receipt-9a15fc5.json) records **14 PASS / 0 FAIL** at
`9a15fc50b52a373c28b1a018a8f61d81a7d3ebac`, each check built from a clean
worktree. `rust-tests` reports 85 binaries, 770 passed, 0 failed and 7 ignored;
`octet` is clean in both the lint and architecture phases with 0 findings;
`boundary-controls` reports all 18 cases passed: 2 positive baselines and 16
rejected fixtures.

The six failures recorded at `f03413e` below were cleared at their source:

| check | repair |
|---|---|
| `documents` | `298673c8` unpacks the twelve compacted archives that 17 canonical links target and de-links two directory targets |
| `rust-tests` | `9ced7fc0` and `e3011209` resolve `gcc`, `mkfifo` and `kill` from the check's `PATH` instead of undeclared absolute paths |
| `octet` | `9c62cafa` declares the `live-test-barrier` feature row; `3822f977` clears the 543 integrated deny-all findings |
| `source-coverage`, `native-assurance` | `9c62cafa` sets the measured 96-unit and shard caps and classifies the `rustix` and `serde` providers; `afda8005` and `795dd9f8` renew the accounting |
| `boundary-controls` | `77cb7803` renews the 96-unit roster; `d7a95765` replaces the 180 s single-job budget, which could not compile the Wasmtime workspace, and accepts the declared feature plan; `9a15fc50` observes the selected Octet's `source-role-unit-mismatch` refusal for `test-source-import` and renews the randomness fixture lock |
| `policy` | `e2c6c174` adds the reviewed providers to the Octet controls |

Two `boundary-controls` invocations stopped at a 3550 s client timeout with no
failing case. Transfer, not compilation, dominates that check's wall time on a
remote builder: the aggregate output is a 7.6 GiB NAR holding a copy of each
case result (about 533 MiB each), and the third invocation, which had only the
aggregate left to build, spent 2799 s copying it back. A green gate set binds
only `9a15fc5`. It is not milestone acceptance,
and all authored bodies remain open refinement obligations.

## Checking change log — 2026-10-21 (current source, commit `f03413e`)

The [receipt](receipt-f03413e.json) records a cold run of all fourteen flake
checks at `f03413edd116548af40e519f4e5ab8958761b48d`; its parent is upstream
`68d16b8a`. **Eight checks pass and six fail**, so the gate is not green.

At `68d16b8a` no check could evaluate at all. `c1d18799` merged the nine-lock
`nix/reviewed-vendor.nix` recipe after `c56df13b` had pinned the eight-lock
vendor, and the merge did not re-pin `component_sync`, so evaluation stopped at
`assert toString reviewedVendor == selectionPolicy.component_sync.vendor`. The
tested commit only re-pins `verification/m5/pins.json` (vendor path and NAR
hash), its `policy/tool-selection-files.ncl` binding and the regenerated
`policy/tool-selection.json` export; no Cargo lock, crate archive or recipe byte
changed. The rebuilt vendor NAR hash is
`sha256-APDZUCJCimbMM32b1rJcNGn5mt2jYfUxgbwO4CxsGhQ=`.

| check | result | attribution |
|---|---|---|
| `tool-selection`, `tool-versions`, `tool-overrides`, `format`, `clippy`, `cairn`, `policy`, `source-inventory-controls` | pass | runs bind `f03413e` only |
| `rust-tests` | fail | `c_backend.rs` hardcodes `/nix/store/788mx…-gcc-wrapper-15.2.0/bin/gcc`, which no current pin produces and no check input declares; reproduced with `--option builders ''` |
| `documents` | fail | 17 links into `.cairn/archive/*` broke when `e282d8fa` compacted those archives into `.cairn/compaction/v1`; 18 bun tests then fail |
| `octet` | fail | `Cargo --all-features plan does not match the explicit architecture policy row` |
| `source-coverage`, `native-assurance` | fail | `selected compiler architecture units exceed policy limit 89` |
| `boundary-controls` | fail | `acyclic-shell` cannot open the missing `deny` artifact root; the same #26-family cause, after one attempt lost `cargo-vendor-dir` to builder signal 15 |

The historical 10 PASS / 4 known FAIL receipt and its `2026-10-16` roster
receipt are unchanged; they bind their own commits and are not relabelled by
this run. No check was weakened or rerun to produce a pass, and no proof
obligation, milestone acceptance or scenario case moved.

## Checking change log — 2026-10-16 (#27 roster only)

The [source-bound roster renewal receipt](boundary-roster-20261016.json)
renews `policy/boundary-controls.ncl` and its Nickel JSON export from **37 to
88 exact reviewed units** at published source
`cd76c608721f25ce7f7bc896dfc7cdeb54e9b004`: all 37 historical rows remain,
51 are added, and no targets are removed. Reuse of the complete `5406170`
collection is justified by **698 byte-identical source/manifest/Nix/tool
context files**, independent Cargo discovery of **77 targets / 88 units**,
reviewed source-role and configuration agreement, **88 complete shards**,
and successful pinned `cargo-octet artifact verify` replay. This is base-source
evidence, not evidence for later changed manifests or all-features code.

The actual Nix comparator rejected `compiler-unit-roster` before renewal and
no longer reports that diagnostic afterwards. Missing/duplicate units and
wrong kind/role still reject; wrong target/features/default-features still
reject. All **52 comparator controls and 20 fixture controls** evaluated,
and the real `checks.x86_64-linux.boundary-controls.drvPath` instantiated
successfully. The positive-input probe retains every real unit and only the
item facts/ownership queried by that path; the original bundle was replayed
separately and the full architecture receipt remains unchanged. In
particular, the warning-only scratch input still rejects
`required-deny-all-result` and `expected-boundary-observation`, with all
**3,745 architecture findings** retained. No clean deny-all result is invented.

**Limits and remaining failures are unchanged.** Actual Nix fixture rendering
and Cargo metadata show that `acyclic-shell` retains all 88 workspace units
and adds the CLI library and library-test units: **90 selected units against
`max_units=88`**; a complete per-unit collection would also need 90 shards
against `max_shards=88`. The library is the normal-dependency endpoint used
by the reverse-dependency control. Removing it breaks that control; pruning
other targets reduces existing coverage. No coverage-preserving correction
within the unchanged limits has been established. This capacity condition
is measured from target discovery, not a newly executed compiler refusal.

No comparator, configuration check, deny-all rule, fixture, cap, or selected
tool pin changed. The known-red aggregate checks were **not rerun**; #25
architecture unknowns and #26 deny-all failures remain independent. The
historical **10 PASS / 4 known FAIL** receipt below is unchanged. A later
integrated manifest/source change (including `safety-observe`) requires a
fresh collection or a new relevant-context identity proof.

Publication `0eb5c0478ebf3419de6f94b8144c2ac7520356ca` is the **historical
prior pending stage**: it published the Numeric-Graph-v1 runner, inputs, docs
and evidence while final source-bound acceptance and independent claims
review were still pending, with no case recording, Cairn sync or archive.
That describes the earlier publication, not the current lifecycle state.

The later immutable [acceptance receipt](../numeric-graph-v1/acceptance.json),
[independent review](../numeric-graph-v1/review.json) and
[review receipt](../numeric-graph-v1/review-receipt.json) support the bounded
[registered state](../../specs/STATUS.json): **8 passed, 7 not-run**
(01/06/07/08/10/13/14/15 passed; 02/03/04/05/09/11/12 held at
`absent/not-run/open/unassessed` with empty evidence). The
[lifecycle record](../numeric-graph-v1/README.md) preserves the supplemental
runtime binding and every remaining per-case gap. The change stays active;
there is no archive or whole-feature acceptance.

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

The accepted 707-file build/policy projection is byte-identical between the
published `0eb5c04` source and tested `bc10df8`; the immutable
[build provenance](../numeric-graph-v1/build-provenance.json) records that
equivalence without relabeling the old logs as a new gate run. CASE-02's
capability closure remains incomplete (637 relevant unknowns), and the
guest/protected counters for all seven held cases are structural rather than
direct observations. NG-PO-A..H remain OPEN; no performance, GPU,
backend-selection or cross-platform claim is added. Lifecycle recording does
not change the historical **10 PASS / 4 known FAIL** gate disposition.

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

## Results at `816c9856` (MW2 closeout)

All fourteen pinned checks pass at one commit for the first time since `e4cfb086`.
`verification/main-gates/receipt-816c985.json` binds them; it records each check's
exit status, duration, log digest, store output path and observation, the 85 passing
`test result:` suites (795 tests, 0 failures), the three repairs below, and the
non-claims. The tested commit is `816c9856e` and its parent is `35fcdc04d`.

| check | result | notes |
|---|---|---|
| `tool-selection` | pass | export equals the committed JSON byte for byte |
| `tool-versions` | pass | reviewed pins only |
| `tool-overrides` | pass | overrides rejected |
| `format` | pass | no rustfmt diff |
| `rust-tests` | pass | 85 suites, 795 tests, 0 failures |
| `clippy` | pass | all targets and features, warnings denied |
| `documents` | pass | specs, links and scenarios |
| `cairn` | pass | 36 changes, no issues or findings |
| `policy` | pass | reviewed inputs fresh |
| `octet` | pass | lint clean, 0 architecture findings |
| `source-inventory-controls` | pass | positive and negative controls |
| `boundary-controls` | pass | 18 of 18 declared fixtures |
| `source-coverage` | pass | `valid: true`, empty diagnostics |
| `native-assurance` | pass | `valid: true`, empty diagnostics |

Repairs needed to reach this state, each recorded in the receipt:

- `format` failed because the new transition predicate and the split worker emitter
  were not rustfmt-clean; `cargo fmt --all` reformatted both and the tool-selection
  pins were renewed in the same chain (`35fcdc04`).
- `source-coverage` then rejected `subject-omission`/`subject-stale` because two
  reviewed subjects moved by formatting; the inventory was rebound from the declared
  `live-test-barrier,safety-observe` collection (`816c9856`).
- `boundary-controls` exceeded a 3500 s wrapper timeout while still copying
  dependency outputs; the unbounded retry finished in 2689 s with exit 0.

The Octet architecture pass is a reviewed classification, not a waiver: the two
`required-compiler-unknown` findings were `anyhow::Context` trait calls on generic
receivers in the composition-root transition host, and `anyhow` now sits in
`dynamic_dispatch.external_crates` alongside its existing infrastructure provider
classification.

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
