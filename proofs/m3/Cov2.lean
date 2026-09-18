/-
Per-word positive coverage over the fragment-v1 word table (PO-10, V-CHECK-05).

One canonical fixture per bootstrap word: a single-node candidate invoking
the word at its table position with the canonical witness (one `i64` entry
per stack variable, `i64` per value variable, the empty bound per effect
variable — `Refine.canonInst`), accepted at exactly the instantiated
interface and derived through the judgment's word rule. The three
eliminators' positions are covered by the same family and additionally by
`coverage_eliminator_exercise`, a five-node composite that feeds a real
quotation-produced program value into the `case` eliminator's join.

`coverage_positive_word` aggregates the family: every one of the 23 table
positions has an accepted, derivable exercising fixture.
-/

import NobleM2.CheckSoundness
import NobleM3.Refine

namespace NobleM2.WordCoverage

open NobleM2 NobleM2.Refinement

/-- The word at one table position, when in range. -/
def wordAt (n : Nat) : Option Word := wordTable[n]?

/-- The canonical fixture's node: one invocation of word `n`. -/
def wordNode (n : Nat) : Node := .invocation n (canonInst n)

/-- The canonical fixture's candidate: the single node, entered at node 0. -/
def wordCandidate (n : Nat) : Candidate := ⟨0, 0, [wordNode n], [0]⟩

/-- The interface the canonical fixture instantiates, when the word and its
witness are well formed (every canonical witness is). -/
def wordIfaceOf (n : Nat) : Option (TyList × TyList × EffSet) :=
  match wordAt n with
  | some word => word.scheme.instantiate (canonInst n)
  | none => none

/-- The checked result the fixture expects: the instantiated interface and
its one node derivation. -/
def wordCheckedOf (n : Nat) : Checked :=
  match wordIfaceOf n with
  | some (stackIn, stackOut, effects) =>
    ⟨⟨stackIn, stackOut, effects⟩, [⟨0, ⟨stackIn, stackOut, effects⟩⟩]⟩
  | none => ⟨⟨.nil, .nil, EffSet.empty⟩, []⟩

/-- The fixture's request: the instantiated interface as the expected one,
its own bound allowed, the kernel tests' default limits. -/
def wordRequestOf (n : Nat) : Request :=
  ⟨64, ⟨(wordCheckedOf n).interface.stackIn, (wordCheckedOf n).interface.stackOut,
    (wordCheckedOf n).interface.effects⟩,
    ⟨65536, 256, 32, 64, 16, 10000, 64⟩⟩

/-! ## The join laws the derivation family consumes -/

/-- A stack's top `length` entries are the whole stack. -/
theorem takeTail_self (s : TyList) : TyList.takeTail s s.length = s := by
  have key : ∀ (n : Nat) (l : TyList), l.length ≤ n → TyList.takeTail l l.length = l := by
    intro n
    induction n with
    | zero =>
        intro l hl
        cases l with
        | nil => rfl
        | cons t ts => simp [TyList.length] at hl
    | succ n ih =>
        intro l _
        cases l with
        | nil => rfl
        | cons t ts =>
            simp only [TyList.takeTail]
            rw [if_pos (Nat.le_refl _), Nat.sub_self]
            rfl
  exact key s.length s (Nat.le_refl _)

theorem tailEquals_self (s : TyList) : tailEquals s s = true := by
  have h : tailOf s s.length = s := takeTail_self s
  simp [tailEquals, h]

/-- Replacing a stack's top segment by `out`, when the segment is the whole
stack, is `out`. -/
theorem replaceTail_self {s out : TyList} : replaceTail s s out = out := by
  simp [replaceTail, prefixStack, Nat.sub_self, TyList.append]

/-- One invoking node derives its instantiated interface: the word rule
under one SEQUENCE step, with the empty body closing the derivation. -/
theorem invoke_derives {n : Nat} {scheme : Scheme} {wit : Inst}
    {stackIn stackOut : TyList} {effects : EffSet} {cand : Candidate}
    (hnode : cand.nodes[0]? = some (.invocation n wit)) (hbody : cand.body = [0])
    (hlook : bootstrapEnv.scheme n = some scheme) (hres : ResolvesTo wit wit)
    (hinst : scheme.instantiate wit = some (stackIn, stackOut, effects))
    (hdata : DataOk bootstrapEnv n wit) :
    Derives bootstrapEnv cand cand.body stackIn stackOut (effects.union EffSet.empty) := by
  rw [hbody]
  exact Derives.sequence hnode (NodeDerives.word hlook hres hinst hdata)
    (tailEquals_self stackIn) replaceTail_self (Derives.empty stackOut)

section EliminatorExercise

/-- The exercise's stack builder. -/
def tl : List Ty → TyList
  | [] => .nil
  | t :: ts => .cons t (tl ts)

/-- The exercise's sum scrutinee type. -/
def eeSum : Ty := .sum .i64 .i64

/-- The exercise's branch program: `[i64, i64] -- [i64, i64, i64] ! {}`,
exactly what `quote` produces for the stack `[i64, i64]` with an `i64`
pushed on top. -/
def eeProg : Ty := .program (tl [.i64, .i64]) (tl [.i64, .i64, .i64]) EffSet.empty

/-- The exercise's candidate: two literals raising the `i64` payload, `inl`
closing the scrutinee sum, two literal-then-quote pairs producing the branch
programs, and the `case` invocation consuming the sum and both programs. -/
def eeCandidate : Candidate :=
  ⟨0, 0,
    [ .literal (.i64 7) ⟨[.stack .nil]⟩,
      .literal (.i64 1) ⟨[.stack (tl [.i64])]⟩,
      .invocation 15 ⟨[.stack (tl [.i64]), .value .i64, .value .i64]⟩,
      .literal (.i64 2) ⟨[.stack (tl [.i64, eeSum])]⟩,
      .invocation 8 ⟨[.stack (tl [.i64, eeSum]), .value .i64, .stack (tl [.i64, .i64])]⟩,
      .literal (.i64 3) ⟨[.stack (tl [.i64, eeSum, eeProg])]⟩,
      .invocation 8
        ⟨[.stack (tl [.i64, eeSum, eeProg]), .value .i64, .stack (tl [.i64, .i64])]⟩,
      .invocation 17
        ⟨[.stack (tl [.i64]), .value .i64, .value .i64,
            .stack (tl [.i64, .i64, .i64]), .effect EffSet.empty, .effect EffSet.empty]⟩ ],
    [0, 1, 2, 3, 4, 5, 6, 7]⟩

/-- The exercise's request. -/
def eeRequest : Request :=
  ⟨64, ⟨.nil, tl [.i64, .i64, .i64], EffSet.empty⟩, ⟨65536, 256, 32, 64, 16, 10000, 64⟩⟩

/-- The exercise's checked result: eight node derivations. -/
def eeChecked : Checked :=
  ⟨⟨.nil, tl [.i64, .i64, .i64], EffSet.empty⟩,
    [ ⟨0, ⟨.nil, tl [.i64], EffSet.empty⟩⟩,
      ⟨1, ⟨tl [.i64], tl [.i64, .i64], EffSet.empty⟩⟩,
      ⟨2, ⟨tl [.i64, .i64], tl [.i64, eeSum], EffSet.empty⟩⟩,
      ⟨3, ⟨tl [.i64, eeSum], tl [.i64, eeSum, .i64], EffSet.empty⟩⟩,
      ⟨4, ⟨tl [.i64, eeSum, .i64], tl [.i64, eeSum, eeProg], EffSet.empty⟩⟩,
      ⟨5, ⟨tl [.i64, eeSum, eeProg], tl [.i64, eeSum, eeProg, .i64], EffSet.empty⟩⟩,
      ⟨6, ⟨tl [.i64, eeSum, eeProg, .i64], tl [.i64, eeSum, eeProg, eeProg], EffSet.empty⟩⟩,
      ⟨7, ⟨tl [.i64, eeSum, eeProg, eeProg], tl [.i64, .i64, .i64], EffSet.empty⟩⟩ ]⟩

/-- The checker accepts the exercise (closed by evaluation). -/
theorem ee_accepted : NobleM2.check bootstrapEnv eeRequest eeCandidate
    = .accepted eeChecked := by
  native_decide

end NobleM2.WordCoverage
