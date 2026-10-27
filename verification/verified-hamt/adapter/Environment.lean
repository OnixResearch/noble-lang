module

public import VerifiedHAMT.Map

@[expose] public section

/-!
# VerifiedHAMT adapter for one Noble named-environment lookup (issue #60)

Isolated evaluation artifact, not Noble proof authority. It maps **one** existing Lean reference
environment's lookup onto the bundled `VerifiedHAMT.Map` and states the list↔map relation explicitly.

## Duplicate policy: first-wins for arbitrary lists

`NamedV1.Environment.lookup` is `entries.find? (fun entry => entry.slot == slot)`: the **first** entry
whose slot matches wins. The bundled constructor `Map.ofList` is last-write-wins (measured in
`receipts/2026-10-26/duplicate-slot-eval.lean`), so converting with it changes behaviour.

`NamedV1.Environment.lookup` is also total over an arbitrary environment, and the `Environment` type
carries no uniqueness proof, so this adapter chooses **first-wins for arbitrary lists** rather than
requiring a unique-slot premise that the type does not carry. `toMap` folds from the right, so the
first occurrence of a slot is inserted last and wins; that is the whole policy, and `lookup_first_match`
proves it agrees with the list rule on every list, not only on duplicate-free ones.

The ordered entry list remains the source of truth for traversal, dependency order and identity: this
file builds an index and compares **complete** entries, never slots or lengths.
-/

namespace NobleVerifiedHamtAdapter

/-- One entry with every field the reference environment carries: slot, provenance, ordered
interfaces, body, dependencies and rank. Comparison is structural, so a substituted provenance or
interface list fails the claimed relation instead of passing on the slot alone. -/
structure Entry where
  slot : Nat
  provenance : Nat
  interfaces : List Nat
  body : List Nat
  dependencies : List Nat
  rank : Nat
  deriving DecidableEq, Repr

/-- The reference environment's ordered entries. -/
abbrev Environment := List Entry

/-- Boolean equality on slots commutes; `find?_insert` states its condition as `q == key` while the
reference rule states it as `entry.slot == slot`. -/
theorem beq_comm_nat (left right : Nat) : (left == right) = (right == left) := by
  by_cases same : left = right
  · subst same
    rfl
  · rw [beq_eq_false_iff_ne.mpr same, beq_eq_false_iff_ne.mpr (Ne.symm same)]

/-- The reference rule, restated here so the adapter relation is checked against the *stated* rule
rather than against the adapter's own conversion. -/
def lookupEntry (environment : Environment) (slot : Nat) : Option Entry :=
  environment.find? (fun entry => entry.slot == slot)

/-- The adapter: fold from the right so the first occurrence of a slot is inserted last and wins. -/
def toMap (environment : Environment) : VerifiedHAMT.Map Nat Entry :=
  environment.foldr (fun entry index => index.insert entry.slot entry) ∅

/-- The adapter's index alone is not the source of truth: the ordered list is retained, and a lookup
is reported as the pair. -/
structure Indexed where
  order : Environment
  index : VerifiedHAMT.Map Nat Entry

def index (environment : Environment) : Indexed :=
  { order := environment, index := toMap environment }

/-- **The adapter relation**: on every environment, duplicate-free or not, the index's lookup is the
reference first-match lookup, and it returns the **complete** first entry. -/
theorem lookup_first_match (environment : Environment) (slot : Nat) :
    (toMap environment).find? slot = lookupEntry environment slot := by
  induction environment with
  | nil =>
      simp only [lookupEntry, List.find?_nil, toMap, List.foldr_nil]
      exact VerifiedHAMT.Map.find?_empty slot
  | cons entry rest fresh =>
      simp only [lookupEntry, List.find?_cons, toMap, List.foldr_cons]
      rw [VerifiedHAMT.Map.find?_insert, beq_comm_nat slot entry.slot]
      by_cases matched : entry.slot == slot
      · simp [matched]
      · simp only [matched, Bool.false_eq_true, if_false]
        exact fresh

/-- The ordered list is preserved exactly, so dependency order and identity do not depend on the
index at all. -/
theorem index_keeps_the_order (environment : Environment) :
    (index environment).order = environment := rfl

/-- Build preservation: an empty environment builds the empty index and looks up nothing. -/
theorem empty_build_looks_up_nothing (slot : Nat) :
    (toMap ([] : Environment)).find? slot = none := by
  simp only [toMap, List.foldr_nil]
  exact VerifiedHAMT.Map.find?_empty slot

/-- A missing slot remains absent: `none` is a real answer, not a default entry. -/
theorem absent_slot_remains_absent (environment : Environment) (slot : Nat)
    (absent : lookupEntry environment slot = none) :
    (toMap environment).find? slot = none := by
  rw [lookup_first_match]
  exact absent

/-- Folding the same entries over two seeds that already agree on `slot` keeps them agreeing: a fold
step inserts under one entry's slot, and every such insertion commutes with the lookup of `slot` when
the seeds already agree. -/
theorem foldr_preserves_lookup_aux (environment : Environment) (slot : Nat) :
    ∀ (left right : VerifiedHAMT.Map Nat Entry), left.find? slot = right.find? slot →
      (environment.foldr (fun entry index => index.insert entry.slot entry) left).find? slot =
        (environment.foldr (fun entry index => index.insert entry.slot entry) right).find? slot := by
  induction environment with
  | nil => intro left right agree; exact agree
  | cons head rest fresh =>
      intro left right agree
      simp only [List.foldr_cons]
      have folded := fresh left right agree
      rw [VerifiedHAMT.Map.find?_insert, VerifiedHAMT.Map.find?_insert, folded]

/-- The general form used below. -/
theorem foldr_preserves_lookup (environment : Environment) (slot : Nat)
    (left right : VerifiedHAMT.Map Nat Entry) (agree : left.find? slot = right.find? slot) :
    (environment.foldr (fun entry index => index.insert entry.slot entry) left).find? slot =
      (environment.foldr (fun entry index => index.insert entry.slot entry) right).find? slot :=
  foldr_preserves_lookup_aux environment slot left right agree

/-- Inserting an entry under a **different** slot does not disturb the lookup of `slot`, at either end
of the list. Lawful equality and hashing remain the premise, as upstream requires. -/
theorem different_slot_insertion_is_unaffected (environment : Environment) (entry : Entry)
    (slot : Nat) (different : entry.slot ≠ slot) :
    (toMap (entry :: environment)).find? slot = (toMap environment).find? slot ∧
      (toMap (environment ++ [entry])).find? slot = (toMap environment).find? slot := by
  have neq : (entry.slot == slot) = false := beq_eq_false_iff_ne.mpr different
  have front : (toMap (entry :: environment)).find? slot = (toMap environment).find? slot := by
    rw [lookup_first_match, lookup_first_match]
    simp only [lookupEntry, List.find?_cons, neq]
  have seeds : ((∅ : VerifiedHAMT.Map Nat Entry).insert entry.slot entry).find? slot =
      (∅ : VerifiedHAMT.Map Nat Entry).find? slot := by
    rw [VerifiedHAMT.Map.find?_insert, beq_comm_nat slot entry.slot, neq]
    rfl
  have back : (toMap (environment ++ [entry])).find? slot = (toMap environment).find? slot := by
    simp only [toMap, List.foldr_append, List.foldr_cons, List.foldr_nil]
    exact foldr_preserves_lookup environment slot _ _ seeds
  exact ⟨front, back⟩

end NobleVerifiedHamtAdapter
