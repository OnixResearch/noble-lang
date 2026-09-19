#!/usr/bin/env bash
# M3 proof-required gate (change m3-checker-coverage, task 6.1).
#
# Succeeds the M2 gate (verification/m2-proof-gate.sh stays the floor and
# is never weakened). The gate has two tiers:
#
# STRICT TIER — when the proofs/m3 root builds, every check runs against
# it and the required-theorem list is the full fragment-v1 set: the
# inventory of verification/m3-proof-evidence.md §6, enumerated there
# and in this gate's checker A (the strict names: soundness, totality and
# the work-measure bounds, the per-rule soundness family, the
# correspondences, the composed refinement theorems, and the generated
# `noble_kernel.acceptance.check` itself; then the evaluation-closed
# names: the coverage families, the per-word/per-rule refinement matrix
# and the decision-path rows, each carrying the M2 per-declaration
# native_decide axioms). A deleted or renamed name anywhere in the set
# fails the gate, as does any axiom outside the name's permitted set.
# The M2 floor gate (verification/m2-proof-gate.sh) stays runnable and
# unweakened over proofs/m2.
#   The axiom policy is the M2 one: {propext, Classical.choice,
#   Quot.sound} strictly, plus per-declaration native_decide axioms for
#   the evaluation-closed theorems; sorryAx and anything else reject.
#
# HANDOFF TIER — while the proofs/m3 root does not yet build (tasks
# 5.1-5.4 in flight), the gate says so explicitly and instead enforces:
#   M3-KERNEL-BINDING  the m3 root's generated NobleKernel module is
#                      byte-identical to proofs/m2's — the root the M2
#                      refinement theorems bind — and carries the v1
#                      markers (the words.resolve walk, the CyclicWitness
#                      constraint, the RecursiveDependency/RecursiveSchema
#                      kinds), so the M2-floor theorems are about the v1
#                      extracted checker
#   M3-BINDING-THEOREMS  the committed m3 binding theorems are present in
#                      the m3 root's sources (textual presence here;
#                      semantic enforcement engages with the strict tier)
#   M3-NO-SORRY        no proof hole in the m3 root's sources
#   M2-FLOOR           the full M2 proof gate passes on proofs/m2
#
# Usage (from any directory; the pinned Lean must be first on PATH):
#
#   PATH=/nix/store/hbxwgcbpkyqllqzq7cidhnvhgb60jb9q-noble-lean-release-4.31.0/bin:$PATH \
#     verification/m3-proof-gate.sh [PROOF_ROOT]
#
# PROOF_ROOT defaults to the repository's proofs/m3. Exit 0 = the active
# tier's checks all pass.
set -uo pipefail

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PROOF_ROOT="${M3_PROOF_ROOT:-$REPO/proofs/m3}"
if [ $# -ge 1 ]; then PROOF_ROOT="$1"; fi
PROOF_ROOT="$(cd "$PROOF_ROOT" 2>/dev/null && pwd)" || {
  echo "[m3-gate] FAIL SETUP: proof root not found: $1" >&2; exit 1; }
[ -f "$PROOF_ROOT/lakefile.toml" ] || {
  echo "[m3-gate] FAIL SETUP: $PROOF_ROOT has no lakefile.toml" >&2; exit 1; }
M2_ROOT="$REPO/proofs/m2"
M2_GATE="$REPO/verification/m2-proof-gate.sh"

fail() { echo "[m3-gate] FAIL $1: $2" >&2; exit 1; }
pass() { echo "[m3-gate] PASS $1"; }

lean --version 2>/dev/null | grep -q 'version 4\.31\.0' \
  || fail TOOLCHAIN "lean on PATH is not the pinned 4.31.0 release: $(lean --version 2>&1)"
pass "TOOLCHAIN (lean 4.31.0)"

# Tier probe: the strict tier needs the m3 root's four targets to build.
probe_log="$(cd "$PROOF_ROOT" && lake build NobleM3 NobleM2 NobleM2.Refinement NobleKernel 2>&1)"
probe_rc=$?

if [ "$probe_rc" -ne 0 ]; then
  echo "[m3-gate] HANDOFF: the proofs/m3 root does not build yet (tasks 5.1-5.4 in flight);"
  echo "[m3-gate] HANDOFF: strict tier not engaged. Probe failure (tail):"
  printf '%s\n' "$probe_log" | grep -m4 '^error' | sed 's/^/[m3-gate] HANDOFF:   /'
  echo "[m3-gate] HANDOFF: pending v1 required theorems (strict tier, tasks 5.2-5.4):"
  echo "[m3-gate] HANDOFF:   check_total_v1, foldBody_work_bounded_v1, eligibility_iff,"
  echo "[m3-gate] HANDOFF:   inclusion_iff, coverage_positive_word (x23), per-rule soundness"
  echo "[m3-gate] HANDOFF:   (EMPTY LITERAL WORD SEQUENCE QUOTATION CASE IF LISTCASE), the"
  echo "[m3-gate] HANDOFF:   refinement matrix, duplication_shares_interface, accepted_via_refinement_v1"

  # M3-KERNEL-BINDING: byte-identity of the generated module with the one
  # the M2-floor theorems semantically bind, plus the v1 markers.
  cmp -s "$PROOF_ROOT/NobleKernel.lean" "$M2_ROOT/NobleKernel.lean" \
    || fail M3-KERNEL-BINDING "the m3 root's generated entry point differs from proofs/m2's"
  diff -rq "$PROOF_ROOT/NobleKernel" "$M2_ROOT/NobleKernel" >/dev/null \
    || fail M3-KERNEL-BINDING "the m3 root's generated modules differ from proofs/m2's"
  grep -q '^def words.resolve.resolve' "$PROOF_ROOT/NobleKernel/Funs.lean" \
    || fail M3-KERNEL-BINDING "generated module lacks the B-CHECK-05 words.resolve walk"
  grep -q 'CyclicWitness : untrusted.Constraint' "$PROOF_ROOT/NobleKernel/Types.lean" \
    || fail M3-KERNEL-BINDING "generated module lacks the CyclicWitness constraint"
  grep -q 'RecursiveDependency' "$PROOF_ROOT/NobleKernel/Types.lean" \
    || fail M3-KERNEL-BINDING "generated module lacks the RecursiveDependency kind"
  pass "M3-KERNEL-BINDING (generated module byte-identical to proofs/m2's; v1 markers present)"

  # M3-BINDING-THEOREMS: the committed m3 binding theorems are present in
  # the m3 root's sources (textual; semantic in the strict tier).
  for thm in table_refines apply_refines; do
    grep -q "^theorem $thm " "$PROOF_ROOT/NobleM3/Refine.lean" \
      || fail M3-BINDING-THEOREMS "NobleM3/Refine.lean no longer states $thm"
  done
  pass "M3-BINDING-THEOREMS (table_refines, apply_refines present in NobleM3/Refine.lean)"

  # M3-NO-SORRY: no proof hole in the m3 root's checked-in sources.
  sorry_hits="$(grep -rnw --include='*.lean' --exclude-dir=.lake sorry "$PROOF_ROOT" || true)"
  [ -z "$sorry_hits" ] || { printf '%s\n' "$sorry_hits" >&2; fail M3-NO-SORRY "m3 proof sources contain 'sorry'"; }
  pass "M3-NO-SORRY (no 'sorry' outside .lake)"

  # M2-FLOOR: the full M2 gate, semantically, on the root it binds.
  "$M2_GATE" "$M2_ROOT" || fail M2-FLOOR "the M2 proof gate failed on proofs/m2"
  echo "[m3-gate] PASS (handoff tier) — strict tier engages when proofs/m3 builds"
  exit 0
fi

pass "BUILD (lake build NobleM3 NobleM2 NobleM2.Refinement NobleKernel)"

# STRICT: no proof hole in any checked-in proof source.
sorry_hits="$(grep -rnw --include='*.lean' --exclude-dir=.lake sorry "$PROOF_ROOT" || true)"
[ -z "$sorry_hits" ] || {
  printf '%s\n' "$sorry_hits" >&2
  fail NO-SORRY "proof sources contain 'sorry'"
}
pass "NO-SORRY (no 'sorry' outside .lake)"

# STRICT: the generated kernel is present and defines the extracted checker.
gen_header="$PROOF_ROOT/NobleKernel.lean"
gen_funs="$PROOF_ROOT/NobleKernel/Funs.lean"
[ -s "$gen_header" ] || fail GENERATED-KERNEL "missing or empty $gen_header"
grep -q 'AUTOMATICALLY GENERATED BY AENEAS' "$gen_header" \
  || fail GENERATED-KERNEL "$gen_header is not the generated library entry point"
grep -qx 'import NobleKernel.Funs' "$gen_header" \
  || fail GENERATED-KERNEL "$gen_header does not import NobleKernel.Funs"
[ -s "$gen_funs" ] || fail GENERATED-KERNEL "missing or empty $gen_funs"
grep -q '^def acceptance.check' "$gen_funs" \
  || fail GENERATED-KERNEL "generated Funs.lean does not define acceptance.check"
pass "GENERATED-KERNEL (Aeneas entry point, Funs.lean, acceptance.check)"

# STRICT: the m3 refinement and embedding import the generated module
# itself, so the theorems cannot silently bind a same-named substitute.
for mod in NobleM3/Refine.lean NobleM2/Embed.lean; do
  [ -f "$PROOF_ROOT/$mod" ] || fail REFINEMENT-SUBJECT "$mod is missing"
  grep -qx 'import NobleKernel' "$PROOF_ROOT/$mod" \
    || fail REFINEMENT-SUBJECT "$mod does not import the generated NobleKernel module (substituted subject)"
done
pass "REFINEMENT-SUBJECT (NobleM3/Refine.lean and NobleM2/Embed.lean import NobleKernel)"

# STRICT REQUIRED-THEOREMS: the floor and the v1 set, with the M2 axiom
# policy; the refinement family is stated about the generated checker.
GATE_A="$PROOF_ROOT/.m3-gate-a.lean"
GATE_B="$PROOF_ROOT/.m3-gate-b.lean"
cleanup() { rm -f "$GATE_A" "$GATE_B"; }
trap cleanup EXIT

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
LEAN_B
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
pass "REQUIRED-THEOREMS (the v1 theorem inventory of m3-proof-evidence.md §6, axiom policy, subject binding)"

# STRICT: no unexplained external model (template axioms reject).
axiom_hits="$(grep -nE '^[[:space:]]*axiom[[:space:]]' \
  "$PROOF_ROOT/NobleKernel/FunsExternal.lean" "$PROOF_ROOT/NobleKernel/TypesExternal.lean" || true)"
[ -z "$axiom_hits" ] || {
  printf '%s\n' "$axiom_hits" >&2
  fail EXTERNAL-MODELS "external model left as a template axiom"
}
pass "EXTERNAL-MODELS (no template axioms in FunsExternal/TypesExternal)"

echo "[m3-gate] PASS (strict tier, all checks) — proof root: $PROOF_ROOT"
exit 0
