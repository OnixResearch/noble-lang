#!/usr/bin/env bash
# Task-4.5 refusal matrix for the M2 proof gate.
#
# Runs the positive baseline (verification/m2-proof-gate.sh on the repository
# proof root), then four mutated scratch copies of proofs/m2. Every mutation
# must be REFUSED: the gate exits non-zero at its named check. The scratch
# copies share the lake package cache through hard links (the mutable
# .lake/build and .lake/config directories are real copies), so each run is
# isolated and the repository tree is never touched.
#
# Usage (from any directory; the pinned Lean must be first on PATH):
#
#   PATH=/nix/store/hbxwgcbpkyqllqzq7cidhnvhgb60jb9q-noble-lean-release-4.31.0/bin:$PATH \
#     verification/m2-proof-gate-refusals.sh
#
# Mutations (each a separate cell, per the M1 control-matrix convention):
#   proof-hole     a `sorry` replacing the check_soundness proof   -> NO-SORRY
#   substituted-   refinement restated about a hand-written        -> REFINEMENT-
#   subject        substitute module with same-named defs             SUBJECT
#   unexplained-   an external-model implementation deleted from    -> BUILD
#   external       FunsExternal.lean (generated module breaks)
#   unexecuted-    the coverage_positive theorem file deleted with -> REQUIRED-
#   coverage       the tree kept building                              THEOREMS
set -uo pipefail

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
GATE="$REPO/verification/m2-proof-gate.sh"
PROOF_ROOT="${M2_PROOF_ROOT:-$REPO/proofs/m2}"
export PATH  # the pinned Lean must already be on PATH; the gate pins 4.31.0

SCRATCH="$(mktemp -d "$REPO/.m2-gate-refusals-XXXXXX")"
trap 'rm -rf "$SCRATCH"' EXIT

echo "== positive baseline =="
if "$GATE" "$PROOF_ROOT" > "$SCRATCH/baseline.log" 2>&1; then
  grep '^\[m2-gate\] PASS' "$SCRATCH/baseline.log" | head -8
  echo "baseline: gate exit 0 (all checks pass)"
else
  echo "baseline: gate FAILED — refusal matrix not run"; exit 1
fi

# A fresh hard-linked scratch copy of the proof root with real (non-shared)
# build/config directories. NOTE: the copy's source files share inodes with
# the repository, so edits must go through write-to-temp-then-`mv` or
# `sed -i`/`rm` (rename/unlink); a plain `> file` redirect would truncate the
# shared inode and clobber the repository's own file.
fresh_copy() {
  local dst="$1"
  cp -al "$PROOF_ROOT" "$dst"
  rm -rf "$dst/.lake/build" "$dst/.lake/config"
  cp -a "$PROOF_ROOT/.lake/build" "$dst/.lake/build"
  if [ -d "$PROOF_ROOT/.lake/config" ]; then
    cp -a "$PROOF_ROOT/.lake/config" "$dst/.lake/config"
  fi
}

run_case() {
  local label="$1" expect="$2"
  local copy="$SCRATCH/$label"
  echo
  echo "== mutation: $label (expected refusal: $expect) =="
  fresh_copy "$copy"

  case "$label" in
  proof-hole)
    # Replace the check_soundness proof body with `sorry` (the theorem header
    # is kept; the proof block up to the regression section is dropped).
    # The result is moved into the copy: `mv` replaces the dirent and leaves
    # the hard-linked repository file untouched.
    awk '
      /^theorem check_soundness/ { header = 1; print; next }
      header && /:= by$/         { print; print "  sorry"; header = 0; skip = 1; next }
      skip && /^\/-! ## Regression/ { skip = 0; print; next }
      skip                       { next }
      { print }
    ' "$PROOF_ROOT/NobleM2/CheckSoundness.lean" > "$SCRATCH/proof-hole.lean"
    grep -qw sorry "$SCRATCH/proof-hole.lean" || { echo "APPLY FAILED"; return 1; }
    mv "$SCRATCH/proof-hole.lean" "$copy/NobleM2/CheckSoundness.lean"
    ;;
  substituted-subject)
    # A hand-written substitute module with same-named definitions
    # (acceptance.check), and the refinement restated about it.
    cat > "$copy/NobleM2/NobleKernelStub.lean" <<'STUB'
/- M2 gate refusal mutation (task 4.5): a hand-written substitute module with
   same-named definitions. Any refinement stated about this constant is a
   theorem about a rewritten Lean copy, not the extracted checker. Written
   into a scratch copy by verification/m2-proof-gate-refusals.sh; not part of
   the repository. -/
import NobleKernel

namespace noble_kernel_stub

open Aeneas Aeneas.Std noble_kernel

/-- Same-named copy of `noble_kernel.acceptance.check`: same name, same
    type, delegated body. -/
def acceptance.check (env : noble_kernel.contracts.Env)
    (request : noble_kernel.untrusted.Request)
    (candidate : noble_kernel.untrusted.Candidate) :
    Result noble_kernel.untrusted.Outcome :=
  noble_kernel.acceptance.check env request candidate

end noble_kernel_stub
STUB
    sed -i \
      -e 's/^import NobleKernel$/import NobleM2.NobleKernelStub/' \
      -e 's/acceptance\.check/noble_kernel_stub.acceptance.check/g' \
      "$copy/NobleM2/Refinement.lean"
    grep -qx 'import NobleM2.NobleKernelStub' "$copy/NobleM2/Refinement.lean" \
      || { echo "APPLY FAILED"; return 1; }
    ;;
  unexplained-external)
    # Delete one external-model implementation (the #[charon::opaque]
    # clone_stack copy loop) from FunsExternal.lean.
    sed -i \
      '/^\/-- \[noble_kernel::types::impls::clone_stack\]:/,/^  fun s => ok (\.from s\.val s\.property)$/d' \
      "$copy/NobleKernel/FunsExternal.lean"
    [ "$(grep -c 'def types.impls.clone_stack' "$copy/NobleKernel/FunsExternal.lean")" -eq 0 ] \
      || { echo "APPLY FAILED"; return 1; }
    ;;
  unexecuted-coverage)
    # Delete the coverage_positive theorem file, keep the tree building:
    # strip its imports and move the DecidableEq instances it provided into
    # CheckSoundness (where the regression's native_decide needs them).
    rm "$copy/NobleM2/Termination.lean"
    sed -i '/^import NobleM2.Termination$/d' \
      "$copy/NobleM2.lean" "$copy/NobleM2/CheckSoundness.lean"
    cat > "$SCRATCH/decidable-eq.lean" <<'DECEQ'
deriving instance DecidableEq for NobleM2.Interface
deriving instance DecidableEq for NobleM2.Derivation
deriving instance DecidableEq for NobleM2.Checked
deriving instance DecidableEq for NobleM2.Constraint
deriving instance DecidableEq for NobleM2.Diagnostic
deriving instance DecidableEq for NobleM2.UnsupportedKind
deriving instance DecidableEq for NobleM2.LimitKind
deriving instance DecidableEq for NobleM2.Outcome
DECEQ
    sed -i "/^\/-! ## Decidable outcome equality/r $SCRATCH/decidable-eq.lean" \
      "$copy/NobleM2/CheckSoundness.lean"
    [ ! -f "$copy/NobleM2/Termination.lean" ] \
      || { echo "APPLY FAILED"; return 1; }
    ;;
  *) echo "unknown case $label"; return 1 ;;
  esac

  local out rc
  out="$("$GATE" "$copy" 2>&1)"; rc=$?
  printf '%s\n' "$out" | grep '^\[m2-gate\]'
  printf '%s\n' "$out" | grep -v '^\[m2-gate\]' | grep -v '^info: m2-gate: axioms' | tail -12
  if [ "$rc" -eq 0 ]; then
    echo "REFUSAL FAILED: gate accepted the mutated tree (exit 0)"
    return 1
  fi
  if ! printf '%s\n' "$out" | grep -q "^\[m2-gate\] FAIL $expect"; then
    echo "REFUSAL at wrong check (expected $expect)"; return 1
  fi
  echo "refused: gate exit $rc at $expect"
  return 0
}

rc_all=0
for case_label in \
  'proof-hole NO-SORRY' \
  'substituted-subject REFINEMENT-SUBJECT' \
  'unexplained-external BUILD' \
  'unexecuted-coverage REQUIRED-THEOREMS'
do
  # shellcheck disable=SC2086
  run_case $case_label || rc_all=1
done

echo
if [ "$rc_all" -eq 0 ]; then
  echo "== refusal matrix complete: baseline passes, all four mutations refused =="
else
  echo "== refusal matrix INCOMPLETE =="
fi
exit "$rc_all"
