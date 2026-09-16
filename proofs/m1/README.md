# M1 extraction probe

This probe extracts `noble_kernel::consume_budget` from the actual workspace.
It compiles the generated `NobleKernel.lean` module without manual edits.
It does not prove refinement, execute Noble programs, or complete M1.
The CLI exposes only an internal budget smoke route.

## Scope and inputs

| Input | Probe configuration |
|---|---|
| Host | `x86_64-linux` |
| Rust target | `x86_64-unknown-linux-gnu`, 64-bit words |
| Subject | `crates/noble-kernel/src/lib.rs`, complete production library |
| Features | All features, currently none |
| Profile | `dev`, overflow checks enabled, panic unwind |
| Test profile | Cargo test harness, panic unwind |
| Aeneas and Lean backend | `505b6ca35217e7be5c96c3e2f8045edfbdf47291` |
| Required Charon | `b104e24fea7d721b71e6c39fd70f26ff20bc0980` |
| Extraction Rust and Miri | `nightly-2026-08-18`, supplied by that Charon input |
| Lean | `v4.31.0`, commit `68218e876d2a38b1985b8590fff244a83c321783` |
| Mathlib | `fabf563a7c95a166b8d7b6efca11c8b4dc9d911f` |
| Extraction limits | 120 seconds per translator |
| Lean limits | 900 seconds, 1,000,000 heartbeats, recursion depth 2048 |

`lake-manifest.json` contains the resolved backend dependency revisions.
The library has one production function and one enum.
The CLI and integration tests are outside this extraction scope.
This count is not the required project-wide compiler inventory.

[Tool selection](../../verification/tool-selection.md) describes the checked Nickel policy and its rejection boundary.
The production quality lane remains `nightly-2026-03-21`.
Extraction-lane tests do not replace the pinned Octet deny-all gate.

## Repeat the probe

Run the following blocks in one shell command through Pueue from the dedicated worktree.
Use a fresh evidence directory outside the disposable worktree.
The commands require Nix, Git, a native linker, and installed backend dependencies.
For a fresh dependency tree, use the [cold-start runbook](../../verification/m1-runbook.md).
This shorter procedure checks existing Git sources before and after Lean execution.
Compiled upstream caches remain a separate trust assumption.

1. Create a fresh evidence directory and retain the selected Nix tools.

```sh
set -eu
ROOT=$(git rev-parse --show-toplevel)
EVIDENCE=$(mktemp -d /home/brittonr/git/noble/.pi/m1-extraction.XXXXXXXX)
mkdir -p "$EVIDENCE/tmp" "$EVIDENCE/llbc" "$EVIDENCE/lean"
export TMPDIR="$EVIDENCE/tmp"
nix build .#toolchain-check --out-link "$EVIDENCE/selection" -L
nix build .#lean-dependencies-check --out-link "$EVIDENCE/backend-check" -L
nix build .#aeneas --out-link "$EVIDENCE/aeneas" -L
nix build .#extraction-rust --out-link "$EVIDENCE/rust" -L
nix build .#lean --out-link "$EVIDENCE/lean-tool" -L
"$EVIDENCE/selection/bin/noble-toolchain-check" > "$EVIDENCE/preflight.log"
"$EVIDENCE/backend-check/bin/noble-lean-dependencies-check" "$ROOT/proofs/m1" \
  > "$EVIDENCE/backend-before.log"
export PATH="$EVIDENCE/rust/bin:$PATH"
export CARGO_HOME="$EVIDENCE/cargo"
export CARGO_TARGET_DIR="$EVIDENCE/target"
```

The preflight rejects named tool overrides. Do not clear a requested override to hide its rejection.
When `/tmp` lacks space, use a supported Nix-owned build directory on a filesystem with enough space.
The local run used `--option build-dir /nix/var/nix/builds --builders ''`.
It did not change home permissions or Nix's directory security checks.

2. Run both translators with the actual workspace library.

```sh
cd "$ROOT"
timeout 120 "$EVIDENCE/aeneas/bin/charon" cargo \
  --preset=aeneas --error-on-warnings \
  --dest-file "$EVIDENCE/llbc/noble_kernel.llbc" -- \
  --manifest-path crates/noble-kernel/Cargo.toml --lib --all-features \
  --target x86_64-unknown-linux-gnu --locked --offline \
  > "$EVIDENCE/charon.log" 2>&1
timeout 120 "$EVIDENCE/aeneas/bin/aeneas" -backend lean \
  -abort-on-error -warnings-as-errors -no-progress-bar -emit-json \
  -dest "$EVIDENCE/lean" "$EVIDENCE/llbc/noble_kernel.llbc" \
  > "$EVIDENCE/aeneas.log" 2>&1
cp "$EVIDENCE/lean/NobleKernel.lean" "$ROOT/proofs/m1/NobleKernel.lean"
```

3. Compile the generated module with the selected Lean executable.

```sh
export PATH="$EVIDENCE/lean-tool/bin:$PATH"
export CI=1
(
  cd "$ROOT/proofs/m1"
  timeout 900 "$EVIDENCE/lean-tool/bin/lake" env \
    "$EVIDENCE/lean-tool/bin/lean" -DmaxHeartbeats=1000000 -DmaxRecDepth=2048 \
    -o "$EVIDENCE/lean/NobleKernel.olean" NobleKernel.lean \
    > "$EVIDENCE/lean.log" 2>&1
)
cmp "$EVIDENCE/lean/NobleKernel.lean" "$ROOT/proofs/m1/NobleKernel.lean"
"$EVIDENCE/backend-check/bin/noble-lean-dependencies-check" "$ROOT/proofs/m1" \
  > "$EVIDENCE/backend-after.log"
b3sum "$EVIDENCE/llbc/noble_kernel.llbc" "$EVIDENCE/lean/NobleKernel.lean" \
  "$EVIDENCE/lean/NobleKernel.olean" > "$EVIDENCE/artifact-blake3.txt"
```

The explicit `lean` command writes a new object. A Lake up-to-date result cannot replace this compilation.
Lean requires the input under its working root. Compile the byte-checked workspace copy, not the external evidence copy.
Lake supplies the selected backend import paths. This does not authenticate compiled dependency caches.

## Scoped Miri probe

This command interprets only the two kernel budget integration tests.
It does not cover the CLI, native FFI, all Rust configurations, or the later M1 native controls.
Prepare a fresh Miri sysroot with the selected driver before the offline test run.
The setup can download dependencies fixed by the Rust source's lock file.

```sh
export PATH="$EVIDENCE/rust/bin:$PATH"
export XDG_CACHE_HOME="$EVIDENCE/miri-cache"
export CARGO_TARGET_DIR="$EVIDENCE/miri-target"
export MIRI="$EVIDENCE/rust/bin/miri"
export MIRIFLAGS=-Zmiri-strict-provenance
timeout 300 cargo miri setup > "$EVIDENCE/miri-setup.log" 2>&1
timeout 180 cargo miri test --manifest-path "$ROOT/crates/noble-kernel/Cargo.toml" \
  --test budget --all-features --target x86_64-unknown-linux-gnu --locked --offline \
  > "$EVIDENCE/miri.log" 2>&1
```

The earlier preflight must reject ambient `MIRI`, `MIRI_SYSROOT`, and `MIRIFLAGS` values.
The probe then supplies its own selected driver and flags explicitly.

## Historical failures and recovery

The first Lake setup failed because `/tmp` was full and `leantar` was absent from the selected sysroot.
A private temporary directory and the pinned Lean directory on `PATH` resolved those failures.
The historical Elan provider was Nixpkgs `b3d51a0365f6695e7dd5cdf3e180604530ed33b4`.
The current tool selection instead uses the hash-pinned Lean 4.31.0 release package.

An unrooted Nix tool later disappeared. Persistent output links now retain tool closures.
An unavailable tool or failed cache phase is not a passing result.

An earlier regeneration used `regenerated.llbc` and therefore produced `Regenerated.lean`.
Keep `noble_kernel.llbc` in each fresh directory when comparing generated `NobleKernel.lean` bytes.
The historical missing-entry control rejected `crate::missing_budget` with an item-resolution error.

The local runs used a native linker, ancestor Cargo configuration, and upstream compiled caches.
They do not establish hermetic builds or independently authenticated observations.
No unrelated temporary files were deleted.

## Remaining acceptance work

The active implementation checklist remains the authority for completion.
Compiler-derived project coverage, source exceptions, native controls, CI, and independent acceptance remain open.
The [control matrix](../../verification/m1-controls.md) names required positive observations, rejected mutations, and unimplemented command families.
Its reserved inventory, native, and acceptance interfaces remain implementation work.
Refinement, spec sync, and archive remain open.

The published-provider quality gate now passes; its earlier receipt, ownership, and no-std failures are historical.
See the [component review](../../verification/component-review.md) for the gate's scope and retained evidence.

The original abort-mode probe remains in `.pi/m1-extraction-probe/result.json` in the primary repository.
The earlier checked-subtraction and unwind artifacts remain in `.pi/m1-quality/`.
The current selection controls and compatibility probe remain in `.pi/m1-tool-selection/`.
These records describe local observations, not final M1 acceptance.
