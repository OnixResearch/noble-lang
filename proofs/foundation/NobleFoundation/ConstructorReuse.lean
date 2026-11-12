/-
  Constructor reuse under a uniqueness witness (issue #36).

  This is a finite observation model only. A heap is a total lookup from
  natural references to optional cells, while each fixture observes finitely
  many roots. A cell records a tag, its allocated size, and an ordered list of
  fields. Each field carries a reference together with an ownership payload,
  so the resource controls can talk directly to `Ownership.containsResource`
  and `Ownership.duplicable`.

  `reachableFrom` is intentionally a depth-one relation: it sees a root and
  the targets of that root's fields, but it does not recurse through those
  targets. Reuse overwrites one address only after the old cell is present and
  the requested size matches. The root at that address is treated as having
  consumed the old value and now observing the replacement. The contract
  theorem's uniqueness premise is what makes "exactly one root occurrence
  observes the replacement" true; the shared-root fixture shows that with two
  aliases the same overwrite changes both occurrences.

  Resource movement is not an operation in this model. Eligibility of a
  replacement cell means that every payload in its fields is duplicable, so a
  wrapped must-use resource can become eligible only by being absent from the
  replacement fields (the resource has been moved outside this observation).
  No allocator, runtime, Wasm lowering, measurement, or source-level
  ownership correspondence is claimed.
-/
import NobleFoundation.Ownership

namespace NobleFoundation.ConstructorReuse

open NobleFoundation

/-- A location in the finite observations of the heap. -/
abbrev Ref := Nat

/-- One cell field: a target reference and the payload stored at that field. -/
structure Field where
  target : Ref
  payload : Ownership.Payload
deriving DecidableEq, Repr

/-- The observable constructor shape. -/
structure Cell where
  tag : Nat
  size : Nat
  fields : List Field
deriving DecidableEq, Repr

/-- A finite-observation heap lookup. Only roots and depth-one field targets are
    observed by this module. -/
abbrev Heap := Nat → Option Cell

/-- The cell currently observed at a reference. -/
def observe (heap : Heap) (reference : Ref) : Option Cell := heap reference

/-- The reference at one position in an ordered root list. -/
def rootAt : List Ref → Nat → Option Ref
  | [], _ => none
  | reference :: _, 0 => some reference
  | _ :: rest, index + 1 => rootAt rest index

/-- The cell observed through one position in an ordered root list. -/
def observedAt (heap : Heap) (roots : List Ref) (index : Nat) : Option Cell :=
  match rootAt roots index with
  | some reference => observe heap reference
  | none => none

/-- Depth-one reachability from the supplied roots. This is not a transitive
    graph reachability relation. -/
def reachableFrom (heap : Heap) (roots : List Ref) (reference : Ref) : Prop :=
  reference ∈ roots ∨
    ∃ root ∈ roots, ∃ cell, observe heap root = some cell ∧
      ∃ field ∈ cell.fields, field.target = reference

/-- Exactly one root occurrence points at the reference. -/
def unique (roots : List Ref) (reference : Ref) : Prop :=
  roots.count reference = 1

/-- The root occurrences whose observed cell differs between two heaps. -/
def changedRoots (heap updated : Heap) (roots : List Ref) : List Ref :=
  roots.filter fun reference => decide (observe updated reference ≠ observe heap reference)

private theorem filter_beq_eq_singleton (roots : List Ref) (reference : Ref)
    (hcount : roots.count reference = 1) :
    roots.filter (fun root => root == reference) = [reference] := by
  have hmem : reference ∈ roots := by
    rw [← List.count_pos_iff]
    omega
  have hlen : (roots.filter (fun root => root == reference)).length = 1 := by
    rw [← List.count_eq_length_filter]
    exact hcount
  have hall : ∀ root ∈ roots.filter (fun root => root == reference), root = reference := by
    intro root member
    exact beq_iff_eq.mp (List.mem_filter.mp member).2
  match hlist : roots.filter (fun root => root == reference) with
  | [] =>
      rw [hlist] at hlen
      simp at hlen
  | head :: rest =>
      have hhead : head = reference := hall head (by rw [hlist]; simp)
      subst hhead
      have hrest : rest = [] := by
        have empty : rest.length = 0 := by
          rw [hlist] at hlen
          simpa using hlen
        exact List.eq_nil_of_length_eq_zero empty
      rw [hrest]

/-- A cell whose size agrees with the number of fields is initialized for this
    model. -/
def initialized (cell : Cell) : Prop :=
  cell.size = cell.fields.length

/-- Construct an initialized cell with the requested tag and ordered fields. -/
def makeCell (tag : Nat) (fields : List Field) : Cell :=
  { tag := tag, size := fields.length, fields := fields }

/-- Update one heap address, leaving every other lookup unchanged. -/
def updateHeap (heap : Heap) (reference : Ref) (newCell : Cell) : Heap :=
  fun candidate => if candidate = reference then some newCell else heap candidate

/-- In-place constructor reuse. A missing old cell and a size mismatch both
    refuse the step. -/
def reuseStep (heap : Heap) (reference : Ref) (newCell : Cell) : Option Heap :=
  match observe heap reference with
  | none => none
  | some oldCell =>
      if oldCell.size = newCell.size then some (updateHeap heap reference newCell) else none

/-- The replacement cell carries the requested tag, shape and initialization. -/
theorem makeCell_requested (tag : Nat) (fields : List Field) :
    (makeCell tag fields).tag = tag ∧
      (makeCell tag fields).size = fields.length ∧
      (makeCell tag fields).fields = fields ∧
      initialized (makeCell tag fields) := by
  simp [makeCell, initialized]

/-- A successful reuse step has the explicit updated heap. -/
theorem reuseStep_success (heap : Heap) (reference : Ref) (oldCell newCell : Cell)
    (hold : observe heap reference = some oldCell)
    (hsize : oldCell.size = newCell.size) :
    reuseStep heap reference newCell = some (updateHeap heap reference newCell) := by
  simp [reuseStep, hold, hsize]

/-- A size mismatch refuses reuse rather than changing the heap. -/
theorem reuseStep_refuses_size_mismatch (heap : Heap) (reference : Ref)
    (oldCell newCell : Cell)
    (hold : observe heap reference = some oldCell)
    (hmismatch : oldCell.size ≠ newCell.size) :
    reuseStep heap reference newCell = none := by
  simp [reuseStep, hold, hmismatch]

/-- With one owner, consuming the old value and observing the replacement at the
    same address changes exactly the one root occurrence that names it, and
    leaves every other root's cell unchanged. The uniqueness premise is used in
    `filter_beq_eq_singleton`: without it the changed occurrence list can hold
    two entries, as `shared_alias_control` shows. When the replacement is the
    old cell itself nothing changes, so the count claim is conditional on the
    cells differing. -/
theorem reuseStep_replaces_unique_root (heap : Heap) (roots : List Ref)
    (reference : Ref) (oldCell newCell : Cell)
    (hunique : unique roots reference)
    (hold : observe heap reference = some oldCell)
    (hsize : oldCell.size = newCell.size) :
    ∃ updated,
      reuseStep heap reference newCell = some updated ∧
        observe updated reference = some newCell ∧
        (newCell ≠ oldCell → changedRoots heap updated roots = [reference]) ∧
        ∀ other, other ∈ roots → other ≠ reference →
          observe updated other = observe heap other := by
  refine ⟨updateHeap heap reference newCell, ?_, ?_, ?_, ?_⟩
  · exact reuseStep_success heap reference oldCell newCell hold hsize
  · simp [observe, updateHeap]
  · intro hnew
    have hpred : ∀ root ∈ roots,
        decide (observe (updateHeap heap reference newCell) root ≠ observe heap root)
          = (root == reference) := by
      intro root member
      by_cases same : root = reference
      · have updated : observe (updateHeap heap reference newCell) root = some newCell := by
          simp [observe, updateHeap, same]
        have holdAt : observe heap root = some oldCell := by
          rw [same]
          exact hold
        have differs : decide (observe (updateHeap heap reference newCell) root
            ≠ observe heap root) = true := by
          rw [updated, holdAt, decide_eq_true_eq]
          intro sameCell
          exact hnew (Option.some.inj sameCell)
        have self : (root == reference) = true := beq_iff_eq.mpr same
        rw [differs, self]
      · have agrees : observe (updateHeap heap reference newCell) root = observe heap root := by
          simp [observe, updateHeap, same]
        have equal : decide (observe (updateHeap heap reference newCell) root
            ≠ observe heap root) = false := by
          simp [agrees]
        have distinct : (root == reference) = false := beq_eq_false_iff_ne.mpr same
        rw [equal, distinct]
    unfold changedRoots
    rw [List.filter_congr hpred]
    exact filter_beq_eq_singleton roots reference hunique
  · intro other _ hother
    simp [observe, updateHeap, hother]

/-- The constructor-specific form exposes tag, size, shape and initialized
    fields at the unique root after reuse. -/
theorem reuseStep_initializes_requested_cell (heap : Heap) (roots : List Ref)
    (reference : Ref) (oldCell : Cell) (tag : Nat) (fields : List Field)
    (hunique : unique roots reference)
    (hold : observe heap reference = some oldCell)
    (hsize : oldCell.size = fields.length) :
    ∃ updated,
      reuseStep heap reference (makeCell tag fields) = some updated ∧
        observe updated reference = some (makeCell tag fields) ∧
        (makeCell tag fields).tag = tag ∧
        (makeCell tag fields).size = fields.length ∧
        (makeCell tag fields).fields = fields ∧
        initialized (makeCell tag fields) := by
  obtain ⟨updated, hstep, hroot, _⟩ :=
    reuseStep_replaces_unique_root heap roots reference oldCell (makeCell tag fields)
      hunique hold hsize
  refine ⟨updated, hstep, hroot, ?_⟩
  simp [makeCell, initialized]

/-- A cell carries a resource exactly when one of its fields carries a payload
    containing a resource at some wrapping depth. -/
def cellContainsResource (cell : Cell) : Bool :=
  cell.fields.any (fun field => Ownership.containsResource field.payload)

/-- A replacement is resource-eligible exactly when all its field payloads are
    duplicable. -/
def reuseEligible (cell : Cell) : Bool :=
  cell.fields.all (fun field => Ownership.duplicable field.payload)

/-- Moving a resource is represented only by its absence from the replacement
    fields; no resource-transfer operation is part of this model. -/
def resourceMoved (oldCell newCell : Cell) : Prop :=
  cellContainsResource oldCell = true ∧ cellContainsResource newCell = false

/-- An eligible replacement cannot still carry a must-use resource. -/
theorem eligible_replacement_has_no_resource (cell : Cell)
    (heligible : reuseEligible cell = true) :
    cellContainsResource cell = false := by
  cases cell with
  | mk tag size fields =>
    induction fields with
    | nil => simp [cellContainsResource, reuseEligible] at *
    | cons field rest ih =>
        simp only [cellContainsResource, reuseEligible, List.any_cons, List.all_cons]
          at heligible ⊢
        rw [Bool.and_eq_true] at heligible
        by_cases hresource : Ownership.containsResource field.payload = true
        · have hfield := (Ownership.duplicable_iff field.payload).mp heligible.1
          simp [hresource] at hfield
        · have hcontains : Ownership.containsResource field.payload = false := by
            cases h : Ownership.containsResource field.payload <;> simp_all
          have hfield : Ownership.duplicable field.payload = true := by
            exact (Ownership.duplicable_iff field.payload).2 hcontains
          have hrest : rest.all (fun item => Ownership.duplicable item.payload) = true := by
            simpa [hfield] using heligible
          have hrestNoResource :
              rest.any (fun item => Ownership.containsResource item.payload) = false := by
            exact ih hrest
          simp [hcontains, hrestNoResource]

/-- A replacement of a resource-carrying cell is resource-moved whenever it is
    eligible: the old resource is absent from the new fields. -/
theorem eligible_replacement_requires_move (oldCell newCell : Cell)
    (hold : cellContainsResource oldCell = true)
    (heligible : reuseEligible newCell = true) :
    resourceMoved oldCell newCell := by
  exact ⟨hold, eligible_replacement_has_no_resource newCell heligible⟩

/-- Resource-bearing wrapped payload control. `by decide` checks both the
    recursive resource detector and the duplicability gate. -/
def wrappedResourceCell : Cell :=
  makeCell 7 [{ target := 3, payload := .wrapped (.resource 11) }]

theorem wrapped_resource_is_not_reuse_eligible :
    cellContainsResource wrappedResourceCell = true ∧
      reuseEligible wrappedResourceCell = false ∧
      Ownership.containsResource (.wrapped (.resource 11)) = true ∧
      Ownership.duplicable (.wrapped (.resource 11)) = false := by
  decide

/-- A tiny two-root heap used to expose the shared-alias failure mode. -/
def aliasReference : Ref := 5

def aliasOldCell : Cell := makeCell 1 []

def aliasNewCell : Cell := makeCell 2 []

def aliasHeap : Heap :=
  fun reference => if reference = aliasReference then some aliasOldCell else none

def aliasRoots : List Ref := [aliasReference, aliasReference]

/-- The root occurrences whose observed cell changes after the overwrite. -/
def aliasChangedRoots : List Ref :=
  match reuseStep aliasHeap aliasReference aliasNewCell with
  | some updated => changedRoots aliasHeap updated aliasRoots
  | none => []

/-- **Shared aliases are not refused by the overwrite**: the step succeeds and
    changes both root occurrences' observed cell, so the reuse contract's
    uniqueness premise is load-bearing rather than decorative. -/
theorem shared_alias_control :
    aliasRoots.count aliasReference = 2 ∧
      aliasChangedRoots = [aliasReference, aliasReference] := by
  decide

end NobleFoundation.ConstructorReuse
