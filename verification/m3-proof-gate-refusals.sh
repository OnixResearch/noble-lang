#!/usr/bin/env bash
# Task-6.2 refusal matrix for the M3 proof gate.
#
# Runs the positive baseline (verification/m3-proof-gate.sh), then mutated
# scratch copies. Every mutation must be REFUSED: the gate exits non-zero
# at its named check. Proof-root mutations share the lake package cache
# through hard links (the mutable .lake/build and .lake/config directories
# are real copies); the two kernel-harness mutations run the kernel test
# suites on a hard-linked scratch copy of the crate. The repository tree is
# never touched. The M2 mutations (proof-hole/substituted-subject/
# unexplained-external against the m2 root, unexecuted-coverage) remain
# owned by verification/m2-proof-gate-refusals.sh, which keeps running
# green as the floor.
#
# Usage (from any directory; the pinned Lean must be first on PATH):
#
#   PATH=/nix/store/kwvw84h9r7w0q40d3cm5xgfz0rpv7yzc-noble-lean-release-4.35.0/bin:$PATH \
#     verification/m3-proof-gate-refusals.sh
#
#   RUST_BIN=/nix/store/1yvh3d6y3fj3xk2dgwczrp1dj5svd92c-rust-default-1.96.0-nightly-2026-03-21/bin
#   (the two kernel-harness cells also need a C linker, `cc`, on PATH, and a
#   CARGO_HOME whose offline registry resolves every workspace member: the
#   scratch crate resolves the whole workspace, so `noble-cli`'s rustix and
#   `noble-wasm`'s wat must be present. The user cargo home works; a partial
#   registry fails before compiling and reads as a missed refusal.)
#   CARGO_HOME="$HOME/.cargo"
#
# Mutations (each a separate cell, per the M1 control-matrix convention):
# The expected check is the strict tier's (the m3 root builds); the case list
# at the end of this file is authoritative.
#   proof-hole                a `sorry` probe theorem added to an m3     -> NO-PROOF-HOLE
#                             proof source
#   broken-build              a type error in an imported m3 root: the   -> BUILD
#                             root does not build, and the gate has no
#                             handoff tier that could pass it
#   admit-tactic              an `admit` probe theorem added to an m3     -> NO-PROOF-HOLE
#                             root (it compiles with a warning, so the
#                             construct scan, not the build, must refuse)
#   local-axiom               a local `axiom` declaration added to an m3 -> NO-PROOF-HOLE
#                             root (an axiom is not a build error)
#   missing-per-word-theorem  the per-word application refinement        -> REQUIRED-
#                             theorem (apply_refines) no longer          THEOREMS
#                             stated
#   substituted-subject       the entry point imports a hand-written     -> GENERATED-
#                             module that re-exports the generated       KERNEL
#                             functions instead of importing them
#                             directly
#   unexplained-external      an external-model implementation turned   -> EXTERNAL-
#                             into a template axiom                      MODELS
#   removed-eliminator-       the `case` arm deleted from the property  -> cargo test
#   oracle-arm                oracle's word_face (kernel/oracle          --test property
#                             disagreement)                              FAILED
#   schema-revision-          the supported candidate format reverted   -> cargo test
#   regression                to 0 (v0 no longer foreign; the executed   --test docexamples
#                             examples' revision controls break)         FAILED
set -uo pipefail

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
GATE="$REPO/verification/m3-proof-gate.sh"
PROOF_ROOT="${M3_PROOF_ROOT:-$REPO/proofs/m3}"
export PATH  # the pinned Lean must already be on PATH; the gate pins 4.35.0
RUST_BIN="${RUST_BIN:-/nix/store/1yvh3d6y3fj3xk2dgwczrp1dj5svd92c-rust-default-1.96.0-nightly-2026-03-21/bin}"
export CARGO_HOME="${CARGO_HOME:-/tmp/cargo-home}"

SCRATCH="$(mktemp -d "$REPO/.m3-gate-refusals-XXXXXX")"
trap 'rm -rf "$SCRATCH"' EXIT

echo "== positive baseline =="
if "$GATE" > "$SCRATCH/baseline.log" 2>&1; then
  grep '^\[m3-gate\] PASS\|^\[m2-gate\] PASS' "$SCRATCH/baseline.log" | tail -8
  echo "baseline: gate exit 0 (all checks pass)"
else
  echo "baseline: gate FAILED — refusal matrix not run"; exit 1
fi

# A fresh hard-linked scratch copy of the proof root with real (non-shared)
# build/config directories. NOTE: the copy's source files share inodes with
# the repository, so edits must go through write-to-temp-then-`mv` or
# `sed -i`/`rm` (rename/unlink); a plain `> file` redirect would truncate
# the shared inode and clobber the repository's own file.
fresh_copy() {
  local dst="$1"
  cp -al "$PROOF_ROOT" "$dst"
  rm -rf "$dst/.lake/build" "$dst/.lake/config"
  cp -a "$PROOF_ROOT/.lake/build" "$dst/.lake/build"
  if [ -d "$PROOF_ROOT/.lake/config" ]; then
    cp -a "$PROOF_ROOT/.lake/config" "$dst/.lake/config"
  fi
}

# A fresh hard-linked scratch copy of the workspace sources for the
# kernel-harness mutations (Cargo refuses to share a target lock, so the
# scratch builds into its own target directory; the kernel crate has no
# dependencies, so the build is local and fast).
fresh_crate_copy() {
  local dst="$1"
  # cargo finds rustc through PATH; the pinned Lean may be first, but the
  # selected Rust toolchain must be present for the harness refusals.
  export PATH="$RUST_BIN:$PATH"
  mkdir -p "$dst"
  cp -al "$REPO/Cargo.toml" "$REPO/Cargo.lock" "$dst/"
  cp -al "$REPO/crates" "$dst/crates"
  # The docexamples harness scans the fragment/example documents relative
  # to the crate; the scratch copy needs them to refuse for the right
  # reason.
  cp -al "$REPO/verification" "$dst/verification"
  find "$dst" -name Cargo.toml -path '*target*' -delete 2>/dev/null || true
  CARGO_TARGET_DIR="$dst/target" "$RUST_BIN/cargo" build --offline -p noble-kernel --tests \
    > "$dst/build.log" 2>&1
}

run_case() {
  local label="$1" expect="$2"
  local copy="$SCRATCH/$label"
  echo
  echo "== mutation: $label (expected refusal: $expect) =="

  case "$label" in
  proof-hole)
    fresh_copy "$copy"
    # A probe theorem proved by `sorry` added to an m3 proof source (the
    # file is replaced through mv: the scratch copy's sources share
    # inodes with the repository's).
    cat > "$SCRATCH/proof-hole.lean" <<'HOLE'
/-- Refusal-mutation probe: a proof hole. Written into a scratch copy by
    verification/m3-proof-gate-refusals.sh; not part of the repository. -/
theorem mutation_probe : False := by sorry
HOLE
    cat "$PROOF_ROOT/NobleM3/CheckV1.lean" >> "$SCRATCH/proof-hole.lean"
    grep -qw sorry "$SCRATCH/proof-hole.lean" || { echo "APPLY FAILED"; return 1; }
    mv "$SCRATCH/proof-hole.lean" "$copy/NobleM3/CheckV1.lean"
    out="$("$GATE" "$copy" 2>&1)"; rc=$?
    ;;

  missing-per-word-theorem)
    fresh_copy "$copy"
    # The per-word application refinement theorem is no longer stated
    # under its required name (renamed; the tree keeps its other content).
    sed -i 's/^theorem apply_refines /theorem apply_refines_renamed /' \
      "$copy/NobleM3/Refine.lean"
    grep -q 'theorem apply_refines_renamed' "$copy/NobleM3/Refine.lean" \
      && ! grep -q '^theorem apply_refines ' "$copy/NobleM3/Refine.lean" \
      || { echo "APPLY FAILED"; return 1; }
    out="$("$GATE" "$copy" 2>&1)"; rc=$?
    ;;

  substituted-subject)
    fresh_copy "$copy"
    # The entry point routed through a hand-written module that re-exports
    # the generated functions, so everything still builds but the library no
    # longer imports the checked generated functions directly. Since M4 the
    # gate does not treat a banner comment as generator provenance (the
    # actual-source extraction gates bind the generated bytes), so a
    # banner-less copy of the real entry point is no longer a substitution.
    cat > "$SCRATCH/stub-substitute.lean" <<'STUB'
/- M3 gate refusal mutation (task 6.2): a hand-written module standing
   between the library entry point and the generated functions. Written into
   a scratch copy by verification/m3-proof-gate-refusals.sh; not part of the
   repository. -/
import NobleKernel.Funs
STUB
    mv "$SCRATCH/stub-substitute.lean" "$copy/NobleKernel/Substitute.lean"
    printf '%s\n' 'import NobleKernel.Substitute' > "$SCRATCH/stub-entry.lean"
    mv "$SCRATCH/stub-entry.lean" "$copy/NobleKernel.lean"
    grep -qx 'import NobleKernel.Substitute' "$copy/NobleKernel.lean" \
      || { echo "APPLY FAILED"; return 1; }
    out="$("$GATE" "$copy" 2>&1)"; rc=$?
    ;;

  unexplained-external)
    fresh_copy "$copy"
    # One external model left as a template axiom: appending a bare `axiom`
    # to the external-model file keeps the generated kernel compiling (so
    # the strict tier still engages) while violating the axiom policy.
    { cat "$PROOF_ROOT/NobleKernel/FunsExternal.lean"
      printf '\n/-- Refusal-mutation probe: an unexplained external model. -/\naxiom mutation_probe_external : Bool\n'
    } > "$SCRATCH/funs-external.lean"
    grep -q '^axiom mutation_probe_external' "$SCRATCH/funs-external.lean" \
      || { echo "APPLY FAILED"; return 1; }
    mv "$SCRATCH/funs-external.lean" "$copy/NobleKernel/FunsExternal.lean"
    out="$("$GATE" "$copy" 2>&1)"; rc=$?
    ;;

  broken-build)
    fresh_copy "$copy"
    # A type error in an imported root: the build step must refuse. Appending
    # to the NobleM3 root re-elaborates only the root, so the refusal is quick.
    { cat "$PROOF_ROOT/NobleM3.lean"
      printf '\n/-- Refusal-mutation probe: a type error. -/\ntheorem mutation_broken_build : (1 : Nat) = (2 : Nat) := rfl\n'
    } > "$SCRATCH/broken-build.lean"
    grep -q 'mutation_broken_build' "$SCRATCH/broken-build.lean" || { echo "APPLY FAILED"; return 1; }
    mv "$SCRATCH/broken-build.lean" "$copy/NobleM3.lean"
    out="$("$GATE" "$copy" 2>&1)"; rc=$?
    ;;

  admit-tactic)
    fresh_copy "$copy"
    # `admit` is not a build error (it compiles with a "declaration uses
    # sorry" warning), so only the construct scan can refuse it.
    { cat "$PROOF_ROOT/NobleM3.lean"
      printf '\n/-- Refusal-mutation probe: admit. -/\ntheorem mutation_admit_probe : False := by admit\n'
    } > "$SCRATCH/admit-tactic.lean"
    grep -qw admit "$SCRATCH/admit-tactic.lean" || { echo "APPLY FAILED"; return 1; }
    mv "$SCRATCH/admit-tactic.lean" "$copy/NobleM3.lean"
    out="$("$GATE" "$copy" 2>&1)"; rc=$?
    ;;

  local-axiom)
    fresh_copy "$copy"
    # A local axiom is legal Lean; the construct scan must refuse it.
    { cat "$PROOF_ROOT/NobleM3.lean"
      printf '\n/-- Refusal-mutation probe: a local axiom. -/\naxiom mutation_local_axiom : False\n'
    } > "$SCRATCH/local-axiom.lean"
    grep -q '^axiom mutation_local_axiom' "$SCRATCH/local-axiom.lean" || { echo "APPLY FAILED"; return 1; }
    mv "$SCRATCH/local-axiom.lean" "$copy/NobleM3.lean"
    out="$("$GATE" "$copy" 2>&1)"; rc=$?
    ;;

  removed-eliminator-oracle-arm)
    fresh_crate_copy "$copy"
    # The `case` arm deleted from the oracle's contract table: the oracle
    # then rejects every `case` candidate the kernel accepts, and the
    # agreement lane must fail.
    sed -E -i 's/^([[:space:]]*)17[[:space:]]*=>/\1999 =>/' "$copy/crates/noble-kernel/tests/property/table.rs"
    grep -q '^[[:space:]]*999[[:space:]]*=>' "$copy/crates/noble-kernel/tests/property/table.rs" \
      || { echo "APPLY FAILED (the case arm pattern moved — update the mutation)"; return 1; }
    rc=0
    (cd "$copy" && CARGO_TARGET_DIR="$copy/target" "$RUST_BIN/cargo" test \
      --offline -p noble-kernel --test property > "$copy/test.log" 2>&1) || rc=1
    out="kernel-harness: cargo test --test property exit $rc"
    ;;

  schema-revision-regression)
    fresh_crate_copy "$copy"
    # The supported candidate format reverted to 0: `noble-candidate/v0`
    # and every other string then decode to the supported revision, so
    # the executed examples' revision controls must fail.
    sed -i 's/^pub const CANDIDATE_FORMAT: u32 = 1;/pub const CANDIDATE_FORMAT: u32 = 0;/' \
      "$copy/crates/noble-kernel/src/untrusted.rs"
    grep -q 'pub const CANDIDATE_FORMAT: u32 = 0;' "$copy/crates/noble-kernel/src/untrusted.rs" \
      || { echo "APPLY FAILED (the CANDIDATE_FORMAT pattern moved — update the mutation)"; return 1; }
    rc=0
    (cd "$copy" && CARGO_TARGET_DIR="$copy/target" "$RUST_BIN/cargo" test \
      --offline -p noble-kernel --test docexamples > "$copy/test.log" 2>&1) || rc=1
    out="kernel-harness: cargo test --test docexamples exit $rc"
    ;;

  *) echo "unknown case $label"; return 1 ;;
  esac

  if [ "$label" = removed-eliminator-oracle-arm ] || [ "$label" = schema-revision-regression ]; then
    # Kernel-harness refusal: the suite itself is the refusing check.
    printf '%s\n' "$out"
    local failing
    if [ "$label" = removed-eliminator-oracle-arm ]; then
      failing='oracle_table_covers_every_pool_word'
    else
      failing='documented_noble_check_examples_run_through_the_actual_checker'
    fi
    grep -m1 "test $failing ... FAILED" "$copy/test.log" \
      || { echo "REFUSAL FAILED: the harness did not fail at $failing"; return 1; }
    grep -m1 'test result: FAILED' "$copy/test.log"
    if [ "$rc" -eq 0 ]; then
      echo "REFUSAL FAILED: the harness accepted the mutated crate (exit 0)"
      return 1
    fi
    echo "refused: harness exit $rc"
    return 0
  fi

  printf '%s\n' "$out" | grep '^\[m3-gate\]'
  if [ "$rc" -eq 0 ]; then
    echo "REFUSAL FAILED: gate accepted the mutated tree (exit 0)"
    return 1
  fi
  if ! printf '%s\n' "$out" | grep -q "^\[m3-gate\] FAIL $expect"; then
    echo "REFUSAL at wrong check (expected $expect)"
    printf '%s\n' "$out" | grep '^\[m3-gate\] FAIL' | head -3
    return 1
  fi
  echo "refused: gate exit $rc at $expect"
  return 0
}

rc_all=0
for case_label in \
  'proof-hole NO-PROOF-HOLE' \
  'broken-build BUILD' \
  'admit-tactic NO-PROOF-HOLE' \
  'local-axiom NO-PROOF-HOLE' \
  'missing-per-word-theorem REQUIRED-THEOREMS' \
  'substituted-subject GENERATED-KERNEL' \
  'unexplained-external EXTERNAL-MODELS' \
  'removed-eliminator-oracle-arm HARNESS' \
  'schema-revision-regression HARNESS'
do
  # shellcheck disable=SC2086
  run_case $case_label || rc_all=1
done

echo
if [ "$rc_all" -eq 0 ]; then
  echo "== all refusals demonstrated =="
else
  echo "== refusal matrix FAILED =="
fi
exit "$rc_all"
