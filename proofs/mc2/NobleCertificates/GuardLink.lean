import NobleCertificates.Guards
import NobleContracts.Expression

/-!
# C1 at model level: recognized preconditions mean the template preconditions

`.cairn/changes/bounded-applicability-guards` states C1: when the guard recognizer selects a
template `t` for a contract, the MC1 meaning of the contract's own `requires` on the single
`I64` input is `GuardPrecondition t`. The recognizer (`companion::guard::recognize`) accepts a
whole precondition in exactly five MC1 spellings, with `x0` the input and a literal being
`ExprKind::I64`, that is `Term.i64` of the same integer:
- `(lt x0 9223372036854775807)` for `ltI64Max`;
- `(eq x0 k)` and `(eq k x0)` for `eqI64Literal k`;
- `(not (eq x0 -9223372036854775808))` and `(not (eq -9223372036854775808 x0))` for `neI64Min`.

Each theorem below proves `Holds e [.i64 x] [] params ↔ GuardPrecondition t x` for one of
those spellings. The recognizer runs only when the input interface is exactly `[I64]`, which
is why the input is `.i64 x`.

Not proved here: that the Rust `Prepared` arena denotes these terms, i.e. the extracted
correspondence for `templates_of`, and C1 for `NonEmptyList`, which waits for the v2 template
set.
-/

open NobleContracts

namespace NobleCertificates

/-- `(lt x0 9223372036854775807)`. -/
def ltI64MaxTerm : Term := .lt (.input 0) (.i64 9223372036854775807)

/-- `(eq x0 k)`. -/
def eqI64LiteralTerm (k : Int) : Term := .eq (.input 0) (.i64 k)

/-- `(eq k x0)`. -/
def eqI64LiteralTermSwapped (k : Int) : Term := .eq (.i64 k) (.input 0)

/-- `(not (eq x0 -9223372036854775808))`. -/
def neI64MinTerm : Term := .not (.eq (.input 0) (.i64 (-9223372036854775808)))

/-- `(not (eq -9223372036854775808 x0))`. -/
def neI64MinTermSwapped : Term := .not (.eq (.i64 (-9223372036854775808)) (.input 0))

private theorem ofInt_min : BitVec.ofInt 64 (-9223372036854775808) = i64min := by decide

/-- **C1, `ltI64Max`.** -/
theorem ltI64Max_link (x : BitVec 64) (params : List Value) :
    Holds ltI64MaxTerm [.i64 x] [] params ↔ GuardPrecondition .ltI64Max x := by
  simp [Holds, ltI64MaxTerm, evaluate, GuardPrecondition, i64max_toInt]

/-- **C1, `eqI64Literal`**, input on the left. -/
theorem eqI64Literal_link (k : Int) (x : BitVec 64) (params : List Value) :
    Holds (eqI64LiteralTerm k) [.i64 x] [] params ↔
      GuardPrecondition (.eqI64Literal (BitVec.ofInt 64 k)) x := by
  simp [Holds, eqI64LiteralTerm, evaluate, GuardPrecondition]

/-- **C1, `eqI64Literal`**, input on the right. -/
theorem eqI64Literal_link_swapped (k : Int) (x : BitVec 64) (params : List Value) :
    Holds (eqI64LiteralTermSwapped k) [.i64 x] [] params ↔
      GuardPrecondition (.eqI64Literal (BitVec.ofInt 64 k)) x := by
  simp [Holds, eqI64LiteralTermSwapped, evaluate, GuardPrecondition, eq_comm]

/-- **C1, `neI64Min`**, input on the left. -/
theorem neI64Min_link (x : BitVec 64) (params : List Value) :
    Holds neI64MinTerm [.i64 x] [] params ↔ GuardPrecondition .neI64Min x := by
  simp [Holds, neI64MinTerm, evaluate, GuardPrecondition, ofInt_min]

/-- **C1, `neI64Min`**, input on the right. -/
theorem neI64Min_link_swapped (x : BitVec 64) (params : List Value) :
    Holds neI64MinTermSwapped [.i64 x] [] params ↔ GuardPrecondition .neI64Min x := by
  simp only [Holds, neI64MinTermSwapped, GuardPrecondition]
  rw [ne_comm (a := x)]
  simp [evaluate, ofInt_min]

/-- The input-first spelling of each template's precondition. -/
def recognizedTerm : GuardTemplate → Term
  | .ltI64Max => ltI64MaxTerm
  | .neI64Min => neI64MinTerm
  | .eqI64Literal k => eqI64LiteralTerm k.toInt

/-- **C1 for the whole v1 set**: every template's recognized precondition means exactly its
`GuardPrecondition`. -/
theorem recognized_link (t : GuardTemplate) (x : BitVec 64) (params : List Value) :
    Holds (recognizedTerm t) [.i64 x] [] params ↔ GuardPrecondition t x := by
  cases t with
  | ltI64Max => exact ltI64Max_link x params
  | neI64Min => exact neI64Min_link x params
  | eqI64Literal k =>
      simpa [recognizedTerm, BitVec.ofInt_toInt] using eqI64Literal_link k.toInt x params

/-- Control: the near-miss spelling `(le x0 9223372036854775807)`, which the recognizer refuses,
holds for `i64::MAX` itself, where `ltI64Max`'s precondition fails. Accepting it would let a
guard establish less than the contract states. -/
theorem le_spelling_is_not_ltI64Max :
    Holds (.le (.input 0) (.i64 9223372036854775807)) [.i64 i64max] [] [] ∧
      ¬ GuardPrecondition .ltI64Max i64max := by
  refine ⟨?_, ?_⟩
  · simp [Holds, evaluate, i64max_toInt]
  · simp [GuardPrecondition]

end NobleCertificates
