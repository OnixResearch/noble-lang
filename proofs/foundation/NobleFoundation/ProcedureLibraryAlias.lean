import Mathlib

/-!
# Exact-pair I64 polynomial normalization

A dependency-free fragment of typed-chunk/1 is interpreted as an ordered I64
stack. Literals, immutable scalar captures, dup/drop/swap and wrapping
addition/subtraction/multiplication are admitted; dynamic calls, dependencies,
quotations, Boolean values and operations are deliberately outside this model.

`MvPolynomial` is the mathematical canonical sparse polynomial over Z/2^64.
The Rust alias checker separately constructs a bounded sorted sparse support,
checks retained source via typed-chunk admission, and recomputes both forms and
both empty-closure digests. This proof establishes normalization soundness for
the model; it does not purport to prove Rust's array codec or bounded sparse
support implementation equivalent to Mathlib's representation.
-/

namespace NobleFoundation.ProcedureLibraryAlias

/-- The residue class of a wrapping signed I64, represented by its low 64 bits. -/
abbrev Word := ZMod (2 ^ 64)

/-- At most eight ordered I64 input slots. -/
abbrev Slot := Fin 8

/-- A canonical formal polynomial with wrapping I64 coefficients. -/
abbrev Polynomial := MvPolynomial Slot Word

/-- Interpret a normalized polynomial at the caller's ordered input values. -/
def evalPolynomial (input : Slot → Word) (polynomial : Polynomial) : Word :=
  MvPolynomial.eval input polynomial

/-- Signed literals are injected modulo 2^64, including negative literals. -/
def literal (value : Int) : Word := value

/-- `ZMod (2^64)` addition is the wrapping I64 operation. -/
theorem wrapping_i64_add (left right : Int) :
    literal (left + right) = literal left + literal right := by
  simp [literal]

/-- `ZMod (2^64)` subtraction is the wrapping I64 operation. -/
theorem wrapping_i64_sub (left right : Int) :
    literal (left - right) = literal left - literal right := by
  simp [literal]

/-- `ZMod (2^64)` multiplication is the wrapping I64 operation. -/
theorem wrapping_i64_mul (left right : Int) :
    literal (left * right) = literal left * literal right := by
  simp [literal]

inductive Instruction where
  | literal (value : Int)
  | capture (value : Int)
  | dup
  | drop
  | swap
  | add
  | sub
  | mul
  deriving DecidableEq, Repr

/-- Runtime stack interpretation, top of stack at the head. Binary operators
consume the top as right operand and the next value as left operand. -/
def evalStep : Instruction → List Word → Option (List Word)
  | .literal value, stack => some (literal value :: stack)
  | .capture value, stack => some (literal value :: stack)
  | .dup, top :: stack => some (top :: top :: stack)
  | .drop, _ :: stack => some stack
  | .swap, top :: below :: stack => some (below :: top :: stack)
  | .add, right :: left :: stack => some ((left + right) :: stack)
  | .sub, right :: left :: stack => some ((left - right) :: stack)
  | .mul, right :: left :: stack => some ((left * right) :: stack)
  | _, _ => none

/-- Canonical formal-polynomial stack interpretation of the same instructions. -/
def normalizeStep : Instruction → List Polynomial → Option (List Polynomial)
  | .literal value, stack => some (MvPolynomial.C (literal value) :: stack)
  | .capture value, stack => some (MvPolynomial.C (literal value) :: stack)
  | .dup, top :: stack => some (top :: top :: stack)
  | .drop, _ :: stack => some stack
  | .swap, top :: below :: stack => some (below :: top :: stack)
  | .add, right :: left :: stack => some ((left + right) :: stack)
  | .sub, right :: left :: stack => some ((left - right) :: stack)
  | .mul, right :: left :: stack => some ((left * right) :: stack)
  | _, _ => none

/-- Finite straight-line stack replay; refusal is an explicit `none`. -/
def run {α : Type} (step : Instruction → List α → Option (List α)) :
    List Instruction → List α → Option (List α)
  | [], stack => some stack
  | operation :: rest, stack =>
      (step operation stack).bind (run step rest)

/-- Each normalization step denotes precisely the wrapping runtime step,
including underflow refusal and the operand order of subtraction. -/
theorem step_sound (operation : Instruction) (stack : List Polynomial)
    (input : Slot → Word) :
    (normalizeStep operation stack).map
      (List.map (evalPolynomial input)) =
    evalStep operation (stack.map (evalPolynomial input)) := by
  cases operation <;> cases stack with
  | nil => simp [normalizeStep, evalStep, evalPolynomial]
  | cons top rest =>
      cases rest with
      | nil => simp [normalizeStep, evalStep, evalPolynomial]
      | cons below tail =>
          simp [normalizeStep, evalStep, evalPolynomial]

/-- Normalizing any admitted sequence, rather than only a single operation,
preserves the complete output stack or the underflow refusal. -/
theorem body_sound (body : List Instruction) (stack : List Polynomial)
    (input : Slot → Word) :
    (run normalizeStep body stack).map (List.map (evalPolynomial input)) =
    run evalStep body (stack.map (evalPolynomial input)) := by
  induction body generalizing stack with
  | nil => simp [run]
  | cons operation rest inductionHypothesis =>
      cases h : normalizeStep operation stack with
      | none =>
          have step := step_sound operation stack input
          simp [h] at step
          simp [run, h, step]
      | some next =>
          have step := step_sound operation stack input
          simp [h] at step
          simp [run, h, step, inductionHypothesis]

/-- Every declared input slot is represented by its formal variable. The
reverse maps bottom-to-top interface order to head-first execution order. -/
def initialPolynomials (slots : List Slot) : List Polynomial :=
  slots.reverse.map MvPolynomial.X

/-- Formal input variables evaluate to the actual ordered input stack. -/
theorem initial_polynomials_sound (slots : List Slot) (input : Slot → Word) :
    (initialPolynomials slots).map (evalPolynomial input) =
      slots.reverse.map input := by
  simp [initialPolynomials, evalPolynomial]

/-- An exact equality of two normalized *full stacks* is sufficient for
functional equivalence, for all eight-slot I64 inputs, including overflow. -/
theorem exact_pair_sound (left right : List Instruction) (slots : List Slot)
    (input : Slot → Word)
    (equalForms : run normalizeStep left (initialPolynomials slots) =
      run normalizeStep right (initialPolynomials slots)) :
    run evalStep left (slots.reverse.map input) =
      run evalStep right (slots.reverse.map input) := by
  simp only [← initial_polynomials_sound slots input]
  calc
    run evalStep left ((initialPolynomials slots).map (evalPolynomial input)) =
        (run normalizeStep left (initialPolynomials slots)).map
          (List.map (evalPolynomial input)) := (body_sound left _ input).symm
    _ = (run normalizeStep right (initialPolynomials slots)).map
          (List.map (evalPolynomial input)) := congrArg _ equalForms
    _ = run evalStep right ((initialPolynomials slots).map (evalPolynomial input)) :=
          body_sound right _ input

end NobleFoundation.ProcedureLibraryAlias
