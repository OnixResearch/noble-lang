/-
Strict-kernel certified graph pairs (issue #56, acceptance item 3).

`SymbolicPairCorpus` decides the fixed `noble-ngraph` pairs on declared universes only: an
agreeing pair agrees on its candidates, and nothing beyond them is claimed there. This module
certifies the three agreeing pairs for **every** input. Each statement is `∀ inputs, agreesOn pair
inputs` over the model's own graph semantics, which wraps `I64` arithmetic at `2^64` (`BitVec 64`) and
compares with `Bool` equality. The proofs unfold the model and close with `bv_omega`, whose proof terms
the kernel checks; no native evaluation is involved, and the foundation lane audits the axioms.

The four refuted pairs cannot be certified: each `*_not_certifiable` derives the negation from the
kernel-checked refuting witness of the corpus, so a certification claim for them is refused rather
than vacuous.

These are graph-model theorems about the checked graphs the corpus states. The one trusted step is
the generator's reading of checked `noble-ngraph` nodes as model nodes, as the corpus records. No
encoder (`extract/symbolic.rs`), formula, solver, emitted Wasm or Rust evaluator is a subject here.
-/
import NobleFoundation.SymbolicPairCorpus

namespace NobleFoundation.SymbolicSolver.Corpus

open NobleFoundation.PackedSchedule NobleFoundation.SymbolicPair NobleFoundation.SymbolicSolver

/-- **`(x + x) + x` agrees with `x · 3` on every input.** -/
theorem sharedInput_certified (inputs : Nat → BitVec 64) : agreesOn sharedInput inputs := by
  intro out hout
  simp [sharedInput] at hout
  subst hout
  simp [sharedInput, value, valuesUpTo, evalStep, NobleFoundation.FirstProjection.ofList]
  bv_omega

/-- **`x · 2^63 · 2` agrees with `x · 0 · 1` on every input**, because arithmetic wraps at `2^64`. -/
theorem wrapOnlyEquivalent_certified (inputs : Nat → BitVec 64) :
    agreesOn wrapOnlyEquivalent inputs := by
  intro out hout
  simp [wrapOnlyEquivalent] at hout
  subst hout
  simp [wrapOnlyEquivalent, value, valuesUpTo, evalStep, NobleFoundation.FirstProjection.ofList]
  bv_omega

/-- **`x + 1 = 0` agrees with `x + 0 = -1` on every input**, a `Bool` output. -/
theorem boolOutput_certified (inputs : Nat → BitVec 64) : agreesOn boolOutput inputs := by
  intro out hout
  simp [boolOutput] at hout
  subst hout
  simp [boolOutput, value, valuesUpTo, evalStep, NobleFoundation.FirstProjection.ofList]
  rw [Bool.eq_iff_iff]
  simp only [beq_iff_eq]
  constructor <;> intro h <;> bv_omega

/-- A pair with a refuting input has no certificate. -/
theorem not_certifiable_of_refuted (pair : Pair) (witness : Nat → BitVec 64)
    (hrefuted : refuted pair witness = true) : ¬ ∀ inputs, agreesOn pair inputs := by
  intro hall
  have hfalse := (not_refuted_iff_agrees pair witness).mpr (hall witness)
  rw [hrefuted] at hfalse
  exact Bool.noConfusion hfalse

/-- Control: `x - y` against `y - x` has no certificate. -/
theorem subtractionOrder_not_certifiable : ¬ ∀ inputs, agreesOn subtractionOrder inputs :=
  not_certifiable_of_refuted subtractionOrder _ subtractionOrder_witness_refutes

/-- Control: `x · 2^63` against `x · 0` has no certificate. -/
theorem wrapBoundary_not_certifiable : ¬ ∀ inputs, agreesOn wrapBoundary inputs :=
  not_certifiable_of_refuted wrapBoundary _ wrapBoundary_witness_refutes

/-- Control: the repeated-outputs pair has no certificate. -/
theorem repeatedOutputs_not_certifiable : ¬ ∀ inputs, agreesOn repeatedOutputs inputs :=
  not_certifiable_of_refuted repeatedOutputs _ repeatedOutputs_witness_refutes

/-- Control: `0 - x` against `x + 0` has no certificate. -/
theorem negation_not_certifiable : ¬ ∀ inputs, agreesOn negation inputs :=
  not_certifiable_of_refuted negation _ negation_witness_refutes

end NobleFoundation.SymbolicSolver.Corpus
