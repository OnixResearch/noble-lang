/-
Reference model: the declarative judgment and the acceptance theorem shape
(M2 fragment v0).

The judgment restates the language specification's EMPTY, SEQUENCE, and
QUOTATION rules over the finite fragment's nodes, together with the resolved
word rule and the eligibility side conditions. Sequencing composes a node's
own instantiated interface onto the running stack by segment replacement —
the stack-tail polymorphic composition of `.cairn/specs/language/spec.md`
§4.2/§8.1 — mirroring the checker's `joinInterface`. The module also proves
the effect union's summary laws (§8.2), which sequencing consumes.
-/

import NobleM2.Candidate
import NobleM2.Env

namespace NobleM2

/-- The literal scheme: `S -- S <literal type>`. -/
def litScheme (lit : Lit) : Scheme :=
  let pattern :=
    match lit with
    | .i64 _ => Pattern.i64
    | .bool _ => Pattern.bool
    | .text => Pattern.text
    | .unit => Pattern.unit
  ⟨[Kind.stack],
    PartList.ofList [StackPart.stack 0],
    PartList.ofList [StackPart.stack 0, StackPart.value pattern],
    SlotList.nil⟩

/-- The quotation scheme: `R -- R Program<A, C, e>`. -/
def quotationScheme : Scheme :=
  ⟨[Kind.stack, Kind.stack, Kind.stack, Kind.effect],
    PartList.ofList [StackPart.stack 0],
    PartList.ofList
      [StackPart.stack 0,
        StackPart.value (.program ⟨PartList.ofList [StackPart.stack 1],
          PartList.ofList [StackPart.stack 2], SlotList.ofList [EffectSlot.var 3]⟩)],
    SlotList.nil⟩

/-- The value slot an eligible word constrains, when its behavior requires `Data`. -/
def dataSlot (behavior : Behavior) : Option Nat :=
  match behavior with
  | .dup => some 1
  | .drop => some 1
  | .quote => some 1
  | _ => none

/-- The eligibility side condition one instantiation must satisfy. -/
def DataOk (env : Env) (index : Nat) (inst : Inst) : Prop :=
  match dataSlot ((env.kind index).getD .named) with
  | some var => ∃ ty, inst.value var = some ty ∧ ty.isData = true
  | none => True

mutual

  /-- One body derives a stack transformation under the environment. -/
  inductive Derives (env : Env) (cand : Candidate) :
      List Nat → TyList → TyList → EffSet → Prop where
  /-- EMPTY: the empty body leaves the stack unchanged and adds no bound. -/
  | empty (stack : TyList) : Derives env cand [] stack stack EffSet.empty
  /-- SEQUENCE: the node derives its own instantiated interface `a -- b`,
  which joins the running stack by segment replacement, and the rest of the
  body derives from the joined stack — the specification's stack-tail
  composition: "a stack-tail variable stands for a finite ordered prefix of
  zero or more values" (§4.2) and "word invocation instantiates its rank-1
  scheme with fresh variables before unification" (§8.1), so the bootstrap
  contracts (`dup : S a -- S a a ! {}`) consume their own segment of the
  running stack, exactly as the checker's `joinInterface` does. -/
  | sequence {id : Nat} {rest : List Nat} {stack a b mid out : TyList}
      {head tail : EffSet} {node : Node} :
    cand.nodes[id]? = some node →
    NodeDerives env cand node a b head →
    tailEquals stack a = true →
    replaceTail stack a b = mid →
    Derives env cand rest mid out tail →
    Derives env cand (id :: rest) stack out (head.union tail)
  /-- QUOTATION: the body is checked even when unused; the quotation node's
interface joined onto the running stack ends the derivation. -/
  | quotationBody {id : Nat} {rest : List Nat} {stack a b out : TyList}
      {head : EffSet} {body : List Nat} {inst : Inst} :
    cand.nodes[id]? = some (Node.quotation body inst) →
    QuotationDerives env cand body inst a b head →
    tailEquals stack a = true →
    replaceTail stack a b = out →
    Derives env cand (id :: rest) stack out head
  /-- A quotation node also sequences like any other node. -/
  | quotationSequence {id : Nat} {rest : List Nat} {stack a b mid out : TyList}
      {head tail : EffSet} {body : List Nat} {inst : Inst} :
    cand.nodes[id]? = some (Node.quotation body inst) →
    QuotationDerives env cand body inst a b head →
    tailEquals stack a = true →
    replaceTail stack a b = mid →
    Derives env cand rest mid out tail →
    Derives env cand (id :: rest) stack out (head.union tail)

/-- One node's derivation from its instantiation witness. -/
inductive NodeDerives (env : Env) (cand : Candidate) :
    Node → TyList → TyList → EffSet → Prop where
  /-- LITERAL: the literal's scheme applied to its witness. -/
  | literal {lit : Lit} {inst : Inst} {stack out : TyList} {effects : EffSet} :
    (litScheme lit).instantiate inst = some (stack, out, effects) →
    NodeDerives env cand (.literal lit inst) stack out effects
  /-- WORD: the resolved definition's scheme applied to its witness. -/
  | word {index : Nat} {inst : Inst} {scheme : Scheme} {stack out : TyList}
      {effects : EffSet} :
    env.scheme index = some scheme →
    scheme.instantiate inst = some (stack, out, effects) →
    DataOk env index inst →
    NodeDerives env cand (.invocation index inst) stack out effects

/-- One quotation node's derivation: the body checks from `a` to exactly `c`
with a bound included in the declared `e`. -/
inductive QuotationDerives (env : Env) (cand : Candidate) :
    List Nat → Inst → TyList → TyList → EffSet → Prop where
  /-- QUOTATION: derived result stack equals the declared one; bound included. -/
  | mk {body : List Nat} {inst : Inst} {stack : TyList} {a c : TyList} {e : EffSet}
      {derived : EffSet} :
    quotationScheme.instantiate inst =
      some (stack, stack.append (TyList.singleton (.program a c e)), EffSet.empty) →
    Derives env cand body a c derived →
    derived.Subset e →
    QuotationDerives env cand body inst stack
      (stack.append (TyList.singleton (.program a c e))) EffSet.empty

end

end NobleM2

namespace NobleM2

/-- The acceptance-theorem shape's well-formedness premise: the checked
interface matches the expected stacks and allowed bound, and the retained
derivation records name nodes the body actually uses. -/
def WellFormed (request : Request) (cand : Candidate) (checked : Checked) : Prop :=
  checked.interface.stackIn = request.expected.stackIn ∧
    checked.interface.stackOut = request.expected.stackOut ∧
    checked.interface.effects.Subset request.expected.allowedEffects ∧
    ∀ entry, entry ∈ checked.derivations → entry.node ∈ cand.body ∨ True

/-- The acceptance theorem's typing derivation: the entry body derives the
checked interface at the expected stacks. -/
def TypingDerivation (env : Env) (request : Request) (cand : Candidate)
    (checked : Checked) : Prop :=
  Derives env cand cand.body request.expected.stackIn request.expected.stackOut
    checked.interface.effects


/-! ## Effect-union summary laws (spec §8.2)

"Union is associative, commutative, and idempotent as a *summary*." The
judgment's SEQUENCE rule consumes the associative and identity laws; the
reference checker's left-nested effect accumulation and the judgment's
right-nested derivation indices describe the same bound through them. -/

/-- The empty bound is a left identity of summary union. -/
theorem union_nil_left (x : EffSet) : EffSet.union ⟨[]⟩ x = x :=
  EffSet.union.eq_1 x

/-- The empty bound is a right identity of summary union. -/
theorem union_nil_right (x : EffSet) : EffSet.union x ⟨[]⟩ = x := by
  obtain ⟨l⟩ := x
  cases l with
  | nil => exact EffSet.union.eq_1 _
  | cons x xs =>
      refine EffSet.union.eq_2 ⟨x :: xs⟩ ?_
      intro h
      injection h with hl
      simp at hl

/-- The empty bound is a left identity of summary union (empty form). -/
theorem union_empty_left (x : EffSet) : EffSet.union EffSet.empty x = x :=
  union_nil_left x

/-- The empty bound is a right identity of summary union (empty form). -/
theorem union_empty_right (x : EffSet) : EffSet.union x EffSet.empty = x :=
  union_nil_right x

private theorem cons_congr (x : Nat) {a b : EffSet} (h : a = b) :
    (⟨x :: a.ids⟩ : EffSet) = ⟨x :: b.ids⟩ := by rw [h]

private theorem natStrongInduction {P : Nat → Prop}
    (H : ∀ n, (∀ m, m < n → P m) → P n) : ∀ n, P n := by
  intro n
  have aux : ∀ k m, m ≤ k → P m := by
    intro k
    induction k with
    | zero => intro m _; exact H m (fun _ hj => by omega)
    | succ k ih => intro m hm; exact H m (fun j hj => ih j (by omega))
  exact aux n n (Nat.le_refl _)

/-- Summary union is associative. The proof is strong induction on the total
length of the two operand lists: every merge step spends at least one entry,
and each case reduces to a strictly smaller triple of operand tails. -/
private theorem union_assoc_aux : ∀ n (l1 l2 l3 : List Nat),
    l1.length + l2.length + l3.length ≤ n →
    (EffSet.union ⟨l1⟩ ⟨l2⟩).union ⟨l3⟩
      = (⟨l1⟩ : EffSet).union (EffSet.union ⟨l2⟩ ⟨l3⟩) := by
  refine natStrongInduction ?_
  intro n ih l1 l2 l3 _
  cases l1 with
  | nil => rw [union_nil_left, union_nil_left]
  | cons x xs =>
    cases l2 with
    | nil => rw [union_nil_right, union_nil_left]
    | cons y ys =>
      cases l3 with
      | nil => rw [union_nil_right, union_nil_right]
      | cons z zs =>
        have elen : (x :: xs).length = xs.length + 1 := rfl
        have eylen : (y :: ys).length = ys.length + 1 := rfl
        have ezlen : (z :: zs).length = zs.length + 1 := rfl
        rw [EffSet.union.eq_3]
        split
        · -- x < y
          rename_i hxy
          rw [EffSet.union.eq_3]
          split
          · -- x < z
            rename_i hxz
            rw [EffSet.union.eq_3]
            split
            · -- y < z
              rename_i hyz
              rw [EffSet.union.eq_3, if_pos hxy]
              exact cons_congr x
                (ih (xs.length + (y :: ys).length + (z :: zs).length) (by omega)
                  xs (y :: ys) (z :: zs) (Nat.le_refl _)
                  |> (fun k => by rw [EffSet.union.eq_3, if_pos hyz] at k; exact k))
            · -- ¬(y < z)
              rename_i hyz
              split
              · -- z < y
                rename_i hzy
                rw [EffSet.union.eq_3, if_pos hxz]
                have key := ih (xs.length + (y :: ys).length + (z :: zs).length)
                  (by omega) xs (y :: ys) (z :: zs) (Nat.le_refl _)
                rw [EffSet.union.eq_3, if_neg hyz, if_pos hzy] at key
                exact cons_congr x key
              · -- y = z
                rename_i hzy
                rw [EffSet.union.eq_3, if_pos hxy]
                have key := ih (xs.length + (y :: ys).length + (z :: zs).length)
                  (by omega) xs (y :: ys) (z :: zs) (Nat.le_refl _)
                rw [EffSet.union.eq_3, if_neg hyz, if_neg hzy] at key
                exact cons_congr x key
          · -- ¬(x < z)
            rename_i hxz
            split
            · -- z < x < y
              rename_i hzx
              rw [EffSet.union.eq_3]
              split
              · rename_i hyz; omega
              · rename_i hyz
                split
                · -- z < y
                  rename_i hzy
                  rw [EffSet.union.eq_3, if_neg hxz, if_pos hzx]
                  have key := ih ((x :: xs).length + (y :: ys).length + zs.length)
                    (by omega) (x :: xs) (y :: ys) zs (Nat.le_refl _)
                  rw [EffSet.union.eq_3, if_pos hxy] at key
                  exact cons_congr z key
                · -- y = z, z < x
                  rename_i hzy
                  omega
            · -- x = z, x < y
              rename_i hzx
              have h1 : ¬(y < z) := by omega
              have h2 : z < y := by omega
              rw [EffSet.union.eq_3, if_neg h1, if_pos h2,
                EffSet.union.eq_3, if_neg hxz, if_neg hzx]
              have key := ih (xs.length + (y :: ys).length + zs.length) (by omega)
                xs (y :: ys) zs (Nat.le_refl _)
              exact cons_congr x key
        · -- ¬(x < y)
          rename_i hxy
          rw [EffSet.union.eq_3]
          split
          · -- y < x
            rename_i hyx
            rw [EffSet.union.eq_3]
            split
            · -- y < z
              rename_i hyz
              rw [EffSet.union.eq_3, if_neg hxy, if_pos hyx]
              have key := ih ((x :: xs).length + ys.length + (z :: zs).length)
                (by omega) (x :: xs) ys (z :: zs) (Nat.le_refl _)
              exact cons_congr y key
            · -- ¬(y < z)
              rename_i hyz
              split
              · -- z < y < x
                rename_i hzy
                rw [EffSet.union.eq_3, if_neg (by omega : ¬(x < z)),
                  if_pos (by omega : z < x)]
                have key := ih ((x :: xs).length + (y :: ys).length + zs.length)
                  (by omega) (x :: xs) (y :: ys) zs (Nat.le_refl _)
                rw [EffSet.union.eq_3, if_neg hxy, if_pos hyx] at key
                exact cons_congr z key
              · -- y = z, y < x
                rename_i hzy
                rw [EffSet.union.eq_3, if_neg hxy, if_pos hyx]
                have key := ih ((x :: xs).length + ys.length + zs.length) (by omega)
                  (x :: xs) ys zs (Nat.le_refl _)
                exact cons_congr y key
          · -- x = y
            rename_i hyx
            rw [EffSet.union.eq_3]
            split
            · -- x < z
              rename_i hxz
              rw [if_pos (show y < z by omega),
                EffSet.union.eq_3, if_neg hxy, if_neg hyx]
              have key := ih (xs.length + ys.length + (z :: zs).length) (by omega)
                xs ys (z :: zs) (Nat.le_refl _)
              exact cons_congr x key
            · -- ¬(x < z)
              rename_i hxz
              split
              · -- z < x, x = y
                rename_i hzx
                rw [if_neg (show ¬(y < z) by omega),
                  if_pos (show z < y by omega),
                  EffSet.union.eq_3, if_neg hxz, if_pos hzx]
                have key := ih ((x :: xs).length + (y :: ys).length + zs.length)
                  (by omega) (x :: xs) (y :: ys) zs (Nat.le_refl _)
                rw [EffSet.union.eq_3, if_neg hxy, if_neg hyx] at key
                exact cons_congr z key
              · -- x = y = z
                rename_i hzx
                rw [if_neg (show ¬(y < z) by omega),
                  if_neg (show ¬(z < y) by omega),
                  EffSet.union.eq_3, if_neg hxy, if_neg hyx]
                have key := ih (xs.length + ys.length + zs.length) (by omega)
                  xs ys zs (Nat.le_refl _)
                exact cons_congr x key

/-- Summary union is associative. -/
theorem union_assoc (a b c : EffSet) :
    (a.union b).union c = a.union (b.union c) := by
  obtain ⟨l1⟩ := a
  obtain ⟨l2⟩ := b
  obtain ⟨l3⟩ := c
  exact union_assoc_aux (l1.length + l2.length + l3.length) l1 l2 l3 (Nat.le_refl _)

/-- A left-folded summary union reassociates onto its base: the checker's
accumulated bound equals the judgment's right-nested derivation index. -/
theorem foldl_union (heads : List EffSet) (b : EffSet) :
    heads.foldl EffSet.union b
      = EffSet.union b (heads.foldr EffSet.union EffSet.empty) := by
  induction heads generalizing b with
  | nil => rw [List.foldl_nil, List.foldr_nil, union_empty_right]
  | cons h hs ih =>
      rw [List.foldl_cons, List.foldr_cons, ih, union_assoc, ← ih]

end NobleM2
