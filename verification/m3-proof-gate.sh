#!/usr/bin/env bash
# M3 proof-required gate (change m3-checker-coverage, task 6.1).
#
# Succeeds the M2 gate (verification/m2-proof-gate.sh stays the floor and
# is never weakened). There is one tier: the proofs/m3 root must build.
# A type error in any of NobleM2, NobleM3, NobleM2.Refinement or the
# generated NobleKernel fails the gate at BUILD; there is no handoff tier
# that could turn a broken proof tree green.
#
# The required-theorem list is the fragment-v1 set: the inventory of
# verification/m3-proof-evidence.md §6, enumerated in this gate's checker A
# (the strict names: soundness, totality and the work-measure bounds, the
# per-rule soundness family, the correspondences, the composed refinement
# theorems, and the generated `noble_kernel.acceptance.check` itself; then
# the evaluation-closed names: the coverage families, the per-word/per-rule
# refinement matrix and the decision-path rows, each carrying the M2
# per-declaration native_decide axioms). A deleted or renamed name anywhere
# in the set fails the gate, as does any axiom outside the name's permitted
# set. The M2 floor gate (verification/m2-proof-gate.sh) stays runnable and
# unweakened over proofs/m2.
#   The axiom policy is the M2 one: {propext, Classical.choice,
#   Quot.sound} strictly, plus per-declaration native_decide axioms for
#   the evaluation-closed theorems; sorryAx and anything else reject.
#
# Beyond the reviewed list, the gate enumerates every theorem the m3
# sources declare, from the elaborated environment: each `.lean` file under
# NobleM2/ and NobleM3/ and the two roots contributes its module, every
# expected module must be imported by the roots (else its theorems are
# elaborated by no checker), and every theorem whose defining module is one
# of them has its transitive axioms audited under the same policy. A new
# theorem or a new module is therefore covered without editing the gate.
# `NobleM2.Refinement` is elaborated by checker B instead (it cannot be
# imported alongside the NobleM2 root), and `NobleM3.CheckV1` is listed as
# an unaudited source: it does not compile and no root imports it.
#
# NO-PROOF-HOLE is a construct scan over the comment- and literal-masked
# sources (verification/lean-source-scan.mjs): `sorry`, `admit`, an
# `axiom` declaration, `debug.skipKernelTC` and `addDecl` all fail, and a
# comment or a doc string that merely mentions one does not.
#
# Usage (from any directory; the pinned Lean first on PATH, plus node for
# the construct scan):
#
#   PATH=/nix/store/kwvw84h9r7w0q40d3cm5xgfz0rpv7yzc-noble-lean-release-4.35.0/bin:$PATH \
#     verification/m3-proof-gate.sh [PROOF_ROOT]
#
# PROOF_ROOT defaults to the repository's proofs/m3. Exit 0 = all checks pass.
set -uo pipefail

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PROOF_ROOT="${M3_PROOF_ROOT:-$REPO/proofs/m3}"
if [ $# -ge 1 ]; then PROOF_ROOT="$1"; fi
PROOF_ROOT="$(cd "$PROOF_ROOT" 2>/dev/null && pwd)" || {
  echo "[m3-gate] FAIL SETUP: proof root not found: $1" >&2; exit 1; }
[ -f "$PROOF_ROOT/lakefile.toml" ] || {
  echo "[m3-gate] FAIL SETUP: $PROOF_ROOT has no lakefile.toml" >&2; exit 1; }
fail() { echo "[m3-gate] FAIL $1: $2" >&2; exit 1; }
pass() { echo "[m3-gate] PASS $1"; }

# Scratch files the checks write next to the proof root; removed on every exit
# path, including a refusal before they are assigned.
cleanup() { rm -f "${GATE_A:-}" "${GATE_B:-}" "${MODULES_A:-}" "${MODULES_B:-}" "${holed:-}"; }
trap cleanup EXIT

lean --version 2>/dev/null | grep -q 'version 4\.35\.0' \
  || fail TOOLCHAIN "lean on PATH is not the pinned 4.35.0 release: $(lean --version 2>&1)"
pass "TOOLCHAIN (lean 4.35.0)"

# The m3 root must build. There is no handoff tier: a type error anywhere in
# these four targets fails the gate here, with the failing diagnostics.
if ! build_log="$(cd "$PROOF_ROOT" && lake build NobleM3 NobleM2 NobleM2.Refinement NobleKernel 2>&1)"; then
  printf '%s\n' "$build_log" | grep -E '^error' | head -20 >&2
  fail BUILD "proofs/m3 does not build (lake build NobleM3 NobleM2 NobleM2.Refinement NobleKernel failed); this gate has no handoff tier"
fi
pass "BUILD (lake build NobleM3 NobleM2 NobleM2.Refinement NobleKernel)"

# NO-PROOF-HOLE: every checked-in proof source, with comments and literal text
# masked, so a doc string that says "sorry" is not a hit and an `admit`, a
# local `axiom`, `debug.skipKernelTC` or `addDecl` is.
SCAN="$REPO/verification/lean-source-scan.mjs"
[ -f "$SCAN" ] || fail NO-PROOF-HOLE "the construct scanner is missing: $SCAN"
# A JS host for the scan: one on PATH, else the reviewed node of
# policy/tool-selection.json. No network and no nix.
NODE_BIN="$(command -v node || true)"
if [ -z "$NODE_BIN" ]; then
  node_output="$(sed -n '/^  "tool_paths": {/,/^  },$/p' "$REPO/policy/tool-selection.json" \
    | sed -n '/^    "node": {/,/^    },$/p' \
    | sed -n 's/^      "output": "\([^"]*\)".*$/\1/p' | head -1)"
  [ -n "$node_output" ] && [ -x "$node_output/bin/node" ] && NODE_BIN="$node_output/bin/node"
fi
[ -n "$NODE_BIN" ] || fail SETUP "no node on PATH and no reviewed node in policy/tool-selection.json"
# (a) `sorry` anywhere in the proof root, the scope the old word grep had,
# comments and literals masked; (b) every construct over the human-written
# sources, where `admit`/`axiom`/`skipKernelTC`/`addDecl` mean what they say
# (the generated NobleKernel has data-model functions named `admit`).
holed="$(mktemp "$PROOF_ROOT/.m3-gate-hole-XXXXXX")"
scan_log="$("$NODE_BIN" "$SCAN" --only sorry "$PROOF_ROOT" 2>&1)" || {
  printf '%s\n' "$scan_log" >&2
  fail NO-PROOF-HOLE "a proof source contains 'sorry'"
}
"$NODE_BIN" "$SCAN" "$PROOF_ROOT/NobleM2" "$PROOF_ROOT/NobleM3" \
  "$PROOF_ROOT/NobleM2.lean" "$PROOF_ROOT/NobleM3.lean" > "$holed" 2>&1 || {
  cat "$holed" >&2
  fail NO-PROOF-HOLE "a proof source contains sorry, admit, an axiom declaration, debug.skipKernelTC or addDecl"
}
pass "NO-PROOF-HOLE ($scan_log; proof sources: $(cat "$holed"))"

# The library entry imports the checked generated functions and exposes
# the extracted checker. A comment is not generator provenance; the separate
# actual-source extraction gates bind generated bytes to Charon/Aeneas runs.
gen_header="$PROOF_ROOT/NobleKernel.lean"
gen_funs="$PROOF_ROOT/NobleKernel/Funs.lean"
[ -s "$gen_header" ] || fail GENERATED-KERNEL "missing or empty $gen_header"
grep -qx 'import NobleKernel.Funs' "$gen_header" \
  || fail GENERATED-KERNEL "$gen_header does not import NobleKernel.Funs"
[ -s "$gen_funs" ] || fail GENERATED-KERNEL "missing or empty $gen_funs"
grep -q '^def acceptance.check' "$gen_funs" \
  || fail GENERATED-KERNEL "generated Funs.lean does not define acceptance.check"
pass "GENERATED-KERNEL (Aeneas entry point, Funs.lean, acceptance.check)"

# The m3 refinement and embedding import the generated module
# itself, so the theorems cannot silently bind a same-named substitute.
for mod in NobleM3/Refine.lean NobleM2/Embed.lean; do
  [ -f "$PROOF_ROOT/$mod" ] || fail REFINEMENT-SUBJECT "$mod is missing"
  grep -qx 'import NobleKernel' "$PROOF_ROOT/$mod" \
    || fail REFINEMENT-SUBJECT "$mod does not import the generated NobleKernel module (substituted subject)"
done
pass "REFINEMENT-SUBJECT (NobleM3/Refine.lean and NobleM2/Embed.lean import NobleKernel)"

# REQUIRED-THEOREMS: the floor and the v1 set, with the M2 axiom policy; the
# refinement family is stated about the generated checker.
GATE_A="$PROOF_ROOT/.m3-gate-a.lean"
GATE_B="$PROOF_ROOT/.m3-gate-b.lean"
MODULES_A="$PROOF_ROOT/.m3-gate-modules-a.txt"
MODULES_B="$PROOF_ROOT/.m3-gate-modules-b.txt"
# The module set the enumeration must cover: every .lean file under NobleM2/
# and NobleM3/, plus the two roots. Checker B elaborates NobleM2.Refinement,
# which cannot be imported alongside the NobleM2 root (duplicate instances),
# so it is that checker's module. NobleM3.CheckV1 does not compile and no root
# imports it; its theorems are therefore UNAUDITED, and the gate prints that
# rather than silently enumerating around it.
{
  printf '%s\n' NobleM2 NobleM3
  for dir in NobleM2 NobleM3; do
    for module_file in "$PROOF_ROOT/$dir"/*.lean; do
      [ -e "$module_file" ] || continue
      module_name="${module_file#"$PROOF_ROOT"/}"
      printf '%s\n' "${module_name%.lean}" | tr '/' '.'
    done
  done
} | grep -vx 'NobleM2.Refinement' | grep -vx 'NobleM3.CheckV1' > "$MODULES_A"
printf '%s\n' 'NobleM2.Refinement' > "$MODULES_B"
[ "$(wc -l < "$MODULES_A")" -gt 2 ] || fail SETUP "no m3 proof modules enumerated under $PROOF_ROOT"
echo "[m3-gate] NON-CLAIM NobleM3.CheckV1 is not imported by any root and does not compile; its theorems are not audited"

cat > "$GATE_A" <<'LEAN_A'
import Lean.Elab.Command
import NobleM2
import NobleM3

open Lean in
run_cmd do
  let strict : List Name := [``propext, ``Classical.choice, ``Quot.sound]
  let evalOk (n : Name) : Bool :=
    strict.contains n || n.toString.contains "_native.native_decide.ax_1_"
  let okAxiom (isStrict : Bool) (a : Name) : Bool :=
    if isStrict then strict.contains a else evalOk a
  -- The v1 theorem contract, exactly the inventory of
  -- verification/m3-proof-evidence.md §6: every listed name must resolve
  -- in the elaborated environment and carry only its permitted axioms.
  -- A deleted or renamed theorem anywhere in the set is a gate failure.
  let words : List String :=
    ["dup", "drop", "swap", "dip", "add", "sub", "mul", "equals", "quote",
     "compose", "run", "reflect", "unit", "pair", "unpair", "inl", "inr",
     "case", "if", "nil", "cons", "list_case", "test_emit"]
  let strictAll : List Name :=
    [``NobleM2.applyScheme_ok, ``NobleM2.applyScheme_ok_resolves,
     ``NobleM2.resolve_resolvesTo, ``NobleM2.followFrom_chain,
     ``NobleM2.resolveOne_position, ``NobleM2.resolveList_positions,
     ``NobleM2.check_soundness, ``NobleM2.foldBody_derives,
     ``NobleM2.foldBody_entry_derives, ``NobleM2.check_total,
     ``NobleM2.check_total_v1, ``NobleM2.foldBody_work_bounded,
     ``NobleM2.foldBody_work_bounded_v1, ``NobleM2.resolve_work_bounded,
     ``NobleM2.followFrom_work_bounded, ``NobleM2.resolveOne_work_bounded,
     ``NobleM2.resolveList_work_bounded, ``NobleM2.validateSchemas_ok_bounded,
     ``NobleM2.word_cost_positive, ``NobleM2.quotationDerives_bound,
     ``NobleM2.derives_cons_inv, ``NobleM2.invocation_word,
     ``NobleM2.rule_empty_sound, ``NobleM2.rule_literal_sound,
     ``NobleM2.rule_sequence_sound, ``NobleM2.rule_quotation_sound,
     ``NobleM2.rule_word_sound,
     ``NobleM2.caseScheme_inv, ``NobleM2.ifScheme_inv,
     ``NobleM2.listCaseScheme_inv, ``NobleM2.dataOkAt_iff,
     ``NobleM2.subsetOf_subset, ``NobleM2.subsetOf_iff,
     ``NobleM2.duplication_shares_one_interface, ``NobleM2.resolvesTo_refl,
     ``NobleM3.Refine.outcomes_via_refinement_v1,
     ``NobleM3.Refine.accepted_via_refinement_v1,
     ``noble_kernel.acceptance.check]
  -- The evaluation-closed theorems may additionally carry the
  -- per-declaration native_decide axiom family (the M2 policy).
  let evalClosed : List Name :=
    [``NobleM2.coverage_positive,
     ``NobleM2.Coverage.accepted, ``NobleM2.Coverage.derives,
     ``NobleM2.Regression.ce_accepted, ``NobleM2.Regression.ce_derives,
     ``NobleM2.WordCoverage.coverage_positive_word,
     ``NobleM2.WordCoverage.coverage_eliminator_exercise,
     ``NobleM2.WordCoverage.ee_accepted, ``NobleM2.WordCoverage.ee_derives,
     ``NobleM2.rule_case_sound, ``NobleM2.rule_if_sound,
     ``NobleM2.rule_listcase_sound, ``NobleM2.fresh_instantiation,
     ``NobleM2.Refinement.table_refines, ``NobleM2.Refinement.apply_refines,
     ``NobleM2.WordRefinement.word_refinements,
     ``NobleM3.Refine.refinement_words,
     ``NobleM3.Refine.refinement_rule_empty, ``NobleM3.Refine.refinement_rule_literal,
     ``NobleM3.Refine.refinement_rule_word, ``NobleM3.Refine.refinement_rule_sequence,
     ``NobleM3.Refine.refinement_rule_quotation, ``NobleM3.Refine.refinement_rule_case,
     ``NobleM3.Refine.refinement_rule_if, ``NobleM3.Refine.refinement_rule_listcase,
     ``NobleM3.Refine.refinement_recursive_dependency_self,
     ``NobleM3.Refine.refinement_recursive_dependency_mutual,
     ``NobleM3.Refine.refinement_dependency_boundary,
     ``NobleM3.Refine.refinement_dependency_exhausted,
     ``NobleM3.Refine.refinement_dependency_before_body,
     ``NobleM3.Refine.refinement_recursive_schema,
     ``NobleM3.Refine.refinement_schema_boundary,
     ``NobleM3.Refine.refinement_schema_exhausted,
     ``NobleM3.Refine.refinement_cyclic_witness_self,
     ``NobleM3.Refine.refinement_cyclic_witness_mutual,
     ``NobleM3.Refine.refinement_resolvable_chain,
     ``NobleM3.Refine.refinement_kind_crossed_reference,
     ``NobleM3.Refine.refinement_duplication_negative,
     ``NobleM3.Refine.refinement_implicit_union_negative,
     ``NobleM3.Refine.eligibility_dup, ``NobleM3.Refine.eligibility_drop,
     ``NobleM3.Refine.eligibility_quote, ``NobleM3.Refine.inclusion_extracted,
     ``NobleM3.Refine.eligibility_iff, ``NobleM3.Refine.inclusion_iff]
    ++ words.map (fun w => (`NobleM2.WordCoverage).mkStr s!"coverage_{w}")
    ++ words.map (fun w => (`NobleM2.WordRefinement).mkStr s!"word_refinement_{w}")
    ++ words.map (fun w => (`NobleM3.Refine).mkStr s!"refinement_word_{w}")
  for (n, isStrict) in
    (strictAll.map (fun n => (n, true))
      ++ evalClosed.map (fun n => (n, false))) do
    match (← getEnv).find? n with
    | none => logError m!"m3-gate: required theorem missing: {n}"
    | some _ =>
      let axs ← Lean.collectAxioms n
      logInfo m!"m3-gate: axioms {n} {axs}"
      let bad := axs.filter (fun a => !(okAxiom isStrict a))
      if !bad.isEmpty then
        logError m!"m3-gate: forbidden axioms on {n}: {bad}"
  -- Enumerated coverage: every theorem the m3 sources declare, not only the
  -- reviewed inventory above. The module list is derived from the .lean files
  -- under NobleM2/ and NobleM3/ plus the roots, so a new theorem or a new
  -- module is audited without editing this gate; a module no checker imports
  -- is an error, because its theorems would be elaborated nowhere.
  let env ← getEnv
  let nameOf (s : String) : Name := s.splitOn "." |>.foldl (fun n p => n.mkStr p) .anonymous
  let payload ← IO.FS.readFile "MODULES_A"
  let expected : Array Name := (payload.splitOn "\n").filterMap (fun line => if line.isEmpty then none else some (nameOf line)) |>.toArray
  for mod in expected do
    unless env.header.moduleNames.contains mod do
      logError m!"m3-gate: module {mod} is not imported, so no checker elaborates its theorems"
  let expectedSet : NameSet := expected.foldl (fun set mod => set.insert mod) {}
  let mut audited := 0
  for (n, ci) in env.constants.toList do
    if !(ci matches .thmInfo _) then continue
    let some idx := env.getModuleIdxFor? n | continue
    if !expectedSet.contains env.header.moduleNames[idx]! then continue
    audited := audited + 1
    let axs ← Lean.collectAxioms n
    let bad := axs.filter (fun a => !(okAxiom false a))
    if !bad.isEmpty then
      logError m!"m3-gate: forbidden axiom on declared theorem {n}: {bad}"
  logInfo m!"m3-gate: audited {audited} theorems declared in {expected.size} m3 modules"
LEAN_A
cat > "$GATE_B" <<'LEAN_B'
import Lean.Elab.Command
import NobleM2.Refinement

open Lean in
run_cmd do
  let gen := ``noble_kernel.acceptance.check
  let strict : List Name := [``propext, ``Classical.choice, ``Quot.sound]
  let evalOk (n : Name) : Bool :=
    strict.contains n || n.toString.contains "_native.native_decide.ax_1_"
  let okAxiom (isStrict : Bool) (a : Name) : Bool :=
    if isStrict then strict.contains a else evalOk a
  let refinementStrict : List Name :=
    [``noble_kernel.acceptance.check,
     ``NobleM2.Refinement.accepted_via_refinement]
  let refinementEval : List Name :=
    [``NobleM2.Refinement.refinement_sequence_and_literals_accept_arithmetic,
     ``NobleM2.Refinement.refinement_quotation_construction_checks_body_and_runs,
     ``NobleM2.Refinement.refinement_limits_nodes_exact,
     ``NobleM2.Refinement.refinement_limits_nodes_over,
     ``NobleM2.Refinement.refinement_limits_depth_deep,
     ``NobleM2.Refinement.refinement_limits_depth_shallow,
     ``NobleM2.Refinement.refinement_limits_work_eight,
     ``NobleM2.Refinement.refinement_limits_work_four,
     ``NobleM2.Refinement.refinement_hidden_emit_rejects_against_empty_bound,
     ``NobleM2.Refinement.refinement_diagnostics_report_order,
     ``NobleM2.Refinement.refinement_diagnostics_truncation,
     ``NobleM2.Refinement.refinement_resource_eligibility_rejects_duplication]
  for (n, isStrict) in (refinementStrict.map (fun n => (n, true))
      ++ refinementEval.map (fun n => (n, false))) do
    match (← getEnv).find? n with
    | none => logError m!"m3-gate: required theorem missing: {n}"
    | some ci =>
      let axs ← Lean.collectAxioms n
      logInfo m!"m3-gate: axioms {n} {axs}"
      let bad := axs.filter (fun a => !(okAxiom isStrict a))
      if !bad.isEmpty then
        logError m!"m3-gate: forbidden axioms on {n}: {bad}"
      if n != gen then
        if ci.type.getUsedConstants.contains gen then
          logInfo m!"m3-gate: subject {n} binds {gen}"
        else
          logError m!"m3-gate: substituted subject: {n} is not stated about the generated {gen}"
  -- Enumerated coverage of this checker's module, as in checker A: every
  -- theorem declared in NobleM2/Refinement.lean is audited, including those
  -- outside the reviewed list above.
  let env ← getEnv
  let nameOf (s : String) : Name := s.splitOn "." |>.foldl (fun n p => n.mkStr p) .anonymous
  let payload ← IO.FS.readFile "MODULES_B"
  let expected : Array Name := (payload.splitOn "\n").filterMap (fun line => if line.isEmpty then none else some (nameOf line)) |>.toArray
  for mod in expected do
    unless env.header.moduleNames.contains mod do
      logError m!"m3-gate: module {mod} is not imported, so no checker elaborates its theorems"
  let expectedSet : NameSet := expected.foldl (fun set mod => set.insert mod) {}
  let mut audited := 0
  for (n, ci) in env.constants.toList do
    if !(ci matches .thmInfo _) then continue
    let some idx := env.getModuleIdxFor? n | continue
    if !expectedSet.contains env.header.moduleNames[idx]! then continue
    audited := audited + 1
    let axs ← Lean.collectAxioms n
    let bad := axs.filter (fun a => !(okAxiom false a))
    if !bad.isEmpty then
      logError m!"m3-gate: forbidden axiom on declared theorem {n}: {bad}"
  logInfo m!"m3-gate: audited {audited} theorems declared in {expected.size} checker-B modules"
LEAN_B
sed -e "s|MODULES_A|$MODULES_A|" "$GATE_A" > "$GATE_A.resolved" && mv "$GATE_A.resolved" "$GATE_A"
sed -e "s|MODULES_B|$MODULES_B|" "$GATE_B" > "$GATE_B.resolved" && mv "$GATE_B.resolved" "$GATE_B"
axioms_log="$(cd "$PROOF_ROOT" && lake env lean .m3-gate-a.lean 2>&1)"
axioms_rc=$?
if [ "$axioms_rc" -ne 0 ]; then
  printf '%s\n' "$axioms_log" | tail -30 >&2
  fail REQUIRED-THEOREMS "reference-model / v1 theorem inventory check exited $axioms_rc"
fi
printf '%s\n' "$axioms_log"
subject_log="$(cd "$PROOF_ROOT" && lake env lean .m3-gate-b.lean 2>&1)"
subject_rc=$?
if [ "$subject_rc" -ne 0 ]; then
  printf '%s\n' "$subject_log" | tail -30 >&2
  fail REQUIRED-THEOREMS "refinement theorem inventory / subject binding exited $subject_rc"
fi
printf '%s\n' "$subject_log"
pass "REQUIRED-THEOREMS (the v1 theorem inventory of m3-proof-evidence.md §6, axiom policy, subject binding, enumerated coverage of every declared theorem)"

# No unexplained external model (template axioms reject).
axiom_hits="$(grep -nE '^[[:space:]]*axiom[[:space:]]' \
  "$PROOF_ROOT/NobleKernel/FunsExternal.lean" "$PROOF_ROOT/NobleKernel/TypesExternal.lean" || true)"
[ -z "$axiom_hits" ] || {
  printf '%s\n' "$axiom_hits" >&2
  fail EXTERNAL-MODELS "external model left as a template axiom"
}
pass "EXTERNAL-MODELS (no template axioms in FunsExternal/TypesExternal)"

echo "[m3-gate] PASS (all checks, no handoff tier) — proof root: $PROOF_ROOT"
exit 0
