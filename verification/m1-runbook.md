# M1 implementation runbook

## Read the command state first

**This runbook records the command plan.** The commands in sections 1–4 exist; section 5 lists reserved interfaces for later implementation tasks.
A missing reserved command is unfinished work, not a passing rejection control.
The source inventory, native controls, CI, and acceptance results are recorded separately.

The [completed task record](../.cairn/archive/2026-09-15-m1-workspace/tasks.md) controls completion.
The [control matrix](m1-controls.md) maps each required positive observation and rejection to a command family.
The [selection policy](../policy/tool-selection.ncl) owns the current configuration.
This document does not create a second configuration authority.

| Lane | Exact current scope | Claim limit |
|---|---|---|
| Quality | `nightly-2026-03-21`, workspace, all targets, all features, compiler host target `x86_64-unknown-linux-gnu` | Lint, tests, and architecture only |
| Extraction | `nightly-2026-08-18`, kernel library, `dev`, all features, explicit `x86_64-unknown-linux-gnu` | One current function and enum, not project-wide accounting |
| Lean | 4.31.0, pinned backend and ten resolved Git dependencies | Generated-module compilation, not refinement or cache authenticity |
| Miri probe | Extraction toolchain, `tests/budget.rs`, explicit target, `-Zmiri-strict-provenance` | Two tests, not CLI or native-FFI assurance |

Both Rust lanes use 64-bit words, unwind, and explicit overflow checks. The kernel currently declares no features.
Miri retains the selected binary's other defaults. Task 3.3 must record any additional required borrow-model configuration explicitly in Nickel.
Other hosts, targets, feature combinations, and memory-model claims remain unsupported until their required matrix executes.

## 1. Prepare one persistent run

Run each procedure through Pueue with the dedicated worktree as its working directory.
The shell variables below belong to one run. Separate Pueue tasks must repeat the relevant assignments.

```sh
set -eu
ROOT=/home/brittonr/git/noble-m1-workspace-drain
cd "$ROOT"
EVIDENCE=$(mktemp -d /home/brittonr/git/noble/.pi/m1-run.XXXXXXXX)
mkdir -p "$EVIDENCE/tmp" "$EVIDENCE/llbc" "$EVIDENCE/lean" "$EVIDENCE/proof"
export TMPDIR="$EVIDENCE/tmp"
PROOF_DIR="$EVIDENCE/proof"
SELECTION="$ROOT/policy/tool-selection.json"
git status --short --branch > "$EVIDENCE/worktree-status.txt"
git diff --check HEAD
```

Stage reviewed new files before a Nix check. Nix snapshots do not include untracked files.
Do not reset the worktree or overwrite an earlier evidence directory.
For a staged run, retain `git write-tree`, its source archive, and the staged patch.
A source tree identifier is not a completion commit or an authenticated observation.

### Retain selected tool outputs

The following local options avoid the full `/tmp` filesystem on the current host.
They use the existing Nix-owned build directory. They do not change its security checks or global configuration.
A cold tool build can take longer than the probe limits. Keep its outcome separate from probe execution.

```sh
nix --option min-free 0 --option build-dir /nix/var/nix/builds --builders '' build \
  .#toolchain-check --out-link "$EVIDENCE/selection" -L
nix --option min-free 0 --option build-dir /nix/var/nix/builds --builders '' build \
  .#lean-dependencies-check --out-link "$EVIDENCE/backend-check" -L
nix --option min-free 0 --option build-dir /nix/var/nix/builds --builders '' build \
  .#aeneas --out-link "$EVIDENCE/aeneas" -L
nix --option min-free 0 --option build-dir /nix/var/nix/builds --builders '' build \
  .#extraction-rust --out-link "$EVIDENCE/rust" -L
nix --option min-free 0 --option build-dir /nix/var/nix/builds --builders '' build \
  .#lean --out-link "$EVIDENCE/lean-tool" -L
nix --option min-free 0 --option build-dir /nix/var/nix/builds --builders '' build \
  .#octet-gate --out-link "$EVIDENCE/octet-gate" -L
nix --option min-free 0 --option build-dir /nix/var/nix/builds --builders '' build \
  .#octet --out-link "$EVIDENCE/octet" -L
"$EVIDENCE/selection/bin/noble-toolchain-check" > "$EVIDENCE/tool-preflight.log" 2>&1
```

The preflight rejects named ambient overrides before tool execution.
Do not clear a requested override to hide a rejection. Do not use an Elan fallback or a sibling provider checkout.
A preflight result establishes tool availability only.

## 2. Restore a fresh Lean dependency tree

The proof directory contains copies of the selected configuration, not a detached Rust example.
The Rust subject remains the actual workspace library in section 3.
Lake reads resolved `rev` values from the manifest. Floating `inputRev` metadata does not replace those values.

```sh
cp "$ROOT/proofs/m1/lakefile.toml" "$ROOT/proofs/m1/lake-manifest.json" \
  "$ROOT/proofs/m1/lean-toolchain" "$PROOF_DIR/"
export PATH="$EVIDENCE/lean-tool/bin:$PATH"
export CI=1
export GIT_TERMINAL_PROMPT=0
(
  cd "$PROOF_DIR"
  timeout 600 "$EVIDENCE/lean-tool/bin/lake" --no-cache env true
) > "$EVIDENCE/backend-bootstrap.log" 2>&1
"$EVIDENCE/backend-check/bin/noble-lean-dependencies-check" "$PROOF_DIR" \
  > "$EVIDENCE/backend-sources.log" 2>&1
cmp "$PROOF_DIR/lake-manifest.json" "$ROOT/proofs/m1/lake-manifest.json"
cmp "$PROOF_DIR/lakefile.toml" "$ROOT/proofs/m1/lakefile.toml"
cmp "$PROOF_DIR/lean-toolchain" "$ROOT/proofs/m1/lean-toolchain"
```

`lake env true` loads package configuration and restores Git dependencies. It does not compile the Noble module.
It can compile upstream Lake configuration. `--no-cache` does not mean no execution or no network access.
The cold-start rehearsal checked all ten Git revisions and preserved all three input files.
No existing proof cache entered that rehearsal's new directory.

### Restore upstream compiled dependencies

```sh
(
  cd "$PROOF_DIR"
  timeout 600 "$EVIDENCE/lean-tool/bin/lake" exe cache get
) > "$EVIDENCE/backend-cache.log" 2>&1
"$EVIDENCE/backend-check/bin/noble-lean-dependencies-check" "$PROOF_DIR" \
  > "$EVIDENCE/backend-after-cache.log" 2>&1
cmp "$PROOF_DIR/lake-manifest.json" "$ROOT/proofs/m1/lake-manifest.json"
```

This step restores upstream compiled artifacts and can download missing files.
The rehearsal reused an existing upstream download cache after the fresh Git restoration. Git-source consistency does not authenticate compiled artifacts.
If restoration or compilation fails, retain the log and stop the affected lane.
Do not run `lake update`, use `--packages`, or replace required revisions to recover a passing result.

## 3. Extract the actual kernel and compile fresh output

```sh
export PATH="$EVIDENCE/rust/bin:$PATH"
export CARGO_HOME="$EVIDENCE/cargo"
export CARGO_TARGET_DIR="$EVIDENCE/extraction-target"
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
cp "$EVIDENCE/lean/NobleKernel.lean" "$PROOF_DIR/NobleKernel.lean"
export PATH="$EVIDENCE/lean-tool/bin:$PATH"
(
  cd "$PROOF_DIR"
  timeout 900 "$EVIDENCE/lean-tool/bin/lake" build NobleKernel
) > "$EVIDENCE/lean-build.log" 2>&1
(
  cd "$PROOF_DIR"
  timeout 900 "$EVIDENCE/lean-tool/bin/lake" env \
    "$EVIDENCE/lean-tool/bin/lean" -DmaxHeartbeats=1000000 -DmaxRecDepth=2048 \
    -o "$EVIDENCE/lean/NobleKernel.olean" NobleKernel.lean
) > "$EVIDENCE/lean-explicit.log" 2>&1
cmp "$EVIDENCE/lean/NobleKernel.lean" "$PROOF_DIR/NobleKernel.lean"
"$EVIDENCE/backend-check/bin/noble-lean-dependencies-check" "$PROOF_DIR" \
  > "$EVIDENCE/backend-after-lean.log" 2>&1
b3sum "$EVIDENCE/llbc/noble_kernel.llbc" "$EVIDENCE/lean/NobleKernel.lean" \
  "$EVIDENCE/lean/NobleKernel.olean" "$EVIDENCE/lean/translation.json" \
  > "$EVIDENCE/extraction-blake3.txt"
```

The Lake build prepares the backend.
When Lake reports an up-to-date target, the explicit Lean command still writes a fresh object.
Lean requires the source under its working root. The byte comparison connects that copy to the translator output.
Keep `noble_kernel.llbc` as the basename during regeneration comparisons.
The [short probe guide](../proofs/m1/README.md) also describes the existing-cache path and historical failures.

### Run the scoped Miri probe

```sh
export PATH="$EVIDENCE/rust/bin:$PATH"
export XDG_CACHE_HOME="$EVIDENCE/miri-cache"
export CARGO_TARGET_DIR="$EVIDENCE/miri-target"
export MIRI="$EVIDENCE/rust/bin/miri"
export MIRIFLAGS=-Zmiri-strict-provenance
timeout 300 "$EVIDENCE/rust/bin/cargo" miri setup > "$EVIDENCE/miri-setup.log" 2>&1
timeout 180 "$EVIDENCE/rust/bin/cargo" miri test \
  --manifest-path "$ROOT/crates/noble-kernel/Cargo.toml" --test budget --all-features \
  --target x86_64-unknown-linux-gnu --locked --offline > "$EVIDENCE/miri.log" 2>&1
```

The run supplies its own pinned driver and flags after the earlier override preflight.
Setup can download Rust sysroot dependencies. The subject run is locked and offline.
A skipped or unsupported required run is not success. The native control family in section 5 remains unimplemented.

## 4. Run the available quality and planning gates

Run this section in a fresh Pueue task with the same `ROOT` and `EVIDENCE` assignments.
The Miri probe's process-local driver settings are not inputs to the quality lane.

```sh
cd "$ROOT"
export TMPDIR="$EVIDENCE/tmp"
nix --option min-free 0 --option build-dir /nix/var/nix/builds --builders '' \
  flake check --keep-going -L > "$EVIDENCE/flake-check.log" 2>&1
nix --option min-free 0 develop -c pre-commit run octet-deny-all --all-files \
  > "$EVIDENCE/pre-commit.log" 2>&1
CARGO_HOME="$EVIDENCE/quality-cargo" CARGO_TARGET_DIR="$EVIDENCE/quality-target" \
  timeout 180 "$EVIDENCE/octet-gate/bin/noble-octet-gate" \
    --workspace --artifact-dir "$EVIDENCE/architecture" -- --all-targets --all-features \
    > "$EVIDENCE/architecture.log" 2>&1
"$EVIDENCE/octet/bin/cargo-octet" artifact verify \
  --artifact-dir "$EVIDENCE/architecture" --output-format json \
  > "$EVIDENCE/architecture-replay.json"
```

The earlier ten Nix checks are `format`, `rust-tests`, `clippy`, `documents`, `cairn`, `policy`, `octet`, `tool-selection`, `tool-versions`, and `tool-overrides`.
Body-owner adoption passed all eleven checks. The expanded `boundary-controls` check now rejects a missing compiler randomness effect.
See [the boundary controls](boundary-controls.md) for exact cases and retained failures. Task 2.2 remains open.
The Rust checks use `cargo fmt --all -- --check`, workspace/all-target/all-feature tests, and strict Clippy with `-D warnings`.
Both test and Clippy commands use `--locked --offline`.
The policy check compares the Nickel export and its generated freshness manifest.
Replay checks stored-artifact consistency. It does not authenticate compiler observations or establish the complete body inventory.

```sh
bun tools/cairn-specs.mjs --self-test
bun test tools/cairn-specs.test.mjs tools/check-specs.test.mjs
bun tools/check-specs.mjs --self-test
bun tools/cairn.mjs validate --root "$ROOT"
bun tools/cairn.mjs gate proposal m1-workspace --root "$ROOT"
bun tools/cairn.mjs gate design m1-workspace --root "$ROOT"
bun tools/cairn.mjs gate tasks m1-workspace --root "$ROOT"
```

These are document and planning checks. They do not execute the required source, native, or acceptance comparators.

### Available metadata is not the source inventory

This read-only command reports Cargo packages and dependency edges. It does not enumerate compiler bodies or establish extraction coverage.
The explicit empty wrapper values disable an inherited compiler wrapper for this query only.
They do not recover a rejected tool-selection request or change the product configuration.

```sh
nix --option min-free 0 develop -c env RUSTC_WRAPPER= RUSTC_WORKSPACE_WRAPPER= \
  CARGO_HOME="$EVIDENCE/metadata-cargo" CARGO_TARGET_DIR="$EVIDENCE/metadata-target" \
  cargo metadata --format-version 1 --all-features --locked --offline \
  > "$EVIDENCE/cargo-metadata.json"
```

An earlier direct Cargo call lacked the selected Rust directory on `PATH` and failed inside an inherited wrapper.
The retained retry supplied the selected path and explicit wrapper settings. It did not establish hermeticity.

## 5. Reserved interfaces — NOT IMPLEMENTED

The following names are exact implementation contracts for later tasks, not available commands or successful stubs.
Each future entry point must retain its own result and required configuration coverage.
Noble owns the deterministic comparison. Shells own compiler processes, file reads, and output publication.
Rust must own new runtime DTOs. Reviewed Nickel must own classification and acceptance policy, with generated contracts registered in the policy registry.

| Interface | Implementing tasks | Required output |
|---|---|---|
| `source-inventory collect` and `source-inventory check` | 2.3, 2.4 | Complete compiler-derived inventory and classification comparison |
| `extract-kernel` | 3.1, 3.2 | Bounded extraction with source-bound output and coverage |
| `native-assurance inventory` and `native-assurance check` | 3.3, 3.4 | Native/dependency records and required interpreted results |
| `m1-controls` | 2.2, 2.4, 3.2, 3.3, 3.4, 4.3, 5.1, 5.2 | Each positive baseline and each expected rejection |
| `m1-acceptance` | 5.1–5.3 | Independent comparison of every required phase |

### Inventory and extraction order

The future `EXPECTED` file comes from independent consumer review, not the producer bundle.
The future classification export lists exact bodies, configurations, ownership, models, exceptions, and open work.
Neither file exists yet. The filenames below reserve interfaces, not runtime schemas.

```sh
# RESERVED — NOT IMPLEMENTED
EXPECTED="$ROOT/policy/m1-acceptance.json"
CLASSIFICATION="$ROOT/policy/source-inventory.json"
nix run .#source-inventory -- collect --root "$ROOT" --selection "$SELECTION" \
  --artifact-dir "$EVIDENCE/inventory"
nix run .#extract-kernel -- --root "$ROOT" --selection "$SELECTION" \
  --inventory "$EVIDENCE/inventory/inventory.json" --artifact-dir "$EVIDENCE/extraction"
nix run .#source-inventory -- check --root "$ROOT" --selection "$SELECTION" \
  --inventory "$EVIDENCE/inventory/inventory.json" --policy "$CLASSIFICATION" \
  --extraction "$EVIDENCE/extraction/extraction.json" --expected "$EXPECTED" \
  --artifact-dir "$EVIDENCE/coverage"
```

Collection precedes extraction. Classification comparison follows extraction, so these dependencies do not form a cycle.
Collection must cover both required Rust lanes and every required compilation unit, including test configurations and generated bodies.
Octet architecture facts and Cargo metadata can supply inputs. Neither alone establishes an exhaustive function/body inventory.
Missing compiler capabilities remain blockers for task 2.3. Source-text search cannot replace compiler-derived coverage.

### Native scope, controls, and acceptance

```sh
# RESERVED — NOT IMPLEMENTED
nix run .#native-assurance -- inventory --root "$ROOT" --selection "$SELECTION" \
  --inventory "$EVIDENCE/inventory/inventory.json" --expected "$EXPECTED" \
  --artifact-dir "$EVIDENCE/native-inventory"
nix run .#native-assurance -- check --root "$ROOT" --selection "$SELECTION" \
  --inventory "$EVIDENCE/native-inventory/native.json" --expected "$EXPECTED" \
  --artifact-dir "$EVIDENCE/native-results"
for family in toolchain boundary source extraction proof octet native evidence; do
  nix run .#m1-controls -- --root "$ROOT" --selection "$SELECTION" --expected "$EXPECTED" \
    --family "$family" --all-required --fixture-dir "$EVIDENCE/fixtures/$family" \
    --artifact-dir "$EVIDENCE/controls/$family"
done
nix run .#m1-acceptance -- --root "$ROOT" --expected "$EXPECTED" \
  --phase-root "$EVIDENCE" --artifact-dir "$EVIDENCE/acceptance"
```

Mutation fixtures must not alter the implementation worktree, reviewed expected inputs, or retained earlier evidence.
The harness must reject unknown families, missing cases, and unsupported required configurations.
A harness exit of zero requires every positive baseline and named rejection to match, with no silent skip.
An arbitrary nonzero subprocess exit is not evidence of the expected domain rejection.
Each rejection must retain the intended diagnostic, status, and artifact outcome.

Task 4.4 must add `source-coverage`, `extraction`, `native-assurance`, `m1-controls`, and `m1-acceptance` Nix checks.
CI must run that complete `nix flake check` contract. Neither the earlier ten-check result nor the incomplete boundary controls satisfy this future set.

## 6. Retain results before cleanup

1. Save each command, working directory, exit status, source identity, tool identity, and complete output.
2. Save full Pueue logs with `pueue log --full --json TASK_IDS` before cleaning those task records.
3. Archive dereferenced Nix result bundles and generated outputs in the primary repository's persistent evidence directory.
4. Record BLAKE3 hashes and check the retained files against the ledger.
5. Preserve failed, unsupported, timed-out, and unexecuted phases without promoting their evidence roles.

A receipt hash is not authentication. An operator note is not a Rust-owned runtime acceptance receipt.
The [control matrix](m1-controls.md) describes which failures must block each claim.

Do not sync or archive Noble until all 21 implementation tasks and their required controls pass.
Before later worktree removal, preserve its ignored artifacts and run the drain-evidence guard with the exact worktree path.
The change was archived after implementation acceptance; this runbook remains the command plan.
