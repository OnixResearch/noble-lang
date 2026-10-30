/-
  Kernel-checked reference for the restricted SGD update published for issue #58
  (`.cairn/changes/optimizer-kernel-experiment/design.md`, "Minimal candidate IR").

  For stored lanes in order the update is `p_i' = Binary32.sub p_i (Binary32.mul eta g_i)`: the
  product rounds once, then the difference rounds once, with no fused multiply-add. The pilot admission
  rule refuses a nonfinite input (`p_i`, `g_i`, `eta`) and a nonfinite output lane; signed zeros and
  subnormals are finite and admissible. Refusals carry the published class and a lane-indexed
  location.

  The published table names each refusal's class and location but not the precedence between
  classes. This model checks `n`, the output-lane budget and the gradient shape before any value, and
  the inputs in envelope order (`p`, then `g`, then `eta`) before the outputs; that order is this
  model's reading, not a published contract. Everything here is a model of the reference: no Rust,
  Wasm, candidate checker or training correspondence is claimed.
-/
import NobleFoundation.Binary32
import NobleFoundation.Binary32Mutations

namespace NobleFoundation.Binary32.Sgd

open NobleFoundation.Binary32
open NobleFoundation.Binary32.Mutations

set_option exponentiation.threshold 4096
set_option maxRecDepth 100000

/-- A finite binary32 value is neither an infinity nor a NaN. Zeros of either sign and subnormals are
finite. -/
def isFinite (b : Bits) : Bool :=
  match classify b with
  | .infinity => false
  | .notANumber => false
  | _ => true

/-- One published lane: the product rounds once, then the difference rounds once. -/
def lane (eta p g : Bits) : Bits := sub p (mul eta g)

/-- The reference transition over stored lanes, in stored order. -/
def update (eta : Bits) (params grads : List Bits) : List Bits :=
  List.zipWith (lane eta) params grads

/-- The published refusal classes this reference can produce. -/
inductive RefusalClass where
  | malformed
  | unsupported
  | exhausted
  deriving DecidableEq, Repr

/-- The published refusal locations this reference can produce: `$.n`, `$.inputs.eta`,
`$.inputs.p[i]`, `$.inputs.g[i]`, the gradient shape, `$.outputs` and `$.outputs.parameters[i]`. -/
inductive Location where
  | n
  | eta
  | param (lane : Nat)
  | grad (lane : Nat)
  | gradShape
  | outputs
  | outputParam (lane : Nat)
  deriving DecidableEq, Repr

/-- The result of one admitted-or-refused reference step. -/
inductive Outcome where
  | accepted (params : List Bits)
  | refused (refusal : RefusalClass) (location : Location)
  deriving DecidableEq, Repr

/-- The index of the first nonfinite value, counting from `start`. -/
def firstNonfinite : Nat → List Bits → Option Nat
  | _, [] => none
  | start, value :: rest => if isFinite value then firstNonfinite (start + 1) rest else some start

/-- The pilot step: the declared lane count `n` must satisfy `1 ≤ n ≤ 64`, the gradient vector must
have `n` lanes, every input must be finite, and so must every output lane. -/
def checkStep (eta : Bits) (params grads : List Bits) : Outcome :=
  if params.length = 0 then .refused .malformed .n
  else if 64 < params.length then .refused .exhausted .outputs
  else if grads.length ≠ params.length then .refused .unsupported .gradShape
  else
    match firstNonfinite 0 params with
    | some index => .refused .unsupported (.param index)
    | none =>
        match firstNonfinite 0 grads with
        | some index => .refused .unsupported (.grad index)
        | none =>
            if isFinite eta then
              match firstNonfinite 0 (update eta params grads) with
              | some index => .refused .unsupported (.outputParam index)
              | none => .accepted (update eta params grads)
            else .refused .unsupported .eta

/-! ## Universal theorems -/

/-- The update preserves the lane count whenever the two input vectors have the same length. -/
theorem update_preserves_shape (eta : Bits) (params grads : List Bits)
    (sameShape : grads.length = params.length) :
    (update eta params grads).length = params.length := by
  simp [update, List.length_zipWith, sameShape]

/-- **Per-lane update**: output lane `i` is `sub p_i (mul eta g_i)` and depends only on `p_i`, `g_i`
and `eta`. -/
theorem update_lane (eta : Bits) (params grads : List Bits) (index : Nat) (p g : Bits)
    (paramAt : params[index]? = some p) (gradAt : grads[index]? = some g) :
    (update eta params grads)[index]? = some (sub p (mul eta g)) := by
  simp [update, List.getElem?_zipWith, paramAt, gradAt, lane]

/-- `firstNonfinite` finds nothing exactly when every value is finite. -/
theorem firstNonfinite_eq_none (start : Nat) (values : List Bits) :
    firstNonfinite start values = none ↔ ∀ value ∈ values, isFinite value = true := by
  induction values generalizing start with
  | nil => simp [firstNonfinite]
  | cons head tail ih =>
      simp only [firstNonfinite]
      split
      · rename_i finite
        rw [ih]
        simp [finite]
      · rename_i notFinite
        simp [notFinite]

/-- A reported lane really holds a nonfinite value: the location is not a guess. -/
theorem firstNonfinite_reports_a_nonfinite_lane (start : Nat) (values : List Bits) (index : Nat)
    (found : firstNonfinite start values = some index) :
    start ≤ index ∧ ∃ value, values[index - start]? = some value ∧ isFinite value = false := by
  induction values generalizing start with
  | nil => simp [firstNonfinite] at found
  | cons head tail ih =>
      simp only [firstNonfinite] at found
      split at found
      · obtain ⟨later, value, at_, notFinite⟩ := ih (start + 1) found
        refine ⟨by omega, value, ?_, notFinite⟩
        have shift : index - start = (index - (start + 1)) + 1 := by omega
        rw [shift, List.getElem?_cons_succ]
        exact at_
      · rename_i notFinite
        simp only [Option.some.injEq] at found
        subst found
        refine ⟨Nat.le_refl _, head, by simp, ?_⟩
        simpa using notFinite

/-- **What an accepted step means**: its output is exactly the reference update, it keeps the lane
count, and every input and output lane is finite. -/
theorem checkStep_accepted (eta : Bits) (params grads out : List Bits)
    (accepted : checkStep eta params grads = .accepted out) :
    out = update eta params grads ∧ out.length = params.length ∧ isFinite eta = true ∧
      (∀ value ∈ params, isFinite value = true) ∧ (∀ value ∈ grads, isFinite value = true) ∧
      (∀ value ∈ out, isFinite value = true) := by
  unfold checkStep at accepted
  split at accepted
  · simp at accepted
  split at accepted
  · simp at accepted
  split at accepted
  · simp at accepted
  rename_i _ _ sameShape
  have shape : grads.length = params.length := by simpa using sameShape
  split at accepted
  · simp at accepted
  rename_i paramsNone
  split at accepted
  · simp at accepted
  rename_i gradsNone
  split at accepted
  · rename_i etaFinite
    split at accepted
    · simp at accepted
    · rename_i outNone
      simp only [Outcome.accepted.injEq] at accepted
      subst accepted
      refine ⟨rfl, update_preserves_shape eta params grads shape, etaFinite,
        (firstNonfinite_eq_none 0 params).mp paramsNone,
        (firstNonfinite_eq_none 0 grads).mp gradsNone,
        (firstNonfinite_eq_none 0 _).mp outNone⟩
  · simp at accepted

/-- **Published nonfinite rule, inputs**: a step with any nonfinite `eta`, parameter or gradient is
never accepted. -/
theorem nonfinite_input_is_never_accepted (eta : Bits) (params grads out : List Bits)
    (nonfinite : isFinite eta = false ∨ (∃ value ∈ params, isFinite value = false) ∨
      (∃ value ∈ grads, isFinite value = false)) :
    checkStep eta params grads ≠ .accepted out := by
  intro accepted
  obtain ⟨_, _, etaFinite, paramsFinite, gradsFinite, _⟩ :=
    checkStep_accepted eta params grads out accepted
  rcases nonfinite with etaBad | ⟨value, member, bad⟩ | ⟨value, member, bad⟩
  · simp [etaFinite] at etaBad
  · simp [paramsFinite value member] at bad
  · simp [gradsFinite value member] at bad

/-- **Published nonfinite rule, parameter location**: in a well-shaped step, the first nonfinite
parameter is refused as `unsupported` at its own lane. -/
theorem nonfinite_parameter_is_refused_at_its_lane (eta : Bits) (params grads : List Bits)
    (index : Nat) (nonEmpty : params.length ≠ 0) (withinBudget : params.length ≤ 64)
    (sameShape : grads.length = params.length) (found : firstNonfinite 0 params = some index) :
    checkStep eta params grads = .refused .unsupported (.param index) := by
  unfold checkStep
  rw [if_neg nonEmpty, if_neg (by omega), if_neg (by simp [sameShape]), found]

/-! ## Kernel-checked expectations -/

/-- A normal step: `eta = 0.5`, `p = [1, 2]`, `g = [1, -2]` gives `[0.5, 3]`. -/
theorem normal_step_expectation :
    checkStep 0x3F000000 [0x3F800000, 0x40000000] [0x3F800000, 0xC0000000] =
      .accepted [0x3F000000, 0x40400000] := by
  decide

/-- Signed zeros follow the #38 model: `-0 - 0.5·(+0)` is `-0`, and `+0 - 0.5·(+0)` is `+0`; both
steps are accepted because zeros are finite. -/
theorem signed_zero_expectation :
    checkStep 0x3F000000 [0x80000000, 0x00000000] [0x00000000, 0x00000000] =
      .accepted [0x80000000, 0x00000000] := by
  decide

/-- A subnormal result is kept, not flushed: `2^-126 - 0.5·2^-126 = 2^-127` (`0x00400000`). -/
theorem subnormal_result_expectation :
    checkStep 0x3F000000 [0x00800000] [0x00800000] = .accepted [0x00400000] := by
  decide +kernel

/-- A boundary-rounding tie: `1 - 1·(-2^-24)` is the exact halfway point `1 + 2^-24`, and ties to even
keep `1.0`. -/
theorem boundary_tie_expectation :
    checkStep 0x3F800000 [0x3F800000] [0xB3800000] = .accepted [0x3F800000] := by
  decide

/-- An overflowing lane is refused at its output location: `max - 1·(-max)` rounds to infinity. -/
theorem overflow_lane_is_refused :
    checkStep 0x3F800000 [0x3F800000, 0x7F7FFFFF] [0x3F800000, 0xFF7FFFFF] =
      .refused .unsupported (.outputParam 1) := by
  decide

/-- Nonfinite inputs are refused at their own locations: a NaN parameter at lane 1, an infinite
gradient at lane 0, and an infinite `eta`. -/
theorem nonfinite_inputs_are_refused_at_their_paths :
    checkStep 0x3F000000 [0x3F800000, 0x7FC00000] [0x3F800000, 0x3F800000] =
        .refused .unsupported (.param 1) ∧
      checkStep 0x3F000000 [0x3F800000] [0x7F800000] = .refused .unsupported (.grad 0) ∧
      checkStep 0x7F800000 [0x3F800000] [0x3F800000] = .refused .unsupported .eta := by
  decide

/-- Shape and budget refusals: an empty vector is malformed at `n`, mismatched gradient length is
unsupported at the gradient shape, and 65 lanes exceed the output budget. -/
theorem shape_and_budget_refusals :
    checkStep 0x3F000000 [] [] = .refused .malformed .n ∧
      checkStep 0x3F000000 [0x3F800000] [0x3F800000, 0x3F800000] =
        .refused .unsupported .gradShape ∧
      checkStep 0x3F000000 (List.replicate 65 0x3F800000) (List.replicate 65 0x3F800000) =
        .refused .exhausted .outputs := by
  decide

/-- **Two roundings, not one**: on this lane the reference (`eta = 1 + 2^-23`, `p = 2^-24`,
`g = -(1 + 2^-23)`) differs from a fused multiply-add that rounds `p - eta·g` once
(`Mutations.contractedSum`), so a contracted implementation is detected. -/
theorem fused_update_is_detected :
    lane 0x3F800001 0x33800000 0xBF800001 ≠ contractedSum 0x3F800001 0x3F800001 0x33800000 := by
  decide

end NobleFoundation.Binary32.Sgd
