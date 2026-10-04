# M1 component review and current evidence

## Reviewed inputs

| Component | Immutable revision | Consumed contract |
|---|---|---|
| Octet | `235255bc4972ced9128fd5b4d1ec66ff7508ded4` | Nix packages, deny-all hook, architecture policy, compiler facts, and receipt validation |
| Aeneas | `505b6ca35217e7be5c96c3e2f8045edfbdf47291` | Lean translation, `charon-pin`, and `backends/lean` |
| Charon | `b104e24fea7d721b71e6c39fd70f26ff20bc0980` | `cargo --preset=aeneas`, complete kernel extraction, and its Rust toolchain |
| Lean | `68218e876d2a38b1985b8590fff244a83c321783` (`v4.31.0`) | Lake compilation with the matching Aeneas backend |
| Mathlib | `fabf563a7c95a166b8d7b6efca11c8b4dc9d911f` | Backend dependency from the upstream Lake manifest |

Nix generated `flake.lock`. No lock field was edited by hand.
The top-level Nixpkgs and Rust overlay follow the pinned Octet inputs.
Cairn retains the existing runner revision `15f00875562025e7ea7e0d1f4af24d1a2e2ac06f`.
The lock retains each upstream dependency graph without manual compatibility overrides.
Octet's optional proof providers do not replace Noble's selected Aeneas input.

The production quality lane uses `nightly-2026-03-21`, as required by Octet's compiler plugin.
The extraction lane uses Charon's `nightly-2026-08-18`.
Both lanes use `x86_64-unknown-linux-gnu` with all current features and explicit overflow checks.
The current CLI declares external production Cargo dependencies, including
`anyhow` and `wasmtime`; these are not yet reviewed into the source/native
dispositions. No current acceptance follows from the earlier dependency claim.

## Ownership and reuse

Noble owns the budget transition, CLI decoding, source scopes, policy, and acceptance claims.
The kernel has no host effects. The CLI reads arguments and writes diagnostics or outcomes.
No generic port or shared infrastructure crate is necessary for this internal smoke route.

The Nix gate runs the published `hooks/octet-deny-all.sh` with Noble's
content-hashed local wrapper built from the pinned Octet source. The current
path-provenance correction remains under review, not an accepted security
claim.
The pre-commit hook is now local/system with the absolute selected gate store
executable, not the historical upstream SSH hook checkout. Run it from the
reviewed devShell; if its selected output is absent, it fails closed instead
of taking an ambient binary from `PATH`. The earlier HTTPS authentication
failure and SSH fetch belong to the historical upstream-hook adoption only.
The development shell supplies the selected Octet executable and clears
ambient compiler flags and wrappers.
The gate does not copy the lint catalog or accept warning-only results.
The upstream `mkConsumerCheck` helper runs plain `cargo-octet check`, so it does not establish this deny-all contract alone.

`policy/architecture.contracts.ncl` is an unchanged copy of the reviewed Octet schema template.
This source reuse relies on the OnixResearch owner's cross-project source grant.
No Octet implementation code enters Noble's production crates.
Upstream tools and dependencies retain their own ownership and license boundaries.

Reviewed Octet interfaces at the pinned revision:

- `templates/pre-commit-config.yaml` and `hooks/octet-deny-all.sh`.
- `templates/octet-architecture.contracts.ncl` and `templates/octet-architecture-export-spec.ncl`.
- `docs/project-architecture-policy.md` and `docs/consumer-guide.md`.
- `crates/octet-architecture-ir/src/{selection,roles,membership}.rs`.
- `src/architecture_collector.rs` and `cargo-octet/src/architecture_ir.rs`.
- `crates/octet-standards/src/project_architecture_gate.rs` and its typed production view.
- `cargo-octet/src/artifact.rs` and its full-bundle replay controls.

The manifest command requires `--nickel-identity` with the observed `nickel --version` output.
An omitted identity becomes `nickel:unspecified` and fails, even when the correct evaluator is on `PATH`.
The selected evaluator reports `nickel-lang-cli nickel 1.17.0 (rev 1320a98)`.

## Configuration experiments

The initial `panic = "abort"` development profile failed architecture unit selection.
Cargo produced two normal kernel units with identical source, target, features, and test configuration.
Their panic modes differed because Cargo's test dependencies use `unwind`.
Octet's observation key omits that profile distinction and rejected the graph as ambiguous.

The selected development and release profiles use `unwind`.
No target, test, or feature was excluded.
The original abort-mode extraction result remains historical evidence, not evidence for the changed configuration.
The updated checked-subtraction kernel translated again and its generated Lean module compiled again.

## Narrow source allowances

The deny-all catalog remains enabled. No warning budget or finding baseline exists.

| Exact scope | Allowance | Reason and evidence |
|---|---|---|
| `noble_cli::read_arguments` | `tigerstyle::ambient_env` | The composition root must observe process arguments. No kernel caller reads them. |
| `noble_cli::parse_budget` | `tigerstyle::missing_const_fn` | The pinned rustc rejected const iterator and OS-string operations with `E0277`. |

The source attributes name `noble-maintainers` as their owner.
The private `InputError` enum uses Octet's sealed-enum marker.
Its diagnostic match must name every variant. It has no catch-all arm.
The allowances require review after changes to these functions or the pinned compiler and lint cohort.

## Published receipt repair

The previous Octet revision, `0b1ca30a7e86bb38369bdf2b8a851772a3b9f213`, rejected Noble's unprotected console output during receipt self-verification.
Its producer selected authorization identities from protected protocols, but its verifier required them for every nonempty outbound-effect list.

The provider repair now requires those identities only when at least one declaration is protected.
The original 22 gate tests pass. The updated suite has 27 passing tests, including the formerly failing positive case.
Protected and mixed receipts still reject missing identity arrays after resealing.
Identity tampering, missing claim limits, and unclassified effects remain rejected.

The provider's all-target standards tests passed: 174 unit tests and 24 integration tests.
Strict package Clippy, Nix formatting, architecture-policy checks, traceability, and the runner build passed.
An additional pedantic experiment reproduced 21 findings in unchanged Architecture IR code. No full-provider pedantic success is claimed.
The provider change was synced, archived, committed, and published on Octet's `main` at `74a5d6b057272b149ae86049107cba5eac986b28`.
Its review record is `.cairn/archive/2026-09-13-fix-unprotected-effect-receipts/evidence.md` in Octet.

Noble first adopted that published revision in Nix and pre-commit. Nix regenerated the lock.
The regenerated policy export and manifest are byte-identical to their previous versions.
The hook, lint catalog, compiler collector, contract template, Rust pins, and dependency locks did not change in the provider repair.
No provider implementation code was copied into Noble.

## Historical integration-test role conflict

Revision `74a5d6b057272b149ae86049107cba5eac986b28` passed the full lint phase but rejected two source-role conflicts:

```text
source scope `crates/noble-cli/tests` conflicts with package `noble-cli` role; mixed source roles are unsupported
source scope `crates/noble-kernel/tests` conflicts with package `noble-kernel` role; mixed source roles are unsupported
```

Noble declares these integration-test scopes as `test`.
The old compiler receipts assigned their separate `test+test` units the production package roles.
Those valid failure receipts remain historical evidence. They did not establish architecture acceptance.

## Published compiler-bound ownership and no-std evidence

Octet published `f8b12bea3ba5d72de74df9f569afcdf396871325` before Noble adopted it in both Nix and pre-commit.
Nix regenerated the lock. The Rust sources, architecture declarations, extraction inputs, and narrow source allowances did not change.

The provider retains compiler-unit roots and fact memberships before deduplication.
Only explicitly declared, compiler-bound independent integration tests receive the `test` role.
Library and binary test configurations retain production roles and obligations.
The typed production view retains every shared production observation and validates reference closure before and after projection.
Receipt v2 requires explicit test-only dispositions. Full artifact replay rejects altered dispositions after outer hashes are refreshed.

The pinned rustc stores the active crate attribute as parsed `AttributeKind::NoStd`.
The collector now reads that compiler variant rather than the unparsed attribute-name API.
Both kernel production units, `lib` and `lib+test`, observe `crate:no-std=noble-kernel`.
Test-only or package-wide witnesses cannot satisfy that per-unit obligation.
This attribute evidence does not prove transitive exclusion of `std` or functional purity.

Provider verification passed 875 scoped tests and strict four-package Clippy with all targets and features.
Eight real compiler runs cover ten units each. A separate symlink control rejects before compiler invocation.
Active/inactive configuration controls, production-source imports, missing shards, and evidence-promotion controls retain rejection.
The UI Nix check executed four harness tests. The compiler-target Nix cohort passed 28 tests, overlapping the scoped count.
Seven unrelated ignored runner tests remain outside this cohort.
The provider synced both accepted specifications and archived the change under `.cairn/archive/2026-09-14-bind-integration-test-ownership/`.
Its `no-std-evidence.md` records the exact scope and historical failures.

At that published-provider checkpoint, Noble's deny-all runs passed with zero
lint or architecture findings. All six then-required compiler units were
covered, including two independently bound integration tests.
Console output remains explicitly unprotected. No target, declaration, lint, or production obligation was removed.
The retained direct bundle passes full artifact replay. Replay establishes consistency, not independently authenticated acceptance.
Noble's M1 acceptance remains open.

## Commands and evidence

Run from the dedicated Noble worktree:

```sh
nix build .#checks.x86_64-linux.format .#checks.x86_64-linux.rust-tests \
  .#checks.x86_64-linux.clippy .#checks.x86_64-linux.documents \
  .#checks.x86_64-linux.cairn .#checks.x86_64-linux.policy
nix develop -c pre-commit run octet-deny-all --all-files
nix flake check --keep-going -L
```

All three commands passed against that historical published ownership revision.
The seven published-provider checks remain incremental gates, not the complete M1 acceptance matrix.
Tool-selection work added three checks; all ten passed together at that checkpoint.
The main repository retains the original failure and unchanged kernel extraction artifacts in `.pi/m1-quality/`.
The earlier receipt repair remains recorded in `.pi/m1-octet-repair/`.
Ownership implementation and failure history remain in `.pi/m1-test-ownership/`.
Its `no-std/` directory records provider completion and publication; `adoption/` records Noble's published-provider checks.
The worktree contains no owner-only evidence under `.pi/`.

## Immutable selection and scoped compatibility

The [selection review](tool-selection.md) records the Nickel policy, pure Nix decision, and thin execution helpers.
The primary repository retains the local record under `.pi/m1-tool-selection/`.
The controls include 96 pure cases, 18 executed shell rejections, and a real same-byte Charon path-override rejection.
Ten selected tool recipe/output pairs bind material transitive build inputs without selecting unused optional providers.

The Nix-packaged Lean 4.31.0 executable compiled a fresh object from the byte-checked generated kernel module.
Ten backend Git dependencies matched before and after execution. Compiled cache authenticity remains unestablished.
Two budget integration tests passed under the selected Miri driver with strict provenance and a fresh sysroot cache.
The remaining M1 native inventory and negative controls are not implied by that small interpreted scope.
No Rust production, Cargo, architecture policy, or extraction manifest changed during this selection work.
The final Nix Octet bundle passes replay, with no target or obligation removed.

## Runbook and workspace review

The [runbook](m1-runbook.md) names the available procedures and reserves five unimplemented app interfaces.
The [control matrix](m1-controls.md) covers all eight required families without promoting planned cases to observed results.
A fresh proof directory restored ten pinned Git dependencies and reused an upstream download cache for compiled artifacts.
Fresh extraction, a Lake build, explicit Lean compilation, and the two scoped Miri tests passed there.
Runbook links now belong to document validation. Three new regressions pass, for 100 Bun regression tests in total.
The document loader excludes private evidence and compiler caches. It does not implement source accounting.

Cargo metadata and the unchanged Rust source establish task 2.1's two-package, inward-dependency budget slice.
The kernel declares no Cargo dependencies or features. At that stage, four workspace tests and all ten then-current Nix checks passed.
The primary repository retains `.pi/m1-runbook/`, including the unsuccessful metadata query and corrected invocations.

M1 remains active at 5/21 tasks. Boundary rejection controls, complete source accounting, native assurance, CI, and independent acceptance remain open.
No Noble refinement proof, spec sync, archive, commit, or push is claimed by these results.

## Published non-function body-owner repair

Octet published `e3e705a7c04a30b814000b251d245cf083894e4a` before this adoption.
Compiler-derived constant kinds replace the invalid function fallback. Expression owners avoid the unsupported named-item visibility query.
Real compiler controls retain constant callers, closures, memberships, and forbidden production effects, including the thread-local macro's inline constant.
The provider passed 879 scoped tests, strict Clippy, four UI harness tests, and thirty overlapping compiler-target tests.
Tested, final-built, and published collector bytes match. Provider evidence is retained in `.pi/m1-inline-owner/`.

Noble pinned that revision in Nix and pre-commit at the adoption checkpoint.
Reviewed source, file, and tool identities were updated explicitly then.
At that adoption, all eleven checks passed, including the unchanged required thread-local fixture.
Two boundary baselines and eleven negatives matched. Five workspace Rust tests and 100 Bun regressions passed.
No production source, architecture policy, target, fixture, or extraction input changed.
The [adoption evidence](../.cairn/archive/2026-09-15-m1-workspace/body-owner-adoption-evidence.md) records the exact commands and limits.
Task 2.2, randomness coverage, the remaining M1 matrix, and independent acceptance remain open.

## Randomness control and provider gap

The expanded [boundary controls](boundary-controls.md) add a pinned real `getrandom` dependency to fixtures only.
The dependency-only control compiles with clean lints, then fails the unchanged kernel dependency policy for the intended reason.
The random-call control reports `ambient_random` and retains the resolved `getrandom::fill` call.
The published collector lacks its required randomness effect. The comparator rejects the bundle despite valid full replay.
No call-only expectation, policy allowance, dependency removal, or fabricated witness replaces that requirement.
Provider repair remains separate from Noble adoption. Persistent observations are under `.pi/m1-boundary-random/`.

## Historical published randomness repair adoption

Octet published `235255bc4972ced9128fd5b4d1ec66ff7508ded4` after byte-verifying its provider build.
At the time of that adoption, Noble pinned that revision in Nix and pre-commit.
The then-reviewed source, file, and tool identities were regenerated explicitly.
At that historical checkpoint all eleven checks passed, including
`boundary-controls` against two baselines and thirteen negatives. The then-published
pre-commit deny-all and direct gate were clean with zero findings, and both retained
bundles replayed. Those results do not describe the current candidate.
The randomness fixture and comparator were unchanged; no production source, architecture policy, target, or extraction input changed.
Provider byte checks, retained bundles, and command records are under `.pi/m1-random-effects/`; the adoption record is under `.pi/m1-random-adoption/`.
Task 2.2, foreign ownership, relocated-body exceptions, complete inventories, native assurance, CI, and independent acceptance remain open.

## Current selected-source diagnostic (not the historical adoption receipt)

The earlier clean receipts above are bound to their earlier 32-unit workspace;
they do not transfer to the current source. On an unchanged `570ed67` source
snapshot with **scratch-only** widened collector limits, the selected strict
`octet-gate` exited 2 at the first kernel shard: 39 lint findings (37 errors,
2 warnings), with no full architecture verdict. Its deny-all flags were not
relaxed. A separate selected `octet` run without that deny-all hook exited 1
with 39/39 complete compiler units, 778 lint warnings and 714 architecture
findings: 712 required compiler unknowns plus two forbidden edges from
`noble-cli` to unclassified external `anyhow` and `wasmtime`. The original
`proofs/mc1/NobleContracts/Rules.lean:1:1` was reported twice as an
`excessive_file_length` warning. The acyclic 41-unit positive remains red
with 385 compiler unknowns; its reverse negative adds one forbidden role
edge. The existing source classifier rejects 1,437 new and 91 stale unique
production paths, and native assurance rejects two external dependencies
against the reviewed zero. No waiver, classification approval or full check
PASS follows from complete compiler collection.

The subsequently selected, content-hashed Octet diagnostic-path correction
was run against fresh COW `570ed67` mirrors with only the staged 41-unit
architecture policy substituted. The actual selected app again collected
39/39 normal and 41/41 acyclic-positive compiler units and shards without
collection issues. Both checks exited 1: the normal receipt remained blocked
on 712 required compiler unknowns and two forbidden external role edges; the
acyclic-positive receipt remained blocked on 385 required compiler unknowns.
The original `Rules.lean` excessive-length warning still appears twice on
the normal run. Both corrected 12-artifact bundles replayed valid without
diagnostics. This is diagnostic collection, not a clean deny-all, source
classification, native-disposition, or M1 acceptance result.

The corrected selected gate was also invoked directly, without Nix building,
on a fresh copy with the same 41-unit policy. Strict deny-all exited 2 on
the first kernel shard: 39 findings (37 errors, two warnings), before a full
architecture verdict. A separate fresh acyclic reverse-dependency fixture
under the corrected selected executable collected 41/41 units and shards
without issues, then exited 1 with the same 385 compiler unknowns plus
exactly one forbidden `noble-kernel` to `noble-cli` normal role edge; its
12-artifact bundle replayed valid. Direct pure comparison of the corrected
normal IR to the reviewed source and native policies rejected both: source
subjects, tests, macros and dependency closure differ, and the two external
production dependencies remain unreviewed. No whole-flake check or approval
is implied by these bounded direct controls.
