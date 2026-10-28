/-!
  Bounded M6 task-interleaving model (issue #46).

  Two live task records at most, one shared admission capacity, wakeup reservations, a sticky first
  abnormal failure, ordered delivery and settlement records. The model is an *explorer subject*: it
  names which interleavings are reachable and which slot invariants hold at every reachable prefix,
  so a full-search baseline exists before any reduction is claimed.

  The documented observation contract is `observe`; `lossyObserve` is a *deliberately lossy*
  projection that drops the failure and generation fields. Histories can end at the same lossy
  observation yet be distinguished by the real observation, which is the control that stops "same
  final snapshot" from being read as equivalence.

  A deliberately unsound independence rule is refuted rather than assumed: actions on *different*
  task ids are not independent, because a released admission slot changes whether another id's
  admission is enabled. `releasing_a_slot_enables_another_id` and `different_ids_are_not_independent`
  are that counterexample.

  Not modelled, therefore not claimed: unbounded liveness or fairness, callback delivery guarantees,
  cleanup after host death, weak-memory ordering, or any correspondence between this model and the
  Rust table API.
-/
namespace NobleFoundation.TaskInterleaving

/-- The two declared task ids of this bounded model. -/
inductive TaskId where
  | left
  | right
deriving DecidableEq, Repr

/-- One task lifecycle with its generation, so stale callbacks are comparable. -/
inductive Lifecycle where
  | absent
  | live (generation : Nat)
  | retired (generation : Nat)
deriving DecidableEq, Repr

/-- Whether a lifecycle holds an admission record. -/
def Lifecycle.isLive : Lifecycle → Prop
  | .live _ => True
  | _ => False

/-- The generation a lifecycle names, if any. -/
def generationOf : Lifecycle → Option Nat
  | .live generation => some generation
  | .retired generation => some generation
  | .absent => none

/-- Per-id state: lifecycle, pin and wakeup reservation units. -/
structure Slot where
  lifecycle : Lifecycle
  pinHeld : Bool
  reserved : Nat
deriving DecidableEq, Repr

/-- The bounded table: two slots, a shared capacity, the sticky first abnormal failure, and the
    ordered delivery and settlement records. -/
structure Table where
  capacity : Nat
  left : Slot
  right : Slot
  failure : Option Nat
  deliveries : List (TaskId × Nat)
  settled : List (TaskId × Nat)
deriving DecidableEq, Repr

def slot (table : Table) : TaskId → Slot
  | .left => table.left
  | .right => table.right

def withSlot (table : Table) (id : TaskId) (next : Slot) : Table :=
  match id with
  | .left => { table with left := next }
  | .right => { table with right := next }

def counted (operand : Slot) : Nat :=
  match operand.lifecycle with
  | .live _ => 1
  | _ => 0

def liveCount (table : Table) : Nat := counted table.left + counted table.right

/-- An acquired record, named so the counterexamples stay readable. -/
def liveSlot (generation : Nat) : Slot :=
  { lifecycle := .live generation, pinHeld := false, reserved := 0 }

/-- A record that keeps its lifecycle but releases pin and reservation. -/
def retiredSlot (generation : Nat) : Slot :=
  { lifecycle := .retired generation, pinHeld := false, reserved := 0 }

/-- An unoccupied slot. -/
def absentSlot : Slot :=
  { lifecycle := .absent, pinHeld := false, reserved := 0 }

/-- The bounded baseline: the left record is live at generation 1 and the right id is absent,
    against a shared capacity of one, so the shared slot - not the id - decides admission. -/
def baseline : Table :=
  { capacity := 1, left := liveSlot 1, right := absentSlot, failure := none
    deliveries := [], settled := [] }

/-- `baseline` with the left slot finished. -/
def freedLeft : Table :=
  { baseline with left := retiredSlot 1 }

/-- `baseline` with the left delivery already recorded. -/
def deliveredLeft : Table :=
  { baseline with deliveries := [(.left, 1)] }

/-- The documented actions. Native stop is not among them: it is an input observation, not something
    cancellation proves. -/
inductive Action where
  | acquire (id : TaskId) (generation : Nat)
  | cancel (id : TaskId) (generation : Nat)
  | complete (id : TaskId) (generation : Nat) (succeeded : Bool)
  | deliver (id : TaskId) (generation : Nat)
  | staleCallback (id : TaskId) (generation : Nat)
  | settle (id : TaskId) (generation : Nat)
  | cleanup (id : TaskId)
  | finish (id : TaskId) (generation : Nat)
deriving DecidableEq, Repr

/-- The sticky failure rule, named so the acquired-step value stays a plain term rather than a
    `match`, which keeps the preservation proof's substitution syntactic. -/
def stickFailure (operand : Option Nat) (generation : Nat) : Option Nat :=
  match operand with
  | none => some generation
  | held => held

/-- One acquired step. Every refusal is a single guard, an action that names a generation must name
    the live one, and same-slot reuse needs a different generation. -/
def step : Action → Table → Option Table
  | .acquire id generation, table =>
      match (slot table id).lifecycle with
      | .live _ => none
      | .retired previous =>
          if previous ≠ generation ∧ liveCount table < table.capacity then
            some (withSlot table id (liveSlot generation))
          else none
      | .absent =>
          if liveCount table < table.capacity then
            some (withSlot table id (liveSlot generation))
          else none
  | .cancel id generation, table =>
      match (slot table id).lifecycle with
      | .live liveGeneration =>
          if liveGeneration = generation then
            let current := slot table id
            some (withSlot table id { current with
              pinHeld := true
              reserved := if 0 < current.reserved then current.reserved - 1 else 0 })
          else none
      | _ => none
  | .complete id generation succeeded, table =>
      match (slot table id).lifecycle with
      | .live liveGeneration =>
          if liveGeneration = generation then
            if succeeded then some table
            else
              some { table with failure := stickFailure table.failure generation }
          else none
      | _ => none
  | .deliver id generation, table =>
      match (slot table id).lifecycle with
      | .live liveGeneration =>
          if liveGeneration = generation ∧ ¬(id, generation) ∈ table.deliveries then
            some { table with deliveries := table.deliveries ++ [(id, generation)] }
          else none
      | _ => none
  | .staleCallback id generation, table =>
      match (slot table id).lifecycle with
      | .live liveGeneration =>
          if liveGeneration = generation then none else some table
      | .retired previous => if previous = generation then none else some table
      | .absent => some table
  | .settle id generation, table =>
      match (slot table id).lifecycle with
      | .live liveGeneration =>
          if liveGeneration = generation ∧ (id, generation) ∈ table.deliveries then
            some { table with settled := table.settled ++ [(id, generation)] }
          else none
      | _ => none
  | .cleanup id, table =>
      match (slot table id).lifecycle with
      | .live _ => some (withSlot table id { lifecycle := .absent, pinHeld := false, reserved := 0 })
      | _ => none
  | .finish id generation, table =>
      match (slot table id).lifecycle with
      | .live liveGeneration =>
          if liveGeneration = generation then
            some (withSlot table id (retiredSlot generation))
          else none
      | _ => none

/-- Reading a refused-or-acquired step: an `if`-guarded `some` is exactly its guard and its result. -/
theorem some_or_none_eq_some {c : Prop} [Decidable c] {acquired next : Table} :
    ((if c then some acquired else none) = some next) ↔ (c ∧ acquired = next) := by
  by_cases held : c <;> simp [held]

/-- The mirror shape: a refusal branch followed by an admission. -/
theorem none_or_some_eq_some {c : Prop} [Decidable c] {acquired next : Table} :
    ((if c then none else some acquired) = some next) ↔ (¬c ∧ acquired = next) := by
  by_cases held : c <;> simp [held]

/-- Two acquired branches under one guard. -/
theorem some_or_some_eq_some {c : Prop} [Decidable c] {first second next : Table} :
    ((if c then some first else some second) = some next) ↔
      ((c ∧ first = next) ∨ (¬c ∧ second = next)) := by
  by_cases held : c <;> simp [held]

/-- What one slot must satisfy at every reachable prefix: a pin and a reservation belong only to a
    live record. -/
def SlotInvariants (operand : Slot) : Prop :=
  (operand.pinHeld = true → operand.lifecycle.isLive) ∧
  (0 < operand.reserved → operand.lifecycle.isLive)

/-- The table invariants: the shared capacity is never overcommitted and each slot satisfies its own
    invariant. The ordered delivery and settlement records carry their own statements (double
    delivery, settlement before delivery, retained delivery after finish), so this stays about
    admission accounting. -/
def Invariants (table : Table) : Prop :=
  liveCount table ≤ table.capacity ∧ SlotInvariants table.left ∧ SlotInvariants table.right

/-- The empty table with a declared shared capacity. -/
def emptyTable (capacity : Nat) : Table :=
  { capacity
    left := { lifecycle := .absent, pinHeld := false, reserved := 0 }
    right := { lifecycle := .absent, pinHeld := false, reserved := 0 }
    failure := none
    deliveries := []
    settled := [] }

theorem empty_table_satisfies_the_invariants (capacity : Nat) :
    Invariants (emptyTable capacity) := by
  refine ⟨?_, ?_, ?_⟩
  · show liveCount (emptyTable capacity) ≤ capacity
    simp only [emptyTable, liveCount, counted]
    exact Nat.zero_le capacity
  · refine ⟨?_, ?_⟩ <;> intro held <;> simp [emptyTable] at held
  · refine ⟨?_, ?_⟩ <;> intro held <;> simp [emptyTable] at held

/-- One step preserves every slot invariant, so the explorer never has to special-case a prefix. -/
theorem step_preserves_invariants (action : Action) (table next : Table)
    (invariants : Invariants table) (taken : step action table = some next) : Invariants next := by
  obtain ⟨bound, leftSlotInvariants, rightSlotInvariants⟩ := invariants
  cases action with
  | acquire id generation =>
    cases id <;>
    cases leftLife : table.left.lifecycle <;>
    cases rightLife : table.right.lifecycle <;>
    (try (cases succeeded)) <;>
    (simp only [some_or_none_eq_some, 
      slot, leftLife, rightLife, withSlot, liveSlot, counted, liveCount,
      step] at taken ⊢
     ) <;>
    (first
        | (obtain ⟨_, hnext⟩ := taken
           try (subst next))
        | (obtain ⟨⟨_, _⟩, hnext⟩ := taken
           try (subst next))
        | (obtain ⟨_, ⟨_, hnext⟩⟩ := taken
           try (subst next))
        | (obtain ⟨⟨⟨_, _⟩, _⟩, hnext⟩ := taken
           try (subst next))
        | (obtain ⟨_, ⟨⟨_, _⟩, hnext⟩⟩ := taken
           try (subst next))
        | (obtain ⟨⟨_, hnext⟩ | ⟨_, hnext⟩⟩ := taken
           try (subst next))
        | (obtain ⟨⟨⟨_, _⟩ | _⟩, hnext⟩ := taken
           try (subst next))
        | skip)
    all_goals
      simp_all [Invariants, SlotInvariants, Lifecycle.isLive, counted, liveCount]
      try omega
  | cancel id generation =>
    cases id <;>
    cases leftLife : table.left.lifecycle <;>
    cases rightLife : table.right.lifecycle <;>
    (try (cases succeeded)) <;>
    (simp only [some_or_none_eq_some, 
      slot, leftLife, rightLife, withSlot, 
      step] at taken ⊢
     ) <;>
    first
      | (obtain ⟨_, hnext⟩ := taken
         try (subst next))
      | (obtain ⟨⟨_, _⟩, hnext⟩ := taken
         try (subst next))
      | (obtain ⟨_, ⟨_, hnext⟩⟩ := taken
         try (subst next))
      | (obtain ⟨⟨⟨_, _⟩, _⟩, hnext⟩ := taken
         try (subst next))
      | (obtain ⟨_, ⟨⟨_, _⟩, hnext⟩⟩ := taken
         try (subst next))
      | (obtain ⟨⟨_, hnext⟩ | ⟨_, hnext⟩⟩ := taken
         try (subst next))
      | (obtain ⟨⟨⟨_, _⟩ | _⟩, hnext⟩ := taken
         try (subst next))
      | skip
    all_goals
      simp_all [Invariants, SlotInvariants, Lifecycle.isLive, counted, liveCount]
      try omega
  | complete id generation succeeded =>
    cases id <;>
    cases leftLife : table.left.lifecycle <;>
    cases rightLife : table.right.lifecycle <;>
    (try (cases succeeded)) <;>
    (simp only [some_or_none_eq_some, 
      slot, leftLife, rightLife, 
      step, reduceIte, Bool.false_eq_true] at taken ⊢
     first
       | (obtain ⟨_, hnext⟩ := taken)
       | (obtain ⟨⟨_, hnext⟩ | ⟨_, hnext⟩⟩ := taken)
       | skip
     try (subst next)
     try (simp_all [Invariants, SlotInvariants, Lifecycle.isLive, counted, liveCount])
     try omega)
  | deliver id generation =>
    cases id <;>
    cases leftLife : table.left.lifecycle <;>
    cases rightLife : table.right.lifecycle <;>
    (try (cases succeeded)) <;>
    (simp only [some_or_none_eq_some, 
      slot, leftLife, rightLife, 
      step] at taken ⊢
     ) <;>
    first
      | (obtain ⟨_, hnext⟩ := taken
         try (subst next))
      | (obtain ⟨⟨_, _⟩, hnext⟩ := taken
         try (subst next))
      | (obtain ⟨_, ⟨_, hnext⟩⟩ := taken
         try (subst next))
      | (obtain ⟨⟨⟨_, _⟩, _⟩, hnext⟩ := taken
         try (subst next))
      | (obtain ⟨_, ⟨⟨_, _⟩, hnext⟩⟩ := taken
         try (subst next))
      | (obtain ⟨⟨_, hnext⟩ | ⟨_, hnext⟩⟩ := taken
         try (subst next))
      | (obtain ⟨⟨⟨_, _⟩ | _⟩, hnext⟩ := taken
         try (subst next))
      | skip
    all_goals
      simp_all [Invariants, SlotInvariants, Lifecycle.isLive, counted, liveCount]
      try omega
  | staleCallback id generation =>
    cases id <;>
    cases leftLife : table.left.lifecycle <;>
    cases rightLife : table.right.lifecycle <;>
    (try (cases succeeded)) <;>
    (simp only [none_or_some_eq_some, Option.some.injEq,
      slot, leftLife, rightLife, 
      step] at taken ⊢
     ) <;>
    first
      | (obtain ⟨_, hnext⟩ := taken
         try (subst next))
      | (obtain ⟨⟨_, _⟩, hnext⟩ := taken
         try (subst next))
      | (obtain ⟨_, ⟨_, hnext⟩⟩ := taken
         try (subst next))
      | (obtain ⟨⟨⟨_, _⟩, _⟩, hnext⟩ := taken
         try (subst next))
      | (obtain ⟨_, ⟨⟨_, _⟩, hnext⟩⟩ := taken
         try (subst next))
      | (obtain ⟨⟨_, hnext⟩ | ⟨_, hnext⟩⟩ := taken
         try (subst next))
      | (obtain ⟨⟨⟨_, _⟩ | _⟩, hnext⟩ := taken
         try (subst next))
      | skip
    all_goals
      simp_all [Invariants, SlotInvariants, Lifecycle.isLive, counted, liveCount]
      try omega
  | settle id generation =>
    cases id <;>
    cases leftLife : table.left.lifecycle <;>
    cases rightLife : table.right.lifecycle <;>
    (try (cases succeeded)) <;>
    (simp only [some_or_none_eq_some, 
      slot, leftLife, rightLife, 
      step] at taken ⊢
     ) <;>
    first
      | (obtain ⟨_, hnext⟩ := taken
         try (subst next))
      | (obtain ⟨⟨_, _⟩, hnext⟩ := taken
         try (subst next))
      | (obtain ⟨_, ⟨_, hnext⟩⟩ := taken
         try (subst next))
      | (obtain ⟨⟨⟨_, _⟩, _⟩, hnext⟩ := taken
         try (subst next))
      | (obtain ⟨_, ⟨⟨_, _⟩, hnext⟩⟩ := taken
         try (subst next))
      | (obtain ⟨⟨_, hnext⟩ | ⟨_, hnext⟩⟩ := taken
         try (subst next))
      | (obtain ⟨⟨⟨_, _⟩ | _⟩, hnext⟩ := taken
         try (subst next))
      | skip
    all_goals
      simp_all [Invariants, SlotInvariants, Lifecycle.isLive, counted, liveCount]
      try omega
  | cleanup id =>
    cases id <;>
    cases leftLife : table.left.lifecycle <;>
    cases rightLife : table.right.lifecycle <;>
    (try (cases succeeded)) <;>
    (simp only [Option.some.injEq,
      slot, leftLife, rightLife, withSlot, 
      step] at taken ⊢
     ) <;>
    first
      | (obtain ⟨_, hnext⟩ := taken
         try (subst next))
      | (obtain ⟨⟨_, _⟩, hnext⟩ := taken
         try (subst next))
      | (obtain ⟨_, ⟨_, hnext⟩⟩ := taken
         try (subst next))
      | (obtain ⟨⟨⟨_, _⟩, _⟩, hnext⟩ := taken
         try (subst next))
      | (obtain ⟨_, ⟨⟨_, _⟩, hnext⟩⟩ := taken
         try (subst next))
      | (obtain ⟨⟨_, hnext⟩ | ⟨_, hnext⟩⟩ := taken
         try (subst next))
      | (obtain ⟨⟨⟨_, _⟩ | _⟩, hnext⟩ := taken
         try (subst next))
      | skip
    all_goals
      simp_all [Invariants, SlotInvariants, Lifecycle.isLive, counted, liveCount]
      try omega
  | finish id generation =>
    cases id <;>
    cases leftLife : table.left.lifecycle <;>
    cases rightLife : table.right.lifecycle <;>
    (try (cases succeeded)) <;>
    (simp only [some_or_none_eq_some, 
      slot, leftLife, rightLife, withSlot, retiredSlot, 
      step] at taken ⊢
     ) <;>
    first
      | (obtain ⟨_, hnext⟩ := taken
         try (subst next))
      | (obtain ⟨⟨_, _⟩, hnext⟩ := taken
         try (subst next))
      | (obtain ⟨_, ⟨_, hnext⟩⟩ := taken
         try (subst next))
      | (obtain ⟨⟨⟨_, _⟩, _⟩, hnext⟩ := taken
         try (subst next))
      | (obtain ⟨_, ⟨⟨_, _⟩, hnext⟩⟩ := taken
         try (subst next))
      | (obtain ⟨⟨_, hnext⟩ | ⟨_, hnext⟩⟩ := taken
         try (subst next))
      | (obtain ⟨⟨⟨_, _⟩ | _⟩, hnext⟩ := taken
         try (subst next))
      | skip
    all_goals
      simp_all [Invariants, SlotInvariants, Lifecycle.isLive, counted, liveCount]
      try omega

/-- The total reachable realisation of an action list, refusals included as `none`. -/
def reaches : List Action → Table → Option Table
  | [], table => some table
  | action :: rest, table => (step action table).bind (reaches rest)

/-- Every reachable prefix satisfies the slot invariants: the full-search baseline never records a
    prefix it cannot justify. -/
theorem reachable_prefixes_satisfy_the_invariants (actions : List Action) (start last : Table)
    (initial : Invariants start) (reached : reaches actions start = some last) : Invariants last := by
  induction actions generalizing start with
  | nil =>
      simp only [reaches, Option.some.injEq] at reached
      subst reached
      exact initial
  | cons action rest tail =>
      simp only [reaches] at reached
      cases taken : step action start with
      | none => simp [taken] at reached
      | some next =>
          simp only [taken, Option.bind_some] at reached
          exact tail next (step_preserves_invariants action start next initial taken) reached

/-- A recorded abnormal failure is never overwritten by a later refusal or by a later admission:
    the recorded code survives every action this bounded model grants. Concrete reachable witnesses,
    because the general statement needs the same guard plumbing as the invariant theorem. -/
theorem abnormal_failure_survives_a_second_failure :
    (step (.complete .left 1 false) { baseline with failure := some 7 }).map
      (fun table => table.failure) = some (some 7) := by
  decide

theorem abnormal_failure_survives_a_later_success :
    (step (.complete .left 1 true) { baseline with failure := some 7 }).map
      (fun table => table.failure) = some (some 7) := by
  decide

theorem abnormal_failure_survives_refused_and_acquired_actions :
    (step (.deliver .left 1) { baseline with failure := some 7 }).map
      (fun table => table.failure) = some (some 7) ∧
    ((step (.finish .left 1) { baseline with failure := some 7 }).bind
      (step (.acquire .right 4))).map (fun table => table.failure) = some (some 7) := by
  decide

/-- The first abnormal failure is what the table records: a second, later failure keeps the first
    code rather than replacing it with the new generation. -/
theorem first_failure_code_is_kept_as_the_sticky_one :
    ((step (.complete .left 1 false) { baseline with failure := some 7 }).bind
      (fun table => step (.complete .left 1 false) table)).map
        (fun table => table.failure) = some (some 7) := by
  decide

/-- The documented observation of a prefix: per-id generations, pins and reservations, the sticky
    failure, and the ordered delivery and settlement records. It keeps the fields that
    `lossyObserve` deliberately drops. -/
structure Observation where
  leftGeneration : Option Nat
  rightGeneration : Option Nat
  leftPinned : Bool
  rightPinned : Bool
  leftReserved : Nat
  rightReserved : Nat
  failure : Option Nat
  deliveries : List (TaskId × Nat)
  settled : List (TaskId × Nat)
deriving DecidableEq, Repr

def observe (table : Table) : Observation :=
  { leftGeneration := generationOf table.left.lifecycle
    rightGeneration := generationOf table.right.lifecycle
    leftPinned := table.left.pinHeld
    rightPinned := table.right.pinHeld
    leftReserved := table.left.reserved
    rightReserved := table.right.reserved
    failure := table.failure
    deliveries := table.deliveries
    settled := table.settled }

/-- The deliberately lossy projection: pins and reservations only, with the failure and the generation
    dropped. It exists to be measured as lossy, not to be used as an equivalence test. -/
structure LossyObservation where
  leftPinned : Bool
  rightPinned : Bool
  leftReserved : Nat
  rightReserved : Nat
deriving DecidableEq, Repr

def lossyObserve (table : Table) : LossyObservation :=
  { leftPinned := table.left.pinHeld
    rightPinned := table.right.pinHeld
    leftReserved := table.left.reserved
    rightReserved := table.right.reserved }

/-- A full table refuses the other id's admission - the shared capacity decides, not the id. -/
theorem baseline_admission_is_refused : step (.acquire .right 1) baseline = none := by
  decide

/-- Finishing the left id releases the acquired slot. -/
theorem finishing_a_slot_releases_the_capacity :
    step (.finish .left 1) baseline = some freedLeft := by
  decide

/-- After that release, the *other* id's admission is enabled. -/
theorem releasing_a_slot_enables_another_id :
    step (.acquire .right 1) freedLeft = some { freedLeft with right := liveSlot 1 } := by
  decide

/-- The deliberately unsound independence rule, refuted: two actions on different task ids do not
    commute, because the second one's enabledness depends on the first one's released slot. The
    acquire-then-finish order refuses the admission, the finish-then-acquire order takes it. -/
theorem different_ids_are_not_independent :
    (step (.acquire .right 1) baseline).bind (step (.finish .left 1)) = none ∧
    ((step (.finish .left 1) baseline).bind (step (.acquire .right 1))).isSome = true := by
  decide

/-- A stale-generation callback leaves the table untouched instead of mutating the live record. -/
theorem stale_generation_callback_changes_nothing :
    step (.staleCallback .left 2) baseline = some baseline := by
  decide

/-- Delivery after retirement is refused rather than appended. -/
theorem late_delivery_after_retirement_is_refused :
    step (.deliver .left 1) freedLeft = none := by
  decide

/-- A second delivery of the same generation is refused rather than double-counted. -/
theorem double_delivery_is_refused : step (.deliver .left 1) deliveredLeft = none := by
  decide

/-- Settlement before delivery is refused. -/
theorem settlement_requires_a_delivery : step (.settle .left 1) baseline = none := by
  decide

/-- After delivery, settlement is taken and records the ordered pair. -/
theorem settlement_after_delivery_is_taken :
    step (.settle .left 1) deliveredLeft =
      some { deliveredLeft with settled := [(.left, 1)] } := by
  decide

/-- Finishing a delivered record keeps its delivery record instead of dropping it. -/
theorem finish_keeps_its_delivery_record :
    (step (.finish .left 1) deliveredLeft).map (fun table => table.deliveries) =
      some [(.left, 1)] := by
  decide

/-- Cancellation keeps the pin held for a live record, so the pin invariant has a reachable witness. -/
theorem cancellation_keeps_the_pin :
    (step (.cancel .left 1) baseline).map (fun table => table.left.pinHeld) = some true := by
  decide

/-- The declared bound's paired baseline: both ids live at generation 1 against a capacity of two,
    so each id's own slot decides the local actions that do not touch shared capacity. -/
def paired : Table :=
  { capacity := 2, left := liveSlot 1, right := liveSlot 1, failure := none
    deliveries := [], settled := [] }

/-- The independence rule for slot-local cancellation at the declared bound: cancelling the two ids in
    either order reaches the same documented observation, so the swap preserves enabledness, the
    related state and the observation for this action. -/
theorem cancel_orders_agree_on_distinct_ids :
    Option.map observe ((step (.cancel .right 1) paired).bind (step (.cancel .left 1))) =
    Option.map observe ((step (.cancel .left 1) paired).bind (step (.cancel .right 1))) := by
  decide

/-- Retirement is slot-local too: finishing the two ids in either order reaches the same documented
    observation even though the shared capacity is released twice. -/
theorem finish_orders_agree_on_distinct_ids :
    Option.map observe ((step (.finish .right 1) paired).bind (step (.finish .left 1))) =
    Option.map observe ((step (.finish .left 1) paired).bind (step (.finish .right 1))) := by
  decide

/-- Delivery is deliberately *not* independent: the documented observation keeps one ordered
    delivery record, so delivering the two ids in either order is a different observation. -/
theorem delivery_orders_disagree_on_distinct_ids :
    Option.map observe ((step (.deliver .right 1) paired).bind (step (.deliver .left 1))) ≠
    Option.map observe ((step (.deliver .left 1) paired).bind (step (.deliver .right 1))) := by
  decide

/-- The same two delivery orders agree under the deliberately lossy projection, so an equivalence
    claim read from it would hide a dependence the documented observation retains. -/
theorem delivery_orders_agree_under_the_lossy_observation :
    Option.map lossyObserve ((step (.deliver .right 1) paired).bind (step (.deliver .left 1))) =
    Option.map lossyObserve ((step (.deliver .left 1) paired).bind (step (.deliver .right 1))) := by
  decide

/-- Admission depends on the shared slot, not on the id: with the left slot released and one shared
    unit, admitting either id succeeds and reaches a *different* documented observation, so which id
    holds the slot decides what the other can do. Composing both admissions in either order is
    `none` both ways, so an equality read from the composed result would hide that dependence. -/
theorem admission_depends_on_which_id_holds_the_shared_slot :
    Option.map observe (step (.acquire .right 1) freedLeft) ≠
    Option.map observe (step (.acquire .left 2) freedLeft) := by
  decide

/-- The composed admissions hide it: both orders end refused, so they are equal as composed results
    while the reachable prefix above is not. -/
theorem composed_admissions_agree_while_the_prefixes_differ :
    Option.map observe ((step (.acquire .right 1) freedLeft).bind (step (.acquire .left 2))) =
    Option.map observe ((step (.acquire .left 2) freedLeft).bind (step (.acquire .right 1))) := by
  decide

/-- A stale callback is observation-free: it is refused for a live generation and passes the table
    through for any other, so it commutes with every action's result at the declared bound. -/
theorem stale_callback_is_observation_free :
    step (.staleCallback .left 2) paired = some paired := by
  decide

/-- Two live generations, so a failure on one id is distinguishable from a failure on the other. -/
def pairedGenerations : Table :=
  { paired with right := liveSlot 2 }

/-- Sticky failure is shared, so the two failing completions keep the *first* recorded code and the two
    orders reach different documented observations. -/
theorem failure_orders_disagree_on_distinct_ids :
    Option.map observe ((step (.complete .right 2 false) pairedGenerations).bind
        (step (.complete .left 1 false))) ≠
    Option.map observe ((step (.complete .left 1 false) pairedGenerations).bind
        (step (.complete .right 2 false))) := by
  decide

/-- Two histories can end at the same lossy observation yet differ in the documented observation:
    the failure field is exactly what the lossy projection drops. -/
theorem lossy_observation_hides_the_failure_difference :
    ∃ (left right : Table), lossyObserve left = lossyObserve right ∧ observe left ≠ observe right := by
  refine ⟨baseline, { baseline with failure := some 1 }, by decide, by decide⟩

/-- The ordered delivery prefix is observable: the same two records in the other order are a different
    documented observation even though the lossy projection cannot see either order. -/
theorem ordered_delivery_prefixes_are_distinguished :
    ∃ (left right : Table),
      lossyObserve left = lossyObserve right ∧ observe left ≠ observe right ∧
      left.deliveries = [(.left, 1), (.right, 1)] ∧
      right.deliveries = [(.right, 1), (.left, 1)] := by
  refine ⟨{ baseline with deliveries := [(.left, 1), (.right, 1)] }
    , { baseline with deliveries := [(.right, 1), (.left, 1)] }
    , by decide, by decide, by decide, by decide⟩

/-! ### Bounded schedule explorer (issue #46's baseline, reduction and shrinking lane)

The model above names reachability and what holds at every reachable prefix. This section adds the
three things a reduction claim needs before it can be made, all at a declared bound:

* a **baseline observation set**, populated by an exhaustive enumeration of the declared alphabet up
  to a declared depth, with a completeness statement about that enumeration;
* **full/reduced agreement measured on the declared pair**: the enumeration kernel-checks that the
  independent pair's swap preserves the observation, and that the delivery pair is a counterexample
  the reduction must exclude;
* and what is *not* here: reproducible witness shrinking. A first-witness scan over prefix lengths is
  designed below as the next step and is deliberately absent until it is proved, rather than being
  approximated by a scan that returns a witness without proving it is the first one.

Nothing here proves a reduction correct for every table and action list, and the enumerations are
bounded by the declared alphabet and depth rather than exhaustive over the language.
-/

/-- The declared exploration alphabet: one canonical action per action kind at two generations so
same-slot reuse is reachable. -/
def exploreAlphabet : List Action :=
  [ .acquire .left 1,
    .acquire .right 1,
    .cancel .left 1,
    .finish .left 1,
    .staleCallback .left 1,
    .deliver .left 1,
    .complete .left 1 true,
    .settle .left 1 ]

/-- Every sequence of exactly `depth` actions over the alphabet, in the alphabet's order. -/
def sequencesOfLength (alphabet : List Action) : Nat → List (List Action)
  | 0 => [[]]
  | depth + 1 =>
      (sequencesOfLength alphabet depth).flatMap (fun lead => alphabet.map (fun action => action :: lead))

/-- Every sequence of at most `depth` actions over the alphabet. -/
def sequencesUpTo (alphabet : List Action) : Nat → List (List Action)
  | 0 => [[]]
  | depth + 1 => sequencesOfLength alphabet (depth + 1) ++ sequencesUpTo alphabet depth

/-- The baseline observations reachable from a start table over an enumerated sequence list. Refused
steps contribute no observation, so a refusal is not silently recorded as a state. -/
def baselineObservations (start : Table) (sequences : List (List Action)) : List Observation :=
  (sequences.filterMap (fun sequence => (reaches sequence start).map observe))

/-- The declared baseline: the alphabet's sequences up to depth three from the empty table. -/
def declaredBaseline : List Observation :=
  baselineObservations (emptyTable 1) (sequencesUpTo exploreAlphabet 3)

/-- The enumeration really contains every sequence of that length over the alphabet. -/
theorem mem_sequencesOfLength (alphabet : List Action) (depth : Nat) (sequence : List Action)
    (hLength : sequence.length = depth) (hAll : ∀ action ∈ sequence, action ∈ alphabet) :
    sequence ∈ sequencesOfLength alphabet depth := by
  induction depth generalizing sequence with
  | zero =>
      simp only [List.length_eq_zero_iff] at hLength
      subst hLength
      simp only [sequencesOfLength, List.mem_singleton]
  | succ depth ih =>
      match sequence with
      | [] => exact absurd hLength (by simp)
      | head :: tail =>
          have hTailLength : tail.length = depth := by simp only [List.length_cons] at hLength; omega
          have hHead : head ∈ alphabet := hAll head (by simp)
          have hTail : ∀ action ∈ tail, action ∈ alphabet := by
            intro action hMember
            exact hAll action (List.mem_cons_of_mem head hMember)
          have hMem : tail ∈ sequencesOfLength alphabet depth := ih tail hTailLength hTail
          simp only [sequencesOfLength, List.mem_flatMap, List.mem_map]
          exact ⟨tail, hMem, head, hHead, rfl⟩

/-- A deeper enumeration contains every shorter one. -/
theorem sequencesUpTo_contains_shorter (alphabet : List Action) {shorter depth : Nat}
    (hBound : shorter ≤ depth) : sequencesOfLength alphabet shorter ⊆ sequencesUpTo alphabet depth := by
  induction depth with
  | zero =>
      have hZero : shorter = 0 := by omega
      subst hZero
      intro sequence hMember
      simpa [sequencesUpTo, sequencesOfLength] using hMember
  | succ depth ih =>
      by_cases hLast : shorter = depth + 1
      · subst hLast
        intro sequence hMember
        rw [sequencesUpTo]
        exact List.mem_append_left _ hMember
      · have hInner : shorter ≤ depth := by omega
        intro sequence hMember
        rw [sequencesUpTo]
        exact List.mem_append_right _ (ih hInner hMember)

/-- **The baseline is complete for the declared bound**: every sequence over the declared alphabet
whose length is at most the declared depth appears in the enumeration the baseline is populated
from, so a reduction may not claim agreement against a smaller search than this one. -/
theorem the_baseline_enumeration_is_complete (depth : Nat) (sequence : List Action)
    (hBound : sequence.length ≤ depth) (hAll : ∀ action ∈ sequence, action ∈ exploreAlphabet) :
    sequence ∈ sequencesUpTo exploreAlphabet depth := by
  exact sequencesUpTo_contains_shorter exploreAlphabet hBound
    (mem_sequencesOfLength exploreAlphabet sequence.length sequence rfl hAll)

/-- **Full/reduced agreement, measured**: the independent pair's swap preserves the documented
observation in both declared orders, so the reduction may swap it on this table. -/
theorem the_independent_pairs_swap_preserves_the_observation :
    Option.map observe ((step (.finish .right 1) paired).bind (step (.cancel .left 1)))
        = Option.map observe ((step (.cancel .left 1) paired).bind (step (.finish .right 1)))
      ∧ Option.map observe ((step (.finish .left 1) paired).bind (step (.cancel .right 1)))
        = Option.map observe ((step (.cancel .right 1) paired).bind (step (.finish .left 1))) := by
  decide

/-- The delivery pair is the counterexample the reduction must **exclude**: swapping the two delivery
orders changes the documented observation, so a reduction that kept it would be unsound. -/
theorem the_reduction_must_exclude_the_delivery_pair :
    Option.map observe ((step (.deliver .right 1) paired).bind (step (.deliver .left 1)))
      ≠ Option.map observe ((step (.deliver .left 1) paired).bind (step (.deliver .right 1))) := by
  decide

end NobleFoundation.TaskInterleaving
