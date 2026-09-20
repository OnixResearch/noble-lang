import Std

/-!
The pure MC1 model.  Stacks in the public interface are bottom-first.  The
internal machine uses the reversed representation solely to make top-of-stack
inversion structural.  `Exec` describes normal returns, not termination,
resource exhaustion, host effects, or a Wasm/runtime implementation.

Word numbers are the ordered bootstrap table in
`noble-kernel/src/contracts/bootstrap/data.rs`.  There is deliberately no
transition or typing rule for effectful word 22 (`test.emit`) or unknown words.
-/

namespace NobleContracts

inductive Ty where
  | unit | bool | i64 | text | «syntax»
  | pair : Ty → Ty → Ty
  | sum : Ty → Ty → Ty
  | list : Ty → Ty
  | program : List Ty → List Ty → Ty
  deriving Repr

mutual
  inductive Value where
    | unit : Value
    | bool : Bool → Value
    | i64 : BitVec 64 → Value
    | text : String → Value
    | pair : Value → Value → Value
    | inl : Value → Value
    | inr : Value → Value
    | list : List Value → Value
    | program : List Op → Value
    | «syntax» : List Op → Value

  inductive Op where
    | lit : Value → Op
    | word : Nat → Op
    | block : List Op → Op
end

abbrev Stack := List Value

/-- Integer literals have exactly the wrapping two's-complement representation. -/
def i64 (n : Int) : Value := .i64 (BitVec.ofInt 64 n)


mutual
  /-- Internal normal-return execution, with the stack top at the list head. -/
  inductive Run : List Op → Stack → Stack → Prop where
    | nil {s} : Run [] s s
    | cons {op ops s m t} : Step op s m → Run ops m t → Run (op :: ops) s t

  /-- One pure operation.  Branch and invocation premises are real executions. -/
  inductive Step : Op → Stack → Stack → Prop where
    | lit {v s} : Step (.lit v) s (v :: s)
    | block {p s} : Step (.block p) s (.program p :: s)
    | dup {v s} : Step (.word 0) (v :: s) (v :: v :: s)
    | drop {v s} : Step (.word 1) (v :: s) s
    | swap {a b s} : Step (.word 2) (b :: a :: s) (a :: b :: s)
    | dip {p v s t} : Run p s t →
        Step (.word 3) (.program p :: v :: s) (v :: t)
    | add {a b : BitVec 64} {s} :
        Step (.word 4) (.i64 b :: .i64 a :: s) (.i64 (a + b) :: s)
    | sub {a b : BitVec 64} {s} :
        Step (.word 5) (.i64 b :: .i64 a :: s) (.i64 (a - b) :: s)
    | mul {a b : BitVec 64} {s} :
        Step (.word 6) (.i64 b :: .i64 a :: s) (.i64 (a * b) :: s)
    | equals {a b : BitVec 64} {s} :
        Step (.word 7) (.i64 b :: .i64 a :: s) (.bool (a == b) :: s)
    | quote {v s} : Step (.word 8) (v :: s) (.program [.lit v] :: s)
    | compose {p q s} : Step (.word 9) (.program q :: .program p :: s)
        (.program (p ++ q) :: s)
    | run {p s t} : Run p s t → Step (.word 10) (.program p :: s) t
    | reflect {p s} : Step (.word 11) (.program p :: s) (.«syntax» p :: s)
    | unit {s} : Step (.word 12) s (.unit :: s)
    | pair {a b s} : Step (.word 13) (b :: a :: s) (.pair a b :: s)
    | unpair {a b s} : Step (.word 14) (.pair a b :: s) (b :: a :: s)
    | inl {v s} : Step (.word 15) (v :: s) (.inl v :: s)
    | inr {v s} : Step (.word 16) (v :: s) (.inr v :: s)
    | caseLeft {p q v s t} : Run p (v :: s) t →
        Step (.word 17) (.program q :: .program p :: .inl v :: s) t
    | caseRight {p q v s t} : Run q (v :: s) t →
        Step (.word 17) (.program q :: .program p :: .inr v :: s) t
    | ifTrue {p q s t} : Run p s t →
        Step (.word 18) (.program q :: .program p :: .bool true :: s) t
    | ifFalse {p q s t} : Run q s t →
        Step (.word 18) (.program q :: .program p :: .bool false :: s) t
    | nil {s} : Step (.word 19) s (.list [] :: s)
    | cons {v vs s} : Step (.word 20) (.list vs :: v :: s) (.list (v :: vs) :: s)
    | listNil {p q s t} : Run p s t →
        Step (.word 21) (.program q :: .program p :: .list [] :: s) t
    | listCons {p q v vs s t} : Run q (.list vs :: v :: s) t →
        Step (.word 21) (.program q :: .program p :: .list (v :: vs) :: s) t
end

/-- Public execution: the final list element is the stack top. -/
def Exec (code : List Op) (before after : Stack) : Prop :=
  Run code before.reverse after.reverse

mutual
  /-- Pure value typing.  Programs need a derivation; an arbitrary body does not
  acquire an arbitrary arrow type merely by being a `Value.program`. -/
  inductive HasType : Value → Ty → Prop where
    | unit : HasType .unit .unit
    | bool {b} : HasType (.bool b) .bool
    | i64 {n} : HasType (.i64 n) .i64
    | text {s} : HasType (.text s) .text
    | «syntax» {p} : HasType (.«syntax» p) .«syntax»
    | pair {a b ta tb} : HasType a ta → HasType b tb →
        HasType (.pair a b) (.pair ta tb)
    | inl {v a b} : HasType v a → HasType (.inl v) (.sum a b)
    | inr {v a b} : HasType v b → HasType (.inr v) (.sum a b)
    | list {vs t} : (∀ v ∈ vs, HasType v t) → HasType (.list vs) (.list t)
    | program {p i o} : CodeTyped p i o → HasType (.program p) (.program i o)

  /-- Declarative pure typing, not a replacement for kernel acceptance. -/
  inductive CodeTyped : List Op → List Ty → List Ty → Prop where
    | nil {s} : CodeTyped [] s s
    | cons {op ops s m t} : OpTyped op s m → CodeTyped ops m t →
        CodeTyped (op :: ops) s t

  inductive OpTyped : Op → List Ty → List Ty → Prop where
    | lit {v ty s} : HasType v ty → OpTyped (.lit v) s (s ++ [ty])
    | block {p i o s} : CodeTyped p i o →
        OpTyped (.block p) s (s ++ [.program i o])
    | dup {a s} : OpTyped (.word 0) (s ++ [a]) (s ++ [a, a])
    | drop {a s} : OpTyped (.word 1) (s ++ [a]) s
    | swap {a b s} : OpTyped (.word 2) (s ++ [a, b]) (s ++ [b, a])
    | dip {a s t} : OpTyped (.word 3) (s ++ [a, .program s t]) (t ++ [a])
    | add {s} : OpTyped (.word 4) (s ++ [.i64, .i64]) (s ++ [.i64])
    | sub {s} : OpTyped (.word 5) (s ++ [.i64, .i64]) (s ++ [.i64])
    | mul {s} : OpTyped (.word 6) (s ++ [.i64, .i64]) (s ++ [.i64])
    | equals {s} : OpTyped (.word 7) (s ++ [.i64, .i64]) (s ++ [.bool])
    | quote {a s t} : OpTyped (.word 8) (s ++ [a]) (s ++ [.program t (t ++ [a])])
    | compose {i m o s} : OpTyped (.word 9)
        (s ++ [.program i m, .program m o]) (s ++ [.program i o])
    | run {s t} : OpTyped (.word 10) (s ++ [.program s t]) t
    | reflect {i o s} : OpTyped (.word 11) (s ++ [.program i o]) (s ++ [.«syntax»])
    | unit {s} : OpTyped (.word 12) s (s ++ [.unit])
    | pair {a b s} : OpTyped (.word 13) (s ++ [a, b]) (s ++ [.pair a b])
    | unpair {a b s} : OpTyped (.word 14) (s ++ [.pair a b]) (s ++ [a, b])
    | inl {a b s} : OpTyped (.word 15) (s ++ [a]) (s ++ [.sum a b])
    | inr {a b s} : OpTyped (.word 16) (s ++ [b]) (s ++ [.sum a b])
    | caseWord {a b s t} : OpTyped (.word 17)
        (s ++ [.sum a b, .program (s ++ [a]) t, .program (s ++ [b]) t]) t
    | ifWord {s t} : OpTyped (.word 18)
        (s ++ [.bool, .program s t, .program s t]) t
    | nil {a s} : OpTyped (.word 19) s (s ++ [.list a])
    | cons {a s} : OpTyped (.word 20) (s ++ [a, .list a]) (s ++ [.list a])
    | listCase {a s t} : OpTyped (.word 21)
        (s ++ [.list a, .program s t, .program (s ++ [a, .list a]) t]) t
end

/-- Ordered, exact stack typing: neither extra inputs nor silently dropped
outputs satisfy the exported boundary. -/
inductive StackTyped : Stack → List Ty → Prop where
  | nil : StackTyped [] []
  | cons {v vs ty tys} : HasType v ty → StackTyped vs tys →
      StackTyped (v :: vs) (ty :: tys)

@[simp] theorem hasType_unit_iff (v : Value) : HasType v .unit ↔ v = .unit := by
  constructor
  · intro h; cases h; rfl
  · intro h; subst v; exact .unit

@[simp] theorem hasType_bool_iff (v : Value) :
    HasType v .bool ↔ ∃ b, v = .bool b := by
  constructor
  · intro h; cases h; exact ⟨_, rfl⟩
  · rintro ⟨b, rfl⟩; exact .bool

@[simp] theorem hasType_i64_iff (v : Value) :
    HasType v .i64 ↔ ∃ n, v = .i64 n := by
  constructor
  · intro h; cases h; exact ⟨_, rfl⟩
  · rintro ⟨n, rfl⟩; exact .i64

@[simp] theorem stackTyped_nil_iff (s : Stack) : StackTyped s [] ↔ s = [] := by
  constructor
  · intro h; cases h; rfl
  · intro h; subst s; exact .nil

@[simp] theorem stackTyped_cons_iff (s : Stack) (ty : Ty) (tys : List Ty) :
    StackTyped s (ty :: tys) ↔
      ∃ v vs, s = v :: vs ∧ HasType v ty ∧ StackTyped vs tys := by
  constructor
  · intro h; cases h with
    | cons hv hvs => exact ⟨_, _, rfl, hv, hvs⟩
  · rintro ⟨v, vs, rfl, hv, hvs⟩; exact .cons hv hvs

@[simp] theorem stackTyped_i64_iff (s : Stack) :
    StackTyped s [.i64] ↔ ∃ n, s = [.i64 n] := by
  simp only [stackTyped_cons_iff, stackTyped_nil_iff, hasType_i64_iff]
  constructor
  · rintro ⟨v, vs, rfl, ⟨n, rfl⟩, rfl⟩; exact ⟨n, rfl⟩
  · rintro ⟨n, rfl⟩; exact ⟨.i64 n, [], rfl, ⟨n, rfl⟩, rfl⟩

end NobleContracts
