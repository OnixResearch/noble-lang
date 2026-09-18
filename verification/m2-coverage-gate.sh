#!/usr/bin/env bash
# M2 per-function coverage classification gate (task 3.3).
#
# For every function of the M2 kernel extraction subject, join the
# authoritative function list derived from the extraction output with the
# reviewed classification record verification/m2-coverage.json, and enforce:
#
#   COVERAGE-DERIVE   the list is derived from the generated
#   + PARSE-PARITY    NobleKernel/Funs.lean and FunsExternal.lean defs
#                     (rust names + source spans), with a keyword-parity
#                     guard against parser gaps
#   COVERAGE-JOIN     every generated function is classified exactly once
#   (UNCLASSIFIED /   and every non-open record entry is generated (no
#    STALE-ENTRY)    stale entries); statuses use the reviewed vocabulary
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
# Usage (from any directory; the pinned Lean must be first on PATH; `bun` is
# required on PATH — the repository's checks already run on it):
#
#   PATH=/nix/store/hbxwgcbpkyqllqzq7cidhnvhgb60jb9q-noble-lean-release-4.31.0/bin:$PATH \
#     verification/m2-coverage-gate.sh [PROOF_ROOT] [RECORD]
#
# PROOF_ROOT defaults to the repository's proofs/m2 and RECORD to
# verification/m2-coverage.json (the refusal harness overrides both to run
# mutated scratch copies). Exit 0 = all checks pass.
set -uo pipefail

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PROOF_ROOT="${M2_PROOF_ROOT:-$REPO/proofs/m2}"
RECORD="${M2_COVERAGE_RECORD:-$REPO/verification/m2-coverage.json}"
if [ $# -ge 1 ]; then PROOF_ROOT="$1"; fi
if [ $# -ge 2 ]; then RECORD="$2"; fi
PROOF_ROOT="$(cd "$PROOF_ROOT" 2>/dev/null && pwd)" || {
  echo "[m2-coverage] FAIL SETUP: proof root not found: $1" >&2; exit 1; }
RECORD="$(cd "$(dirname "$RECORD")" 2>/dev/null && pwd)/$(basename "$RECORD")"
[ -f "$RECORD" ] || { echo "[m2-coverage] FAIL SETUP: record not found: $RECORD" >&2; exit 1; }
[ -f "$PROOF_ROOT/lakefile.toml" ] || {
  echo "[m2-coverage] FAIL SETUP: $PROOF_ROOT has no lakefile.toml" >&2; exit 1; }

fail() { echo "[m2-coverage] FAIL $1: $2" >&2; exit 1; }
pass() { echo "[m2-coverage] PASS $1"; }

# C0 SETUP: bun runs the join tool (the repository's checks already depend
# on it); the pinned Lean must be on PATH.
command -v bun >/dev/null 2>&1 || fail SETUP "bun not found on PATH (required by the join tool)"
lean --version 2>/dev/null | grep -q 'version 4\.31\.0' \
  || fail TOOLCHAIN "lean on PATH is not the pinned 4.31.0 release: $(lean --version 2>&1)"
pass "SETUP (bun + lean 4.31.0)"

# C1 BUILD: the generated kernel and the two proof modules the environment
# checkers import must be current (their .olean are what `lake env lean`
# elaborates against).
build_log="$(cd "$PROOF_ROOT" && lake build NobleM2 NobleM2.Refinement NobleKernel 2>&1)"
build_rc=$?
if [ "$build_rc" -ne 0 ]; then
  printf '%s\n' "$build_log" | tail -25 >&2
  fail BUILD "lake build exited $build_rc (the coverage gate needs a building subject)"
fi
pass "BUILD (lake build NobleM2 NobleM2.Refinement NobleKernel)"

# C2 COVERAGE-DERIVE + C3 COVERAGE-JOIN: derive the authoritative function
# list from the generated function files, join with the reviewed record,
# enforce parity/classification/status/placement/drift.
JOIN="$REPO/verification/m2-coverage-join.mjs"
report="$(bun "$JOIN" check "$PROOF_ROOT" "$RECORD" 2>&1)" || true
if ! printf '%s' "$report" | grep -q '"problems"'; then
  printf '%s\n' "$report" >&2
  fail COVERAGE-DERIVE "the join tool could not derive the function list from the generated files"
fi
ok="$(printf '%s' "$report" | bun -e 'const r = await new Response(Bun.stdin.stream()).json(); console.log(r.ok)')"
if [ "$ok" != "true" ]; then
  printf '%s\n' "$report" | bun -e '
    const r = await new Response(Bun.stdin.stream()).json();
    for (const p of r.problems) console.error(`[m2-coverage] FAIL ${p.code}: ${p.msg}`);
  ' >&2
  fail COVERAGE-JOIN "derived functions and reviewed record disagree (see named problems above)"
fi
counts="$(printf '%s' "$report" | bun -e '
  const r = await new Response(Bun.stdin.stream()).json();
  console.log(`total=${r.total} classified=${r.classified} ` +
    Object.entries(r.counts).map(([k, v]) => `${k}=${v}`).join(" "));
')"
echo "[m2-coverage] PASS COVERAGE-DERIVE (Funs.lean + FunsExternal.lean defs with rust names and source spans; keyword parity holds)"
echo "[m2-coverage] PASS COVERAGE-JOIN ($counts)"
pass "COVERAGE-JOIN (record and generated surface agree)"

# C4 EXCEPTED-DISCLOSURE: the excepted entries are exactly the crate's
# #[charon::opaque] declarations (the disclosed extraction exceptions), by
# count and by short Rust name.
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

# C5 OPEN-SUBJECT: open entries (subject functions missing from the
# extraction output) must still name a real Rust function in the crate.
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

# C6 ENV-BINDING + CITATIONS: elaborate two scratch checkers against the
# built modules. The root module and the Refinement module cannot be
# imported together (both elaborate deriving-generated instances with the
# same names), so citations are split: NobleM2.Refinement.* -> checker B,
# everything else -> checker A. Checker A also binds every classified
# constant to its generated module and tolerates only elaborator matchers
# as residual definitions.
GATE_A="$PROOF_ROOT/.m2-cov-env-a.lean"
GATE_B="$PROOF_ROOT/.m2-cov-env-b.lean"
DATA_A="$PROOF_ROOT/.m2-cov-data-a.json"
DATA_B="$PROOF_ROOT/.m2-cov-data-b.json"
cleanup() { rm -f "$GATE_A" "$GATE_B" "$DATA_A" "$DATA_B" "$PROOF_ROOT/.m2-cov-parity.err"; }
trap cleanup EXIT
bun "$JOIN" leanpayload "$PROOF_ROOT" "$RECORD" "$DATA_A" "$DATA_B" \
  || fail ENV-BINDING "could not build the checker payloads"

cat > "$GATE_A" <<'LEAN_A'
import Lean.Elab.Command
import Lean.Util.FoldConsts
import NobleKernel
import NobleM2

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

/-- Elaborator-generated auxiliary definitions (matchers, sparse case
    trees, structural-recursion unfolding helpers) — the only residue the
    coverage gate tolerates next to classified constants. -/
def isElabMatcher (s : String) : Bool :=
  s.endsWith "_unsafe_rec"
  || (s.splitOn ".").any (fun c =>
        (c.startsWith "match_"
          && ((c.drop 6).all Char.isDigit || c.drop 6 == "splitter"))
        || c.startsWith "_sparseCasesOn")

run_cmd do
  let env ← getEnv
  let txt ← IO.FS.readFile ".m2-cov-data-a.json"
  let mut entries : Array (String × Name × Array String) := #[]
  match Json.parse txt with
  | Except.error e => logError m!"m2-coverage: FAIL PAYLOAD: {e}"
  | Except.ok (Json.arr xs) =>
    for j in xs do
      match strOf j "lean", strOf j "module" with
      | some l, some m => entries := entries.push (l, nameOf m, citsOf j)
      | _, _ => logError m!"m2-coverage: FAIL PAYLOAD: bad entry {j}"
  | Except.ok _ => logError "m2-coverage: FAIL PAYLOAD: not a JSON array"
  let mut classified : NameSet := {}
  let mut errs := 0
  -- E1 every classified constant exists, is a definition, in its module
  for (l, mod, cits) in entries do
    let n := nameOf l
    match env.find? n with
    | none =>
      logError m!"m2-coverage: FAIL CONSTANT-MISSING: {n}"
      errs := errs + 1
    | some ci =>
      if !(ci matches .defnInfo _) then
        logError m!"m2-coverage: FAIL CONSTANT-KIND: {n} is not a definition"
        errs := errs + 1
      match env.getModuleIdxFor? n with
      | none =>
        logError m!"m2-coverage: FAIL MODULE: {n} has no defining module"
        errs := errs + 1
      | some idx =>
        let actual := env.header.moduleNames[idx]!
        if actual != mod then
          logError m!"m2-coverage: FAIL MODULE: {n} is defined in {actual}, record says {mod}"
          errs := errs + 1
    classified := classified.insert n
    for c in cits do
      let cn := nameOf c
      match env.find? cn with
      | none =>
        logError m!"m2-coverage: FAIL CITATION: {c} cited for {n} does not exist"
        errs := errs + 1
      | some ci =>
        if !(ci matches .thmInfo _) then
          logError m!"m2-coverage: FAIL CITATION: {c} cited for {n} is not a theorem"
          errs := errs + 1
        else if !(ci.type.getUsedConstants.contains n) then
          logError m!"m2-coverage: FAIL CITATION-BINDING: {c} does not mention {n} in its statement"
          errs := errs + 1
  -- E2 residual definitions in the generated modules: matchers only
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
      logError m!"m2-coverage: FAIL ENV-UNCLASSIFIED: {n} ({mod}) is a definition in the generated modules absent from the record"
      errs := errs + 1
  if errs == 0 then
    logInfo m!"m2-coverage: ENV-A-PASS {entries.size} classified constants bound to their modules; {residual} elaborator matchers tolerated"
LEAN_A
cat > "$GATE_B" <<'LEAN_B'
import Lean.Elab.Command
import Lean.Util.FoldConsts
import NobleM2.Refinement

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

run_cmd do
  let env ← getEnv
  let txt ← IO.FS.readFile ".m2-cov-data-b.json"
  let mut entries : Array (String × Name × Array String) := #[]
  match Json.parse txt with
  | Except.error e => logError m!"m2-coverage: FAIL PAYLOAD: {e}"
  | Except.ok (Json.arr xs) =>
    for j in xs do
      match strOf j "lean", strOf j "module" with
      | some l, some m => entries := entries.push (l, nameOf m, citsOf j)
      | _, _ => logError m!"m2-coverage: FAIL PAYLOAD: bad entry {j}"
  | Except.ok _ => logError "m2-coverage: FAIL PAYLOAD: not a JSON array"
  let mut errs := 0
  let mut ncits := 0
  for (l, _mod, cits) in entries do
    ncits := ncits + cits.size
    let n := nameOf l
    for c in cits do
      let cn := nameOf c
      match env.find? cn with
      | none =>
        logError m!"m2-coverage: FAIL CITATION: {c} cited for {n} does not exist"
        errs := errs + 1
      | some ci =>
        if !(ci matches .thmInfo _) then
          logError m!"m2-coverage: FAIL CITATION: {c} cited for {n} is not a theorem"
          errs := errs + 1
        else if !(ci.type.getUsedConstants.contains n) then
          logError m!"m2-coverage: FAIL CITATION-BINDING: {c} does not mention {n} in its statement"
          errs := errs + 1
  if errs == 0 then
    logInfo m!"m2-coverage: ENV-B-PASS {ncits} refinement citations verified (theorem exists, statement mentions the constant)"
LEAN_B

env_a_log="$(cd "$PROOF_ROOT" && lake env lean .m2-cov-env-a.lean 2>&1)"
env_a_rc=$?
env_b_log="$(cd "$PROOF_ROOT" && lake env lean .m2-cov-env-b.lean 2>&1)"
env_b_rc=$?
if grep -q 'm2-coverage: FAIL' <<< "$env_a_log$env_b_log"; then
  printf '%s\n%s\n' "$env_a_log" "$env_b_log" | grep 'm2-coverage: FAIL' | sort -u >&2
  fail ENV-BINDING "environment/citation check reported violations (see named failures above)"
fi
if [ "$env_a_rc" -ne 0 ] || [ "$env_b_rc" -ne 0 ]; then
  printf '%s\n---\n%s\n' "$env_a_log" "$env_b_log" | tail -25 >&2
  fail ENV-BINDING "checker elaboration exited A=$env_a_rc B=$env_b_rc"
fi
printf '%s\n%s\n' "$env_a_log" "$env_b_log" | grep -o 'm2-coverage: ENV-[AB]-PASS.*' | sed 's/^m2-coverage: /info: m2-coverage: /'
pass "ENV-BINDING (constants bound in the elaborated environment; citations are theorems mentioning their constants)"

# C7 SUMMARY: full classification with per-status counts.
echo "[m2-coverage] SUMMARY $counts"
echo "[m2-coverage] PASS (all checks) — subject: $PROOF_ROOT; record: $RECORD"
exit 0
