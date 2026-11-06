#!/usr/bin/env bash
# Task-3.3 refusal matrix for the M2 coverage gate.
#
# Runs the positive baseline (verification/m2-coverage-gate.sh on the
# repository proof root and record), then seven mutated scratch copies.
# Every mutation must be REFUSED: the gate exits non-zero with its named
# check. A final positive control shows the strength classifier is not
# vacuous: a synthetic universal theorem makes its function proved. Proof-root mutations share the lake package cache through hard
# links (the mutable .lake/build and .lake/config directories are real
# copies); record mutations edit a scratch copy of
# verification/m2-coverage.json. The repository tree is never touched.
#
# Usage (from any directory; the pinned Lean must be first on PATH; bun
# must be on PATH):
#
#   PATH=/nix/store/kwvw84h9r7w0q40d3cm5xgfz0rpv7yzc-noble-lean-release-4.35.0/bin:$PATH \
#     verification/m2-coverage-refusals.sh
#
# Mutations (each a separate cell):
#   unclassified-     a plausible generated def appended to      -> UNCLASSIFIED
#   def               NobleKernel/Funs.lean (record stale)
#   dangling-         a cited theorem renamed in the record      -> CITATION
#   citation
#   misbound-         the citation repointed at a real theorem   -> CITATION-
#   citation          about a different subject (the reference   BINDING
#                     model's check_total)
#   stale-entry       a record entry for a function that is not  -> STALE-
#                     generated                                  ENTRY
#   bad-status        a status outside the reviewed vocabulary   -> STATUS
#   native-fixtures-  acceptance.check marked proved while every -> STRENGTH-
#   as-proved         citation is a native fixture or            STATUS
#                     conditional handoff (#88 control a)
#   conditional-      the refinement handoff, whose hypothesis   -> STRENGTH-
#   claimed-universal is the refinement equation, recorded as    DRIFT
#                     universal and the function as proved
#                     (#88 control b)
# Positive control (#88 control c):
#   universal-probe   a synthetic universal theorem about a toy  -> accepted,
#                     extracted function (EffId clone) makes it   proved=1
#                     proved
set -uo pipefail

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
GATE="$REPO/verification/m2-coverage-gate.sh"
PROOF_ROOT="${M2_PROOF_ROOT:-$REPO/proofs/m2}"
RECORD="${M2_COVERAGE_RECORD:-$REPO/verification/m2-coverage.json}"
export PATH  # the pinned Lean must already be on PATH; the gate pins 4.35.0

SCRATCH="$(mktemp -d "$REPO/.m2-cov-refusals-XXXXXX")"
trap 'rm -rf "$SCRATCH"' EXIT

echo "== positive baseline =="
if "$GATE" "$PROOF_ROOT" "$RECORD" > "$SCRATCH/baseline.log" 2>&1; then
  grep '^\[m2-coverage\] PASS' "$SCRATCH/baseline.log" | head -9
  grep '^\[m2-coverage\] SUMMARY' "$SCRATCH/baseline.log"
  echo "baseline: gate exit 0 (all checks pass)"
else
  echo "baseline: gate FAILED — refusal matrix not run"
  tail -20 "$SCRATCH/baseline.log"
  exit 1
fi

# A fresh hard-linked scratch copy of the proof root with real (non-shared)
# build/config directories. NOTE: the copy's source files share inodes with
# the repository, so edits must go through write-to-temp-then-`mv` or
# `sed -i`/`rm` (rename/unlink); a plain `>> file` redirect would write the
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

# Mutate a scratch copy of the classification record with bun.
mutate_record() { # IN OUT SCRIPT
  bun -e '
    const r = await Bun.file(process.argv[1]).json();
    '"$3"'
    await Bun.write(process.argv[2], JSON.stringify(r, null, 1) + "\n");
  ' "$1" "$2"
}

run_case() {
  local label="$1" expect="$2"
  local copy="$SCRATCH/$label"
  echo
  echo "== mutation: $label (expected refusal: $expect) =="
  local record="$RECORD"

  case "$label" in
  unclassified-def)
    fresh_copy "$copy"
    # A plausible new generated function: doc header with rust name and
    # source span, column-0 def, spliced just before the generated file's
    # final `end noble_kernel` so the file keeps its generated shape. It
    # elaborates (the file-level `open`s still apply), so the tree keeps
    # building — only the classification join can catch it.
    local last_end
    last_end="$(awk '/^end noble_kernel$/ { n = NR } END { print n }' \
      "$PROOF_ROOT/NobleKernel/Funs.lean")"
    [ -n "$last_end" ] || { echo "APPLY FAILED: no end noble_kernel"; return 1; }
    {
      head -n $((last_end - 1)) "$PROOF_ROOT/NobleKernel/Funs.lean"
      printf '\n/-- [noble_kernel::types::impls::coverage_probe]:\n'
      printf '    Source: %s, lines 999:0-999:1\n' "'crates/noble-kernel/src/types/impls.rs'"
      printf '    Visibility: public -/\n'
      printf 'def types.impls.coverage_probe : Result Bool := do\n'
      printf '  ok true\n'
      printf '\nend noble_kernel\n'
      tail -n +"$((last_end + 1))" "$PROOF_ROOT/NobleKernel/Funs.lean"
    } > "$SCRATCH/funs-mutated.lean"
    grep -q 'def types.impls.coverage_probe' "$SCRATCH/funs-mutated.lean" \
      || { echo "APPLY FAILED"; return 1; }
    mv "$SCRATCH/funs-mutated.lean" "$copy/NobleKernel/Funs.lean"
    ;;
  dangling-citation)
    fresh_copy "$copy"
    record="$SCRATCH/$label.json"
    mutate_record "$RECORD" "$record" '
      const e = r.functions.find(f => f.lean === "noble_kernel.acceptance.check");
      e.citations[0].theorem = "NobleM2.Refinement.refinement_ghost_theorem";
    ' || { echo "APPLY FAILED"; return 1; }
    ;;
  misbound-citation)
    fresh_copy "$copy"
    record="$SCRATCH/$label.json"
    mutate_record "$RECORD" "$record" '
      const e = r.functions.find(f => f.lean === "noble_kernel.acceptance.check");
      e.citations[0].theorem = "NobleM2.check_total";
    ' || { echo "APPLY FAILED"; return 1; }
    ;;
  stale-entry)
    fresh_copy "$copy"
    record="$SCRATCH/$label.json"
    mutate_record "$RECORD" "$record" '
      r.functions.push({
        lean: "noble_kernel.types.ghost_function", kind: "def",
        rust: "noble_kernel::types::ghost_function", file: "Funs.lean", line: 1,
        status: "extracted",
      });
    ' || { echo "APPLY FAILED"; return 1; }
    ;;
  bad-status)
    fresh_copy "$copy"
    record="$SCRATCH/$label.json"
    mutate_record "$RECORD" "$record" '
      r.functions.find(f => f.lean === "noble_kernel.types.EffSet.empty").status = "reviewed";
    ' || { echo "APPLY FAILED"; return 1; }
    ;;
  native-fixtures-as-proved)
    fresh_copy "$copy"
    record="$SCRATCH/$label.json"
    mutate_record "$RECORD" "$record" '
      r.functions.find(f => f.lean === "noble_kernel.acceptance.check").status = "proved";
    ' || { echo "APPLY FAILED"; return 1; }
    ;;
  conditional-claimed-universal)
    fresh_copy "$copy"
    record="$SCRATCH/$label.json"
    mutate_record "$RECORD" "$record" '
      const e = r.functions.find(f => f.lean === "noble_kernel.acceptance.check");
      e.citations.find(c => c.theorem === "NobleM2.Refinement.accepted_via_refinement").strength = "universal";
      e.status = "proved";
    ' || { echo "APPLY FAILED"; return 1; }
    ;;
  *) echo "unknown case $label"; return 1 ;;
  esac

  local out rc
  out="$("$GATE" "$copy" "$record" 2>&1)"; rc=$?
  printf '%s\n' "$out" | grep '^\[m2-coverage\] FAIL' | sort -u
  printf '%s\n' "$out" | grep 'm2-coverage: FAIL' | sort -u | head -4
  if [ "$rc" -eq 0 ]; then
    echo "REFUSAL FAILED: gate accepted the mutated tree (exit 0)"
    return 1
  fi
  if ! printf '%s\n' "$out" | grep -q "$expect"; then
    echo "REFUSAL FAILED: gate exited $rc but not at the expected check $expect"
    return 1
  fi
  echo "refused at $expect (gate exit $rc)"
  return 0
}

# Positive control: a synthetic universal theorem about a toy extracted
# function, appended (write-then-mv, so the hard-linked source is untouched)
# to a scratch copy of NobleM2/Refinement.lean and cited for that function.
# The classifier must call it universal with the strict axiom set, and the
# gate must then accept the function as proved.
run_universal_probe() {
  local copy="$SCRATCH/universal-probe"
  echo
  echo "== positive control: universal-probe (expected: accepted, proved=1) =="
  fresh_copy "$copy"
  {
    cat "$PROOF_ROOT/NobleM2/Refinement.lean"
    printf '\n/-- #88 positive control: universal over every EffId, no refinement hypothesis. -/\n'
    printf 'theorem NobleM2.Refinement.coverage_strength_universal_probe (e : noble_kernel.types.EffId) :\n'
    printf '    noble_kernel.types.EffId.Insts.CoreCloneClone.clone e = Aeneas.Std.Result.ok e := rfl\n'
  } > "$SCRATCH/refinement-probe.lean"
  mv "$SCRATCH/refinement-probe.lean" "$copy/NobleM2/Refinement.lean"
  local record="$SCRATCH/universal-probe.json"
  mutate_record "$RECORD" "$record" '
    const e = r.functions.find(f => f.lean === "noble_kernel.types.EffId.Insts.CoreCloneClone.clone");
    e.status = "proved";
    e.citations = [{ theorem: "NobleM2.Refinement.coverage_strength_universal_probe", strength: "universal", axioms: "strict" }];
  ' || { echo "APPLY FAILED"; return 1; }
  local out rc
  out="$("$GATE" "$copy" "$record" 2>&1)"; rc=$?
  printf '%s\n' "$out" | grep -E '^\[m2-coverage\] (PASS STRENGTH|SUMMARY|FAIL)' | sort -u
  if [ "$rc" -ne 0 ]; then
    echo "POSITIVE CONTROL FAILED: gate refused the universal probe (exit $rc)"
    return 1
  fi
  if ! printf '%s\n' "$out" | grep -q 'proved=1 .*strict_universal=1'; then
    echo "POSITIVE CONTROL FAILED: the probe was not counted as one strict universal proof"
    return 1
  fi
  echo "accepted: the universal probe is classified universal/strict and its function is proved"
  return 0
}

rc_all=0
for case_label in \
  'unclassified-def UNCLASSIFIED' \
  'dangling-citation CITATION' \
  'misbound-citation CITATION-BINDING' \
  'stale-entry STALE-ENTRY' \
  'bad-status STATUS' \
  'native-fixtures-as-proved STRENGTH-STATUS' \
  'conditional-claimed-universal STRENGTH-DRIFT'
do
  # shellcheck disable=SC2086
  run_case $case_label || rc_all=1
done
run_universal_probe || rc_all=1

echo
if [ "$rc_all" -eq 0 ]; then
  echo "== all refusals demonstrated =="
else
  echo "== refusal matrix FAILED =="
fi
exit "$rc_all"
