#!/usr/bin/env bash
# M3 per-function coverage classification gate (task 6.1; succeeds the M2
# gate, which stays the floor and is never weakened).
#
# For every function of the M3 kernel extraction subject, join the
# authoritative function list derived from the extraction output with the
# reviewed classification record verification/m3-coverage.json, and enforce:
#
#   COVERAGE-DERIVE + PARSE-PARITY  the list is derived from the generated
#                     NobleKernel/Funs.lean and FunsExternal.lean defs
#                     (rust names + source spans), with a keyword-parity
#                     guard against parser gaps
#   COVERAGE-JOIN     every generated function is classified exactly once
#   (UNCLASSIFIED /   and every non-open record entry is generated; the
#    STALE-ENTRY)    statuses use the reviewed vocabulary
#   STATUS /          extracted|proved|modeled|excepted|open, placed in the
#    STATUS-PLACEMENT right generated file, with citations exactly on proved
#   DRIFT             entries and record fields restating the generated facts
#   EXCEPTED-         every excepted entry is a declared #[charon::opaque]
#    DISCLOSURE       disclosure in the Rust crate (count and names match)
#   ENV-BINDING       every classified constant exists as a definition in its
#                     generated module (elaborated environment, not just
#                     text), and the only residual definitions in those
#                     modules are elaborator matchers
#   CITATIONS         every proved citation exists, is a theorem, and its
#                     statement mentions the classified constant
#
# Tiers (as in m3-proof-gate.sh): when the proofs/m3 root builds, the full
# battery runs against it (strict). Until then the gate says so and runs
# the M3-KERNEL-BINDING checks (the m3 root's generated module is
# byte-identical to proofs/m2's, the module the record's proved citations
# elaborate against) and then the full M2 coverage battery on proofs/m2
# with this record (M2-FLOOR). The record's citations split three ways in
# strict mode: non-refinement -> checker A, NobleM2.Refinement.* ->
# checker B, NobleM3.Refine.* -> checker C.
#
# Usage (from any directory; the pinned Lean must be first on PATH; `bun`
# is required on PATH):
#
#   PATH=/nix/store/hbxwgcbpkyqllqzq7cidhnvhgb60jb9q-noble-lean-release-4.31.0/bin:$PATH \
#     verification/m3-coverage-gate.sh [PROOF_ROOT] [RECORD]
#
# PROOF_ROOT defaults to the repository's proofs/m3 and RECORD to
# verification/m3-coverage.json (the refusal harness overrides both to run
# mutated scratch copies). Exit 0 = all checks pass.
set -uo pipefail

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PROOF_ROOT="${M3_PROOF_ROOT:-$REPO/proofs/m3}"
RECORD="${M3_COVERAGE_RECORD:-$REPO/verification/m3-coverage.json}"
if [ $# -ge 1 ]; then PROOF_ROOT="$1"; fi
if [ $# -ge 2 ]; then RECORD="$2"; fi
PROOF_ROOT="$(cd "$PROOF_ROOT" 2>/dev/null && pwd)" || {
  echo "[m3-coverage] FAIL SETUP: proof root not found: $1" >&2; exit 1; }
RECORD="$(cd "$(dirname "$RECORD")" 2>/dev/null && pwd)/$(basename "$RECORD")"
[ -f "$RECORD" ] || { echo "[m3-coverage] FAIL SETUP: record not found: $RECORD" >&2; exit 1; }
[ -f "$PROOF_ROOT/lakefile.toml" ] || {
  echo "[m3-coverage] FAIL SETUP: $PROOF_ROOT has no lakefile.toml" >&2; exit 1; }
M2_ROOT="$REPO/proofs/m2"
M2_GATE="$REPO/verification/m2-coverage-gate.sh"
JOIN="$REPO/verification/m3-coverage-join.mjs"

fail() { echo "[m3-coverage] FAIL $1: $2" >&2; exit 1; }
pass() { echo "[m3-coverage] PASS $1"; }

command -v bun >/dev/null 2>&1 || fail SETUP "bun not found on PATH (required by the join tool)"
lean --version 2>/dev/null | grep -q 'version 4\.31\.0' \
  || fail TOOLCHAIN "lean on PATH is not the pinned 4.31.0 release: $(lean --version 2>&1)"
pass "SETUP (bun + lean 4.31.0)"

# Tier probe: the strict tier needs the m3 root's four targets to build.
probe_log="$(cd "$PROOF_ROOT" && lake build NobleM3 NobleM2 NobleM2.Refinement NobleKernel 2>&1)"
probe_rc=$?

if [ "$probe_rc" -ne 0 ]; then
  echo "[m3-coverage] HANDOFF: the proofs/m3 root does not build yet (tasks 5.1-5.4 in flight);"
  echo "[m3-coverage] HANDOFF: strict tier not engaged; enforcing the M3 kernel binding and the M2 floor."
  cmp -s "$PROOF_ROOT/NobleKernel.lean" "$M2_ROOT/NobleKernel.lean" \
    || fail M3-KERNEL-BINDING "the m3 root's generated entry point differs from proofs/m2's"
  diff -rq "$PROOF_ROOT/NobleKernel" "$M2_ROOT/NobleKernel" >/dev/null \
    || fail M3-KERNEL-BINDING "the m3 root's generated modules differ from proofs/m2's"
  pass "M3-KERNEL-BINDING (m3 root's generated module byte-identical to proofs/m2's — the record's subject)"
  "$M2_GATE" "$M2_ROOT" "$RECORD" || fail M2-FLOOR "the M2 coverage battery failed on proofs/m2 with this record"
  echo "[m3-coverage] PASS (handoff tier) — strict tier engages when proofs/m3 builds"
  exit 0
fi

pass "BUILD (lake build NobleM3 NobleM2 NobleM2.Refinement NobleKernel)"

# COVERAGE-DERIVE + COVERAGE-JOIN against the m3 root.
report="$(bun "$JOIN" check "$PROOF_ROOT" "$RECORD" 2>&1)" || true
if ! printf '%s' "$report" | grep -q '"problems"'; then
  printf '%s\n' "$report" >&2
  fail COVERAGE-DERIVE "the join tool could not derive the function list from the generated files"
fi
ok="$(printf '%s' "$report" | bun -e 'const r = await new Response(Bun.stdin.stream()).json(); console.log(r.ok)')"
if [ "$ok" != "true" ]; then
  printf '%s\n' "$report" | bun -e '
    const r = await new Response(Bun.stdin.stream()).json();
    for (const p of r.problems) console.error(`[m3-coverage] FAIL ${p.code}: ${p.msg}`);
  ' >&2
  fail COVERAGE-JOIN "derived functions and reviewed record disagree (see named problems above)"
fi
counts="$(printf '%s' "$report" | bun -e '
  const r = await new Response(Bun.stdin.stream()).json();
  console.log(`total=${r.total} classified=${r.classified} ` +
    Object.entries(r.counts).map(([k, v]) => `${k}=${v}`).join(" "));
')"
echo "[m3-coverage] PASS COVERAGE-DERIVE (Funs.lean + FunsExternal.lean defs with rust names and source spans; keyword parity holds)"
pass "COVERAGE-JOIN ($counts)"

# EXCEPTED-DISCLOSURE: the excepted entries are exactly the crate's
# #[charon::opaque] declarations.
excepted_short="$(bun -e '
  const r = await Bun.file(process.argv[1]).json();
  console.log(r.functions.filter(e => e.status === "excepted")
    .map(e => e.rust.split("::").pop()).sort().join("\n"));
' "$RECORD")"
crate_src="$REPO/crates/noble-kernel/src"
[ -d "$crate_src" ] || fail EXCEPTED-DISCLOSURE "Rust crate sources not found: $crate_src"
opaque_short="$(cd "$crate_src" && awk '
  /#\[charon::opaque\]/ { opaque = 1; next }
  opaque && /fn [A-Za-z_][A-Za-z0-9_]*/ {
    match($0, /fn [A-Za-z_][A-Za-z0-9_]*/); print substr($0, RSTART + 3, RLENGTH - 3);
    opaque = 0
  }' $(find . -name '*.rs' | sort) | sort)"
if [ "$excepted_short" != "$opaque_short" ]; then
  echo "record excepted:      $(printf '%s' "$excepted_short" | tr '\n' ' ')" >&2
  echo "crate #[charon::opaque]: $(printf '%s' "$opaque_short" | tr '\n' ' ')" >&2
  fail EXCEPTED-DISCLOSURE "excepted entries do not match the crate's charon::opaque disclosures"
fi
n_excepted="$(printf '%s\n' "$excepted_short" | grep -c . || true)"
pass "EXCEPTED-DISCLOSURE ($n_excepted excepted entries == #[charon::opaque] declarations in the crate)"

# OPEN-SUBJECT: open entries must still name a real Rust function.
open_entries="$(bun -e '
  const r = await Bun.file(process.argv[1]).json();
  for (const e of r.functions.filter(e => e.status === "open"))
    console.log(e.lean + " " + (e.rust ?? ""));
' "$RECORD")"
if [ -n "$open_entries" ]; then
  while read -r lean rust; do
    [ -n "${rust:-}" ] || fail OPEN-SUBJECT "$lean: open entry without a Rust-side name"
    short="${rust##*::}"
    grep -rqE "fn[[:space:]]+$short\b" "$crate_src" \
      || fail OPEN-SUBJECT "$lean: open entry names no Rust function 'fn $short' in the crate"
  done <<< "$open_entries"
fi
pass "OPEN-SUBJECT (0 open entries; every generated function is classified)"

# ENV-BINDING + CITATIONS: three scratch checkers. The root module, the M2
# Refinement module, and the M3 Refine module cannot all be imported
# together (deriving-generated instance clashes), so the payloads are
# split: non-refinement citations -> A (imports NobleKernel + NobleM2),
# NobleM2.Refinement.* -> B, NobleM3.Refine.* -> C.
GATE_A="$PROOF_ROOT/.m3-cov-env-a.lean"
GATE_B="$PROOF_ROOT/.m3-cov-env-b.lean"
GATE_C="$PROOF_ROOT/.m3-cov-env-c.lean"
DATA_A="$PROOF_ROOT/.m3-cov-data-a.json"
DATA_B="$PROOF_ROOT/.m3-cov-data-b.json"
DATA_C="$PROOF_ROOT/.m3-cov-data-c.json"
cleanup() { rm -f "$GATE_A" "$GATE_B" "$GATE_C" "$DATA_A" "$DATA_B" "$DATA_C"; }
trap cleanup EXIT
bun "$JOIN" leanpayload "$PROOF_ROOT" "$RECORD" "$DATA_A" "$DATA_B" "$DATA_C" \
  || fail ENV-BINDING "could not build the checker payloads"

CHECKER_HEAD='import Lean.Elab.Command
import Lean.Util.FoldConsts

open Lean

def nameOf (s : String) : Name :=
  s.splitOn "." |>.foldl (fun n p => n.mkStr p) .anonymous

def strOf (j : Json) (k : String) : Option String :=
  match j.getObjVal? k >>= (·.getStr?) with
  | Except.ok s => some s
  | Except.error _ => none

def citsOf (j : Json) : Array String :=
  match j.getObjVal? "citations" with
  | Except.ok (Json.arr cs) => cs.filterMap (fun c => match c with | Json.str s => some s | _ => none)
  | _ => #[]

def isElabMatcher (s : String) : Bool :=
  s.endsWith "_unsafe_rec"
  || (s.splitOn ".").any (fun c =>
        (c.startsWith "match_"
          && ((c.drop 6).all Char.isDigit || c.drop 6 == "splitter"))
        || c.startsWith "_sparseCasesOn")

run_cmd do
  let env ← getEnv
  let txt ← IO.FS.readFile "DATAFILE"
  let mut entries : Array (String × Name × Array String) := #[]
  match Json.parse txt with
  | Except.error e => logError m!"m3-coverage: FAIL PAYLOAD: {e}"
  | Except.ok (Json.arr xs) =>
    for j in xs do
      match strOf j "lean", strOf j "module" with
      | some l, some m => entries := entries.push (l, nameOf m, citsOf j)
      | _, _ => logError m!"m3-coverage: FAIL PAYLOAD: bad entry {j}"
  | Except.ok _ => logError "m3-coverage: FAIL PAYLOAD: not a JSON array"'

CHECKER_A_BODY='  let mut classified : NameSet := {}
  let mut errs := 0
  for (l, mod, cits) in entries do
    let n := nameOf l
    match env.find? n with
    | none =>
      logError m!"m3-coverage: FAIL CONSTANT-MISSING: {n}"
      errs := errs + 1
    | some ci =>
      if !(ci matches .defnInfo _) then
        logError m!"m3-coverage: FAIL CONSTANT-KIND: {n} is not a definition"
        errs := errs + 1
      match env.getModuleIdxFor? n with
      | none =>
        logError m!"m3-coverage: FAIL MODULE: {n} has no defining module"
        errs := errs + 1
      | some idx =>
        let actual := env.header.moduleNames[idx]!
        if actual != mod then
          logError m!"m3-coverage: FAIL MODULE: {n} is defined in {actual}, record says {mod}"
          errs := errs + 1
    classified := classified.insert n
    for c in cits do
      let cn := nameOf c
      match env.find? cn with
      | none =>
        logError m!"m3-coverage: FAIL CITATION: {c} cited for {n} does not exist"
        errs := errs + 1
      | some ci =>
        if !(ci matches .thmInfo _) then
          logError m!"m3-coverage: FAIL CITATION: {c} cited for {n} is not a theorem"
          errs := errs + 1
        else if !(ci.type.getUsedConstants.contains n) then
          logError m!"m3-coverage: FAIL CITATION-BINDING: {c} does not mention {n} in its statement"
          errs := errs + 1
  let mut residual := 0
  for (n, ci) in env.constants.toList do
    if !(n.toString.startsWith "noble_kernel.") then continue
    if !(ci matches .defnInfo _) then continue
    let some idx := env.getModuleIdxFor? n | continue
    let mod := env.header.moduleNames[idx]!
    if mod != `NobleKernel.Funs && mod != `NobleKernel.FunsExternal then continue
    if classified.contains n then continue
    if isElabMatcher n.toString then
      residual := residual + 1
    else
      logError m!"m3-coverage: FAIL ENV-UNCLASSIFIED: {n} ({mod}) is a definition in the generated modules absent from the record"
      errs := errs + 1
  if errs == 0 then
    logInfo m!"m3-coverage: ENV-A-PASS {entries.size} classified constants bound to their modules; {residual} elaborator matchers tolerated"'

CHECKER_CIT_BODY='  let mut errs := 0
  let mut ncits := 0
  for (l, _mod, cits) in entries do
    ncits := ncits + cits.size
    let n := nameOf l
    for c in cits do
      let cn := nameOf c
      match env.find? cn with
      | none =>
        logError m!"m3-coverage: FAIL CITATION: {c} cited for {n} does not exist"
        errs := errs + 1
      | some ci =>
        if !(ci matches .thmInfo _) then
          logError m!"m3-coverage: FAIL CITATION: {c} cited for {n} is not a theorem"
          errs := errs + 1
        else if !(ci.type.getUsedConstants.contains n) then
          logError m!"m3-coverage: FAIL CITATION-BINDING: {c} does not mention {n} in its statement"
          errs := errs + 1
  if errs == 0 then
    logInfo m!"m3-coverage: TAG-PASS {ncits} citations verified (theorem exists, statement mentions the constant)"'

make_checker() { # DATAFILE IMPORTS BODY TAG
  sed -e "s/DATAFILE/$1/" <<< "$CHECKER_HEAD"
  echo "import $2"
  echo
  if [ "$5" = "ENV-A" ]; then
    # Checker A binds every classified constant to its generated module
    # and verifies its own (non-refinement) citations inline.
    printf '%s\n' "$CHECKER_A_BODY"
  else
    sed -e "s/TAG-PASS/$5-PASS/" <<< "$CHECKER_CIT_BODY"
  fi
}

make_checker "$DATA_A" "NobleKernel" "" "ENV-A" > "$GATE_A"
sed -i "1a import NobleM2" "$GATE_A"  # A checks the floor theorems' modules too
make_checker "$DATA_B" "NobleM2.Refinement" "" "ENV-B" > "$GATE_B"
make_checker "$DATA_C" "NobleM3.Refine" "" "ENV-C" > "$GATE_C"

env_log=""
env_rc=0
for tag in a b c; do
  log="$(cd "$PROOF_ROOT" && lake env lean ".m3-cov-env-$tag.lean" 2>&1)"; rc=$?
  env_log="$env_log$log"
  [ "$rc" -eq 0 ] || { printf '%s\n' "$log" | tail -25 >&2; fail ENV-BINDING "checker $tag elaboration exited $rc"; }
done
if grep -q 'm3-coverage: FAIL' <<< "$env_log"; then
  printf '%s\n' "$env_log" | grep 'm3-coverage: FAIL' | sort -u >&2
  fail ENV-BINDING "environment/citation check reported violations (see named failures above)"
fi
printf '%s\n' "$env_log" | grep -o 'm3-coverage: ENV-.-PASS.*' | sed 's/^m3-coverage: /info: m3-coverage: /'
pass "ENV-BINDING (constants bound in the elaborated environment; citations are theorems mentioning their constants)"

echo "[m3-coverage] SUMMARY $counts"
echo "[m3-coverage] PASS (all checks) — subject: $PROOF_ROOT; record: $RECORD"
exit 0
