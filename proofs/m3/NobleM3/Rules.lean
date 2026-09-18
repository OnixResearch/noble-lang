/-
The per-rule soundness family (V-CHECK-03) and the reference-level
decision-path correspondences (B-CHECK-04) over fragment v1.

Acceptance through each checker path implies a `TypingDerivation` whose
corresponding judgment rule instance appears at that node: EMPTY, LITERAL,
WORD, SEQUENCE, QUOTATION for the head node, and — through the shape
inversions of `NobleM3.Inv` — CASE, IF, and LISTCASE for the three
eliminators, whose branch programs share the common result stack and whose
derived bound is exactly the union of the branch bounds (B-CHECK-06,
K-EFFECT-01/02). The inclusion guard decides its declarative side, and
duplication and freshness hold at the reference level (B-CHECK-03,
K-CHECK-01).
-/

import NobleM2.CheckSoundness
import NobleM3.Inv
import NobleM3.CoverageWords

namespace NobleM2

/-! ## Head inversion -/

/-- A quotation derivation's effect index is the construction bound: empty. -/
theorem quotationDerives_bound {env : Env} {cand : Candidate} {body : List Nat}
    {resolved : Inst} {a b : TyList} {head : EffSet}
    (hq : QuotationDerives env cand body resolved a b head) : head = EffSet.empty := by
  cases hq
  rfl

/-- One derivation step over a nonempty body is a SEQUENCE-rooted step (a
node rule under the frame join, the rest of the body deriving from the
joined stack) or one of the two quotation forms of the same shape; both
quotation forms expose the node's QUOTATION derivation over the resolved
witness. -/
theorem derives_cons_inv {env : Env} {cand : Candidate} {id : Nat} {rest : List Nat}
    {stack out : TyList} {effects : EffSet}
    (h : Derives env cand (id :: rest) stack out effects) :
    (∃ node a b head mid tail,
        cand.nodes[id]? = some node ∧ NodeDerives env cand node a b head ∧
        tailEquals stack a = true ∧ replaceTail stack a b = mid ∧
        Derives env cand rest mid out tail ∧ effects = head.union tail)
    ∨ ∃ body inst resolved a b head mid,
        cand.nodes[id]? = some (Node.quotation body inst) ∧
        ResolvesTo inst resolved ∧
        QuotationDerives env cand body resolved a b head ∧
        tailEquals stack a = true ∧ replaceTail stack a b = mid ∧
        ((∃ tail, Derives env cand rest mid out tail ∧ effects = head.union tail)
          ∨ out = mid ∧ effects = head) := by
  cases h with
  | sequence hnode hnd htail hreplace hrest =>
      exact Or.inl ⟨_, _, _, _, _, _, hnode, hnd, htail, hreplace, hrest, rfl⟩
  | quotationBody hnode hres hq htail hreplace =>
      exact Or.inr ⟨_, _, _, _, _, _, _, hnode, hres, hq, htail, hreplace,
        Or.inr ⟨rfl, rfl⟩⟩
  | quotationSequence hnode hres hq htail hreplace hrest =>
      exact Or.inr ⟨_, _, _, _, _, _, _, hnode, hres, hq, htail, hreplace,
        Or.inl ⟨_, hrest, rfl⟩⟩

/-- Any derivation of an invocation node yields the WORD instance: the
eliminator rules carry the word rule's premises (the same lookup,
resolution, instantiation, and eligibility), so the WORD rule instance is
available wherever the node invokes a definition. -/
theorem invocation_word {env : Env} {cand : Candidate} {index : Nat} {inst : Inst}
    {a b : TyList} {head : EffSet}
    (h : NodeDerives env cand (.invocation index inst) a b head) :
    ∃ resolved scheme, env.scheme index = some scheme ∧ ResolvesTo inst resolved ∧
      scheme.instantiate resolved = some (a, b, head) ∧ DataOk env index resolved := by
  cases h with
  | word hlook hres hinst hdata => exact ⟨_, _, hlook, hres, hinst, hdata⟩
  | caseRule hkind hlook hres hinst hdata hshape => exact ⟨_, _, hlook, hres, hinst, hdata⟩
  | ifRule hkind hlook hres hinst hdata hshape => exact ⟨_, _, hlook, hres, hinst, hdata⟩
  | listCaseRule hkind hlook hres hinst hdata hshape =>
      exact ⟨_, _, hlook, hres, hinst, hdata⟩

/-! ## The per-rule family (V-CHECK-03) -/

variable {env : Env} {request : Request} {cand : Candidate} {checked : Checked}

/-- EMPTY: an accepted candidate with an empty entry body derives the
identity transformation at the expected interface. -/
theorem rule_empty_sound (h : check env request cand = .accepted checked)
    (hbody : cand.body = []) :
    Derives env cand [] request.expected.stackIn request.expected.stackOut
      checked.interface.effects := by
  have hd : Derives env cand cand.body request.expected.stackIn
      request.expected.stackOut checked.interface.effects := (check_soundness h).2
  rw [hbody] at hd
  exact hd

/-- LITERAL: acceptance through a literal node at the body's head carries a
LITERAL rule instance at that node, joined by segment replacement onto the
expected entry stack. -/
theorem rule_literal_sound (h : check env request cand = .accepted checked)
    {id : Nat} {rest : List Nat} {lit : Lit} {inst : Inst}
    (hbody : cand.body = id :: rest) (hnode : cand.nodes[id]? = some (.literal lit inst)) :
    ∃ a b head mid tail,
      NodeDerives env cand (.literal lit inst) a b head ∧
      tailEquals request.expected.stackIn a = true ∧
      replaceTail request.expected.stackIn a b = mid ∧
      Derives env cand rest mid request.expected.stackOut tail ∧
      checked.interface.effects = head.union tail := by
  have hd : Derives env cand cand.body request.expected.stackIn
      request.expected.stackOut checked.interface.effects := (check_soundness h).2
  rw [hbody] at hd
  rcases derives_cons_inv hd with
    ⟨node, a, b, head, mid, tail, hnode', hnd, htail, hreplace, hrest, heff⟩ |
    ⟨body', inst', resolved, a', b', head', mid', hnode', _, _, _, _, _⟩
  · rw [hnode] at hnode'
    cases hnode'
    exact ⟨a, b, head, mid, tail, hnd, htail, hreplace, hrest, heff⟩
  · exact absurd hnode' (by rw [hnode]; simp)

/-- SEQUENCE: acceptance of a body with a nonempty tail, whose head node is
not a quotation, carries a SEQUENCE step at the head node: its node rule
instance, the frame join, and the tail's derivation from the joined stack. -/
theorem rule_sequence_sound (h : check env request cand = .accepted checked)
    {id : Nat} {rest : List Nat} {node : Node}
    (hbody : cand.body = id :: rest) (hrest : rest ≠ [])
    (hnode : cand.nodes[id]? = some node)
    (hnotq : ∀ body inst, node ≠ Node.quotation body inst) :
    ∃ a b head mid tail,
      NodeDerives env cand node a b head ∧
      tailEquals request.expected.stackIn a = true ∧
      replaceTail request.expected.stackIn a b = mid ∧
      Derives env cand rest mid request.expected.stackOut tail ∧
      checked.interface.effects = head.union tail := by
  have hd : Derives env cand cand.body request.expected.stackIn
      request.expected.stackOut checked.interface.effects := (check_soundness h).2
  rw [hbody] at hd
  rcases derives_cons_inv hd with
    ⟨_, a, b, head, mid, tail, hnode', hnd, htail, hreplace, hrest', heff⟩ |
    ⟨body', inst', resolved, a', b', head', mid', hnode', _, _, _, _, _⟩
  · rw [hnode] at hnode'
    cases hnode'
    exact ⟨a, b, head, mid, tail, hnd, htail, hreplace, hrest', heff⟩
  · rw [hnode] at hnode'
    cases hnode'
    exact absurd rfl (hnotq _ _)

/-- QUOTATION: acceptance through a quotation node ending the body carries a
QUOTATION rule instance at that node: the node's resolved witness checks
its body from the declared entry stack to exactly the declared result stack
with the bound inside the declared one, and the node's join produces the
expected result with an empty effect index. -/
theorem rule_quotation_sound (h : check env request cand = .accepted checked)
    {id : Nat} {body : List Nat} {inst : Inst}
    (hbody : cand.body = [id]) (hnode : cand.nodes[id]? = some (Node.quotation body inst)) :
    ∃ resolved a b head,
      ResolvesTo inst resolved ∧
      QuotationDerives env cand body resolved a b head ∧
      tailEquals request.expected.stackIn a = true ∧
      replaceTail request.expected.stackIn a b = request.expected.stackOut ∧
      head = EffSet.empty := by
  have hd : Derives env cand cand.body request.expected.stackIn
      request.expected.stackOut checked.interface.effects := (check_soundness h).2
  rw [hbody] at hd
  rcases derives_cons_inv hd with
    ⟨node, a', b', head', mid, tail, hnode', hnd, htail, hreplace, hrest', heff⟩ |
    ⟨body', inst', resolved, a, b, head, mid, hnode', hres, hq, htail, hreplace, hout⟩
  · rw [hnode] at hnode'
    cases hnode'
    cases hnd
  · rw [hnode] at hnode'
    cases hnode'
    have hbd : head = EffSet.empty := quotationDerives_bound hq
    rcases hout with ⟨_, hrest', _⟩ | ⟨hmid, _⟩
    · cases hrest' with
      | empty => exact ⟨resolved, a, b, head, hres, hq, htail, hreplace, hbd⟩
    · rw [← hmid] at hreplace
      exact ⟨resolved, a, b, head, hres, hq, htail, hreplace, hbd⟩

/-- WORD: acceptance through an invocation node at the body's head carries a
WORD rule instance at that node, joined onto the expected entry stack. -/
theorem rule_word_sound (h : check env request cand = .accepted checked)
    {id : Nat} {rest : List Nat} {index : Nat} {inst : Inst}
    (hbody : cand.body = id :: rest) (hnode : cand.nodes[id]? = some (.invocation index inst)) :
    ∃ resolved scheme a b head mid tail,
      env.scheme index = some scheme ∧ ResolvesTo inst resolved ∧
      NodeDerives env cand (.invocation index inst) a b head ∧
      tailEquals request.expected.stackIn a = true ∧
      replaceTail request.expected.stackIn a b = mid ∧
      Derives env cand rest mid request.expected.stackOut tail ∧
      checked.interface.effects = head.union tail := by
  have hd : Derives env cand cand.body request.expected.stackIn
      request.expected.stackOut checked.interface.effects := (check_soundness h).2
  rw [hbody] at hd
  rcases derives_cons_inv hd with
    ⟨node, a, b, head, mid, tail, hnode', hnd, htail, hreplace, hrest, heff⟩ |
    ⟨body', inst', resolved', a, b, head, mid, hnode', hres, hq, htail, hreplace, hout⟩
  · rw [hnode] at hnode'
    cases hnode'
    obtain ⟨resolved, scheme, hlook, hres, hinst, hdata⟩ := invocation_word hnd
    exact ⟨resolved, scheme, a, b, head, mid, tail, hlook, hres,
      NodeDerives.word hlook hres hinst hdata, htail, hreplace, hrest, heff⟩
  · rw [hnode] at hnode'
    exact absurd hnode' (by simp)
/-! ## The eliminator rules (B-CHECK-06, K-EFFECT-01/02) -/

/-- CASE: acceptance through a `case` invocation at the body's head carries a
CASE rule instance at that node — the two branch programs' common result
stack and conservative union bound made explicit. -/
theorem rule_case_sound {request : Request} {cand : Candidate} {checked : Checked}
    (h : check bootstrapEnv request cand = .accepted checked)
    {id : Nat} {rest : List Nat} {inst : Inst}
    (hbody : cand.body = id :: rest)
    (hnode : cand.nodes[id]? = some (.invocation caseDef inst)) :
    ∃ resolved a b head mid tail s x y t e f,
      ResolvesTo inst resolved ∧
      caseScheme.instantiate resolved = some (a, b, head) ∧
      DataOk bootstrapEnv caseDef resolved ∧
      NodeDerives bootstrapEnv cand (.invocation caseDef inst) a b head ∧
      tailEquals request.expected.stackIn a = true ∧
      replaceTail request.expected.stackIn a b = mid ∧
      Derives bootstrapEnv cand rest mid request.expected.stackOut tail ∧
      checked.interface.effects = head.union tail ∧
      resolved.stack 0 = some s ∧ resolved.value 1 = some x ∧
      resolved.value 2 = some y ∧ resolved.stack 3 = some t ∧
      resolved.effects 4 = some e ∧ resolved.effects 5 = some f ∧
      b = t ∧ head = e.union f := by
  have hd : Derives bootstrapEnv cand cand.body request.expected.stackIn
      request.expected.stackOut checked.interface.effects := (check_soundness h).2
  rw [hbody] at hd
  rcases derives_cons_inv hd with
    ⟨node, a, b, head, mid, tail, hnode', hnd, htail, hreplace, hrest, heff⟩ |
    ⟨body', inst', resolved', a, b, head, mid, hnode', _, _, _, _, _⟩
  · rw [hnode] at hnode'
    cases hnode'
    obtain ⟨resolved, scheme, hlook, hres, hinst, hdata⟩ := invocation_word hnd
    have htab : bootstrapEnv.scheme caseDef = some caseScheme := by native_decide
    rw [hlook] at htab
    injection htab with hcase
    rw [hcase] at hinst
    obtain ⟨x0, x1, x2, x3, x4, x5, hl0, hl1, hl2, hl3, hl4, hl5, hout, hunion⟩ :=
      caseScheme_inv hinst
    exact ⟨resolved, a, b, head, mid, tail, x0, x1, x2, x3, x4, x5, hres, hinst, hdata,
      NodeDerives.caseRule (by native_decide) (by native_decide) hres hinst hdata
        hl0 hl1 hl2 hl3 hl4 hl5 hout hunion,
      htail, hreplace, hrest, heff, hl0, hl1, hl2, hl3, hl4, hl5, hout, hunion⟩
  · rw [hnode] at hnode'
    exact absurd hnode' (by simp)

/-- IF: acceptance through an `if` invocation at the body's head carries an
IF rule instance at that node — both branch programs' common result stack
and conservative union bound made explicit. -/
theorem rule_if_sound {request : Request} {cand : Candidate} {checked : Checked}
    (h : check bootstrapEnv request cand = .accepted checked)
    {id : Nat} {rest : List Nat} {inst : Inst}
    (hbody : cand.body = id :: rest)
    (hnode : cand.nodes[id]? = some (.invocation ifDef inst)) :
    ∃ resolved a b head mid tail s t e f,
      ResolvesTo inst resolved ∧
      ifScheme.instantiate resolved = some (a, b, head) ∧
      DataOk bootstrapEnv ifDef resolved ∧
      NodeDerives bootstrapEnv cand (.invocation ifDef inst) a b head ∧
      tailEquals request.expected.stackIn a = true ∧
      replaceTail request.expected.stackIn a b = mid ∧
      Derives bootstrapEnv cand rest mid request.expected.stackOut tail ∧
      checked.interface.effects = head.union tail ∧
      resolved.stack 0 = some s ∧ resolved.stack 1 = some t ∧
      resolved.effects 2 = some e ∧ resolved.effects 3 = some f ∧
      b = t ∧ head = e.union f := by
  have hd : Derives bootstrapEnv cand cand.body request.expected.stackIn
      request.expected.stackOut checked.interface.effects := (check_soundness h).2
  rw [hbody] at hd
  rcases derives_cons_inv hd with
    ⟨node, a, b, head, mid, tail, hnode', hnd, htail, hreplace, hrest, heff⟩ |
    ⟨body', inst', resolved', a, b, head, mid, hnode', _, _, _, _, _⟩
  · rw [hnode] at hnode'
    cases hnode'
    obtain ⟨resolved, scheme, hlook, hres, hinst, hdata⟩ := invocation_word hnd
    have htab : bootstrapEnv.scheme ifDef = some ifScheme := by native_decide
    rw [hlook] at htab
    injection htab with hif
    rw [hif] at hinst
    obtain ⟨x0, x1, x2, x3, hl0, hl1, hl2, hl3, hout, hunion⟩ := ifScheme_inv hinst
    exact ⟨resolved, a, b, head, mid, tail, x0, x1, x2, x3, hres, hinst, hdata,
      NodeDerives.ifRule (by native_decide) (by native_decide) hres hinst hdata
        hl0 hl1 hl2 hl3 hout hunion,
      htail, hreplace, hrest, heff, hl0, hl1, hl2, hl3, hout, hunion⟩
  · rw [hnode] at hnode'
    exact absurd hnode' (by simp)

/-- LISTCASE: acceptance through a `list.case` invocation at the body's head
carries a LISTCASE rule instance at that node — the empty and cons branch
programs' common result stack and conservative union bound made explicit. -/
theorem rule_listcase_sound {request : Request} {cand : Candidate} {checked : Checked}
    (h : check bootstrapEnv request cand = .accepted checked)
    {id : Nat} {rest : List Nat} {inst : Inst}
    (hbody : cand.body = id :: rest)
    (hnode : cand.nodes[id]? = some (.invocation listCaseDef inst)) :
    ∃ resolved a b head mid tail s x t e f,
      ResolvesTo inst resolved ∧
      listCaseScheme.instantiate resolved = some (a, b, head) ∧
      DataOk bootstrapEnv listCaseDef resolved ∧
      NodeDerives bootstrapEnv cand (.invocation listCaseDef inst) a b head ∧
      tailEquals request.expected.stackIn a = true ∧
      replaceTail request.expected.stackIn a b = mid ∧
      Derives bootstrapEnv cand rest mid request.expected.stackOut tail ∧
      checked.interface.effects = head.union tail ∧
      resolved.stack 0 = some s ∧ resolved.value 1 = some x ∧
      resolved.stack 2 = some t ∧ resolved.effects 3 = some e ∧
      resolved.effects 4 = some f ∧
      b = t ∧ head = e.union f := by
  have hd : Derives bootstrapEnv cand cand.body request.expected.stackIn
      request.expected.stackOut checked.interface.effects := (check_soundness h).2
  rw [hbody] at hd
  rcases derives_cons_inv hd with
    ⟨node, a, b, head, mid, tail, hnode', hnd, htail, hreplace, hrest, heff⟩ |
    ⟨body', inst', resolved', a, b, head, mid, hnode', _, _, _, _, _⟩
  · rw [hnode] at hnode'
    cases hnode'
    obtain ⟨resolved, scheme, hlook, hres, hinst, hdata⟩ := invocation_word hnd
    have htab : bootstrapEnv.scheme listCaseDef = some listCaseScheme := by native_decide
    rw [hlook] at htab
    injection htab with hlc
    rw [hlc] at hinst
    obtain ⟨x0, x1, x2, x3, x4, hl0, hl1, hl2, hl3, hl4, hout, hunion⟩ :=
      listCaseScheme_inv hinst
    exact ⟨resolved, a, b, head, mid, tail, x0, x1, x2, x3, x4, hres, hinst, hdata,
      NodeDerives.listCaseRule (by native_decide) (by native_decide) hres hinst hdata
        hl0 hl1 hl2 hl3 hl4 hout hunion,
      htail, hreplace, hrest, heff, hl0, hl1, hl2, hl3, hl4, hout, hunion⟩
  · rw [hnode] at hnode'
    exact absurd hnode' (by simp)

/-! ## Decision-path correspondences (B-CHECK-04) -/

/-- The decided inclusion is exactly the declarative subset relation —
the reference side of `inclusion_iff`; the extracted side is the matrix in
`NobleM3.Refinement`. -/
theorem subsetOf_iff {a b : EffSet} : a.subsetOf b = true ↔ a.Subset b := by
  constructor
  · exact subsetOf_subset
  · intro h
    unfold EffSet.subsetOf
    rw [List.all_eq_true]
    intro id hid
    simp [h id hid]

/-- Duplication shares one interface (B-CHECK-03, K-CHECK-01): `dup` applied
to a first-class program value yields both duplicated positions carrying
the SAME instantiated program interface — the second use cannot claim an
independent instantiation. -/
theorem duplication_shares_one_interface (s i o : TyList) (e : EffSet) :
    dupScheme.instantiate ⟨[.stack s, .value (.program i o e)]⟩
      = some (s.append (TyList.singleton (.program i o e)),
          s.append (TyList.cons (.program i o e)
            (TyList.singleton (.program i o e))), EffSet.empty) := by
  simp [dupScheme, Scheme.instantiate, Scheme.substStack, Scheme.substPattern,
    Scheme.substSignature, Scheme.substEffects, PartList.ofList, SlotList.ofList,
    Inst.value, Inst.stack, Inst.effects]
  exact ⟨rfl, rfl⟩

/-- Fresh instantiation (K-CHECK-01): the same definition invoked twice in
one accepted body instantiates its scheme afresh at each use — here `unit`
twice, binding different stack segments, both derived. -/
def freshCandidate : Candidate :=
  ⟨0, 0,
    [.invocation 12 ⟨[.stack (WordCoverage.tl [.i64])]⟩,
     .invocation 12 ⟨[.stack (WordCoverage.tl [.i64, .unit])]⟩],
    [0, 1]⟩

def freshRequest : Request :=
  ⟨64, ⟨WordCoverage.tl [.i64], WordCoverage.tl [.i64, .unit, .unit], EffSet.empty⟩,
    ⟨65536, 256, 32, 64, 16, 10000, 64⟩⟩

def freshChecked : Checked :=
  ⟨⟨WordCoverage.tl [.i64], WordCoverage.tl [.i64, .unit, .unit], EffSet.empty⟩,
    [⟨0, ⟨WordCoverage.tl [.i64], WordCoverage.tl [.i64, .unit], EffSet.empty⟩⟩,
     ⟨1, ⟨WordCoverage.tl [.i64, .unit], WordCoverage.tl [.i64, .unit, .unit],
       EffSet.empty⟩⟩]⟩

theorem fresh_instantiation :
    ∃ (req : Request) (cand : Candidate) (checked : Checked) (w1 w2 : Inst),
      check bootstrapEnv req cand = .accepted checked ∧
      cand.nodes[0]? = some (.invocation 12 w1) ∧
      cand.nodes[1]? = some (.invocation 12 w2) ∧
      w1.bindings ≠ w2.bindings ∧
      Derives bootstrapEnv cand cand.body req.expected.stackIn req.expected.stackOut
        checked.interface.effects := by
  refine ⟨freshRequest, freshCandidate, freshChecked,
    ⟨[.stack (WordCoverage.tl [.i64])]⟩,
    ⟨[.stack (WordCoverage.tl [.i64, .unit])]⟩,
    by native_decide, rfl, rfl, by decide, ?_⟩
  have h1 : unitScheme.instantiate ⟨[.stack (WordCoverage.tl [.i64, .unit])]⟩
      = some (WordCoverage.tl [.i64, .unit], WordCoverage.tl [.i64, .unit, .unit],
        EffSet.empty) := by native_decide
  have h2 : Derives bootstrapEnv freshCandidate [1] (WordCoverage.tl [.i64, .unit])
      (WordCoverage.tl [.i64, .unit, .unit]) (EffSet.empty.union EffSet.empty) :=
    Derives.sequence
      (by native_decide : (freshCandidate.nodes)[1]? =
        some (Node.invocation 12 ⟨[.stack (WordCoverage.tl [.i64, .unit])]⟩))
      (NodeDerives.word (by native_decide : bootstrapEnv.scheme 12 = some unitScheme)
        (resolvesTo_refl _ (by native_decide)) h1 trivial)
      (WordCoverage.tailEquals_self _) WordCoverage.replaceTail_self (Derives.empty _)
  have h0 : Derives bootstrapEnv freshCandidate [0, 1] (WordCoverage.tl [.i64])
      (WordCoverage.tl [.i64, .unit, .unit])
      (EffSet.empty.union (EffSet.empty.union EffSet.empty)) :=
    Derives.sequence
      (by native_decide : (freshCandidate.nodes)[0]? =
        some (Node.invocation 12 ⟨[.stack (WordCoverage.tl [.i64])]⟩))
      (NodeDerives.word (by native_decide : bootstrapEnv.scheme 12 = some unitScheme)
        (resolvesTo_refl _ (by native_decide))
        (by native_decide : unitScheme.instantiate ⟨[.stack (WordCoverage.tl [.i64])]⟩ =
          some (WordCoverage.tl [.i64], WordCoverage.tl [.i64, .unit], EffSet.empty)) trivial)
      (WordCoverage.tailEquals_self _) WordCoverage.replaceTail_self h2
  have hu : EffSet.empty.union (EffSet.empty.union EffSet.empty) = EffSet.empty := by
    native_decide
  rw [hu] at h0
  exact h0

end NobleM2
