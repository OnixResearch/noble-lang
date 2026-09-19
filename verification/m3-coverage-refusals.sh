#!/usr/bin/env bash
# Task-6.2 refusal matrix for the M3 coverage gate.
#
# Runs the positive baseline (verification/m3-coverage-gate.sh), then
# mutated scratch copies. Record mutations edit a scratch copy of
# verification/m3-coverage.json (the m3 gate passes the record through to
# the delegated M2 battery, where the join/citation checks run); the
# module mutation edits a hard-linked scratch copy of the m3 proof root
# and is refused by the kernel binding. The repository tree is never
# touched.
#
# Usage (from any directory; the pinned Lean must be first on PATH; bun
# must be on PATH):
#
#   PATH=/nix/store/hbxwgcbpkyqllqzq7cidhnvhgb60jb9q-noble-lean-release-4.31.0/bin:$PATH \
#     verification/m3-coverage-refusals.sh
#
# Mutations (each a separate cell):
#   missing-per-word-  the proved acceptance.check entry (the per-word    -> UNCLASSIFIED
#   coverage-case      coverage witnesses' subject; one entry per word
#                      as 5.3's per-word witnesses land) deleted from the
#                      record while the function stays generated
#   unclassified-def   a plausible generated def appended to the m3      -> UNCLASSIFIED
#                      root's NobleKernel/Funs.lean (record stale; strict tier)
#   dangling-citation  a cited theorem renamed in the record             -> CITATION
#   misbound-citation  the citation repointed at a real theorem about    -> CITATION-
#                      a different subject                                BINDING
#   stale-entry        a record entry for a function that is not         -> STALE-
#                      generated                                          ENTRY
#   bad-status         a status outside the reviewed vocabulary          -> STATUS
set -uo pipefail

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
GATE="$REPO/verification/m3-coverage-gate.sh"
PROOF_ROOT="${M3_PROOF_ROOT:-$REPO/proofs/m3}"
RECORD="${M3_COVERAGE_RECORD:-$REPO/verification/m3-coverage.json}"
export PATH  # the pinned Lean must already be on PATH; the gate pins 4.31.0

SCRATCH="$(mktemp -d "$REPO/.m3-cov-refusals-XXXXXX")"
trap 'rm -rf "$SCRATCH"' EXIT

echo "== positive baseline =="
if "$GATE" > "$SCRATCH/baseline.log" 2>&1; then
  grep '^\[m3-coverage\] PASS\|^\[m2-coverage\] PASS\|^\[m3-coverage\] SUMMARY\|^\[m2-coverage\] SUMMARY' "$SCRATCH/baseline.log" | tail -9
  echo "baseline: gate exit 0 (all checks pass)"
else
  echo "baseline: gate FAILED — refusal matrix not run"
  tail -20 "$SCRATCH/baseline.log"
  exit 1
fi

# A fresh hard-linked scratch copy of the m3 proof root with real (non-
# shared) build/config directories. NOTE: the copy's source files share
# inodes with the repository, so edits must go through write-to-temp-then-
# `mv`; a plain `>> file` redirect would clobber the repository's file.
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
  missing-per-word-coverage-case)
    fresh_copy "$copy"
    # The proved subject function's entry deleted while the generated
    # module still defines it: the classification join must refuse.
    record="$SCRATCH/$label.json"
    mutate_record "$RECORD" "$record" '
      r.functions = r.functions.filter(
        f => f.lean !== "noble_kernel.acceptance.check");
    ' || { echo "APPLY FAILED"; return 1; }
    fresh_copy "$copy"
    ;;
  unclassified-def)
    fresh_copy "$copy"
    # A plausible new generated function spliced into the m3 root's
    # generated Funs.lean: the generated surface changes, so the kernel
    # binding refuses (the join-level UNCLASSIFIED check engages the same
    # way once the strict tier runs the join on this root).
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
      e.citations[0] = "NobleM2.Refinement.refinement_ghost_theorem";
    ' || { echo "APPLY FAILED"; return 1; }
    ;;
  misbound-citation)
    fresh_copy "$copy"
    record="$SCRATCH/$label.json"
    mutate_record "$RECORD" "$record" '
      const e = r.functions.find(f => f.lean === "noble_kernel.acceptance.check");
      e.citations = e.citations.map(() => "NobleM2.check_total");
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
  *) echo "unknown case $label"; return 1 ;;
  esac

  local out rc
  out="$("$GATE" "$copy" "$record" 2>&1)"; rc=$?
  printf '%s\n' "$out" | grep '^\[m3-coverage\] FAIL' | sort -u
  printf '%s\n' "$out" | grep '^\[m2-coverage\] FAIL\|m[23]-coverage: FAIL' | sort -u | head -4
  if [ "$rc" -eq 0 ]; then
    echo "REFUSAL FAILED: gate accepted the mutated copy (exit 0)"
    return 1
  fi
  if ! printf '%s\n' "$out" | grep -q "$expect"; then
    echo "REFUSAL FAILED: gate exited $rc but not at the expected check $expect"
    return 1
  fi
  echo "refused at $expect (gate exit $rc)"
  return 0
}

rc_all=0
for case_label in \
  'missing-per-word-coverage-case UNCLASSIFIED' \
  'unclassified-def UNCLASSIFIED' \
  'dangling-citation CITATION' \
  'misbound-citation CITATION-BINDING' \
  'stale-entry STALE-ENTRY' \
  'bad-status STATUS'
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
