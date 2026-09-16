# Kernel and shell boundary controls

**Task 2.2 boundary controls pass.** The expanded `boundary-controls` check matches two positive baselines and sixteen rejected fixtures with published Octet `235255bc4972ced9128fd5b4d1ec66ff7508ded4`.
The earlier missing-effect rejection under `e3e705a7c04a30b814000b251d245cf083894e4a` is retained as historical evidence, not rewritten.

## Run the check

From the dedicated worktree, run:

```sh
nix --option min-free 0 --option build-dir /nix/var/nix/builds --builders '' \
  build .#checks.x86_64-linux.boundary-controls --keep-going --keep-failed -L \
  --out-link "$EVIDENCE/boundary-controls"
```

Use the private temporary directory and retention procedure in the [runbook](m1-runbook.md).
The check compiles fixtures. It does not execute their filesystem, network, process, or state operations.

## Ownership and mechanisms

[The Nickel policy](../policy/boundary-controls.ncl) owns the expected cases and diagnostics.
Its JSON export is checked, not refreshed, during the build.
Pure Nix separates source mutation, observed-result comparison, and file loading.
A thin build helper runs the selected tools and retains their outputs.
Octet owns the compiler DTOs, architecture policy evaluation, and artifact replay.

Every baseline requires the published full deny-all gate.
Every negative requires an intended error from that same gate.
An unrelated compiler error, timeout, or missing artifact does not pass.

Deny-level lints can prevent compiler collection. Most negatives therefore have a separate default-lint observation run.
This auxiliary run is not deny-all acceptance. It must compile, retain the intended compiler facts, and pass full artifact replay.
Both kernel production units must observe the relevant facts. Integration tests cannot supply these witnesses.

The comparator requires the exact unit roster, source roots, roles, target, features, toolchain, gate mode, and operation-specific evidence.
The production architecture policy stays unchanged. No target or production obligation is removed.
Seventy synthetic harness self-tests cover the comparator and the source-mutation layer. They are not compiler evidence.

## Rejection mechanisms

Deny-level lints stop the compiler before architecture collection. Unsafe fixtures therefore reject at the lint phase and retain no compiler IR.
Their kernel ownership is established by the unchanged source tree and by the twelve artifact-verified cases, not by an architecture receipt for the unsafe fixture itself.
That is an explicit limit of this harness, not a hidden exemption.

The `no-std-test-witness` case declares `no_std` only outside the `test` configuration. The auxiliary run then reports `no-std-obligation` for the `noble-kernel` scope: a test-configured witness cannot discharge the production obligation.
The case requires the finding to be open, to name the declared scope, and to carry the exact compiler-confirmation message.

The `test-source-import` case appends a `#[path]` import to the kernel root that resolves through `..` into a test-labelled directory.
The collector rejects the escaping diagnostic path before collection, so no status, receipt, or artifact bundle exists.
The case requires exit 2, the exact diagnostic substrings, and the absence of a published bundle.

## Fixture matrix

Each row is one case on `x86_64-unknown-linux-gnu`, with the selected quality Rust toolchain and all targets and features.

| Case | Required observation | Current result |
|---|---|---|
| `workspace` | Unmodified workspace, clean deny-all and replay | Matched |
| `randomness-dependency` | Compile success, clean lints, exact forbidden kernel dependency on `getrandom`, valid replay | Matched |
| `acyclic-shell` | Independent shell baseline, clean deny-all and replay | Matched |
| `filesystem` | `impure_call_in_core`, compiler filesystem effect in both kernel units | Matched |
| `network` | `impure_call_in_core`, compiler network effect in both kernel units | Matched |
| `process` | `impure_call_in_core`, compiler process effect in both kernel units | Matched |
| `environment` | `ambient_env`, compiler environment effect in both kernel units | Matched |
| `clock` | `ambient_clock`, compiler time effect in both kernel units | Matched |
| `randomness` | `ambient_random`, compiler randomness effect in both kernel units | Missing provider effect |
| `global-state` | `global_state_in_core`, resolved atomic update in both kernel units | Matched |
| `interior-state` | `interior_mutability_in_core`, resolved thread-local cell update in both kernel units | Matched |
| `unsafe-block` | Compiler `unsafe_code` error at the kernel source | Matched |
| `unsafe-function` | Compiler `unsafe_code` error at the kernel source | Matched |
| `unsafe-foreign-declaration` | Compiler `unsafe_code` error for a public foreign `extern` declaration at the kernel source | Matched |
| `host-return` | `resource_return_from_core`, resolved `std::fs::File` type in both kernel units | Matched |
| `test-source-import` | Rejection before collection: a production root importing test-labelled source by a parent-relative path | Matched |
| `no-std-test-witness` | `no-std-obligation`, the kernel scope lacks compiler-confirmed `crate:no_std` when the attribute holds only outside the test configuration | Matched |
| `reverse-dependency` | `forbidden-role-edge`, successful compilation, blocked architecture gate and valid replay | Matched |

The reverse-edge case starts from the acyclic shell baseline, not a Cargo dependency cycle.
That baseline removes the shell-to-kernel edge and adds an empty shell library. All existing targets remain present.
Only its negative adds the kernel-to-shell edge. The workspace baseline still checks the actual product without these changes.

An earlier interior-state fixture only constructed a local cell. It did not demonstrate hidden state and is historical harness evidence only.
The current fixture mutates thread-local state through an inline-constant initializer. It was preserved unchanged through the provider repair.
The random-call observation and the rest of the [M1 matrix](m1-controls.md) remain open.
Test-configured no-std evidence and production imports of test source are rejected by their own cases, so no test witness is promoted to production.

## Registry randomness controls

The fixture-only dependency is `getrandom =0.4.3`. The production Cargo manifests remain unchanged.
Nickel owns the selected version. Cargo generated `verification/fixtures/randomness.Cargo.lock`.
Nix reuses its checksum-bound vendor adapter. Fixture compilation runs offline and rejects lock drift.
The fixture lock also binds `cfg-if 1.0.4`, `libc 0.2.189`, and target-conditional `r-efi 6.0.0`.
These are fixture dependencies, not a production native inventory or a dependency-safety claim.

Adding the dependency without a call already violates the kernel's dependency policy.
The first positive-baseline hypothesis therefore failed. The unchanged policy requires this mutation to remain a separate negative control.
The random-call fixture must additionally produce its intended deny-all diagnostic and both production randomness effects.
The current bundle contains `getrandom::fill`, but the published collector emits no randomness effect for that call.
Auxiliary compilation and full replay pass. Neither result substitutes for the missing effect.
Provider repair and publication must precede adoption. A local same-named stub cannot satisfy this control.

## Shell tests

The CLI tests cover five successful budget inputs, ten invalid argument cases, and a Unix non-UTF-8 argument.
They require exact output and exit status. Five workspace Rust tests pass, including the two kernel tests.
The CLI remains an internal smoke shell, not a language evaluator.

## Historical provider blocker and repair

The fixture passed plain Rust checking with the same pinned compiler and all targets and features.
The earlier Octet run reported `unexpected sort of node in fn_sig(): ConstBlock`.
The backtrace reaches `architecture_collector::signature_shape_identity`, `collect_item`, and `collect_call`.
The collector's fallback in `caller_item_id` requests a function signature for an unseen non-closure body owner.
An inline constant is not a function.

The earlier JSON run also rejected an internal-error severity and lacked a replayable artifact bundle.
The check rejected this incomplete evidence; that failure never counted as boundary enforcement.

The published repair preserves compiler-derived constant kinds and avoids invalid signature and named-item visibility queries.
It retains callers, production memberships, closures, and effects. Compiler regressions include the thread-local macro's separate inline constant.
Noble adopted the published revision in both Nix and pre-commit, with reviewed selection identities updated explicitly.
The required fixture now produces intended deny-all errors, complete auxiliary observations, and valid replay.
Generic ICE reporting remains outside this repair. No policy exemption was added.

## Evidence and limits

Persistent operator evidence is under `.pi/m1-boundary/` in the primary repository.
It includes the initial comparator failure, the corrected historical fixtures, the stronger failed fixture, and plain-Rust and backtrace logs.
The first comparator used the wrong call field and unresolved API aliases. Current comparisons use exact compiler `resolved_definition` values.

The earlier expanded run passed ten checks and failed the eleventh. Body-owner adoption then passed all eleven checks.
Those results remain under `.pi/m1-inline-adoption/` and `.pi/m1-inline-owner/`.
The new randomness check fails. Its retained bundles and failures are under `.pi/m1-boundary-random/`.
Artifact replay establishes stored-input consistency, not independently authenticated compiler execution.
These controls do not establish complete source coverage, native assurance, extraction coverage, refinement, or M1 acceptance.
