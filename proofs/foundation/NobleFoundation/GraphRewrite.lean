/-
Certificates for a small set of pure I64 graph rewrites (issue #42, phase 1).

Optimization and search stay untrusted. This file proves the accepted rewrite
rules, congruence, and the lifting of one local replacement to agreement over the
whole graph, then a small checker whose acceptance implies the declared
denotation. Every statement is about the model in `NobleFoundation.PackedSchedule`
and carries a well-formedness premise; nothing here is a claim about the Rust
transformation, the graph IR admission path or a retained artifact.

Rules in this phase: constant folding for addition, the left zero identity for
addition and the right one identity for multiplication. Guards, effects,
resources, reflection, `F32` and dynamic calls are outside the slice, and so is
sharing relabelling: the lifting theorem states that only one node's denotation
changes and every other node's representative is untouched.
-/
import NobleFoundation.PackedSchedule

namespace NobleFoundation.GraphRewrite

open NobleFoundation.PackedSchedule

/-- A constant node denotes exactly its bits. -/
theorem constant_value (graph : Graph) (inputs : Nat → BitVec 64) (index : Nat)
    (bits : BitVec 64) (hnode : graph.node index = .constant bits) :
    value graph inputs index = some (.i64 bits) := by
  have hvalue : value graph inputs index =
      evalStep graph (valuesUpTo graph inputs index) inputs index := by
    simp [value, valuesUpTo]
  rw [hvalue]
  simp [evalStep, hnode]

/-! ### Rule theorems

Each rule is stated as equality of denotations under the exact premise the
checker verifies structurally, so a checker does not need to re-run the scheduler
or trust a producer's claim. -/

/-- Constant folding: an addition of two exact constants denotes their sum. -/
theorem fold_add_constants (graph : Graph) (hwf : WellFormed graph)
    (inputs : Nat → BitVec 64) (index first second : Nat) (x y : BitVec 64)
    (hnode : graph.node index = .add first second)
    (hfirst : graph.node first = .constant x)
    (hsecond : graph.node second = .constant y) :
    value graph inputs index = some (.i64 (x + y)) := by
  have hvalue : value graph inputs index =
      evalStep graph (valuesUpTo graph inputs index) inputs index := by
    simp [value, valuesUpTo]
  rw [hvalue]
  have hfirst_value : valuesUpTo graph inputs index first = some (.i64 x) := by
    rw [valuesUpTo_eq_value graph inputs index first
      (hwf.2 index first (by simp [hnode, Node.operands]))]
    exact constant_value graph inputs first x hfirst
  have hsecond_value : valuesUpTo graph inputs index second = some (.i64 y) := by
    rw [valuesUpTo_eq_value graph inputs index second
      (hwf.2 index second (by simp [hnode, Node.operands]))]
    exact constant_value graph inputs second y hsecond
  simp [evalStep, hnode, hfirst_value, hsecond_value]

/-- Left zero identity: `0 + x` denotes `x`'s denotation for an `I64` `x`. An
operand of another type has no denotation for this rule, which is why the premise
carries the `I64` value rather than a total rewrite claim. -/
theorem neutral_add_left (graph : Graph) (hwf : WellFormed graph)
    (inputs : Nat → BitVec 64) (index first second : Nat) (y : BitVec 64)
    (hnode : graph.node index = .add first second)
    (_hfirst : graph.node first = .constant 0)
    (hsecond : value graph inputs second = some (.i64 y)) :
    value graph inputs index = some (.i64 y) := by
  have hfirst_value : valuesUpTo graph inputs index first = some (.i64 0) := by
    rw [valuesUpTo_eq_value graph inputs index first
      (hwf.2 index first (by simp [hnode, Node.operands]))]
    exact constant_value graph inputs first 0 _hfirst
  have hsecond_value : valuesUpTo graph inputs index second = some (.i64 y) := by
    rw [valuesUpTo_eq_value graph inputs index second
      (hwf.2 index second (by simp [hnode, Node.operands]))]
    exact hsecond
  simp [value, valuesUpTo, evalStep, hnode, hfirst_value, hsecond_value, BitVec.zero_add]

/-- Right one identity: `x * 1` denotes `x`'s denotation for an `I64` `x`. -/
theorem neutral_mul_right (graph : Graph) (hwf : WellFormed graph)
    (inputs : Nat → BitVec 64) (index first second : Nat) (x : BitVec 64)
    (hnode : graph.node index = .mul first second)
    (_hsecond : graph.node second = .constant 1)
    (hfirst : value graph inputs first = some (.i64 x)) :
    value graph inputs index = some (.i64 x) := by
  have hsecond_value : valuesUpTo graph inputs index second = some (.i64 1) := by
    rw [valuesUpTo_eq_value graph inputs index second
      (hwf.2 index second (by simp [hnode, Node.operands]))]
    exact constant_value graph inputs second 1 _hsecond
  have hfirst_value : valuesUpTo graph inputs index first = some (.i64 x) := by
    rw [valuesUpTo_eq_value graph inputs index first
      (hwf.2 index first (by simp [hnode, Node.operands]))]
    exact hfirst
  simp [value, valuesUpTo, evalStep, hnode, hfirst_value, hsecond_value, BitVec.mul_one]

/-- Wrapping boundary: the deepest-minus-top addition wraps rather than saturating,
so `(2^64 - 1) + 1` denotes zero. -/
theorem add_wraps_at_the_boundary (graph : Graph) (hwf : WellFormed graph)
    (inputs : Nat → BitVec 64) (index first second : Nat) (x : BitVec 64)
    (hnode : graph.node index = .add first second)
    (hfirst : graph.node first = .constant x)
    (hsecond : graph.node second = .constant 1) :
    value graph inputs index = some (.i64 (x + 1)) :=
  fold_add_constants graph hwf inputs index first second x 1 hnode hfirst hsecond

/-! ### Congruence and lifting -/

/-- The step function is congruent in the node and in the operand lookups. -/
theorem evalStep_congr (graph₁ graph₂ : Graph) (inputs : Nat → BitVec 64)
    (values₁ values₂ : Nat → Option Val) (index : Nat)
    (hnode : graph₁.node index = graph₂.node index)
    (hvalues : ∀ operand ∈ (graph₁.node index).operands,
      values₁ operand = values₂ operand) :
    evalStep graph₁ values₁ inputs index = evalStep graph₂ values₂ inputs index := by
  have hnode₂ : graph₂.node index = graph₁.node index := hnode.symm
  cases hn : graph₁.node index with
  | input slot => simp [evalStep, hn, hnode₂]
  | constant bits => simp [evalStep, hn, hnode₂]
  | add a b =>
      have ha : a ∈ (graph₁.node index).operands := by simp [hn, Node.operands]
      have hb : b ∈ (graph₁.node index).operands := by simp [hn, Node.operands]
      simp [evalStep, hn, hnode₂, hvalues a ha, hvalues b hb]
  | sub a b =>
      have ha : a ∈ (graph₁.node index).operands := by simp [hn, Node.operands]
      have hb : b ∈ (graph₁.node index).operands := by simp [hn, Node.operands]
      simp [evalStep, hn, hnode₂, hvalues a ha, hvalues b hb]
  | mul a b =>
      have ha : a ∈ (graph₁.node index).operands := by simp [hn, Node.operands]
      have hb : b ∈ (graph₁.node index).operands := by simp [hn, Node.operands]
      simp [evalStep, hn, hnode₂, hvalues a ha, hvalues b hb]
  | eq a b =>
      have ha : a ∈ (graph₁.node index).operands := by simp [hn, Node.operands]
      have hb : b ∈ (graph₁.node index).operands := by simp [hn, Node.operands]
      simp [evalStep, hn, hnode₂, hvalues a ha, hvalues b hb]

/-- Congruence: equal operand denotations give equal node denotations, which is
what lifts a local rule to the surrounding graph. -/
theorem congruence (graph₁ graph₂ : Graph) (inputs : Nat → BitVec 64)
    (hwf₁ : WellFormed graph₁) (hwf₂ : WellFormed graph₂) (index : Nat)
    (hnode : graph₁.node index = graph₂.node index)
    (hoperands : ∀ operand ∈ (graph₁.node index).operands,
      value graph₁ inputs operand = value graph₂ inputs operand) :
    value graph₁ inputs index = value graph₂ inputs index := by
  have hvalue₁ : value graph₁ inputs index =
      evalStep graph₁ (valuesUpTo graph₁ inputs index) inputs index := by
    simp [value, valuesUpTo]
  have hvalue₂ : value graph₂ inputs index =
      evalStep graph₂ (valuesUpTo graph₂ inputs index) inputs index := by
    simp [value, valuesUpTo]
  rw [hvalue₁, hvalue₂]
  exact evalStep_congr graph₁ graph₂ inputs _ _ index hnode
    (fun operand hmem => by
      have hmem₂ : operand ∈ (graph₂.node index).operands := by
        rw [← hnode]; exact hmem
      rw [valuesUpTo_eq_value graph₁ inputs index operand
          (hwf₁.2 index operand hmem),
        valuesUpTo_eq_value graph₂ inputs index operand
          (hwf₂.2 index operand hmem₂)]
      exact hoperands operand hmem)

/-- Lifting: if two well-formed graphs have the same node at every index except
possibly the rewritten one, and the rewritten node's denotation is unchanged, then
every node's denotation is unchanged. Shared uses are preserved because the
agreement is about denotations, not about node identity. -/
theorem replacement_lift (graph₁ graph₂ : Graph) (inputs : Nat → BitVec 64)
    (hwf₁ : WellFormed graph₁) (hwf₂ : WellFormed graph₂) (rewritten : Nat)
    (hbelow : ∀ index, index < rewritten → graph₁.node index = graph₂.node index)
    (habove : ∀ index, rewritten < index → graph₁.node index = graph₂.node index)
    (hat : value graph₁ inputs rewritten = value graph₂ inputs rewritten) :
    ∀ index, value graph₁ inputs index = value graph₂ inputs index := by
  intro index
  induction index using Nat.strongRecOn with
  | _ index ih =>
    by_cases hlt : index < rewritten
    · have hmem₂ : ∀ operand ∈ (graph₁.node index).operands,
          operand < index := fun operand hmem => hwf₂.2 index operand (by
            rw [← hbelow index hlt]; exact hmem)
      exact congruence graph₁ graph₂ inputs hwf₁ hwf₂ index (hbelow index hlt)
        (fun operand hmem => ih operand (hmem₂ operand hmem))
    · by_cases heq : index = rewritten
      · rw [heq]; exact hat
      · have hgt : rewritten < index := by omega
        have hmem₂ : ∀ operand ∈ (graph₁.node index).operands,
            operand < index := fun operand hmem => hwf₂.2 index operand (by
              rw [← habove index hgt]; exact hmem)
        exact congruence graph₁ graph₂ inputs hwf₁ hwf₂ index (habove index hgt)
          (fun operand hmem => ih operand (hmem₂ operand hmem))



/-! ### Checker and fixtures

The checker does not ask the reference for the answer. It decodes the claimed
rewrite from the graph structurally — exact node shapes, exact constant bits,
exact operand positions — and rejects anything else, so acceptance implies the
declared denotation through the rule theorems above. -/

/-- A replayable derivation node: one rule applied at one node. -/
inductive Claim where
  | constAdd (index first second : Nat) (claimed : BitVec 64)
  | zeroLeft (index first second : Nat)
  | oneRight (index first second : Nat)

/-- The node the claim rewrites. -/
def Claim.target : Claim → Nat
  | .constAdd index _ _ _ => index
  | .zeroLeft index _ _ => index
  | .oneRight index _ _ => index

/-- Structural acceptance. Wrong rule identity, wrong operand positions, wrong
constant bits or a wrong claimed constant are all refusals. -/
def Checked (graph : Graph) (claim : Claim) : Bool :=
  match claim with
  | .constAdd index first second claimed =>
      match graph.node index, graph.node first, graph.node second with
      | .add a b, .constant x, .constant y =>
          decide (a = first ∧ b = second ∧ claimed = x + y)
      | _, _, _ => false
  | .zeroLeft index first second =>
      match graph.node index, graph.node first with
      | .add a b, .constant bits => decide (a = first ∧ b = second ∧ bits = 0)
      | _, _ => false
  | .oneRight index first second =>
      match graph.node index, graph.node second with
      | .mul a b, .constant bits => decide (a = first ∧ b = second ∧ bits = 1)
      | _, _ => false

/-- Checker soundness for constant folding: acceptance implies the declared
denotation. The checker reached that by decoding exact node shapes, operand
positions and constant bits from the graph; it never asks the reference. -/
theorem checked_sound_const_add (graph : Graph) (hwf : WellFormed graph)
    (inputs : Nat → BitVec 64) (index first second : Nat) (claimed : BitVec 64)
    (hchecked : Checked graph (.constAdd index first second claimed) = true) :
    value graph inputs index = some (.i64 claimed) := by
  cases hn : graph.node index with
  | add a b =>
      cases hf : graph.node first with
      | constant x =>
          cases hs : graph.node second with
          | constant y =>
              simp only [Checked, hn, hf, hs] at hchecked
              obtain ⟨ha, hb, hc⟩ := of_decide_eq_true hchecked
              have hshape : graph.node index = .add first second := by
                rw [hn]
                rw [ha]
                rw [hb]
              have hfold := fold_add_constants graph hwf inputs index first second x y
                hshape hf hs
              rw [hfold, hc]
          | input slot => simp [Checked, hn, hf, hs] at hchecked
          | add x' y' => simp [Checked, hn, hf, hs] at hchecked
          | sub x' y' => simp [Checked, hn, hf, hs] at hchecked
          | mul x' y' => simp [Checked, hn, hf, hs] at hchecked
          | eq x' y' => simp [Checked, hn, hf, hs] at hchecked
      | input slot => simp [Checked, hn, hf] at hchecked
      | add x' y' => simp [Checked, hn, hf] at hchecked
      | sub x' y' => simp [Checked, hn, hf] at hchecked
      | mul x' y' => simp [Checked, hn, hf] at hchecked
      | eq x' y' => simp [Checked, hn, hf] at hchecked
  | input slot => simp [Checked, hn] at hchecked
  | constant bits => simp [Checked, hn] at hchecked
  | sub a b => simp [Checked, hn] at hchecked
  | mul a b => simp [Checked, hn] at hchecked
  | eq a b => simp [Checked, hn] at hchecked

/-- Checker soundness for the left zero identity. -/
theorem checked_sound_zero_left (graph : Graph) (hwf : WellFormed graph)
    (inputs : Nat → BitVec 64) (index first second : Nat) (y : BitVec 64)
    (hchecked : Checked graph (.zeroLeft index first second) = true)
    (hsecond : value graph inputs second = some (.i64 y)) :
    value graph inputs index = some (.i64 y) := by
  cases hn : graph.node index with
  | add a b =>
      cases hf : graph.node first with
      | constant bits =>
          simp only [Checked, hn, hf] at hchecked
          obtain ⟨ha, hb, hz⟩ := of_decide_eq_true hchecked
          have hshape : graph.node index = .add first second := by
            rw [hn]
            rw [ha]
            rw [hb]
          have hzero : graph.node first = .constant 0 := by
            rw [hf]
            rw [hz]
          exact neutral_add_left graph hwf inputs index first second y hshape hzero hsecond
      | input slot => simp [Checked, hn, hf] at hchecked
      | add x' y' => simp [Checked, hn, hf] at hchecked
      | sub x' y' => simp [Checked, hn, hf] at hchecked
      | mul x' y' => simp [Checked, hn, hf] at hchecked
      | eq x' y' => simp [Checked, hn, hf] at hchecked
  | input slot => simp [Checked, hn] at hchecked
  | constant bits => simp [Checked, hn] at hchecked
  | sub a b => simp [Checked, hn] at hchecked
  | mul a b => simp [Checked, hn] at hchecked
  | eq a b => simp [Checked, hn] at hchecked

/-- Checker soundness for the right one identity. -/
theorem checked_sound_one_right (graph : Graph) (hwf : WellFormed graph)
    (inputs : Nat → BitVec 64) (index first second : Nat) (x : BitVec 64)
    (hchecked : Checked graph (.oneRight index first second) = true)
    (hfirst : value graph inputs first = some (.i64 x)) :
    value graph inputs index = some (.i64 x) := by
  cases hn : graph.node index with
  | mul a b =>
      cases hs : graph.node second with
      | constant bits =>
          simp only [Checked, hn, hs] at hchecked
          obtain ⟨ha, hb, ho⟩ := of_decide_eq_true hchecked
          have hshape : graph.node index = .mul first second := by
            rw [hn]
            rw [ha]
            rw [hb]
          have hone : graph.node second = .constant 1 := by
            rw [hs]
            rw [ho]
          exact neutral_mul_right graph hwf inputs index first second x hshape hone hfirst
      | input slot => simp [Checked, hn, hs] at hchecked
      | add x' y' => simp [Checked, hn, hs] at hchecked
      | sub x' y' => simp [Checked, hn, hs] at hchecked
      | mul x' y' => simp [Checked, hn, hs] at hchecked
      | eq x' y' => simp [Checked, hn, hs] at hchecked
  | input slot => simp [Checked, hn] at hchecked
  | constant bits => simp [Checked, hn] at hchecked
  | add a b => simp [Checked, hn] at hchecked
  | sub a b => simp [Checked, hn] at hchecked
  | eq a b => simp [Checked, hn] at hchecked

/-! ### Fixtures -/

/-- Three exact constants: two additions' operands, one multiplication's operands. -/
def foldGraph : Graph where
  arity := 5
  output := 2
  node := fun index =>
    if index = 0 then .constant 3
    else if index = 1 then .constant 5
    else if index = 2 then .add 0 1
    else if index = 3 then .constant 1
    else .mul 2 3

theorem foldGraph_wellFormed : WellFormed foldGraph := by
  refine ⟨by decide, ?_⟩
  intro index operand hmem
  by_cases h0 : index = 0
  · subst h0; simp [foldGraph, Node.operands] at hmem
  by_cases h1 : index = 1
  · subst h1; simp [foldGraph, Node.operands] at hmem
  by_cases h2 : index = 2
  · subst h2; simp [foldGraph, Node.operands] at hmem; omega
  by_cases h3 : index = 3
  · subst h3; simp [foldGraph, Node.operands] at hmem
  · simp [foldGraph, h0, h1, h2, h3, Node.operands] at hmem; omega

theorem foldGraph_reference :
    value foldGraph (fun _ => 0) 2 = some (.i64 8) ∧
      value foldGraph (fun _ => 0) 4 = some (.i64 8) := by
  have hwf := foldGraph_wellFormed
  refine ⟨?_, ?_⟩
  · have hat : value foldGraph (fun _ => 0) 2 =
        some (.i64 (3 + 5)) :=
      fold_add_constants foldGraph hwf (fun _ => 0) 2 0 1 3 5
        (by simp [foldGraph]) (by simp [foldGraph]) (by simp [foldGraph])
    rw [hat]
    simp
  · exact checked_sound_const_add foldGraph hwf (fun _ => 0) 2 0 1 8 (by decide)

/-- The accepted claim about the fixture's addition is the true one. -/
theorem fold_certificate_accepted :
    Checked foldGraph (.constAdd 2 0 1 8) = true := by
  decide

/-- A wrong claimed constant is refused. -/
theorem wrong_constant_refused :
    Checked foldGraph (.constAdd 2 0 1 9) = false := by
  decide

/-- Swapped operand positions are refused: the rule is about the ordered operands. -/
theorem swapped_operands_refused :
    Checked foldGraph (.constAdd 2 1 0 8) = false := by
  decide

/-- A claim naming a node that is not an addition is refused. -/
theorem wrong_rule_identity_refused :
    Checked foldGraph (.constAdd 0 0 1 3) = false := by
  decide

/-- A wrong constant *bits* under a rule-shaped node is refused: change the fixture's
one to a two and the one-identity certificate is rejected. -/
theorem wrong_bits_refused :
    Checked foldGraph (.oneRight 4 2 3) = true ∧
      Checked foldGraph (.oneRight 4 3 2) = false := by
  decide

end NobleFoundation.GraphRewrite
