/-
  Root-pinned live-dispatch model (issue #47, phase 1).

  The selected live-reference design pins one immutable registry map per root,
  selects from that map at every live dispatch, and checks current authority
  independently at the dispatch. Saved exact Programs stay immutable. This file
  names the bounded transition model and the properties the design claims; it
  makes no claim about the JavaScript runtime's correspondence to it.

  The model has bounded roots, bounded slots and a finite publication sequence.
  Observations are exact selected identities together with the ordered guest
  request/response/commit trace, and control events (denials, publication
  notices) are separately identifiable rather than interleaved into the guest
  trace.
-/
namespace NobleFoundation.LiveDispatch

/-- One slot identity with the generation an entry map pins for it. -/
structure Entry where
  key : Nat
  generation : Nat
deriving DecidableEq, Repr

/-- A pinned registry map: an ordered finite list of entries. Later entries for
    the same key shadow earlier ones, mirroring a map update. -/
abbrev EntryMap := List Entry

/-- A root pins its entry map at a publication epoch. The map is immutable:
    nothing in this model can replace it in place. -/
structure Root where
  epoch : Nat
  pinned : EntryMap
deriving DecidableEq, Repr

/-- The selection an entry map gives for one key, if any. -/
def select (entries : EntryMap) (key : Nat) : Option Nat :=
  match entries with
  | [] => none
  | entry :: rest => if entry.key = key then some entry.generation else select rest key

/-- One single-slot publication: the existing global-epoch compare-and-swap
    replaces exactly one slot's generation. -/
def publish (entries : EntryMap) (key : Nat) (generation : Nat) : EntryMap :=
  ⟨key, generation⟩ :: entries

/-- A fresh root pins the map as published so far. -/
def pinRoot (epoch : Nat) (entries : EntryMap) : Root := ⟨epoch, entries⟩

/-- One guest-visible event of a bounded execution. A dispatch records the
    exact identity the root's pinned map selected. -/
inductive GuestEvent where
  | dispatch (root : Nat) (key : Nat) (generation : Nat)
  | committed
deriving DecidableEq, Repr

/-- Control events are kept separately identifiable: a denial stops execution
    before the event it guards, and a publication notice is not a guest event. -/
inductive ControlEvent where
  | denied (root : Nat) (key : Nat)
  | published (epoch : Nat) (key : Nat) (generation : Nat)
deriving DecidableEq, Repr

/-- The ordered trace a request through one root produces when every dispatch is
    authorized. Selections come from the root's pinned map only. -/
def trace (root : Root) (requests : List Nat) : List GuestEvent :=
  requests.filterMap fun key =>
    match select root.pinned key with
    | some generation => some (.dispatch root.epoch key generation)
    | none => none

/-- The frozen-entry-map reference: the same requests evaluated against the map
    captured when the root was pinned, independent of any later publication. -/
def reference (epoch : Nat) (pinned : EntryMap) (requests : List Nat) : List GuestEvent :=
  requests.filterMap fun key =>
    match select pinned key with
    | some generation => some (.dispatch epoch key generation)
    | none => none

/-- Publishing one slot changes exactly that slot's selection. -/
theorem publish_preserves_other_keys (entries : EntryMap) (key other generation : Nat)
    (distinct : key ≠ other) :
    select (publish entries key generation) other = select entries other := by
  unfold publish
  simp only [select, distinct, if_false]

/-- Publishing one slot changes that slot's selection to the published one. -/
theorem publish_selects_the_published_generation (entries : EntryMap) (key generation : Nat) :
    select (publish entries key generation) key = some generation := by
  unfold publish select
  simp

/-- Pinning is the only way to give a root a map, and the pinned map is never
    replaced in place: two pins of the same epoch agree. -/
theorem pin_is_immutable (epoch : Nat) (entries : EntryMap) :
    (pinRoot epoch entries).pinned = entries := by
  rfl

/-- A root's selections use its own pinned map, so a later single-slot
    publication cannot change any selection the root makes. -/
theorem root_selections_use_pinned_map (root : Root) (requests : List Nat) :
    trace root requests
      = requests.filterMap fun key =>
          match select root.pinned key with
          | some generation => some (.dispatch root.epoch key generation)
          | none => none := by
  rfl

/-- Execution through a root equals the frozen-entry-map reference given the
    same request sequence: the pin, not the current map, decides selection. -/
theorem frozen_entry_map_reference_agrees (root : Root) (requests : List Nat) :
    trace root requests = reference root.epoch root.pinned requests := by
  unfold trace reference
  rfl

/-- Two separate single-slot publications are not one atomic replacement: a map
    pinned between them can select a mixed pair. -/
def mixedMap (fresh : EntryMap) : EntryMap := publish fresh 0 2

theorem mixed_map_selects_old_and_new (fresh : EntryMap) :
    select (mixedMap fresh) 0 = some 2 := by
  unfold mixedMap
  exact publish_selects_the_published_generation fresh 0 2

/-- A dispatch that is denied produces no guest event: authorization is checked
    at the dispatch even though the map selection is frozen. -/
def authorize (allowed : Bool) (root : Root) (key : Nat) : Option GuestEvent :=
  match allowed with
  | true =>
      match select root.pinned key with
      | some generation => some (.dispatch root.epoch key generation)
      | none => none
  | false => none

theorem denied_dispatch_produces_no_guest_event (root : Root) (key : Nat) :
    authorize false root key = none := by
  rfl

theorem authorized_dispatch_selects_the_pinned_generation (root : Root) (key : Nat)
    (generation : Nat) (pinned : select root.pinned key = some generation) :
    authorize true root key = some (.dispatch root.epoch key generation) := by
  unfold authorize
  rw [pinned]

/-- A finite abnormal prefix keeps its committed guest events: the model never
    rolls a committed event back when a later dispatch is denied. -/
def abnormalPrefix (root : Root) (before : List Nat) (deniedKey : Nat) :
    List GuestEvent × Option ControlEvent :=
  (trace root before, some (.denied root.epoch deniedKey))

theorem abnormal_prefix_preserves_committed_events (root : Root) (before : List Nat)
    (deniedKey : Nat) :
    (abnormalPrefix root before deniedKey).1 = trace root before := by
  rfl

end NobleFoundation.LiveDispatch
