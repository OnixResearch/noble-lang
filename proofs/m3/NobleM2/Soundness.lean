/-
Reference model: soundness of the reference decision function (PO-09).

The acceptance theorem's shape (V-CHECK-03) is:
`check(candidate, expected_interface, environment) = Accepted(checked)` implies
`WellFormed(checked) and TypingDerivation(checked, expected_interface)`.

This module develops the lemmas that carry the fold's successful outcome back
to the declarative judgment. Fragment v1: the scheme application resolves
the witness's reference bindings first, so the exposed instantiation is the
*resolved* witness's substitution, and the resolution walk decides the
judgment's declarative `ResolvesTo` relation (B-CHECK-05).
-/

import NobleM2.Check
import NobleM2.Judgment

namespace NobleM2

/-- An accepted scheme application exposes the instantiated interface: the
interface's stacks and bound are exactly the substitution's results over the
*resolved* witness (every reference binding replaced by its terminal
binding), and the resolved witness is the one the application carried
through. -/
theorem applyScheme_ok {env : Env} {limits : Limits} {scheme : Scheme} {inst : Inst}
    {iface : Interface} {resolved : Inst}
    (accepted : applyScheme env limits scheme inst = .ok (iface, resolved)) :
    scheme.instantiate resolved = some (iface.stackIn, iface.stackOut, iface.effects) := by
  unfold applyScheme at accepted
  split at accepted
  · contradiction
  · split at accepted
    · contradiction
    · contradiction
    · contradiction
    · contradiction
    · split at accepted
      · contradiction
      · split at accepted
        · contradiction
        · split at accepted
          · contradiction
          · injection accepted with pair_eq
            injection pair_eq with iface_eq resolved_eq
            subst iface_eq
            subst resolved_eq
            assumption

/-! ## The resolution walk decides the declarative resolution (B-CHECK-05) -/

/-- A chain the walk follows successfully ends where the declarative
`ChainEnds` relation says it does: the walk's recursive steps are exactly
the relation's hops. -/
theorem followFrom_chain (witness : Inst) :
    ∀ (remaining current hops target left : Nat),
      witness.followFrom current remaining hops = .ok (target, left) →
      ChainEnds witness current target := by
  intro remaining
  induction remaining with
  | zero =>
      intro current hops target left h
      rw [Inst.followFrom] at h
      split at h
      · exact absurd h (by simp)
      · split at h
        · exact absurd h (by simp)
        · exact absurd rfl (by assumption)
  | succ n ih =>
      intro current hops target left h
      rw [Inst.followFrom] at h
      split at h
      · exact absurd h (by simp)
      · split at h
        · exact absurd h (by simp)
        · cases hbind : witness.bindings[current]? with
          | none => simp only [hbind] at h; exact absurd h (by simp)
          | some binding =>
              simp only [hbind] at h
              cases binding with
              | stack seg =>
                  injection h with hp
                  injection hp with hc _
                  subst hc
                  exact ChainEnds.here ⟨.stack seg, hbind, by simp [Binding.kindOf]⟩
              | value ty =>
                  injection h with hp
                  injection hp with hc _
                  subst hc
                  exact ChainEnds.here ⟨.value ty, hbind, by simp [Binding.kindOf]⟩
              | effect set =>
                  injection h with hp
                  injection hp with hc _
                  subst hc
                  exact ChainEnds.here ⟨.effect set, hbind, by simp [Binding.kindOf]⟩
              | ref next =>
                  exact ChainEnds.hop hbind
                    (ih next (hops + 1) target left (by simpa using h))

/-- One position the resolution pass resolves successfully carries the
binding the declarative `PositionResolves` names: its own direct binding,
or the terminal binding of the chain its reference follows. -/
theorem resolveOne_position {kinds : List Kind} {witness : Inst} {index : Nat}
    {binding : Binding} {remaining : Nat} {b : Binding} {left : Nat}
    (h : Inst.resolveOne kinds witness index binding remaining = .ok (b, left))
    (hlook : witness.bindings[index]? = some binding) :
    PositionResolves witness index b := by
  simp only [Inst.resolveOne] at h
  cases binding with
  | stack _ | value _ | effect _ =>
      simp only [] at h
      injection h with pair_eq
      injection pair_eq with hb _
      subst hb
      exact PositionResolves.direct hlook (by simp [Binding.kindOf])
  | ref v =>
      simp only [Inst.resolveOne] at h
      cases hf : witness.followFrom v remaining 0 with
      | error => simp only [hf] at h; exact absurd h (by simp)
      | ok pair =>
          obtain ⟨terminal, left1⟩ := pair
          simp only [hf] at h
          have hchain := followFrom_chain witness remaining v 0 terminal left1 hf
          cases hk : kinds[index]? with
          | none => simp only [hk] at h; exact absurd h (by simp)
          | some kind =>
              simp only [hk] at h
              cases ht : witness.bindings[terminal]? with
              | none => simp only [ht] at h; exact absurd h (by simp)
              | some term =>
                  simp only [ht] at h
                  split at h
                  · rename_i hcond
                    injection h with pair_eq
                    injection pair_eq with hb _
                    subst hb
                    refine PositionResolves.chain hlook hchain ht ?_
                    cases hterm : term.kindOf with
                    | none => rw [hterm] at hcond; simp at hcond
                    | some k => simp [hterm]
                  · exact absurd h (by simp)

/-- The resolution pass over one witness, stated against the positions the
pass has reached: every resolved position satisfies `PositionResolves`,
and the pass preserves the witness's shape. -/
theorem resolveList_positions (kinds : List Kind) (witness : Inst) :
    ∀ (index remaining : Nat) (bs result : List Binding) (left : Nat),
      (hget : ∀ i, i < bs.length → witness.bindings[index + i]? = bs[i]?) →
      Inst.resolveList kinds witness index remaining bs = .ok (result, left) →
      result.length = bs.length ∧
        ∀ i, i < bs.length → ∃ b, result[i]? = some b ∧
          PositionResolves witness (index + i) b := by
  intro index remaining bs
  induction bs generalizing index remaining with
  | nil =>
      intro result left _ h
      simp only [Inst.resolveList] at h
      injection h with pair_eq
      injection pair_eq with result_eq _
      subst result_eq
      exact ⟨rfl, fun i hi => absurd hi (by simp)⟩
  | cons binding rest ih =>
      intro result left hget h
      simp only [Inst.resolveList] at h
      have hlook : witness.bindings[index]? = some binding := by
        simpa using hget 0 (by simp)
      cases hone : Inst.resolveOne kinds witness index binding remaining with
      | error => simp only [hone] at h; exact absurd h (by simp)
      | ok pair1 =>
          obtain ⟨b, left1⟩ := pair1
          simp only [hone] at h
          cases hrec : Inst.resolveList kinds witness (index + 1) left1 rest with
          | error => simp only [hrec] at h; exact absurd h (by simp)
          | ok pair2 =>
              obtain ⟨bs', left2⟩ := pair2
              simp only [hrec] at h
              injection h with pair_eq
              injection pair_eq with result_eq _
              subst result_eq
              obtain ⟨hlen, hpos⟩ := ih (index + 1) left1 bs' left2
                (fun i hi => by
                  simpa only [Nat.add_assoc, Nat.add_comm, Nat.add_left_comm,
                    List.getElem?_cons_succ] using hget (i + 1) (by simpa using hi))
                hrec
              refine ⟨by simp [hlen], fun i hi => ?_⟩
              cases i with
              | zero =>
                  exact ⟨b, rfl, resolveOne_position hone hlook⟩
              | succ i =>
                  obtain ⟨b', hb', hpos'⟩ := hpos i (by simpa using hi)
                  exact ⟨b', by simpa using hb',
                    by simpa only [Nat.add_assoc, Nat.add_comm, Nat.add_left_comm] using hpos'⟩

/-- The checker's resolution decides the judgment's declarative
`ResolvesTo`: a witness the walk resolves successfully satisfies the
relation, position by position. -/
theorem resolve_resolvesTo {kinds : List Kind} {witness resolved : Inst} {fuel left : Nat}
    (h : Inst.resolve kinds witness fuel = .ok (resolved, left)) :
    ResolvesTo witness resolved := by
  rw [Inst.resolve] at h
  cases hres : Inst.resolveList kinds witness 0 fuel witness.bindings with
  | error => simp only [hres] at h; exact absurd h (by simp)
  | ok pair =>
      obtain ⟨bs, left'⟩ := pair
      simp only [hres] at h
      injection h with pair_eq
      injection pair_eq with inst_eq left_eq
      subst inst_eq
      subst left_eq
      obtain ⟨hlen, hpos⟩ := resolveList_positions kinds witness 0 fuel
        witness.bindings bs left' (fun i _ => by simpa using rfl) hres
      exact ⟨hlen.symm, fun i hi => by
        obtain ⟨b, hb, hpos'⟩ := hpos i (by omega)
        exact ⟨b, hb, by simpa using hpos'⟩⟩

/-- A successful scheme application resolves its witness: the resolved
witness it returns satisfies the judgment's declarative `ResolvesTo`. -/
theorem applyScheme_ok_resolves {env : Env} {limits : Limits} {scheme : Scheme}
    {inst : Inst} {iface : Interface} {resolved : Inst}
    (accepted : applyScheme env limits scheme inst = .ok (iface, resolved)) :
    ResolvesTo inst resolved := by
  unfold applyScheme at accepted
  split at accepted
  · contradiction
  · cases hres : Inst.resolve scheme.varKinds inst limits.work with
    | error e =>
        cases e with
        | cyclic => simp only [hres] at accepted; exact absurd accepted (by simp)
        | exhausted => simp only [hres] at accepted; exact absurd accepted (by simp)
        | unknown => simp only [hres] at accepted; exact absurd accepted (by simp)
        | kindMismatch => simp only [hres] at accepted; exact absurd accepted (by simp)
    | ok pair =>
        obtain ⟨resolved', left'⟩ := pair
        simp only [hres] at accepted
        cases hin : scheme.instantiate resolved' with
        | none => simp only [hin] at accepted; exact absurd accepted (by simp)
        | some triple =>
            obtain ⟨stackIn, stackOut, effects⟩ := triple
            simp only [hin] at accepted
            split at accepted
            · contradiction
            · cases hfind : effects.ids.find? (fun id => !effectKnown env id) with
              | some id => simp only [hfind] at accepted; exact absurd accepted (by simp)
              | none =>
                  simp only [hfind] at accepted
                  injection accepted with pair_eq
                  injection pair_eq with iface_eq resolved_eq
                  subst iface_eq
                  subst resolved_eq
                  exact resolve_resolvesTo hres

end NobleM2
