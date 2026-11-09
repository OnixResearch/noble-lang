import NobleContracts.Rules

/-!
`wire@1` laws in the pure MC1 fragment (`.cairn/changes/resource-safe-wiring-laws`, laws
L1-L8 and L10-L14 of its law matrix; L9, exchange, is not adopted).

The library words are inlined as code (route (a) of the change's proof plan): each definition
below is the body of the `wire@1` word of the same name, with `rot` spelled out. A law `x ≈ y`
is stated as `Exec x s t ↔ Exec y s t`, the normal-return relation of the model. Where the
right side is empty code or consumes a value, the law is stated on the stack shape that the
signature requires, because the untyped relation is otherwise vacuous on one side only.

These are model theorems. They say nothing about effects, resources, failure, quota, the
kernel's typing schemes or the runtime; those remain the change's separate obligations.
-/

namespace NobleContracts.Wire

open NobleContracts

/-- `def rot [ [ swap ] dip swap ]`: `S A B C -- S B C A`. -/
def rot : List Op := [.block [.word 2], .word 3, .word 2]

/-- `def first [ swap unpair rot dip pair ]`. -/
def first : List Op := [.word 2, .word 14] ++ rot ++ [.word 3, .word 13]

/-- `def second [ swap unpair swap rot dip swap pair ]`. -/
def second : List Op := [.word 2, .word 14, .word 2] ++ rot ++ [.word 3, .word 2, .word 13]

/-- `def split [ [ first ] dip second ]`. -/
def split : List Op := [.block first, .word 3] ++ second

/-- `def fanout [ [ [ dup pair ] dip ] dip split ]`. -/
def fanout : List Op := [.block [.block [.word 0, .word 13], .word 3], .word 3] ++ split

/-- `def left [ [ inl ] compose [ inr ] case ]`. -/
def left : List Op := [.block [.word 15], .word 9, .block [.word 16], .word 17]

/-- `def join [ case ]`. -/
def join : List Op := [.word 17]

/-- `def swap_pair [ unpair swap pair ]`. -/
def swapPair : List Op := [.word 14, .word 2, .word 13]

/-- `def assoc_pair [ unpair [ unpair ] dip pair pair ]`. -/
def assocPair : List Op := [.word 14, .block [.word 14], .word 3, .word 13, .word 13]

/-- The inverse of `assoc_pair`: `unpair unpair [ pair ] dip pair`. -/
def assocPairInv : List Op := [.word 14, .word 14, .block [.word 13], .word 3, .word 13]

/-! ## The frame lemma -/

mutual
  /-- A run leaves every value below the stack it touches unchanged. -/
  theorem run_frame {p : List Op} {s t : Stack} (r : Stack) :
      Run p s t → Run p (s ++ r) (t ++ r)
    | .nil => .nil
    | .cons h rest => .cons (step_frame r h) (run_frame r rest)

  /-- One operation leaves every value below the stack it touches unchanged. -/
  theorem step_frame {op : Op} {s t : Stack} (r : Stack) :
      Step op s t → Step op (s ++ r) (t ++ r)
    | .lit => .lit
    | .block => .block
    | .dup => .dup
    | .drop => .drop
    | .swap => .swap
    | .dip h => .dip (run_frame r h)
    | .add => .add
    | .sub => .sub
    | .mul => .mul
    | .equals => .equals
    | .quote => .quote
    | .compose => .compose
    | .run h => .run (run_frame r h)
    | .reflect => .reflect
    | .unit => .unit
    | .pair => .pair
    | .unpair => .unpair
    | .inl => .inl
    | .inr => .inr
    | .caseLeft h => .caseLeft (run_frame r h)
    | .caseRight h => .caseRight (run_frame r h)
    | .ifTrue h => .ifTrue (run_frame r h)
    | .ifFalse h => .ifFalse (run_frame r h)
    | .nil => .nil
    | .cons => .cons
    | .listNil h => .listNil (run_frame r h)
    | .listCons h => .listCons (run_frame r h)
end

/-- **The frame lemma** (public, bottom-first stacks): values below the ones a program uses
pass through it unchanged. -/
theorem exec_frame {p : List Op} {s t : Stack} (r : Stack) (h : Exec p s t) :
    Exec p (r ++ s) (r ++ t) := by
  unfold Exec at *
  simpa [List.reverse_append] using run_frame r.reverse h

/-! ## Helpers -/

/-- Rewriting the code after a leading operation. -/
theorem run_cons_congr {op : Op} {xs ys : List Op} {s t : Stack}
    (h : ∀ m, Run xs m t ↔ Run ys m t) : Run (op :: xs) s t ↔ Run (op :: ys) s t := by
  simp only [Run.cons_iff, h]

/-- Pushing a block and dipping under it runs the dipped code, then pushes the block. -/
theorem run_push_block_dip {q c rest : List Op} {s t : Stack} :
    Run (.block q :: .block c :: .word 3 :: rest) s t ↔ Run (c ++ .block q :: rest) s t := by
  simp only [Run.cons_iff, Run.append_iff, Step.block_iff, Step.dip_iff, exists_eq_left]
  constructor
  · rintro ⟨_, ⟨u, hc, rfl⟩, hr⟩
    exact ⟨u, hc, hr⟩
  · rintro ⟨u, hc, hr⟩
    exact ⟨_, ⟨u, hc, rfl⟩, hr⟩

/-- `pair` on a variable stack: it needs a second value. -/
theorem step_pair_iff {b : Value} {u t : Stack} :
    Step (.word 13) (b :: u) t ↔ ∃ c rest, u = c :: rest ∧ t = .pair c b :: rest := by
  constructor
  · intro h
    cases h
    exact ⟨_, _, rfl, rfl⟩
  · rintro ⟨c, rest, rfl, rfl⟩
    exact .pair

/-- `[p] first` on a pair, top-first: `p` runs on the first component with the second hidden. -/
theorem run_first_iff {p : List Op} {sr tr : Stack} {a b : Value} :
    Run (.block p :: first) (.pair a b :: sr) tr ↔
      ∃ c rest, Run p (a :: sr) (c :: rest) ∧ tr = .pair c b :: rest := by
  simp only [first, rot, List.cons_append, List.nil_append, Run.cons_iff, Run.nil_iff,
    Step.block_iff, Step.swap_iff, Step.unpair_iff, Step.dip_iff, exists_eq_left]
  constructor
  · rintro ⟨_, ⟨u, hp, rfl⟩, m, hpair, rfl⟩
    obtain ⟨c, rest, rfl, rfl⟩ := step_pair_iff.mp hpair
    exact ⟨c, rest, hp, rfl⟩
  · rintro ⟨c, rest, hp, rfl⟩
    exact ⟨_, ⟨_, hp, rfl⟩, _, step_pair_iff.mpr ⟨c, rest, rfl, rfl⟩, rfl⟩

/-- `[p] first` on a pair runs `p` on the pair's first component, hiding the second. -/
theorem first_iff {p : List Op} {s : Stack} {a b : Value} {t : Stack} :
    Exec (.block p :: first) (s ++ [.pair a b]) t ↔
      ∃ u c, Exec p (s ++ [a]) (u ++ [c]) ∧ t = u ++ [.pair c b] := by
  simp only [Exec, List.reverse_append, List.reverse_cons, List.reverse_nil, List.nil_append,
    List.cons_append]
  rw [run_first_iff]
  constructor
  · rintro ⟨c, rest, hp, ht⟩
    refine ⟨rest.reverse, c, by simpa using hp, ?_⟩
    simpa using congrArg List.reverse ht
  · rintro ⟨u, c, hp, rfl⟩
    exact ⟨c, u.reverse, by simpa using hp, by simp⟩

/-! ## Sequencing: L1-L4 -/

/-- **L1** left identity: `[ ] p compose` ≈ `p`. -/
theorem then_left_id {p : List Op} {s t : Stack} :
    Exec [.block [], .block p, .word 9] s t ↔ Exec [.block p] s t := by
  simp [Exec]

/-- **L2** right identity: `p [ ] compose` ≈ `p`. -/
theorem then_right_id {p : List Op} {s t : Stack} :
    Exec [.block p, .block [], .word 9] s t ↔ Exec [.block p] s t := by
  simp [Exec]

/-- **L3** associativity: `p q compose r compose` ≈ `p q r compose compose`. -/
theorem then_assoc {p q r : List Op} {s t : Stack} :
    Exec [.block p, .block q, .word 9, .block r, .word 9] s t ↔
      Exec [.block p, .block q, .block r, .word 9, .word 9] s t := by
  simp [Exec]

/-- **L4** apply, for a quotation: `v quote run` ≈ pushing `v`. -/
theorem apply_quote {v : Value} {s t : Stack} :
    Exec [.lit v, .word 8, .word 10] s t ↔ Exec [.lit v] s t := by
  simp [Exec]

/-- **L4** apply, for a composition: `p q compose run` ≈ `p run q run`. -/
theorem apply_then {p q : List Op} {s t : Stack} :
    Exec [.block p, .block q, .word 9, .word 10] s t ↔
      Exec [.block p, .word 10, .block q, .word 10] s t := by
  simp [Exec, Run.append_iff]

/-! ## Frames and pairs: L5-L8, L10 -/

/-- `[ p ] under [ q ] under` against `[ p q ] under`, top-first. -/
theorem run_under_fuse {p q : List Op} {sr tr : Stack} {v : Value} :
    Run [.block p, .word 3, .block q, .word 3] (v :: sr) tr ↔
      Run [.block (p ++ q), .word 3] (v :: sr) tr := by
  simp only [Run.cons_iff, Run.nil_iff, Step.block_iff, Step.dip_iff, exists_eq_left,
    Run.append_iff]
  constructor
  · rintro ⟨_, ⟨u, hp, rfl⟩, m, hdip, rfl⟩
    obtain ⟨w, hq, rfl⟩ := Step.dip_iff.mp hdip
    exact ⟨_, ⟨w, ⟨u, hp, hq⟩, rfl⟩, rfl⟩
  · rintro ⟨_, ⟨w, ⟨u, hp, hq⟩, rfl⟩, rfl⟩
    exact ⟨_, ⟨u, hp, rfl⟩, _, Step.dip_iff.mpr ⟨w, hq, rfl⟩, rfl⟩

/-- **L5** frame fusion: `[ p ] under [ q ] under` ≈ `[ p q ] under`, for the held `v`. -/
theorem under_fuse {p q : List Op} {s : Stack} {v : Value} {t : Stack} :
    Exec [.block p, .word 3, .block q, .word 3] (s ++ [v]) t ↔
      Exec [.block (p ++ q), .word 3] (s ++ [v]) t := by
  simp only [Exec, List.reverse_append, List.reverse_cons, List.reverse_nil, List.nil_append,
    List.cons_append]
  exact run_under_fuse

/-- **L6** `pair unpair` ≈ `[ ]` on two values. -/
theorem pair_unpair {a b : Value} {s t : Stack} :
    Exec [.word 13, .word 14] (s ++ [a, b]) t ↔ t = s ++ [a, b] := by
  simp [Exec, List.reverse_append]

/-- **L6** `unpair pair` ≈ `[ ]` on a pair. -/
theorem unpair_pair {a b : Value} {s t : Stack} :
    Exec [.word 14, .word 13] (s ++ [.pair a b]) t ↔ t = s ++ [.pair a b] := by
  simp [Exec, List.reverse_append]

/-- **L6** `swap_pair` swaps a pair's components. -/
theorem swap_pair_iff {a b : Value} {s t : Stack} :
    Exec swapPair (s ++ [.pair a b]) t ↔ t = s ++ [.pair b a] := by
  simp [Exec, swapPair, List.reverse_append]

/-- **L6** `swap_pair swap_pair` ≈ `[ ]` on a pair. -/
theorem swap_pair_invol {a b : Value} {s t : Stack} :
    Exec (swapPair ++ swapPair) (s ++ [.pair a b]) t ↔ t = s ++ [.pair a b] := by
  simp [Exec, swapPair, List.reverse_append]

/-- **L6** `assoc_pair` reassociates a nested pair. -/
theorem assoc_pair_iff {a b c : Value} {s t : Stack} :
    Exec assocPair (s ++ [.pair (.pair a b) c]) t ↔ t = s ++ [.pair a (.pair b c)] := by
  simp [Exec, assocPair, List.reverse_append]

/-- **L6** `assoc_pair` has an inverse: followed by it, the identity on its input. -/
theorem assoc_pair_iso {a b c : Value} {s t : Stack} :
    Exec (assocPair ++ assocPairInv) (s ++ [.pair (.pair a b) c]) t ↔
      t = s ++ [.pair (.pair a b) c] := by
  simp [Exec, assocPair, assocPairInv, List.reverse_append]

/-- **L6** the inverse followed by `assoc_pair` is the identity on its input. -/
theorem assoc_pair_iso_symm {a b c : Value} {s t : Stack} :
    Exec (assocPairInv ++ assocPair) (s ++ [.pair a (.pair b c)]) t ↔
      t = s ++ [.pair a (.pair b c)] := by
  simp [Exec, assocPair, assocPairInv, List.reverse_append]

/-- **L7** first fusion: `[p] first [q] first` ≈ `[p] [q] compose first`. The premise is what
`Program<S+A,S+C>` guarantees: every result of `p` on the first component has a top value. -/
theorem first_fuse {p q : List Op} {s : Stack} {a b : Value} {t : Stack}
    (typed : ∀ m, Exec p (s ++ [a]) m → m ≠ []) :
    Exec (.block p :: first ++ .block q :: first) (s ++ [.pair a b]) t ↔
      Exec (.block p :: .block q :: .word 9 :: first) (s ++ [.pair a b]) t := by
  have composed : Exec (.block p :: .block q :: .word 9 :: first) (s ++ [.pair a b]) t ↔
      Exec (.block (p ++ q) :: first) (s ++ [.pair a b]) t := by
    simp only [Exec, Run.cons_iff, Step.block_iff, Step.compose_iff, exists_eq_left]
  rw [composed, first_iff, exec_append_iff]
  constructor
  · rintro ⟨m, h1, h2⟩
    obtain ⟨u, c, hp, rfl⟩ := first_iff.mp h1
    obtain ⟨w, d, hq, rfl⟩ := first_iff.mp h2
    exact ⟨w, d, exec_append_iff.mpr ⟨u ++ [c], hp, hq⟩, rfl⟩
  · rintro ⟨w, d, hpq, rfl⟩
    obtain ⟨m, hp, hq⟩ := exec_append_iff.mp hpq
    obtain ⟨u, c, rfl⟩ := (List.eq_nil_or_concat m).resolve_left (typed m hp)
    rw [List.concat_eq_append] at hp hq
    exact ⟨u ++ [.pair c b], first_iff.mpr ⟨u, c, hp, rfl⟩, first_iff.mpr ⟨w, d, hq, rfl⟩⟩

/-- **L8** split definition: `[p] [q] split` ≈ `[p] first [q] second`. -/
theorem split_def {p q : List Op} {s t : Stack} :
    Exec ([.block p, .block q] ++ split) s t ↔
      Exec (.block p :: first ++ .block q :: second) s t := by
  unfold Exec
  simp only [split, List.cons_append, List.nil_append]
  exact run_cons_congr fun _ => run_push_block_dip

/-- **L10** fanout definition: `x [p] [q] fanout` ≈ `x dup pair [p] [q] split`. -/
theorem fanout_def {p q : List Op} {s t : Stack} :
    Exec ([.block p, .block q] ++ fanout) s t ↔
      Exec ([.word 0, .word 13, .block p, .block q] ++ split) s t := by
  unfold Exec
  simp only [fanout, List.cons_append, List.nil_append]
  rw [run_cons_congr fun _ => run_push_block_dip]
  simp only [List.cons_append, List.nil_append]
  exact run_push_block_dip

/-! ## Sums and branches: L11-L13 -/

/-- **L11** choice β on `inl`: `v inl [p] left` ≈ `v p inl`. -/
theorem left_beta_inl {p : List Op} {s : Stack} {v : Value} {t : Stack} :
    Exec ([.word 15, .block p] ++ left) (s ++ [v]) t ↔ Exec (p ++ [.word 15]) (s ++ [v]) t := by
  simp [Exec, left, List.reverse_append]

/-- **L11** choice β on `inr`: `v inr [p] left` ≈ `v inr`. -/
theorem left_beta_inr {p : List Op} {s : Stack} {v : Value} {t : Stack} :
    Exec ([.word 16, .block p] ++ left) (s ++ [v]) t ↔ Exec [.word 16] (s ++ [v]) t := by
  simp [Exec, left, List.reverse_append]

/-- **L12** join naturality on `inl`: `s [p] [q] join r` ≈ `s [p r] [q r] join`. -/
theorem join_natural_inl {p q r : List Op} {s : Stack} {v : Value} {t : Stack} :
    Exec ([.block p, .block q] ++ join ++ r) (s ++ [.inl v]) t ↔
      Exec ([.block (p ++ r), .block (q ++ r)] ++ join) (s ++ [.inl v]) t := by
  simp [Exec, join, List.reverse_append, Run.append_iff]

/-- **L12** join naturality on `inr`. -/
theorem join_natural_inr {p q r : List Op} {s : Stack} {v : Value} {t : Stack} :
    Exec ([.block p, .block q] ++ join ++ r) (s ++ [.inr v]) t ↔
      Exec ([.block (p ++ r), .block (q ++ r)] ++ join) (s ++ [.inr v]) t := by
  simp [Exec, join, List.reverse_append, Run.append_iff]

/-- **L13** branch β: `true [p] [q] if` ≈ `p`. -/
theorem if_beta_true {p q : List Op} {s t : Stack} :
    Exec [.block p, .block q, .word 18] (s ++ [.bool true]) t ↔ Exec p s t := by
  simp [Exec, List.reverse_append]

/-- **L13** branch β: `false [p] [q] if` ≈ `q`. -/
theorem if_beta_false {p q : List Op} {s t : Stack} :
    Exec [.block p, .block q, .word 18] (s ++ [.bool false]) t ↔ Exec q s t := by
  simp [Exec, List.reverse_append]

/-! ## L14: behavioral equivalence is not identity -/

/-- `[ 1 1 + ]`, one of the two recipes of L14. -/
def onePlusOne : List Op := [.lit (i64 1), .lit (i64 1), .word 4]

/-- `[ 2 ]`, the other. -/
def two : List Op := [.lit (i64 2)]

/-- **L14** the two recipes have the same normal result when run. -/
theorem recipes_run_alike {s t : Stack} :
    Exec [.block onePlusOne, .word 10] s t ↔ Exec [.block two, .word 10] s t := by
  simp [Exec, onePlusOne, two, i64]

/-- **L14** `reflect` tells them apart, so they are not identical. -/
theorem recipe_distinct {s t : Stack} (h : Exec [.block onePlusOne, .word 11] s t) :
    ¬ Exec [.block two, .word 11] s t := by
  intro h'
  simp [Exec] at h h'
  rw [h] at h'
  simp [onePlusOne, two] at h'

end NobleContracts.Wire
