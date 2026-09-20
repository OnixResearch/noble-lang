import NobleContracts.Model

namespace NobleContracts

attribute [local simp] List.append_assoc

/-! Structural inversion lemmas are the symbolic executor.  They use only
kernel-checked constructor inversion; no evaluator, fuel, reflection oracle,
or native decision procedure is trusted by an application proof. -/

namespace Step

@[simp] theorem lit_iff {v s t} : Step (.lit v) s t ↔ t = v :: s := by
  constructor
  · intro h; cases h; rfl
  · intro h; subst t; exact .lit

@[simp] theorem block_iff {p s t} : Step (.block p) s t ↔ t = .program p :: s := by
  constructor
  · intro h; cases h; rfl
  · intro h; subst t; exact .block

@[simp] theorem dup_iff {v s t} : Step (.word 0) (v :: s) t ↔ t = v :: v :: s := by
  constructor
  · intro h; cases h; rfl
  · intro h; subst t; exact .dup

@[simp] theorem drop_iff {v s t} : Step (.word 1) (v :: s) t ↔ t = s := by
  constructor
  · intro h; cases h; rfl
  · intro h; subst t; exact .drop

@[simp] theorem swap_iff {a b s t} :
    Step (.word 2) (b :: a :: s) t ↔ t = a :: b :: s := by
  constructor
  · intro h; cases h; rfl
  · intro h; subst t; exact .swap

@[simp] theorem dip_iff {p v s t} :
    Step (.word 3) (.program p :: v :: s) t ↔ ∃ u, Run p s u ∧ t = v :: u := by
  constructor
  · intro h; cases h with
    | dip hp => exact ⟨_, hp, rfl⟩
  · rintro ⟨u, hp, rfl⟩; exact .dip hp

@[simp] theorem add_iff {a b : BitVec 64} {s t} :
    Step (.word 4) (.i64 b :: .i64 a :: s) t ↔ t = .i64 (a + b) :: s := by
  constructor
  · intro h; cases h; rfl
  · intro h; subst t; exact .add

@[simp] theorem sub_iff {a b : BitVec 64} {s t} :
    Step (.word 5) (.i64 b :: .i64 a :: s) t ↔ t = .i64 (a - b) :: s := by
  constructor
  · intro h; cases h; rfl
  · intro h; subst t; exact .sub

@[simp] theorem mul_iff {a b : BitVec 64} {s t} :
    Step (.word 6) (.i64 b :: .i64 a :: s) t ↔ t = .i64 (a * b) :: s := by
  constructor
  · intro h; cases h; rfl
  · intro h; subst t; exact .mul

@[simp] theorem equals_iff {a b : BitVec 64} {s t} :
    Step (.word 7) (.i64 b :: .i64 a :: s) t ↔ t = .bool (a == b) :: s := by
  constructor
  · intro h; cases h; rfl
  · intro h; subst t; exact .equals

@[simp] theorem quote_iff {v s t} :
    Step (.word 8) (v :: s) t ↔ t = .program [.lit v] :: s := by
  constructor
  · intro h; cases h; rfl
  · intro h; subst t; exact .quote

@[simp] theorem compose_iff {p q s t} :
    Step (.word 9) (.program q :: .program p :: s) t ↔ t = .program (p ++ q) :: s := by
  constructor
  · intro h; cases h; rfl
  · intro h; subst t; exact .compose

@[simp] theorem run_iff {p s t} : Step (.word 10) (.program p :: s) t ↔ Run p s t := by
  constructor
  · intro h; cases h with
    | run hp => exact hp
  · intro hp; exact .run hp

@[simp] theorem reflect_iff {p s t} :
    Step (.word 11) (.program p :: s) t ↔ t = .«syntax» p :: s := by
  constructor
  · intro h; cases h; rfl
  · intro h; subst t; exact .reflect

@[simp] theorem unit_iff {s t} : Step (.word 12) s t ↔ t = .unit :: s := by
  constructor
  · intro h; cases h; rfl
  · intro h; subst t; exact .unit

@[simp] theorem pair_iff {a b s t} :
    Step (.word 13) (b :: a :: s) t ↔ t = .pair a b :: s := by
  constructor
  · intro h; cases h; rfl
  · intro h; subst t; exact .pair

@[simp] theorem unpair_iff {a b s t} :
    Step (.word 14) (.pair a b :: s) t ↔ t = b :: a :: s := by
  constructor
  · intro h; cases h; rfl
  · intro h; subst t; exact .unpair

@[simp] theorem inl_iff {v s t} :
    Step (.word 15) (v :: s) t ↔ t = .inl v :: s := by
  constructor
  · intro h; cases h; rfl
  · intro h; subst t; exact .inl

@[simp] theorem inr_iff {v s t} :
    Step (.word 16) (v :: s) t ↔ t = .inr v :: s := by
  constructor
  · intro h; cases h; rfl
  · intro h; subst t; exact .inr

@[simp] theorem case_left_iff {p q v s t} :
    Step (.word 17) (.program q :: .program p :: .inl v :: s) t ↔ Run p (v :: s) t := by
  constructor
  · intro h; cases h with
    | caseLeft hp => exact hp
  · intro hp; exact .caseLeft hp

@[simp] theorem case_right_iff {p q v s t} :
    Step (.word 17) (.program q :: .program p :: .inr v :: s) t ↔ Run q (v :: s) t := by
  constructor
  · intro h; cases h with
    | caseRight hp => exact hp
  · intro hp; exact .caseRight hp

@[simp] theorem if_true_iff {p q s t} :
    Step (.word 18) (.program q :: .program p :: .bool true :: s) t ↔ Run p s t := by
  constructor
  · intro h; cases h with
    | ifTrue hp => exact hp
  · intro hp; exact .ifTrue hp

@[simp] theorem if_false_iff {p q s t} :
    Step (.word 18) (.program q :: .program p :: .bool false :: s) t ↔ Run q s t := by
  constructor
  · intro h; cases h with
    | ifFalse hp => exact hp
  · intro hp; exact .ifFalse hp

@[simp] theorem nil_iff {s t} : Step (.word 19) s t ↔ t = .list [] :: s := by
  constructor
  · intro h; cases h; rfl
  · intro h; subst t; exact .nil

@[simp] theorem cons_iff {v vs s t} :
    Step (.word 20) (.list vs :: v :: s) t ↔ t = .list (v :: vs) :: s := by
  constructor
  · intro h; cases h; rfl
  · intro h; subst t; exact .cons

@[simp] theorem list_nil_iff {p q s t} :
    Step (.word 21) (.program q :: .program p :: .list [] :: s) t ↔ Run p s t := by
  constructor
  · intro h; cases h with
    | listNil hp => exact hp
  · intro hp; exact .listNil hp

@[simp] theorem list_cons_iff {p q v vs s t} :
    Step (.word 21) (.program q :: .program p :: .list (v :: vs) :: s) t ↔
      Run q (.list vs :: v :: s) t := by
  constructor
  · intro h; cases h with
    | listCons hp => exact hp
  · intro hp; exact .listCons hp

/-- MC1 cannot prove a normal pure transition for the host operation. -/
@[simp] theorem host_unsupported {s t} : ¬ Step (.word 22) s t := by
  intro h; cases h

end Step

namespace Run

@[simp] theorem nil_iff {s t} : Run [] s t ↔ t = s := by
  constructor
  · intro h; cases h; rfl
  · intro h; subst t; exact .nil

@[simp] theorem cons_iff {op ops s t} :
    Run (op :: ops) s t ↔ ∃ m, Step op s m ∧ Run ops m t := by
  constructor
  · intro h; cases h with
    | cons hs hr => exact ⟨_, hs, hr⟩
  · rintro ⟨m, hs, hr⟩; exact .cons hs hr

/-- Concatenation factors through the actual intermediate stack. -/
theorem append_iff {a b s t} :
    Run (a ++ b) s t ↔ ∃ m, Run a s m ∧ Run b m t := by
  induction a generalizing s with
  | nil => simp
  | cons op ops ih =>
      constructor
      · intro h
        obtain ⟨u, hop, hrest⟩ := cons_iff.mp h
        obtain ⟨m, ha, hb⟩ := ih.mp hrest
        exact ⟨m, .cons hop ha, hb⟩
      · rintro ⟨m, ha, hb⟩
        obtain ⟨u, hop, hrest⟩ := cons_iff.mp ha
        exact .cons hop (ih.mpr ⟨m, hrest, hb⟩)

end Run

@[simp] theorem exec_nil_iff {s t} : Exec [] s t ↔ t = s := by
  simp [Exec]

/-- Sequencing is relational composition, not an assumption about matching
preconditions.  The implication needed by contract composition appears below. -/
theorem exec_append_iff {a b s t} :
    Exec (a ++ b) s t ↔ ∃ m, Exec a s m ∧ Exec b m t := by
  constructor
  · intro h
    obtain ⟨m, ha, hb⟩ := Run.append_iff.mp h
    refine ⟨m.reverse, ?_, ?_⟩
    · simpa [Exec] using ha
    · simpa [Exec] using hb
  · rintro ⟨m, ha, hb⟩
    exact Run.append_iff.mpr ⟨m.reverse, ha, hb⟩

@[simp] theorem exec_lit_iff {v s t} : Exec [.lit v] s t ↔ t = s ++ [v] := by
  simp [Exec]

@[simp] theorem exec_block_iff {p s t} :
    Exec [.block p] s t ↔ t = s ++ [.program p] := by
  simp [Exec]

@[simp] theorem exec_dup_iff {v s t} :
    Exec [.word 0] (s ++ [v]) t ↔ t = s ++ [v, v] := by
  simp [Exec, List.reverse_append]

@[simp] theorem exec_drop_iff {v s t} : Exec [.word 1] (s ++ [v]) t ↔ t = s := by
  simp [Exec, List.reverse_append]

@[simp] theorem exec_swap_iff {a b s t} :
    Exec [.word 2] (s ++ [a, b]) t ↔ t = s ++ [b, a] := by
  simp [Exec, List.reverse_append]

@[simp] theorem exec_add_iff {a b : BitVec 64} {s t} :
    Exec [.word 4] (s ++ [.i64 a, .i64 b]) t ↔ t = s ++ [.i64 (a + b)] := by
  simp [Exec, List.reverse_append]

@[simp] theorem exec_sub_iff {a b : BitVec 64} {s t} :
    Exec [.word 5] (s ++ [.i64 a, .i64 b]) t ↔ t = s ++ [.i64 (a - b)] := by
  simp [Exec, List.reverse_append]

@[simp] theorem exec_mul_iff {a b : BitVec 64} {s t} :
    Exec [.word 6] (s ++ [.i64 a, .i64 b]) t ↔ t = s ++ [.i64 (a * b)] := by
  simp [Exec, List.reverse_append]

@[simp] theorem exec_equals_iff {a b : BitVec 64} {s t} :
    Exec [.word 7] (s ++ [.i64 a, .i64 b]) t ↔ t = s ++ [.bool (a == b)] := by
  simp [Exec, List.reverse_append]

@[simp] theorem exec_quote_iff {v s t} :
    Exec [.word 8] (s ++ [v]) t ↔ t = s ++ [.program [.lit v]] := by
  simp [Exec, List.reverse_append]

@[simp] theorem exec_compose_iff {p q s t} :
    Exec [.word 9] (s ++ [.program p, .program q]) t ↔
      t = s ++ [.program (p ++ q)] := by
  simp [Exec, List.reverse_append]

@[simp] theorem exec_run_iff {p s t} :
    Exec [.word 10] (s ++ [.program p]) t ↔ Exec p s t := by
  simp [Exec, List.reverse_append]

@[simp] theorem exec_reflect_iff {p s t} :
    Exec [.word 11] (s ++ [.program p]) t ↔ t = s ++ [.«syntax» p] := by
  simp [Exec, List.reverse_append]

@[simp] theorem exec_unit_iff {s t} : Exec [.word 12] s t ↔ t = s ++ [.unit] := by
  simp [Exec]

@[simp] theorem exec_pair_iff {a b s t} :
    Exec [.word 13] (s ++ [a, b]) t ↔ t = s ++ [.pair a b] := by
  simp [Exec, List.reverse_append]

@[simp] theorem exec_unpair_iff {a b s t} :
    Exec [.word 14] (s ++ [.pair a b]) t ↔ t = s ++ [a, b] := by
  simp [Exec, List.reverse_append]

@[simp] theorem exec_inl_iff {v s t} :
    Exec [.word 15] (s ++ [v]) t ↔ t = s ++ [.inl v] := by
  simp [Exec, List.reverse_append]

@[simp] theorem exec_inr_iff {v s t} :
    Exec [.word 16] (s ++ [v]) t ↔ t = s ++ [.inr v] := by
  simp [Exec, List.reverse_append]

@[simp] theorem exec_nil_word_iff {s t} : Exec [.word 19] s t ↔ t = s ++ [.list []] := by
  simp [Exec]

@[simp] theorem exec_cons_iff {v vs s t} :
    Exec [.word 20] (s ++ [v, .list vs]) t ↔ t = s ++ [.list (v :: vs)] := by
  simp [Exec, List.reverse_append]

/-- The hidden value is restored after, not before, the invoked program. -/
theorem exec_dip_iff {p v s t} :
    Exec [.word 3] (s ++ [v, .program p]) t ↔
      ∃ u, Exec p s u ∧ t = u ++ [v] := by
  constructor
  · intro h
    have h' : ∃ u, Run p s.reverse u ∧ t.reverse = v :: u := by
      simpa [Exec, List.reverse_append] using h
    obtain ⟨u, hp, ht⟩ := h'
    refine ⟨u.reverse, ?_, ?_⟩
    · simpa [Exec] using hp
    · have hr := congrArg List.reverse ht
      simpa using hr
  · rintro ⟨u, hp, rfl⟩
    have h' : ∃ r, Run p s.reverse r ∧ (u ++ [v]).reverse = v :: r :=
      ⟨u.reverse, hp, by simp⟩
    simpa [Exec, List.reverse_append] using h'

@[simp] theorem exec_case_left_iff {p q v s t} :
    Exec [.word 17] (s ++ [.inl v, .program p, .program q]) t ↔
      Exec p (s ++ [v]) t := by
  simp [Exec, List.reverse_append]

@[simp] theorem exec_case_right_iff {p q v s t} :
    Exec [.word 17] (s ++ [.inr v, .program p, .program q]) t ↔
      Exec q (s ++ [v]) t := by
  simp [Exec, List.reverse_append]

@[simp] theorem exec_if_true_iff {p q s t} :
    Exec [.word 18] (s ++ [.bool true, .program p, .program q]) t ↔ Exec p s t := by
  simp [Exec, List.reverse_append]

@[simp] theorem exec_if_false_iff {p q s t} :
    Exec [.word 18] (s ++ [.bool false, .program p, .program q]) t ↔ Exec q s t := by
  simp [Exec, List.reverse_append]

@[simp] theorem exec_list_nil_iff {p q s t} :
    Exec [.word 21] (s ++ [.list [], .program p, .program q]) t ↔ Exec p s t := by
  simp [Exec, List.reverse_append]

@[simp] theorem exec_list_cons_iff {p q v vs s t} :
    Exec [.word 21] (s ++ [.list (v :: vs), .program p, .program q]) t ↔
      Exec q (s ++ [v, .list vs]) t := by
  simp [Exec, List.reverse_append]

/-- A run of a composition must pass through the first program's actual result. -/
theorem exec_compose_run_iff {p q s t} :
    Exec [.word 9, .word 10] (s ++ [.program p, .program q]) t ↔
      ∃ u, Exec p s u ∧ Exec q u t := by
  have h : Exec [.word 9, .word 10] (s ++ [.program p, .program q]) t ↔
      Exec (p ++ q) s t := by simp [Exec, List.reverse_append]
  exact h.trans exec_append_iff

/-- Quoting captures the value, and running that quotation pushes that value. -/
@[simp] theorem exec_quote_run_iff {v s t} :
    Exec [.word 8, .word 10] (s ++ [v]) t ↔ t = s ++ [v] := by
  simp [Exec, List.reverse_append]

/-- Partial correctness: only normal returns are constrained. -/
def PC (code : List Op) (P : Stack → Prop) (Q : Stack → Stack → Prop) : Prop :=
  ∀ s t, P s → Exec code s t → Q s t

/-- Ordinary consequence, with both implications stated explicitly. -/
theorem pc_consequence {code P P' Q Q'} (h : PC code P Q)
    (pre : ∀ s, P' s → P s) (post : ∀ s t, P' s → Q s t → Q' s t) :
    PC code P' Q' := by
  intro s t hs he
  exact post s t hs (h s t (pre s hs) he)

/-- General sequence rule. `bridge` is the indispensable intermediate
implication; sharing the same I/O types alone is not enough. -/
theorem pc_sequence {first second P R P' Q T}
    (left : PC first P R) (right : PC second P' Q)
    (bridge : ∀ s m, P s → R s m → P' m)
    (join : ∀ s m t, P s → R s m → Q m t → T s t) :
    PC (first ++ second) P T := by
  intro s t hs he
  obtain ⟨m, ha, hb⟩ := exec_append_iff.mp he
  have hr := left s m hs ha
  exact join s m t hs hr (right m t (bridge s m hs hr) hb)

/-- Exact-result rules specialize directly to a partial-correctness contract. -/
theorem pc_exact {code s expected}
    (rule : ∀ t, Exec code s t ↔ t = expected) :
    PC code (fun before => before = s) (fun _ after => after = expected) := by
  intro before after hbefore he
  subst before
  exact (rule after).mp he

/-- `dip` lifts a contract without granting the body access to the saved value. -/
theorem pc_dip {p P Q} (body : PC p P Q) (v : Value) :
    PC [.word 3] (fun input => ∃ s, P s ∧ input = s ++ [v, .program p])
      (fun input output => ∃ s t,
        input = s ++ [v, .program p] ∧ output = t ++ [v] ∧ Q s t) := by
  intro input output hi he
  obtain ⟨s, hs, rfl⟩ := hi
  obtain ⟨t, hp, hout⟩ := exec_dip_iff.mp he
  exact ⟨s, t, rfl, hout, body s t hs hp⟩

/-- Invocation uses the supplied program's contract; no property is inferred
from the fact that the input merely has a program type. -/
theorem pc_run {p P Q} (body : PC p P Q) :
    PC [.word 10] (fun input => ∃ s, P s ∧ input = s ++ [.program p])
      (fun input output => ∃ s, input = s ++ [.program p] ∧ Q s output) := by
  intro input output hi he
  obtain ⟨s, hs, rfl⟩ := hi
  exact ⟨s, rfl, body s output hs (exec_run_iff.mp he)⟩

/-- Both boolean branches establish the same postcondition. -/
theorem pc_if_join {p q P Q} (yes : PC p P Q) (no : PC q P Q)
    (b : Bool) {s t} (pre : P s)
    (he : Exec [.word 18] (s ++ [.bool b, .program p, .program q]) t) : Q s t := by
  cases b with
  | false => exact no s t pre (exec_if_false_iff.mp he)
  | true => exact yes s t pre (exec_if_true_iff.mp he)

/-- Sum branches keep the payload and share the declared join predicate. -/
theorem pc_case_join {p q} {L R : Value → Stack → Prop} {Q : Stack → Stack → Prop}
    (left : ∀ v s t, L v s → Exec p (s ++ [v]) t → Q s t)
    (right : ∀ v s t, R v s → Exec q (s ++ [v]) t → Q s t)
    {value s t}
    (pre : (∃ v, value = .inl v ∧ L v s) ∨ (∃ v, value = .inr v ∧ R v s))
    (he : Exec [.word 17] (s ++ [value, .program p, .program q]) t) : Q s t := by
  rcases pre with ⟨v, rfl, hv⟩ | ⟨v, rfl, hv⟩
  · exact left v s t hv (exec_case_left_iff.mp he)
  · exact right v s t hv (exec_case_right_iff.mp he)

/-- The nonempty-list branch receives head then tail, with the tail on top. -/
theorem pc_listcase_join {p q} {P : List Value → Stack → Prop}
    {Q : Stack → Stack → Prop}
    (nilBranch : ∀ s t, P [] s → Exec p s t → Q s t)
    (consBranch : ∀ v vs s t, P (v :: vs) s →
      Exec q (s ++ [v, .list vs]) t → Q s t)
    {vs s t} (pre : P vs s)
    (he : Exec [.word 21] (s ++ [.list vs, .program p, .program q]) t) : Q s t := by
  cases vs with
  | nil => exact nilBranch s t pre (exec_list_nil_iff.mp he)
  | cons v vs => exact consBranch v vs s t pre (exec_list_cons_iff.mp he)

end NobleContracts
