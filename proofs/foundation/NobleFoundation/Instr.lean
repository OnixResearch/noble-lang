import NobleFoundation.Repr
import NobleContracts.Rules

/-!
# Instruction-level signed I64 arithmetic over the MC1 model (issue #15, Phase 1)

Orientation, stated once and then proved:

* **Public** stacks (`NobleContracts.Exec`, `OpTyped`, `StackTyped`) are
  bottom-first: the *last* list element is the stack top.  In the Noble source
  `x y -` the value `x` is pushed first, so it is the deeper (left) operand and
  `y` is the top (right) operand.
* **Internal** stacks (`NobleContracts.Run`/`Step`) are top-first; `Exec` is
  `Run` on reversed stacks.  `Step.sub` therefore matches
  `.i64 b :: .i64 a :: s` and produces `a - b`: the head `b` is the public top.
* The runtime helper `$op_sub` in `crates/noble-wasm/runtime/operations.wat`
  pops `$right` first (the top) and then `$left`, and computes
  `i64.sub left right`; that agreement is recorded in the boundary table and
  checked by tests, not proved here.

All results quantify over an arbitrary preserved prefix `pre`; the prefix is
typed (`StackTyped pre tys`) only where a typing conclusion is drawn.
-/

namespace NobleFoundation

open NobleContracts

/-- The I64 value an in-range integer denotes. -/
def i64v (n : ℤ) : Value := .i64 (BitVec.ofInt 64 n)

/-- Representation relation: `v` is an I64 value denoting the integer `n`. -/
def Represents (v : Value) (n : ℤ) : Prop := I64Range n ∧ v = i64v n

theorem represents_iff (v : Value) (n : ℤ) :
    Represents v n ↔ ∃ x : BitVec 64, v = .i64 x ∧ x.toInt = n := by
  constructor
  · rintro ⟨hr, rfl⟩; exact ⟨_, rfl, toInt_ofInt_of_range hr⟩
  · rintro ⟨x, rfl, rfl⟩
    exact ⟨toInt_range x, by rw [i64v, BitVec.ofInt_toInt]⟩

/-- Each I64 value denotes exactly one integer. -/
theorem represents_functional {v n m} (hn : Represents v n) (hm : Represents v m) : n = m := by
  obtain ⟨x, rfl, rfl⟩ := (represents_iff _ _).mp hn
  obtain ⟨y, hy, rfl⟩ := (represents_iff _ _).mp hm
  cases hy; rfl

/-- Every well-typed I64 value denotes some integer, and only I64 values do. -/
theorem represents_of_hasType {v} (h : HasType v .i64) : Represents v (match v with
    | .i64 x => x.toInt | _ => 0) := by
  obtain ⟨x, rfl⟩ := (hasType_i64_iff v).mp h
  exact (represents_iff _ _).mpr ⟨x, rfl, rfl⟩

theorem hasType_of_represents {v n} (h : Represents v n) : HasType v .i64 := by
  rw [h.2]; exact .i64

theorem represents_wrap (z : ℤ) : Represents (i64v (MathlibI64.wrap z)) (MathlibI64.wrap z) :=
  ⟨by rw [wrap_eq_toInt_ofInt]; exact toInt_range _, rfl⟩

/-- `StackTyped` respects concatenation in the public orientation. -/
theorem _root_.NobleContracts.StackTyped.append {s t : Stack} {ts tt : List Ty}
    (hs : StackTyped s ts) (ht : StackTyped t tt) : StackTyped (s ++ t) (ts ++ tt) := by
  induction hs with
  | nil => simpa using ht
  | cons hv _ ih => exact .cons hv ih

theorem stackTyped_snoc_i64 {s : Stack} {ts : List Ty} (hs : StackTyped s ts) (n : ℤ) :
    StackTyped (s ++ [i64v n]) (ts ++ [.i64]) :=
  hs.append (.cons .i64 .nil)

/-! ## Orientation reconciliation -/

/-- The public operands `a` (deeper) and `b` (top) appear internally as
`b :: a :: …`. -/
theorem internal_of_public (pre : Stack) (a b : Value) :
    (pre ++ [a, b]).reverse = b :: a :: pre.reverse := by simp

/-- Public subtraction *is* the internal `Step.sub` on the reversed stack,
whose head (the public top) is the subtrahend. -/
theorem exec_sub_is_internal_step (pre : Stack) (a b : BitVec 64) (t : Stack) :
    Exec [.word 5] (pre ++ [.i64 a, .i64 b]) t ↔
      Step (.word 5) (.i64 b :: .i64 a :: pre.reverse) t.reverse := by
  simp [Exec, List.reverse_append]

/-! ## Universal instruction lemmas

For every prefix, every pair of represented integers `x` (deeper) and `y`
(top), the instruction has exactly one normal result: the prefix followed by
the oracle's wrapped value.  Typing: the instruction is `OpTyped` at the
prefix's types, and the result stack is typed `tys ++ [.i64]`. -/

/-- Shared shape of the three arithmetic instruction theorems. -/
def ArithSpec (word : Nat) (f : ℤ → ℤ → ℤ) : Prop :=
  ∀ (pre : Stack) (tys : List Ty) (va vb : Value) (x y : ℤ),
    StackTyped pre tys → Represents va x → Represents vb y →
      OpTyped (.word word) (tys ++ [.i64, .i64]) (tys ++ [.i64]) ∧
      StackTyped (pre ++ [va, vb]) (tys ++ [.i64, .i64]) ∧
      (∀ t, Exec [.word word] (pre ++ [va, vb]) t ↔
        t = pre ++ [i64v (MathlibI64.wrap (f x y))]) ∧
      StackTyped (pre ++ [i64v (MathlibI64.wrap (f x y))]) (tys ++ [.i64]) ∧
      Represents (i64v (MathlibI64.wrap (f x y))) (MathlibI64.wrap (f x y))

private theorem typed_pair {pre tys va vb x y} (hpre : StackTyped pre tys)
    (ha : Represents va x) (hb : Represents vb y) :
    StackTyped (pre ++ [va, vb]) (tys ++ [.i64, .i64]) :=
  hpre.append (.cons (hasType_of_represents ha) (.cons (hasType_of_represents hb) .nil))

theorem add_spec : ArithSpec 4 (· + ·) := by
  intro pre tys va vb x y hpre ha hb
  refine ⟨.add, typed_pair hpre ha hb, ?_, stackTyped_snoc_i64 hpre _, represents_wrap _⟩
  intro t
  rw [ha.2, hb.2, i64v, i64v, exec_add_iff, i64v, ofInt_wrap, BitVec.ofInt_add]

theorem sub_spec : ArithSpec 5 (· - ·) := by
  intro pre tys va vb x y hpre ha hb
  refine ⟨.sub, typed_pair hpre ha hb, ?_, stackTyped_snoc_i64 hpre _, represents_wrap _⟩
  intro t
  rw [ha.2, hb.2, i64v, i64v, exec_sub_iff, i64v, ofInt_wrap, ofInt_sub]

theorem mul_spec : ArithSpec 6 (· * ·) := by
  intro pre tys va vb x y hpre ha hb
  refine ⟨.mul, typed_pair hpre ha hb, ?_, stackTyped_snoc_i64 hpre _, represents_wrap _⟩
  intro t
  rw [ha.2, hb.2, i64v, i64v, exec_mul_iff, i64v, ofInt_wrap, BitVec.ofInt_mul]

/-- Bit-vector form for arbitrary operands (no range premise needed): the result
decodes to the oracle's wrap of the decoded operands, deeper minus top. -/
theorem exec_arith_toInt (pre : Stack) (a b : BitVec 64) (t : Stack) :
    (Exec [.word 4] (pre ++ [.i64 a, .i64 b]) t ↔
      ∃ r : BitVec 64, t = pre ++ [.i64 r] ∧ r.toInt = MathlibI64.wrap (a.toInt + b.toInt)) ∧
    (Exec [.word 5] (pre ++ [.i64 a, .i64 b]) t ↔
      ∃ r : BitVec 64, t = pre ++ [.i64 r] ∧ r.toInt = MathlibI64.wrap (a.toInt - b.toInt)) ∧
    (Exec [.word 6] (pre ++ [.i64 a, .i64 b]) t ↔
      ∃ r : BitVec 64, t = pre ++ [.i64 r] ∧ r.toInt = MathlibI64.wrap (a.toInt * b.toInt)) := by
  refine ⟨?_, ?_, ?_⟩ <;>
  · simp only [exec_add_iff, exec_sub_iff, exec_mul_iff]
    constructor
    · rintro rfl; exact ⟨_, rfl, by first | exact toInt_add a b | exact toInt_sub a b |
        exact toInt_mul a b⟩
    · rintro ⟨r, rfl, hr⟩
      congr
      apply BitVec.eq_of_toInt_eq
      first | rw [hr, toInt_add] | rw [hr, toInt_sub] | rw [hr, toInt_mul]

/-- A non-I64 operand has no normal arithmetic transition (both orientations of
a mistyped pair are stuck rather than coerced). -/
theorem add_needs_i64 (pre : Stack) (b : BitVec 64) (t : Stack) :
    ¬ Exec [.word 4] (pre ++ [.bool true, .i64 b]) t ∧
    ¬ Exec [.word 4] (pre ++ [.i64 b, .bool true]) t := by
  constructor <;> intro h <;>
    simp [Exec, List.reverse_append] at h <;> cases h

end NobleFoundation
