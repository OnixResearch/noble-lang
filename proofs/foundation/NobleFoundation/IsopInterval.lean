import NobleFoundation.Isop

namespace NobleFoundation.IsopInterval

open NobleFoundation.Isop

/-- The interval's lower endpoint is empty on the finite valuation space. -/
def allFalse (n : Nat) (f : BFun n) : Bool :=
  (valuations n).all fun row => !(f row)

/-- The interval's upper endpoint is universal on the finite valuation space. -/
def allTrue (n : Nat) (f : BFun n) : Bool :=
  (valuations n).all f

/-- The Minato–Morreale ISOP recurrence, interpreted as the characteristic
function of its cube cover. For split variable `x`, `R0` covers
`L0 \ U1`, `R1` covers `L1 \ U0`, and `R2` covers the remaining onset inside
`U0 ∩ U1`. Its finite truth-table interpretation makes `Bdd(Ri)` explicit. -/
def isop : (n : Nat) → BFun n → BFun n → BFun n
  | 0, lower, _ => lower
  | n + 1, lower, upper =>
      if allFalse (n + 1) lower then fun _ => false
      else if allTrue (n + 1) upper then fun _ => true
      else
        let lower0 := cofactor lower false
        let lower1 := cofactor lower true
        let upper0 := cofactor upper false
        let upper1 := cofactor upper true
        let lower0Only := fun row => lower0 row && !(upper1 row)
        let lower1Only := fun row => lower1 row && !(upper0 row)
        let r0 := isop n lower0Only upper0
        let r1 := isop n lower1Only upper1
        let lower2 := fun row => (lower0 row && !(r0 row)) || (lower1 row && !(r1 row))
        let upper2 := fun row => upper0 row && upper1 row
        let r2 := isop n lower2 upper2
        fun row =>
          match row with
          | .cons value tail => (if value then r1 tail else r0 tail) || r2 tail

/-- Every row is in the finite valuation list. -/
private theorem allFalse_row (n : Nat) (f : BFun n) (h : allFalse n f = true)
    (row : Valuation n) : f row = false := by
  have hall := (List.all_eq_true.mp h) row (valuation_mem n row)
  simpa [allFalse] using hall

private theorem allTrue_row (n : Nat) (f : BFun n) (h : allTrue n f = true)
    (row : Valuation n) : f row = true := by
  exact (List.all_eq_true.mp h) row (valuation_mem n row)

/-- The interval recurrence always returns a function between its endpoints. -/
theorem isop_bounds (n : Nat) (lower upper : BFun n)
    (hInterval : pointwiseLe lower upper) :
    pointwiseLe lower (isop n lower upper) ∧ pointwiseLe (isop n lower upper) upper := by
  induction n with
  | zero =>
      constructor
      · intro row hLower
        exact hLower
      · intro row hResult
        exact hInterval row hResult
  | succ n ih =>
      cases hZero : allFalse (n + 1) lower with
      | true =>
          have hLowerFalse := allFalse_row (n + 1) lower hZero
          constructor
          · intro row hLower
            rw [hLowerFalse row] at hLower
            cases hLower
          · intro row hResult
            simp [isop, hZero] at hResult
      | false =>
          cases hOne : allTrue (n + 1) upper with
          | true =>
              have hUpperTrue := allTrue_row (n + 1) upper hOne
              constructor
              · intro row hLower
                simp [isop, hZero, hOne]
              · intro row hResult
                exact hUpperTrue row
          | false =>
              let lower0 : BFun n := cofactor lower false
              let lower1 : BFun n := cofactor lower true
              let upper0 : BFun n := cofactor upper false
              let upper1 : BFun n := cofactor upper true
              let lower0Only : BFun n := fun row => lower0 row && !(upper1 row)
              let lower1Only : BFun n := fun row => lower1 row && !(upper0 row)
              let r0 : BFun n := isop n lower0Only upper0
              let r1 : BFun n := isop n lower1Only upper1
              let lower2 : BFun n := fun row =>
                (lower0 row && !(r0 row)) || (lower1 row && !(r1 row))
              let upper2 : BFun n := fun row => upper0 row && upper1 row
              let r2 : BFun n := isop n lower2 upper2
              have h0Interval : pointwiseLe lower0Only upper0 := by
                intro row hLower
                have hparts : lower (.cons false row) = true ∧ upper (.cons true row) = false := by
                  simpa [lower0Only, lower0, upper1, cofactor] using hLower
                have hbound := hInterval (.cons false row) hparts.1
                simpa [upper0, cofactor] using hbound
              have h1Interval : pointwiseLe lower1Only upper1 := by
                intro row hLower
                have hparts : lower (.cons true row) = true ∧ upper (.cons false row) = false := by
                  simpa [lower1Only, lower1, upper0, cofactor] using hLower
                have hbound := hInterval (.cons true row) hparts.1
                simpa [upper1, cofactor] using hbound
              have h0Bounds := ih lower0Only upper0 h0Interval
              have h1Bounds := ih lower1Only upper1 h1Interval
              have h2Interval : pointwiseLe lower2 upper2 := by
                intro row hLower
                have hLower0 : lower0 row = true → upper0 row = true := by
                  intro h
                  exact hInterval (.cons false row) (by simpa [lower0, cofactor] using h)
                have hLower1 : lower1 row = true → upper1 row = true := by
                  intro h
                  exact hInterval (.cons true row) (by simpa [lower1, cofactor] using h)
                have hR0 : lower0Only row = true → r0 row = true := h0Bounds.1 row
                have hR1 : lower1Only row = true → r1 row = true := h1Bounds.1 row
                have hParts :
                    (lower0 row && !(r0 row) = true) ∨
                      (lower1 row && !(r1 row) = true) := by
                  simpa [lower2] using hLower
                rcases hParts with h0 | h1
                · have h0Pair : lower0 row = true ∧ !(r0 row) = true := by
                    simpa [Bool.and_eq_true] using h0
                  have hLower0Value := h0Pair.1
                  have hNotR0 := h0Pair.2
                  have hR0False : r0 row = false := by
                    cases hr0 : r0 row <;> simp_all
                  cases hUpper1Value : upper1 row with
                  | true =>
                      have hUpper0Value := hLower0 hLower0Value
                      simp [upper2, hUpper0Value, hUpper1Value]
                  | false =>
                      have hOnly : lower0Only row = true := by
                        simp [lower0Only, hLower0Value, hUpper1Value]
                      have hContradiction := hR0 hOnly
                      rw [hR0False] at hContradiction
                      cases hContradiction
                · have h1Pair : lower1 row = true ∧ !(r1 row) = true := by
                    simpa [Bool.and_eq_true] using h1
                  have hLower1Value := h1Pair.1
                  have hNotR1 := h1Pair.2
                  have hR1False : r1 row = false := by
                    cases hr1 : r1 row <;> simp_all
                  cases hUpper0Value : upper0 row with
                  | true =>
                      have hUpper1Value := hLower1 hLower1Value
                      simp [upper2, hUpper0Value, hUpper1Value]
                  | false =>
                      have hOnly : lower1Only row = true := by
                        simp [lower1Only, hLower1Value, hUpper0Value]
                      have hContradiction := hR1 hOnly
                      rw [hR1False] at hContradiction
                      cases hContradiction
              have h2Bounds := ih lower2 upper2 h2Interval
              have hEvalFalse (row : Valuation n) :
                  isop (n + 1) lower upper (.cons false row) = (r0 row || r2 row) := by
                simp [isop, hZero, hOne, lower0, lower1, upper0, upper1,
                  lower0Only, lower1Only, r0, r1, lower2, upper2, r2]
              have hEvalTrue (row : Valuation n) :
                  isop (n + 1) lower upper (.cons true row) = (r1 row || r2 row) := by
                simp [isop, hZero, hOne, lower0, lower1, upper0, upper1,
                  lower0Only, lower1Only, r0, r1, lower2, upper2, r2]
              constructor
              · intro row hLower
                cases row with
                | cons value tail =>
                    cases value with
                    | false =>
                        have hLower0 : lower0 tail = true := by
                          simpa [lower0, cofactor] using hLower
                        rw [hEvalFalse tail]
                        cases hUpper1 : upper1 tail with
                        | false =>
                            have hR0 : r0 tail = true := by
                              simpa [r0] using h0Bounds.1 tail (by simp [lower0Only, hLower0, hUpper1])
                            simp [hR0]
                        | true =>
                            cases hR0 : r0 tail with
                            | true => simp
                            | false =>
                                have hR2 : r2 tail = true := by
                                  simpa [r2] using h2Bounds.1 tail (by simp [lower2, hLower0, hR0])
                                simp [hR2]
                    | true =>
                        have hLower1 : lower1 tail = true := by
                          simpa [lower1, cofactor] using hLower
                        rw [hEvalTrue tail]
                        cases hUpper0 : upper0 tail with
                        | false =>
                            have hR1 : r1 tail = true := by
                              simpa [r1] using h1Bounds.1 tail (by simp [lower1Only, hLower1, hUpper0])
                            simp [hR1]
                        | true =>
                            cases hR1 : r1 tail with
                            | true => simp
                            | false =>
                                have hR2 : r2 tail = true := by
                                  simpa [r2] using h2Bounds.1 tail (by simp [lower2, hLower1, hR1])
                                simp [hR2]
              · intro row hResult
                cases row with
                | cons value tail =>
                    cases value with
                    | false =>
                        rw [hEvalFalse tail] at hResult
                        cases hR0 : r0 tail with
                        | true =>
                            have hUpper := h0Bounds.2 tail (by simpa [r0] using hR0)
                            simpa [upper0, cofactor] using hUpper
                        | false =>
                            have hR2 : r2 tail = true := by simpa [hR0] using hResult
                            have hUpper := h2Bounds.2 tail (by simpa [r2] using hR2)
                            have hparts : upper0 tail = true ∧ upper1 tail = true := by
                              simpa [upper2] using hUpper
                            exact hparts.1
                    | true =>
                        rw [hEvalTrue tail] at hResult
                        cases hR1 : r1 tail with
                        | true =>
                            have hUpper := h1Bounds.2 tail (by simpa [r1] using hR1)
                            simpa [upper1, cofactor] using hUpper
                        | false =>
                            have hR2 : r2 tail = true := by simpa [hR1] using hResult
                            have hUpper := h2Bounds.2 tail (by simpa [r2] using hR2)
                            have hparts : upper0 tail = true ∧ upper1 tail = true := by
                              simpa [upper2] using hUpper
                            simpa [upper1, cofactor] using hparts.2

/-- Exact-mode interval ISOP preserves the input function. -/
theorem isop_exact (n : Nat) (f : BFun n) : ∀ row, isop n f f row = f row := by
  have hBounds := isop_bounds n f f (fun _ h => h)
  intro row
  cases hf : f row with
  | false =>
      cases hout : isop n f f row with
      | false => rfl
      | true =>
          have hUpper := hBounds.2 row hout
          simp [hf] at hUpper
  | true =>
      have hLower := hBounds.1 row hf
      exact hLower

private def fullCube : (n : Nat) → Cube n
  | 0 => .nil
  | n + 1 => .cons none (fullCube n)

private theorem fullCube_matches (n : Nat) (row : Valuation n) :
    Cube.matches (fullCube n) row = true := by
  induction n with
  | zero => cases row; rfl
  | succ n ih =>
      cases row with
      | cons _ tail => simpa [fullCube, Cube.matches] using ih tail

private theorem coverEval_append (left right : Cover n) (row : Valuation n) :
    coverEval (left ++ right) row = (coverEval left row || coverEval right row) := by
  induction left with
  | nil => rfl
  | cons cube rest ih =>
      simp only [List.cons_append, coverEval]
      rw [ih]
      cases cube.matches row <;> cases coverEval rest row <;>
        cases coverEval right row <;> rfl

private theorem attach_false_eval_false (cover : Cover n) (row : Valuation n) :
    coverEval (cover.map (Cube.cons (some false))) (.cons false row) = coverEval cover row := by
  induction cover with
  | nil => rfl
  | cons cube rest ih => simp [coverEval, Cube.matches, ih]

private theorem attach_false_eval_true (cover : Cover n) (row : Valuation n) :
    coverEval (cover.map (Cube.cons (some false))) (.cons true row) = false := by
  induction cover with
  | nil => rfl
  | cons cube rest ih => simp [coverEval, Cube.matches, ih]

private theorem attach_true_eval_false (cover : Cover n) (row : Valuation n) :
    coverEval (cover.map (Cube.cons (some true))) (.cons false row) = false := by
  induction cover with
  | nil => rfl
  | cons cube rest ih => simp [coverEval, Cube.matches, ih]

private theorem attach_true_eval_true (cover : Cover n) (row : Valuation n) :
    coverEval (cover.map (Cube.cons (some true))) (.cons true row) = coverEval cover row := by
  induction cover with
  | nil => rfl
  | cons cube rest ih => simp [coverEval, Cube.matches, ih]

private theorem attach_none_eval_false (cover : Cover n) (row : Valuation n) :
    coverEval (cover.map (Cube.cons none)) (.cons false row) = coverEval cover row := by
  induction cover with
  | nil => rfl
  | cons cube rest ih => simp [coverEval, Cube.matches, ih]

private theorem attach_none_eval_true (cover : Cover n) (row : Valuation n) :
    coverEval (cover.map (Cube.cons none)) (.cons true row) = coverEval cover row := by
  induction cover with
  | nil => rfl
  | cons cube rest ih => simp [coverEval, Cube.matches, ih]

/-- The three-branch Minato–Morreale interval recurrence as a cube cover.
`R0` and `R1` are emitted with their split literals; `R2` is emitted without
one. `fullCube` represents the all-true upper endpoint. -/
def isopCover : (n : Nat) → BFun n → BFun n → Cover n
  | 0, lower, _ => if lower .nil = true then [.nil] else []
  | n + 1, lower, upper =>
      if allFalse (n + 1) lower then []
      else if allTrue (n + 1) upper then [fullCube (n + 1)]
      else
        let lower0 := cofactor lower false
        let lower1 := cofactor lower true
        let upper0 := cofactor upper false
        let upper1 := cofactor upper true
        let lower0Only := fun row => lower0 row && !(upper1 row)
        let lower1Only := fun row => lower1 row && !(upper0 row)
        let r0 := isop n lower0Only upper0
        let r1 := isop n lower1Only upper1
        let lower2 := fun row => (lower0 row && !(r0 row)) || (lower1 row && !(r1 row))
        let upper2 := fun row => upper0 row && upper1 row
        attach false (isopCover n lower0Only upper0) ++
          attach true (isopCover n lower1Only upper1) ++
            attachNone (isopCover n lower2 upper2)

theorem isopCover_eval (n : Nat) (lower upper : BFun n) (row : Valuation n) :
    coverEval (isopCover n lower upper) row = isop n lower upper row := by
  induction n with
  | zero =>
      cases row
      cases hLower : lower .nil <;> simp [isopCover, isop, hLower, coverEval, Cube.matches]
  | succ n ih =>
      cases hZero : allFalse (n + 1) lower with
      | true =>
          cases row with
          | cons value tail => cases value <;> simp [isopCover, isop, hZero, coverEval]
      | false =>
          cases hOne : allTrue (n + 1) upper with
          | true =>
              cases row with
              | cons value tail =>
                  simp [isopCover, isop, hZero, hOne, coverEval, fullCube_matches]
          | false =>
              cases row with
              | cons value tail =>
                  let lower0 := cofactor lower false
                  let lower1 := cofactor lower true
                  let upper0 := cofactor upper false
                  let upper1 := cofactor upper true
                  let lower0Only := fun row => lower0 row && !(upper1 row)
                  let lower1Only := fun row => lower1 row && !(upper0 row)
                  let r0 := isop n lower0Only upper0
                  let r1 := isop n lower1Only upper1
                  let lower2 := fun row =>
                    (lower0 row && !(r0 row)) || (lower1 row && !(r1 row))
                  let upper2 := fun row => upper0 row && upper1 row
                  have h0 := ih lower0Only upper0 tail
                  have h1 := ih lower1Only upper1 tail
                  have h2 := ih lower2 upper2 tail
                  cases value with
                  | false =>
                      simp [isopCover, isop, hZero, hOne, attach, attachNone,
                        coverEval_append, attach_false_eval_false, attach_true_eval_false,
                        attach_none_eval_false, lower0, lower1, upper0, upper1,
                        lower0Only, lower1Only, r0, r1, lower2, upper2, h0, h2]
                  | true =>
                      simp [isopCover, isop, hZero, hOne, attach, attachNone,
                        coverEval_append, attach_false_eval_true, attach_true_eval_true,
                        attach_none_eval_true, lower0, lower1, upper0, upper1,
                        lower0Only, lower1Only, r0, r1, lower2, upper2, h1, h2]

/-- The generated cover remains inside the interval. -/
theorem isopCover_bounds (n : Nat) (lower upper : BFun n)
    (hInterval : pointwiseLe lower upper) :
    pointwiseLe lower (coverEval (isopCover n lower upper)) ∧
      pointwiseLe (coverEval (isopCover n lower upper)) upper := by
  have hFunction := isop_bounds n lower upper hInterval
  constructor
  · intro row hLower
    rw [isopCover_eval]
    exact hFunction.1 row hLower
  · intro row hCover
    rw [isopCover_eval] at hCover
    exact hFunction.2 row hCover

/-- Independent finite check of both endpoints of a support interval. -/
def intervalChecker (n : Nat) (lower upper : BFun n) (cover : Cover n) : Bool :=
  (valuations n).all fun row =>
    (decide (lower row = false) || coverEval cover row) &&
      (!coverEval cover row || decide (upper row = true))

theorem intervalChecker_sound (n : Nat) (lower upper : BFun n) (cover : Cover n)
    (hchecked : intervalChecker n lower upper cover = true) :
    pointwiseLe lower (coverEval cover) ∧ pointwiseLe (coverEval cover) upper := by
  constructor
  · intro row hLower
    have hmem := valuation_mem n row
    have hrow := (List.all_eq_true.mp hchecked) row hmem
    simp only [Bool.and_eq_true, Bool.or_eq_true, decide_eq_true_eq] at hrow
    rcases hrow with ⟨hLowerOrCover, _⟩
    rcases hLowerOrCover with hFalse | hCover
    · simp_all
    · exact hCover
  · intro row hCover
    have hmem := valuation_mem n row
    have hrow := (List.all_eq_true.mp hchecked) row hmem
    simp only [Bool.and_eq_true, Bool.or_eq_true, decide_eq_true_eq] at hrow
    rcases hrow with ⟨_, hCoverOrUpper⟩
    rcases hCoverOrUpper with hNotCover | hUpper
    · simp_all
    · exact hUpper

theorem isopCover_interval_checked (n : Nat) (lower upper : BFun n)
    (hInterval : pointwiseLe lower upper) :
    intervalChecker n lower upper (isopCover n lower upper) = true := by
  have hBounds := isopCover_bounds n lower upper hInterval
  apply List.all_eq_true.mpr
  intro row _hmem
  have hLowerOrCover : lower row = false ∨ coverEval (isopCover n lower upper) row = true := by
    cases hLowerValue : lower row with
    | false => exact Or.inl rfl
    | true => exact Or.inr (hBounds.1 row hLowerValue)
  have hCoverOrUpper : coverEval (isopCover n lower upper) row = false ∨ upper row = true := by
    cases hCover : coverEval (isopCover n lower upper) row with
    | false => exact Or.inl rfl
    | true => exact Or.inr (hBounds.2 row hCover)
  simp [hLowerOrCover, hCoverOrUpper]

/-- Exact mode returns a cover equivalent to the source function. -/
theorem isopCover_exact (n : Nat) (f : BFun n) :
    ∀ row, coverEval (isopCover n f f) row = f row := by
  intro row
  rw [isopCover_eval]
  exact isop_exact n f row

/-- All finite cubes over the three-valued literal alphabet. -/
def allCubes : (n : Nat) → List (Cube n)
  | 0 => [.nil]
  | n + 1 =>
      (allCubes n).flatMap fun tail =>
        [.cons none tail, .cons (some false) tail, .cons (some true) tail]

theorem allCubes_complete (cube : Cube n) : cube ∈ allCubes n := by
  induction cube with
  | nil => simp [allCubes]
  | cons head tail ih =>
      cases head with
      | none => simp [allCubes, ih]
      | some value => cases value <;> simp [allCubes, ih]

/-- Every emitted cube is an implicant with no strictly wider implicant. -/
def primeCover (n : Nat) (f : BFun n) (cover : Cover n) : Bool :=
  cover.all fun cube =>
    implicant n f cube &&
      !((allCubes n).any fun wider =>
        wider.subsumes cube && !cube.subsumes wider && implicant n f wider)

theorem majority_isop_cover_prime_and_irredundant :
    primeCover 3 majority3.function (isopCover 3 majority3.function majority3.function) = true ∧
      irredundant (isopCover 3 majority3.function majority3.function) = true := by
  decide

theorem constant_true_independently_checked :
    checker 0 trueConstant.function (isopCover 0 trueConstant.function trueConstant.function) = true := by
  decide

theorem constant_false_independently_checked :
    checker 0 falseConstant.function (isopCover 0 falseConstant.function falseConstant.function) = true := by
  decide

theorem contradiction_independently_checked :
    checker 1 contradiction.function (isopCover 1 contradiction.function contradiction.function) = true := by
  decide

theorem repeated_atom_independently_checked :
    checker 1 repeatedAtom.function (isopCover 1 repeatedAtom.function repeatedAtom.function) = true := by
  decide

theorem parity3_independently_checked :
    checker 3 parity3.function (isopCover 3 parity3.function parity3.function) = true := by
  decide

theorem parity3_cover_growth :
    (isopCover 3 parity3.function parity3.function).length = 4 := by
  decide

def reversedParity3Cover : Cover 3 :=
  reverseCover (isopCover 3 (fun row => parity3.function (reverseValuation row))
    (fun row => parity3.function (reverseValuation row)))

theorem parity3_orders_differ :
    isopCover 3 parity3.function parity3.function ≠ reversedParity3Cover := by
  decide

theorem parity3_orders_equivalent :
    checker 3 parity3.function reversedParity3Cover = true := by
  decide

def careLower : BFun 1 := fun row => !(x0.function row) && x0.function row

def careUpper : BFun 1 := fun row => x0.function row || x0.function row

theorem care_set_interval_checked :
    intervalChecker 1 careLower careUpper (isopCover 1 careLower careUpper) = true := by
  decide

def wrongCareSetCover : Cover 1 := [fullCube 1]

theorem seeded_care_set_fault_rejected :
    intervalChecker 1 careLower careUpper wrongCareSetCover = false := by
  decide

def conjunction2 : Formula 2 :=
  .and (.var ⟨0, by decide⟩) (.var ⟨1, by decide⟩)

def wrongMappedConjunction2 : BFun 2 :=
  fun row => row.get ⟨0, by decide⟩ && row.get ⟨0, by decide⟩

theorem seeded_atom_map_fault_rejected :
    checker 2 conjunction2.function (shannonCover 2 wrongMappedConjunction2) = false := by
  decide

end NobleFoundation.IsopInterval
