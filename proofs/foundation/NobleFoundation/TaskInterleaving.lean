/-!
  Bounded M6 task-interleaving model (issue #46).

  Two live task records at most, one shared admission capacity, a lifetime wake budget with one
  queued wakeup per record, a retiring mark, a sticky first abnormal failure, ordered delivery and
  settlement records. The model is an *explorer subject*: it names which interleavings are reachable
  and which slot invariants hold at every reachable prefix, so a full-search baseline exists before
  any reduction is claimed.

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

/-- Per-id state: lifecycle, pin, queued wakeup units, the remaining lifetime wake budget, and the
    retiring mark that cancellation and wake-budget exhaustion set. The last two fields default to an
    exhausted budget and an unmarked record, so an absent or retired record carries neither. -/
structure Slot where
  lifecycle : Lifecycle
  pinHeld : Bool
  reserved : Nat
  wakeups : Nat := 0
  retiring : Bool := false
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

/-- The lifetime wake budget an acquired record starts with: one wakeup, taken and then requested
    again, reaches exhaustion within the declared bound. -/
def wakeBudget : Nat := 1

/-- An acquired record, named so the counterexamples stay readable. -/
def liveSlot (generation : Nat) : Slot :=
  { lifecycle := .live generation, pinHeld := false, reserved := 0, wakeups := wakeBudget }

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
  | wake (id : TaskId) (generation : Nat)
  | takeWake (id : TaskId) (generation : Nat)
deriving DecidableEq, Repr

/-- The sticky failure rule, named so the acquired-step value stays a plain term rather than a
    `match`, which keeps the preservation proof's substitution syntactic. -/
def stickFailure (operand : Option Nat) (generation : Nat) : Option Nat :=
  match operand with
  | none => some generation
  | held => held

/-- One acquired step. Every refusal is a single guard, an action that names a generation must name
    the live one, and same-slot reuse needs a different generation.

    Cancellation keeps the pin, releases a queued wakeup and marks the record retiring, so a later
    delivery is refused. A wake on a record that is neither retiring nor delivered queues one wakeup
    from the budget, coalesces into an already queued one, or, with the budget exhausted, records the
    sticky failure and marks the record retiring. `takeWake` removes the queued wakeup. -/
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
              reserved := if 0 < current.reserved then current.reserved - 1 else 0
              retiring := true })
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
          if liveGeneration = generation ∧ (slot table id).retiring = false ∧
              ¬(id, generation) ∈ table.deliveries then
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
  | .wake id generation, table =>
      match (slot table id).lifecycle with
      | .live liveGeneration =>
          let current := slot table id
          if liveGeneration = generation ∧ current.retiring = false ∧
              ¬(id, generation) ∈ table.deliveries then
            if 0 < current.reserved then some table
            else if 0 < current.wakeups then
              some (withSlot table id { current with
                reserved := 1
                wakeups := current.wakeups - 1 })
            else
              some { withSlot table id { current with retiring := true } with
                failure := stickFailure table.failure generation }
          else none
      | _ => none
  | .takeWake id generation, table =>
      match (slot table id).lifecycle with
      | .live liveGeneration =>
          let current := slot table id
          if liveGeneration = generation ∧ current.retiring = false ∧
              ¬(id, generation) ∈ table.deliveries ∧ 0 < current.reserved then
            some (withSlot table id { current with reserved := current.reserved - 1 })
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
  | wake id generation =>
    cases id <;>
    cases leftLife : table.left.lifecycle <;>
    cases rightLife : table.right.lifecycle <;>
    simp only [slot, leftLife, rightLife, withSlot, step] at taken <;>
    (repeat' split at taken) <;>
    simp only [Option.some.injEq, reduceCtorEq] at taken <;>
    (try subst next) <;>
    simp_all [Invariants, SlotInvariants, Lifecycle.isLive, counted, liveCount]
  | takeWake id generation =>
    cases id <;>
    cases leftLife : table.left.lifecycle <;>
    cases rightLife : table.right.lifecycle <;>
    simp only [slot, leftLife, rightLife, withSlot, step] at taken <;>
    (repeat' split at taken) <;>
    simp only [Option.some.injEq, reduceCtorEq] at taken <;>
    (try subst next) <;>
    simp_all [Invariants, SlotInvariants, Lifecycle.isLive, counted, liveCount]

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

/-- The documented observation of a prefix: per-id generations, pins, queued wakeups, remaining wake
    budgets and retiring marks, the sticky failure, and the ordered delivery and settlement records.
    It keeps the fields that `lossyObserve` deliberately drops. -/
structure Observation where
  leftGeneration : Option Nat
  rightGeneration : Option Nat
  leftPinned : Bool
  rightPinned : Bool
  leftReserved : Nat
  rightReserved : Nat
  leftWakeups : Nat
  rightWakeups : Nat
  leftRetiring : Bool
  rightRetiring : Bool
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
    leftWakeups := table.left.wakeups
    rightWakeups := table.right.wakeups
    leftRetiring := table.left.retiring
    rightRetiring := table.right.retiring
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

The model above names reachability and what holds at every reachable prefix. This section and the
next add what a reduction claim needs before it can be made:

* a **baseline observation set**, populated by an exhaustive enumeration of the declared alphabet up
  to a declared depth, with a completeness statement about that enumeration;
* **full/reduced agreement measured on the declared pair**: the enumeration kernel-checks that the
  independent pair's swap preserves the observation, and that the delivery pair is a counterexample
  the reduction must exclude;
* **reproducible witness shrinking**: a scan over prefix lengths whose first reported disagreement
  is proved to be the shortest;
* and, in the next section, a **reduced search** that is enabled by a proof rather than by finding
  fewer traces: an independence relation proved to commute on every table, a canonical
  representative for every sequence, and agreement of the full and reduced observation sets at every
  bound.

The enumerations are bounded by the declared alphabet and depth rather than exhaustive over the
language.
-/

/-- The declared exploration alphabet: every action kind on the left id, the left id's admission at a
second generation so same-slot reuse and a stale callback are reachable, both completion outcomes,
and admission, delivery and finish on the right id so the shared slot is contended. -/
def exploreAlphabet : List Action :=
  [ .acquire .left 1,
    .acquire .left 2,
    .acquire .right 1,
    .cancel .left 1,
    .complete .left 1 true,
    .complete .left 1 false,
    .deliver .left 1,
    .staleCallback .left 1,
    .settle .left 1,
    .wake .left 1,
    .takeWake .left 1,
    .cleanup .left,
    .finish .left 1,
    .deliver .right 1,
    .finish .right 1 ]

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

/-- The declared starts. `baseline` holds one live record against one shared unit, so admission is
contended and same-slot reuse is reachable; `paired` holds both records live against two units, so
distinct-slot histories run concurrently. Both are empty tables after admissions
(`the_declared_starts_are_reachable`), so every declared history is a reachable history of an empty
table. -/
def declaredStarts : List Table := [baseline, paired]

/-- The declared baseline: the alphabet's sequences up to depth three from each declared start. -/
def declaredBaseline : List Observation :=
  declaredStarts.flatMap (fun start => baselineObservations start (sequencesUpTo exploreAlphabet 3))

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

/-- A sequence disagrees with a reference table when its reachable observation differs, or when it is
refused where the reference is reachable. -/
def disagrees (start reference : Table) (sequence : List Action) : Bool :=
  match reaches sequence start with
  | none => true
  | some reached => decide (observe reached ≠ observe reference)

/-- Examine prefix lengths upward from `offset`: the *first* disagreeing length found is therefore the
shortest one, which is what makes a shrunk witness reproducible from the sequence rather than
hand-picked. -/
def scanFrom (start reference : Table) (sequence : List Action) : Nat → Nat → Option Nat
  | 0, _offset => none
  | fuel + 1, offset =>
      if disagrees start reference (sequence.take offset) then some offset
      else scanFrom start reference sequence fuel (offset + 1)

/-- When the scan reports no disagreement, every prefix in its window agrees. -/
theorem scanFrom_none_all_agree (start reference : Table) (sequence : List Action) :
    ∀ fuel offset, (∀ shorter, shorter < offset →
        disagrees start reference (sequence.take shorter) = false) →
      scanFrom start reference sequence fuel offset = none →
      ∀ length, offset ≤ length → length < offset + fuel →
        disagrees start reference (sequence.take length) = false := by
  intro fuel
  induction fuel with
  | zero =>
      intro offset _ _ length hLow hHigh
      omega
  | succ fuel ih =>
      intro offset hAgreed hScan length hLow hHigh
      by_cases hHere : disagrees start reference (sequence.take offset) = true
      · simp only [scanFrom] at hScan
        rw [if_pos hHere] at hScan
        simp at hScan
      · have hHereFalse : disagrees start reference (sequence.take offset) = false := by
          cases hValue : disagrees start reference (sequence.take offset) <;> simp_all
        simp only [scanFrom] at hScan
        rw [if_neg hHere] at hScan
        have hNext : ∀ shorter, shorter < offset + 1 →
            disagrees start reference (sequence.take shorter) = false := by
          intro shorter hShorter
          by_cases hEq : shorter = offset
          · subst hEq
            exact hHereFalse
          · exact hAgreed shorter (by omega)
        by_cases hEq : length = offset
        · subst hEq
          exact hHereFalse
        · exact ih (offset + 1) hNext hScan length (by omega) (by omega)

/-- When the scan reports a disagreement, it is the first one in its window and every shorter prefix
agrees. -/
theorem scanFrom_some_finds_the_first (start reference : Table) (sequence : List Action) :
    ∀ fuel offset, (∀ shorter, shorter < offset →
        disagrees start reference (sequence.take shorter) = false) →
      ∀ length, scanFrom start reference sequence fuel offset = some length →
      offset ≤ length
        ∧ length < offset + fuel
        ∧ disagrees start reference (sequence.take length) = true
        ∧ ∀ shorter, shorter < length →
            disagrees start reference (sequence.take shorter) = false := by
  intro fuel
  induction fuel with
  | zero =>
      intro offset _ length hScan
      simp only [scanFrom] at hScan
      cases hScan
  | succ fuel ih =>
      intro offset hAgreed length hScan
      by_cases hHere : disagrees start reference (sequence.take offset) = true
      · simp only [scanFrom] at hScan
        rw [if_pos hHere] at hScan
        simp only [Option.some.injEq] at hScan
        subst hScan
        exact ⟨Nat.le_refl offset, by omega, hHere, hAgreed⟩
      · have hHereFalse : disagrees start reference (sequence.take offset) = false := by
          cases hValue : disagrees start reference (sequence.take offset) <;> simp_all
        simp only [scanFrom] at hScan
        rw [if_neg hHere] at hScan
        have hNext : ∀ shorter, shorter < offset + 1 →
            disagrees start reference (sequence.take shorter) = false := by
          intro shorter hShorter
          by_cases hEq : shorter = offset
          · subst hEq
            exact hHereFalse
          · exact hAgreed shorter (by omega)
        have hWindow := ih (offset + 1) hNext length hScan
        exact ⟨by omega, by omega, hWindow.2.2.1, hWindow.2.2.2⟩

/-- **Reproducible witness shrinking**: a disagreeing sequence has a shortest prefix witness found by
the scan, so a shrunk witness is replayed from the sequence's own prefix and every shorter prefix is
recorded as agreeing. -/
theorem every_disagreement_has_a_shortest_witness (start reference : Table) (sequence : List Action)
    (h : disagrees start reference sequence = true) :
    ∃ length, scanFrom start reference sequence (sequence.length + 1) 0 = some length
      ∧ length ≤ sequence.length
      ∧ disagrees start reference (sequence.take length) = true
      ∧ ∀ shorter, shorter < length → disagrees start reference (sequence.take shorter) = false := by
  have hVacuous : ∀ shorter, shorter < 0 →
      disagrees start reference (sequence.take shorter) = false := by omega
  cases hScan : scanFrom start reference sequence (sequence.length + 1) 0 with
  | none =>
      have hFull : disagrees start reference (sequence.take sequence.length) = false :=
        scanFrom_none_all_agree start reference sequence (sequence.length + 1) 0 hVacuous hScan
          sequence.length (by omega) (by omega)
      rw [List.take_length] at hFull
      rw [hFull] at h
      cases h
  | some length =>
      have hWindow := scanFrom_some_finds_the_first start reference sequence (sequence.length + 1) 0
        hVacuous length hScan
      exact ⟨length, rfl, by omega, hWindow.2.2.1, hWindow.2.2.2⟩

/-! ### Proved reduction (issue #46's reduced search)

The reduction is enabled by a proof, not by finding fewer traces. `independent` names which pairs
of actions may be reordered; `independent_actions_commute` proves that every such pair reaches the
same result in either order on *every* table, refusals included. A sequence is `Canonical` when no
adjacent related pair has the right id first. `canonicalize` reaches a canonical representative by
adjacent swaps of related pairs only, so the representative is a permutation of the sequence and
reaches exactly what the sequence reaches. The reduced search `reducedUpTo` enumerates only
canonical sequences, pruning while it extends rather than filtering a full enumeration, and
`reduced_search_agrees` proves its observation set equal to the full search's for every sound
relation, start table, alphabet and depth.

The deliberately unsound relation `differentIds` is the control: it is refuted as a commuting
relation (`different_ids_rule_is_unsound`), and the reduced search it would enable is measured
losing an observation that the full search reaches.

The independence relation is sound but not complete, and canonical forms are not unique: a
reduced search may still contain several representatives of one class. No optimality is claimed.
-/

/-- The task id an action names. -/
def Action.id : Action → TaskId
  | .acquire id _ => id
  | .cancel id _ => id
  | .complete id _ _ => id
  | .deliver id _ => id
  | .staleCallback id _ => id
  | .settle id _ => id
  | .cleanup id => id
  | .finish id _ => id
  | .wake id _ => id
  | .takeWake id _ => id

/-- Admission reads the shared capacity through `liveCount`. -/
def Action.readsCapacity : Action → Bool
  | .acquire _ _ => true
  | _ => false

/-- Admission, cleanup and finish change which records are live, and so move `liveCount`. -/
def Action.movesCapacity : Action → Bool
  | .acquire _ _ => true
  | .cleanup _ => true
  | .finish _ _ => true
  | _ => false

/-- A failing completion and a wake, through budget exhaustion, may write the shared sticky failure. -/
def Action.writesFailure : Action → Bool
  | .complete _ _ succeeded => !succeeded
  | .wake _ _ => true
  | _ => false

/-- Delivery appends to the ordered delivery record. -/
def Action.isDelivery : Action → Bool
  | .deliver _ _ => true
  | _ => false

/-- Settlement appends to the ordered settlement record. -/
def Action.isSettlement : Action → Bool
  | .settle _ _ => true
  | _ => false

/-- The independence relation of the reduction: different ids, and no conflict on shared state —
neither reads the capacity the other moves, they do not both write the sticky failure, and they do
not both append to one ordered record. Every pair on one id stays dependent, and so do the
cancellation/delivery races, generation reuse and shared-capacity conflicts, which all act on one id
or on the shared slot. -/
def independent (first second : Action) : Bool :=
  first.id != second.id
    && !(first.readsCapacity && second.movesCapacity)
    && !(first.movesCapacity && second.readsCapacity)
    && !(first.writesFailure && second.writesFailure)
    && !(first.isDelivery && second.isDelivery)
    && !(first.isSettlement && second.isSettlement)

/-- Binding through a guarded result binds each branch. -/
theorem bind_ite {c : Prop} [Decidable c] (first second : Option Table)
    (next : Table → Option Table) :
    (if c then first else second).bind next = if c then first.bind next else second.bind next := by
  by_cases held : c <;> simp [held]

/-- The relation is symmetric, so a proof for one order of ids covers the other. -/
theorem independent_symm (first second : Action) :
    independent first second = independent second first := by
  cases first <;> cases second <;>
    simp [independent, Action.id, Action.readsCapacity, Action.movesCapacity,
      Action.writesFailure, Action.isDelivery, Action.isSettlement, bne_comm, Bool.and_comm]

set_option maxHeartbeats 8000000 in
/-- Every independent pair with the left action first commutes on every table: either order reaches
the same table, or both orders are refused. -/
theorem left_then_right_commute (first second : Action) (table : Table)
    (hFirst : first.id = .left) (hSecond : second.id = .right)
    (h : independent first second = true) :
    (step first table).bind (step second) = (step second table).bind (step first) := by
  obtain ⟨capacity, ⟨leftLife, leftPin, leftReserved, leftWakeups, leftRetiring⟩,
    ⟨rightLife, rightPin, rightReserved, rightWakeups, rightRetiring⟩,
    failure, deliveries, settled⟩ := table
  cases first <;> cases second <;>
    (try simp only [Action.id] at hFirst hSecond) <;>
    subst hFirst hSecond <;>
    simp [independent, Action.id, Action.readsCapacity, Action.movesCapacity, Action.writesFailure,
      Action.isDelivery, Action.isSettlement] at h <;>
    (try (rcases h with h | h <;> subst h)) <;>
    cases leftLife <;> cases rightLife <;>
    simp only [step, slot, withSlot, bind_ite, Option.bind_some, Option.bind_none, liveCount, counted,
      liveSlot, retiredSlot] <;>
    (repeat' split) <;> simp_all

/-- **Independent actions commute on every table**: swapping an independent pair preserves
enabledness and reaches the same table, so it preserves every observation and every prefix
invariant of the swapped history's continuation. -/
theorem independent_actions_commute (first second : Action) (table : Table)
    (h : independent first second = true) :
    (step first table).bind (step second) = (step second table).bind (step first) := by
  cases hFirst : first.id <;> cases hSecond : second.id
  · simp [independent, hFirst, hSecond] at h
  · exact left_then_right_commute first second table hFirst hSecond h
  · rw [independent_symm] at h
    exact (left_then_right_commute second first table hSecond hFirst h).symm
  · simp [independent, hFirst, hSecond] at h

/-- A relation the reduction may use: every related pair commutes on every table. -/
def Sound (related : Action → Action → Bool) : Prop :=
  ∀ first second table, related first second = true →
    (step first table).bind (step second) = (step second table).bind (step first)

theorem independent_is_sound : Sound independent :=
  fun first second table h => independent_actions_commute first second table h

/-- The order the reduction prunes: a related pair with the right id first. Of the two orders of a
related pair on the two ids, only the left-first one is kept. -/
def outOfOrder (related : Action → Action → Bool) (first second : Action) : Bool :=
  related first second && first.id == .right && second.id == .left

/-- Whether an action may lead a sequence without forming a pruned pair with its head. -/
def extendable (related : Action → Action → Bool) (action : Action) : List Action → Bool
  | [] => true
  | head :: _ => !outOfOrder related action head

/-- A sequence is canonical when no adjacent pair is in the pruned order. -/
def Canonical (related : Action → Action → Bool) : List Action → Bool
  | [] => true
  | action :: rest => extendable related action rest && Canonical related rest

/-- Move an action past the leading actions it is out of order with, by adjacent swaps only. -/
def insertCanonical (related : Action → Action → Bool) (action : Action) :
    List Action → List Action
  | [] => [action]
  | head :: rest =>
      if outOfOrder related action head then head :: insertCanonical related action rest
      else action :: head :: rest

/-- The canonical representative of a sequence. -/
def canonicalize (related : Action → Action → Bool) : List Action → List Action
  | [] => []
  | action :: rest => insertCanonical related action (canonicalize related rest)

/-- A pruned pair is never pruned in the other order. -/
theorem outOfOrder_asymm (related : Action → Action → Bool) {first second : Action}
    (h : outOfOrder related first second = true) : outOfOrder related second first = false := by
  simp only [outOfOrder, Bool.and_eq_true, beq_iff_eq] at h
  simp [outOfOrder, h.2]

/-- Inserting a related action reaches what the unmoved sequence reaches. -/
theorem reaches_insertCanonical (related : Action → Action → Bool) (hSound : Sound related)
    (action : Action) (rest : List Action) (start : Table) :
    reaches (insertCanonical related action rest) start = reaches (action :: rest) start := by
  induction rest generalizing start with
  | nil => rfl
  | cons head tail ih =>
      simp only [insertCanonical]
      split
      · rename_i hOut
        have hRelated : related action head = true := by
          simp only [outOfOrder, Bool.and_eq_true] at hOut
          exact hOut.1.1
        have hInner : reaches (insertCanonical related action tail)
            = fun next => (step action next).bind (reaches tail) := funext ih
        show (step head start).bind (reaches (insertCanonical related action tail))
            = (step action start).bind (fun next => (step head next).bind (reaches tail))
        rw [hInner, ← Option.bind_assoc, ← Option.bind_assoc, hSound action head start hRelated]
      · rfl

/-- The canonical representative reaches exactly what the sequence reaches. -/
theorem reaches_canonicalize (related : Action → Action → Bool) (hSound : Sound related)
    (sequence : List Action) (start : Table) :
    reaches (canonicalize related sequence) start = reaches sequence start := by
  induction sequence generalizing start with
  | nil => rfl
  | cons action rest ih =>
      simp only [canonicalize]
      rw [reaches_insertCanonical related hSound]
      have hRest : reaches (canonicalize related rest) = reaches rest := funext ih
      simp only [reaches, hRest]

theorem insertCanonical_perm (related : Action → Action → Bool) (action : Action)
    (rest : List Action) : (insertCanonical related action rest).Perm (action :: rest) := by
  induction rest with
  | nil => exact List.Perm.refl _
  | cons head tail ih =>
      simp only [insertCanonical]
      split
      · exact (List.Perm.cons head ih).trans (List.Perm.swap action head tail)
      · exact List.Perm.refl _

/-- The canonical representative is a permutation of the sequence. -/
theorem canonicalize_perm (related : Action → Action → Bool) (sequence : List Action) :
    (canonicalize related sequence).Perm sequence := by
  induction sequence with
  | nil => exact List.Perm.refl _
  | cons action rest ih =>
      exact (insertCanonical_perm related action (canonicalize related rest)).trans
        (List.Perm.cons action ih)

theorem insertCanonical_shape (related : Action → Action → Bool) (action : Action)
    (rest : List Action) :
    insertCanonical related action rest = action :: rest ∨
      ∃ head tail, rest = head :: tail ∧ outOfOrder related action head = true ∧
        insertCanonical related action rest = head :: insertCanonical related action tail := by
  cases rest with
  | nil => exact Or.inl rfl
  | cons head tail =>
      by_cases hOut : outOfOrder related action head = true
      · exact Or.inr ⟨head, tail, rfl, hOut, by simp [insertCanonical, hOut]⟩
      · exact Or.inl (by simp [insertCanonical, hOut])

theorem canonical_insertCanonical (related : Action → Action → Bool) (action : Action)
    (rest : List Action) (h : Canonical related rest = true) :
    Canonical related (insertCanonical related action rest) = true := by
  induction rest with
  | nil => rfl
  | cons head tail ih =>
      simp only [Canonical, Bool.and_eq_true] at h
      by_cases hOut : outOfOrder related action head = true
      · have hInner := ih h.2
        simp only [insertCanonical, hOut, ↓reduceIte, Canonical, Bool.and_eq_true]
        refine ⟨?_, hInner⟩
        rcases insertCanonical_shape related action tail with hEq | ⟨next, rest, hTail, _, hEq⟩
        · rw [hEq]
          simp only [extendable, outOfOrder_asymm related hOut, Bool.not_false]
        · rw [hEq]
          subst hTail
          exact h.1
      · have hOutFalse : outOfOrder related action head = false := by simpa using hOut
        simp only [insertCanonical, hOutFalse, Bool.false_eq_true, ↓reduceIte, Canonical,
          Bool.and_eq_true]
        exact ⟨by simp only [extendable, hOutFalse, Bool.not_false], h.1, h.2⟩

/-- The canonical representative is canonical. -/
theorem canonicalize_canonical (related : Action → Action → Bool) (sequence : List Action) :
    Canonical related (canonicalize related sequence) = true := by
  induction sequence with
  | nil => rfl
  | cons action rest ih =>
      exact canonical_insertCanonical related action (canonicalize related rest) ih

/-- The reduced enumeration: extend only by an action that keeps the sequence canonical, so a
pruned pair is never generated rather than generated and then filtered. -/
def reducedOfLength (related : Action → Action → Bool) (alphabet : List Action) :
    Nat → List (List Action)
  | 0 => [[]]
  | depth + 1 =>
      (reducedOfLength related alphabet depth).flatMap (fun lead =>
        (alphabet.filter (fun action => extendable related action lead)).map
          (fun action => action :: lead))

/-- Every reduced sequence of at most `depth` actions. -/
def reducedUpTo (related : Action → Action → Bool) (alphabet : List Action) :
    Nat → List (List Action)
  | 0 => [[]]
  | depth + 1 => reducedOfLength related alphabet (depth + 1) ++ reducedUpTo related alphabet depth

/-- Exactly the sequences of the declared length over the alphabet. -/
theorem mem_sequencesOfLength_iff (alphabet : List Action) (depth : Nat) (sequence : List Action) :
    sequence ∈ sequencesOfLength alphabet depth ↔
      sequence.length = depth ∧ ∀ action ∈ sequence, action ∈ alphabet := by
  induction depth generalizing sequence with
  | zero =>
      simp only [sequencesOfLength, List.mem_singleton, List.length_eq_zero_iff]
      constructor
      · intro h
        subst h
        simp
      · exact fun h => h.1
  | succ depth ih =>
      simp only [sequencesOfLength, List.mem_flatMap, List.mem_map]
      constructor
      · rintro ⟨lead, hLead, action, hAction, rfl⟩
        obtain ⟨hLength, hAll⟩ := (ih lead).1 hLead
        refine ⟨by simp [hLength], ?_⟩
        intro member hMember
        simp only [List.mem_cons] at hMember
        rcases hMember with rfl | hMember
        · exact hAction
        · exact hAll member hMember
      · rintro ⟨hLength, hAll⟩
        match sequence, hLength with
        | action :: lead, hLength =>
            refine ⟨lead, (ih lead).2 ⟨by simpa using hLength, ?_⟩, action, hAll action (by simp), rfl⟩
            intro member hMember
            exact hAll member (List.mem_cons_of_mem action hMember)

/-- Exactly the sequences of at most the declared depth over the alphabet. -/
theorem mem_sequencesUpTo (alphabet : List Action) (depth : Nat) (sequence : List Action) :
    sequence ∈ sequencesUpTo alphabet depth ↔
      sequence.length ≤ depth ∧ ∀ action ∈ sequence, action ∈ alphabet := by
  induction depth with
  | zero =>
      simp only [sequencesUpTo, List.mem_singleton, Nat.le_zero, List.length_eq_zero_iff]
      constructor
      · intro h
        subst h
        simp
      · exact fun h => h.1
  | succ depth ih =>
      simp only [sequencesUpTo, List.mem_append, mem_sequencesOfLength_iff, ih]
      constructor
      · rintro (⟨hLength, hAll⟩ | ⟨hLength, hAll⟩)
        · exact ⟨by omega, hAll⟩
        · exact ⟨by omega, hAll⟩
      · rintro ⟨hLength, hAll⟩
        by_cases hTop : sequence.length = depth + 1
        · exact Or.inl ⟨hTop, hAll⟩
        · exact Or.inr ⟨by omega, hAll⟩

/-- Exactly the canonical sequences of the declared length over the alphabet. -/
theorem mem_reducedOfLength (related : Action → Action → Bool) (alphabet : List Action)
    (depth : Nat) (sequence : List Action) :
    sequence ∈ reducedOfLength related alphabet depth ↔
      sequence.length = depth ∧ (∀ action ∈ sequence, action ∈ alphabet) ∧
        Canonical related sequence = true := by
  induction depth generalizing sequence with
  | zero =>
      simp only [reducedOfLength, List.mem_singleton, List.length_eq_zero_iff]
      constructor
      · intro h
        subst h
        simp [Canonical]
      · exact fun h => h.1
  | succ depth ih =>
      simp only [reducedOfLength, List.mem_flatMap, List.mem_map, List.mem_filter]
      constructor
      · rintro ⟨lead, hLead, action, ⟨hAction, hExtend⟩, rfl⟩
        obtain ⟨hLength, hAll, hCanonical⟩ := (ih lead).1 hLead
        refine ⟨by simp [hLength], ?_, by simp [Canonical, hExtend, hCanonical]⟩
        intro member hMember
        simp only [List.mem_cons] at hMember
        rcases hMember with rfl | hMember
        · exact hAction
        · exact hAll member hMember
      · rintro ⟨hLength, hAll, hCanonical⟩
        match sequence, hLength with
        | action :: lead, hLength =>
            simp only [Canonical, Bool.and_eq_true] at hCanonical
            refine ⟨lead, (ih lead).2 ⟨by simpa using hLength, ?_, hCanonical.2⟩, action,
              ⟨hAll action (by simp), hCanonical.1⟩, rfl⟩
            intro member hMember
            exact hAll member (List.mem_cons_of_mem action hMember)

/-- Exactly the canonical sequences of at most the declared depth over the alphabet. -/
theorem mem_reducedUpTo (related : Action → Action → Bool) (alphabet : List Action)
    (depth : Nat) (sequence : List Action) :
    sequence ∈ reducedUpTo related alphabet depth ↔
      sequence.length ≤ depth ∧ (∀ action ∈ sequence, action ∈ alphabet) ∧
        Canonical related sequence = true := by
  induction depth with
  | zero =>
      simp only [reducedUpTo, List.mem_singleton, Nat.le_zero, List.length_eq_zero_iff]
      constructor
      · intro h
        subst h
        simp [Canonical]
      · exact fun h => h.1
  | succ depth ih =>
      simp only [reducedUpTo, List.mem_append, mem_reducedOfLength, ih]
      constructor
      · rintro (⟨hLength, hAll, hCanonical⟩ | ⟨hLength, hAll, hCanonical⟩)
        · exact ⟨by omega, hAll, hCanonical⟩
        · exact ⟨by omega, hAll, hCanonical⟩
      · rintro ⟨hLength, hAll, hCanonical⟩
        by_cases hTop : sequence.length = depth + 1
        · exact Or.inl ⟨hTop, hAll, hCanonical⟩
        · exact Or.inr ⟨by omega, hAll, hCanonical⟩

/-- **Every full-search history has a replayable reduced representative**: its canonical form is in
the reduced search, is a permutation of it reached by adjacent swaps of related pairs, and reaches
exactly what it reaches from every start table. -/
theorem every_sequence_has_a_reduced_representative (related : Action → Action → Bool)
    (hSound : Sound related) (alphabet : List Action) (depth : Nat) (sequence : List Action)
    (h : sequence ∈ sequencesUpTo alphabet depth) :
    canonicalize related sequence ∈ reducedUpTo related alphabet depth
      ∧ (canonicalize related sequence).Perm sequence
      ∧ ∀ start, reaches (canonicalize related sequence) start = reaches sequence start := by
  obtain ⟨hLength, hAll⟩ := (mem_sequencesUpTo alphabet depth sequence).1 h
  have hPerm := canonicalize_perm related sequence
  refine ⟨(mem_reducedUpTo related alphabet depth _).2 ⟨?_, ?_, canonicalize_canonical related sequence⟩,
    hPerm, reaches_canonicalize related hSound sequence⟩
  · rw [hPerm.length_eq]
    exact hLength
  · intro action hMember
    exact hAll action (hPerm.mem_iff.1 hMember)

/-- **The reduced search agrees with the full search**: for every sound relation, start table,
alphabet and depth, the two searches reach the same set of documented observations. -/
theorem reduced_search_agrees (related : Action → Action → Bool) (hSound : Sound related)
    (start : Table) (alphabet : List Action) (depth : Nat) (observation : Observation) :
    observation ∈ baselineObservations start (sequencesUpTo alphabet depth) ↔
      observation ∈ baselineObservations start (reducedUpTo related alphabet depth) := by
  simp only [baselineObservations, List.mem_filterMap]
  constructor
  · rintro ⟨sequence, hMember, hReach⟩
    obtain ⟨hReduced, _, hSame⟩ :=
      every_sequence_has_a_reduced_representative related hSound alphabet depth sequence hMember
    exact ⟨canonicalize related sequence, hReduced, by rw [hSame start]; exact hReach⟩
  · rintro ⟨sequence, hMember, hReach⟩
    obtain ⟨hLength, hAll, _⟩ := (mem_reducedUpTo related alphabet depth sequence).1 hMember
    exact ⟨sequence, (mem_sequencesUpTo alphabet depth sequence).2 ⟨hLength, hAll⟩, hReach⟩

/-- The declared reduced baseline: the reduced search over the declared starts, alphabet and depth. -/
def declaredReducedBaseline : List Observation :=
  declaredStarts.flatMap (fun start =>
    baselineObservations start (reducedUpTo independent exploreAlphabet 3))

/-- The declared starts are empty tables after admissions. -/
theorem the_declared_starts_are_reachable :
    reaches [.acquire .left 1] (emptyTable 1) = some baseline ∧
      reaches [.acquire .left 1, .acquire .right 1] (emptyTable 2) = some paired := by
  decide

/-- Full and reduced agreement at the declared bound, as an instance of the universal theorem. -/
theorem the_declared_reduction_agrees (observation : Observation) :
    observation ∈ declaredBaseline ↔ observation ∈ declaredReducedBaseline := by
  simp only [declaredBaseline, declaredReducedBaseline, List.mem_flatMap]
  constructor <;> rintro ⟨start, hStart, hMember⟩ <;> refine ⟨start, hStart, ?_⟩
  · exact (reduced_search_agrees independent independent_is_sound start exploreAlphabet 3
      observation).1 hMember
  · exact (reduced_search_agrees independent independent_is_sound start exploreAlphabet 3
      observation).2 hMember

/-- Deterministic counts of the sequences each declared search enumerates per start, kernel-checked:
the reduced search generates fewer sequences because it never extends a pruned pair. -/
theorem the_declared_sequence_counts :
    (sequencesUpTo exploreAlphabet 3).length = 3616 ∧
      (reducedUpTo independent exploreAlphabet 3).length = 2717 := by
  decide +kernel

/-- Deterministic counts of the reachable histories, the enumerated sequences no step refuses, per
declared start: one fewer from `baseline`, where the right id is rarely enabled, and about a third
fewer from `paired`, where both records run concurrently. -/
theorem the_declared_history_counts :
    (baselineObservations baseline (sequencesUpTo exploreAlphabet 3)).length = 244 ∧
      (baselineObservations baseline (reducedUpTo independent exploreAlphabet 3)).length = 243 ∧
      (baselineObservations paired (sequencesUpTo exploreAlphabet 3)).length = 500 ∧
      (baselineObservations paired (reducedUpTo independent exploreAlphabet 3)).length = 338 := by
  decide +kernel

/-- Deterministic counts of the distinct documented observations per declared start: the full and
reduced searches reach the same number, as `reduced_search_agrees` requires. -/
theorem the_declared_observation_counts :
    (baselineObservations baseline (sequencesUpTo exploreAlphabet 3)).eraseDups.length = 44 ∧
      (baselineObservations baseline (reducedUpTo independent exploreAlphabet 3)).eraseDups.length
        = 44 ∧
      (baselineObservations paired (sequencesUpTo exploreAlphabet 3)).eraseDups.length = 87 ∧
      (baselineObservations paired (reducedUpTo independent exploreAlphabet 3)).eraseDups.length
        = 87 := by
  decide +kernel

/-- One history per behaviour the baseline must cover, with its declared start. -/
def coverageWitnesses : List (Table × List Action) :=
  [ -- contention for the shared unit, and its release admitting the other id
    (baseline, [.acquire .right 1]),
    (baseline, [.finish .left 1, .acquire .right 1]),
    -- cancellation against delivery, in both orders
    (baseline, [.cancel .left 1, .deliver .left 1]),
    (baseline, [.deliver .left 1, .cancel .left 1]),
    -- duplicate and late callbacks
    (baseline, [.deliver .left 1, .deliver .left 1]),
    (baseline, [.complete .left 1 true, .complete .left 1 true]),
    (baseline, [.finish .left 1, .deliver .left 1]),
    (baseline, [.finish .left 1, .complete .left 1 true]),
    -- a stale handle after same-slot reuse
    (baseline, [.finish .left 1, .acquire .left 2]),
    (baseline, [.finish .left 1, .acquire .left 2, .staleCallback .left 1]),
    (baseline, [.finish .left 1, .acquire .left 2, .deliver .left 1]),
    -- wakeup coalescing and exhaustion
    (baseline, [.wake .left 1]),
    (baseline, [.wake .left 1, .wake .left 1]),
    (baseline, [.takeWake .left 1]),
    (baseline, [.wake .left 1, .takeWake .left 1, .wake .left 1]),
    -- settlement and cleanup ordering
    (baseline, [.settle .left 1]),
    (baseline, [.deliver .left 1, .settle .left 1, .cleanup .left]),
    (baseline, [.deliver .left 1, .cleanup .left, .settle .left 1]),
    -- once-only finish
    (baseline, [.finish .left 1, .finish .left 1]),
    -- concurrent distinct-slot histories
    (paired, [.deliver .left 1, .deliver .right 1]),
    (paired, [.deliver .right 1, .deliver .left 1]),
    (paired, [.cancel .left 1, .finish .right 1]),
    (paired, [.finish .right 1, .cancel .left 1]) ]

/-- **The full search covers every listed behaviour**: each coverage witness starts at a declared
start and is enumerated by the declared full search, so its canonical representative is in the
reduced search (`every_sequence_has_a_reduced_representative`). -/
theorem the_coverage_witnesses_are_in_the_full_search (witness : Table × List Action)
    (h : witness ∈ coverageWitnesses) :
    witness.1 ∈ declaredStarts ∧ witness.2 ∈ sequencesUpTo exploreAlphabet 3 := by
  have hAll : ∀ witness ∈ coverageWitnesses, witness.1 ∈ declaredStarts ∧ witness.2.length ≤ 3 ∧
      ∀ action ∈ witness.2, action ∈ exploreAlphabet := by
    decide
  obtain ⟨hStart, hLength, hAlphabet⟩ := hAll witness h
  exact ⟨hStart, (mem_sequencesUpTo exploreAlphabet 3 witness.2).2 ⟨hLength, hAlphabet⟩⟩

/-- Contention: the held unit refuses the other id, and releasing it admits that id. -/
theorem contention_is_covered :
    reaches [.acquire .right 1] baseline = none ∧
      (reaches [.finish .left 1, .acquire .right 1] baseline).isSome = true := by
  decide

/-- Cancellation against delivery: a cancelled record refuses delivery, while a delivered record can
still be cancelled, so the two orders are different histories. -/
theorem cancel_versus_deliver_is_covered :
    reaches [.cancel .left 1, .deliver .left 1] baseline = none ∧
      (reaches [.deliver .left 1, .cancel .left 1] baseline).map
        (fun table => (table.deliveries, table.left.retiring, table.left.pinHeld))
        = some ([(.left, 1)], true, true) := by
  decide

/-- Duplicate and late callbacks: a second delivery and a delivery or completion after finish are
refused, and a repeated successful completion changes nothing. -/
theorem late_and_duplicate_callbacks_are_covered :
    reaches [.deliver .left 1, .deliver .left 1] baseline = none ∧
      (reaches [.complete .left 1 true, .complete .left 1 true] baseline).map observe
        = some (observe baseline) ∧
      reaches [.finish .left 1, .deliver .left 1] baseline = none ∧
      reaches [.finish .left 1, .complete .left 1 true] baseline = none := by
  decide

/-- A stale handle after same-slot reuse: the old generation's callback is observation-free and its
delivery is refused. -/
theorem stale_handles_are_covered :
    (reaches [.finish .left 1, .acquire .left 2] baseline).isSome = true ∧
      (reaches [.finish .left 1, .acquire .left 2, .staleCallback .left 1] baseline).map observe
        = (reaches [.finish .left 1, .acquire .left 2] baseline).map observe ∧
      reaches [.finish .left 1, .acquire .left 2, .deliver .left 1] baseline = none := by
  decide

/-- Wakeups: a second wake coalesces into the queued one, taking a wakeup needs one queued, and a wake
after the budget is spent records the sticky failure and marks the record retiring. -/
theorem wakeup_exhaustion_is_covered :
    (reaches [.wake .left 1, .wake .left 1] baseline).map observe
        = (reaches [.wake .left 1] baseline).map observe ∧
      (reaches [.wake .left 1] baseline).map (fun table => (table.left.reserved, table.left.wakeups))
        = some (1, 0) ∧
      reaches [.takeWake .left 1] baseline = none ∧
      (reaches [.wake .left 1, .takeWake .left 1, .wake .left 1] baseline).map
        (fun table => (table.failure, table.left.retiring, table.left.reserved, table.left.wakeups))
        = some (some 1, true, 0, 0) := by
  decide

/-- Settlement and cleanup ordering: settlement needs a delivery, cleanup after settlement keeps the
settlement record, and settlement after cleanup is refused. -/
theorem settlement_and_cleanup_ordering_is_covered :
    reaches [.settle .left 1] baseline = none ∧
      (reaches [.deliver .left 1, .settle .left 1, .cleanup .left] baseline).map
        (fun table => (table.settled, generationOf table.left.lifecycle))
        = some ([(.left, 1)], none) ∧
      reaches [.deliver .left 1, .cleanup .left, .settle .left 1] baseline = none := by
  decide

/-- Once-only finish. -/
theorem once_only_finish_is_covered : reaches [.finish .left 1, .finish .left 1] baseline = none := by
  decide

/-- Concurrent distinct-slot histories: the two delivery orders are different documented
observations, while cancelling one record and finishing the other commute. -/
theorem concurrent_distinct_slot_histories_are_covered :
    (reaches [.deliver .left 1, .deliver .right 1] paired).map observe
        ≠ (reaches [.deliver .right 1, .deliver .left 1] paired).map observe ∧
      (reaches [.cancel .left 1, .finish .right 1] paired).map observe
        = (reaches [.finish .right 1, .cancel .left 1] paired).map observe := by
  decide

/-- The deliberately unsound rule "different task ids are independent", as a relation. -/
def differentIds (first second : Action) : Bool :=
  first.id != second.id

/-- The unsound rule is refuted as a commuting relation by the delivery pair. -/
theorem different_ids_rule_is_unsound : ¬ Sound differentIds := by
  intro hSound
  apply delivery_orders_disagree_on_distinct_ids
  rw [hSound (.deliver .right 1) (.deliver .left 1) paired (by decide)]

/-- The two delivery actions the unsound control is measured on. -/
def deliveryAlphabet : List Action :=
  [.deliver .left 1, .deliver .right 1]

/-- **The full/reduced comparison detects the unsound rule**: with "different ids are independent",
the reduced search loses the right-first delivery order the full search reaches, while the proved
relation keeps every observation at the same bound. -/
theorem the_unsound_reduction_loses_an_observation :
    (baselineObservations paired (sequencesUpTo deliveryAlphabet 2)).all
        (fun observation =>
          decide (observation ∈ baselineObservations paired (reducedUpTo differentIds deliveryAlphabet 2)))
      = false
    ∧ (baselineObservations paired (sequencesUpTo deliveryAlphabet 2)).all
        (fun observation =>
          decide (observation ∈ baselineObservations paired (reducedUpTo independent deliveryAlphabet 2)))
      = true := by
  decide

/-- **Measured agreement at one start is not evidence of soundness**: at the declared bound the
unsound reduction loses observations from `paired`, but from `baseline`, where the right id is rarely
enabled, it happens to keep every observation. Only the commutation proof enables a reduction. -/
theorem the_unsound_reduction_is_detected_only_where_ids_interleave :
    (baselineObservations paired (sequencesUpTo exploreAlphabet 3)).all
        (fun observation => decide (observation ∈
          baselineObservations paired (reducedUpTo differentIds exploreAlphabet 3))) = false ∧
      (baselineObservations baseline (sequencesUpTo exploreAlphabet 3)).all
        (fun observation => decide (observation ∈
          baselineObservations baseline (reducedUpTo differentIds exploreAlphabet 3))) = true := by
  decide +kernel

/-! ### Valid client histories and the client projection (issue #46's Relinche-derived lane)

`reaches` ends a history at its first refused step. A *client* keeps going: the table answers each
call `accepted` or `rejected`, a rejected call leaves the table unchanged, and the client issues its
next call. `advance`, `accepts` and `respond` give that client semantics, and `afterHistory` runs a
whole client history.

A *valid client* names only what it holds (`ValidClient`). It holds the handles its start table
issued and the handles its own accepted admissions returned. A stale handle from an earlier
generation stays held, so a stale call is a valid call that the table may reject. A forged handle,
and a handle a rejected admission would have named, are not.

The documented client projection (`projectClient`) keeps, per id, every call paired with its
response, in order: handle identity and refusal outcomes included. It also keeps the documented
observation of the final table: failure, pin, reservation, wake, cleanup and the ordered delivery
and settlement records. The order between calls on different ids is observed only through shared
state. `lossyClient` drops rejected calls and is measured as lossy.

Independent calls commute under the client semantics too (`independent_is_client_sound`): neither
changes whether the other is accepted, and the two orders reach the same table. So the canonical
representative of a valid client history is valid and has the same projection, and the reduced
search reaches the same projections as the full search (`reduced_client_search_agrees`).

This is a sequential explorer. It is not a weak-memory or linearizability result. No concurrent
wrapper is selected, so weak-memory controls are not applicable rather than passed, and no
source/runtime event correspondence is claimed (that belongs to the #50 trace relation).
-/

/-- The table after a client call: an accepted call moves it, a rejected one leaves it unchanged. -/
def advance (action : Action) (table : Table) : Table :=
  (step action table).getD table

/-- Whether the table accepts a client call. -/
def accepts (action : Action) (table : Table) : Bool :=
  (step action table).isSome

/-- Testing a guarded result tests each branch. -/
theorem isSome_ite {c : Prop} [Decidable c] (first second : Option Table) :
    (if c then first else second).isSome = if c then first.isSome else second.isSome := by
  by_cases held : c <;> simp [held]

set_option maxHeartbeats 8000000 in
/-- A right-id step taken first never changes whether an independent left-id call is accepted. -/
theorem acceptance_kept_left_right (first second : Action) (table next : Table)
    (hFirst : first.id = .left) (hSecond : second.id = .right)
    (h : independent first second = true) (taken : step second table = some next) :
    (step first next).isSome = (step first table).isSome := by
  obtain ⟨capacity, ⟨leftLife, leftPin, leftReserved, leftWakeups, leftRetiring⟩,
    ⟨rightLife, rightPin, rightReserved, rightWakeups, rightRetiring⟩,
    failure, deliveries, settled⟩ := table
  cases first <;> cases second <;>
    (try simp only [Action.id] at hFirst hSecond) <;>
    subst hFirst hSecond <;>
    simp [independent, Action.id, Action.readsCapacity, Action.movesCapacity, Action.writesFailure,
      Action.isDelivery, Action.isSettlement] at h <;>
    (try (rcases h with h | h <;> subst h)) <;>
    cases leftLife <;> cases rightLife <;>
    simp only [step, slot, withSlot, liveCount, counted, liveSlot, retiredSlot] at taken <;>
    (repeat' split at taken) <;>
    simp only [Option.some.injEq, reduceCtorEq] at taken <;>
    (try subst taken) <;>
    simp only [step, slot, withSlot, isSome_ite, Option.isSome_some, Option.isSome_none, liveCount,
      counted, liveSlot, retiredSlot] <;>
    (repeat' split) <;> simp_all

set_option maxHeartbeats 8000000 in
/-- A left-id step taken first never changes whether an independent right-id call is accepted. -/
theorem acceptance_kept_right_left (first second : Action) (table next : Table)
    (hFirst : first.id = .right) (hSecond : second.id = .left)
    (h : independent first second = true) (taken : step second table = some next) :
    (step first next).isSome = (step first table).isSome := by
  obtain ⟨capacity, ⟨leftLife, leftPin, leftReserved, leftWakeups, leftRetiring⟩,
    ⟨rightLife, rightPin, rightReserved, rightWakeups, rightRetiring⟩,
    failure, deliveries, settled⟩ := table
  cases first <;> cases second <;>
    (try simp only [Action.id] at hFirst hSecond) <;>
    subst hFirst hSecond <;>
    simp [independent, Action.id, Action.readsCapacity, Action.movesCapacity, Action.writesFailure,
      Action.isDelivery, Action.isSettlement] at h <;>
    (try (rcases h with h | h <;> subst h)) <;>
    cases leftLife <;> cases rightLife <;>
    simp only [step, slot, withSlot, liveCount, counted, liveSlot, retiredSlot] at taken <;>
    (repeat' split at taken) <;>
    simp only [Option.some.injEq, reduceCtorEq] at taken <;>
    (try subst taken) <;>
    simp only [step, slot, withSlot, isSome_ite, Option.isSome_some, Option.isSome_none, liveCount,
      counted, liveSlot, retiredSlot] <;>
    (repeat' split) <;> simp_all

/-- An independent call never changes whether the other is accepted, whether it was itself accepted
or rejected. -/
theorem independent_keeps_acceptance (first second : Action) (table : Table)
    (h : independent first second = true) :
    accepts first (advance second table) = accepts first table := by
  unfold accepts advance
  cases hStep : step second table with
  | none => rfl
  | some next =>
      simp only [Option.getD_some]
      cases hFirst : first.id <;> cases hSecond : second.id
      · simp [independent, hFirst, hSecond] at h
      · exact acceptance_kept_left_right first second table next hFirst hSecond h hStep
      · exact acceptance_kept_right_left first second table next hFirst hSecond h hStep
      · simp [independent, hFirst, hSecond] at h

/-- Independent calls commute under the client semantics, rejected calls included. -/
theorem independent_advance_commute (first second : Action) (table : Table)
    (h : independent first second = true) :
    advance first (advance second table) = advance second (advance first table) := by
  have hFirstKept := independent_keeps_acceptance first second table h
  have hSecondKept := independent_keeps_acceptance second first table
    (by rw [independent_symm]; exact h)
  have hBind := independent_actions_commute first second table h
  unfold accepts advance at *
  cases hFirstStep : step first table with
  | none =>
      cases hSecondStep : step second table with
      | none => simp [hFirstStep, hSecondStep]
      | some afterSecond =>
          simp only [hFirstStep, hSecondStep, Option.getD_some, Option.isSome_none,
            Option.getD_none] at hFirstKept ⊢
          cases hLate : step first afterSecond with
          | none => simp
          | some _ => simp [hLate] at hFirstKept
  | some afterFirst =>
      cases hSecondStep : step second table with
      | none =>
          simp only [hFirstStep, hSecondStep, Option.getD_some, Option.isSome_none,
            Option.getD_none] at hSecondKept ⊢
          cases hLate : step second afterFirst with
          | none => simp
          | some _ => simp [hLate] at hSecondKept
      | some afterSecond =>
          simp only [hFirstStep, hSecondStep, Option.getD_some, Option.isSome_some,
            Option.bind_some] at hFirstKept hSecondKept hBind ⊢
          cases hBoth : step first afterSecond with
          | none => simp [hBoth] at hFirstKept
          | some both =>
              rw [hBoth] at hBind
              simp [hBind]

/-- A relation the client reduction may use: related calls are on different ids, neither changes
whether the other is accepted, and the two orders reach the same table. -/
def ClientSound (related : Action → Action → Bool) : Prop :=
  ∀ first second, related first second = true → first.id ≠ second.id ∧
    ∀ table, accepts first (advance second table) = accepts first table ∧
      accepts second (advance first table) = accepts second table ∧
      advance first (advance second table) = advance second (advance first table)

/-- **The independence relation is sound for client histories.** -/
theorem independent_is_client_sound : ClientSound independent := by
  intro first second h
  refine ⟨?_, fun table => ⟨independent_keeps_acceptance first second table h,
    independent_keeps_acceptance second first table (by rw [independent_symm]; exact h),
    independent_advance_commute first second table h⟩⟩
  intro hSame
  simp [independent, hSame] at h

/-- The table's answer to a client call. -/
inductive Response where
  | accepted
  | rejected
deriving DecidableEq, Repr

def respond (action : Action) (table : Table) : Response :=
  if accepts action table then .accepted else .rejected

/-- The table after a whole client history. -/
def afterHistory : List Action → Table → Table
  | [], table => table
  | action :: rest, table => afterHistory rest (advance action table)

/-- The calls naming one id, in order, each paired with the table's response. -/
def callsOf (id : TaskId) : List Action → Table → List (Action × Response)
  | [], _ => []
  | action :: rest, table =>
      (if action.id = id then [(action, respond action table)] else []) ++
        callsOf id rest (advance action table)

/-- The documented client projection: per-id calls with their responses, and the documented
observation of the final table. -/
structure ClientObservation where
  leftCalls : List (Action × Response)
  rightCalls : List (Action × Response)
  final : Observation
deriving DecidableEq, Repr

def projectClient (start : Table) (history : List Action) : ClientObservation :=
  { leftCalls := callsOf .left history start
    rightCalls := callsOf .right history start
    final := observe (afterHistory history start) }

/-- The deliberately lossy client projection: rejected calls are dropped. -/
def lossyClient (start : Table) (history : List Action) : ClientObservation :=
  { leftCalls := (callsOf .left history start).filter (fun call => call.2 == .accepted)
    rightCalls := (callsOf .right history start).filter (fun call => call.2 == .accepted)
    final := observe (afterHistory history start) }

/-- The handle a call names. Admission names none, because it is how a handle is issued; cleanup
names its slot only. -/
def Action.handle : Action → Option (TaskId × Nat)
  | .acquire _ _ => none
  | .cancel id generation => some (id, generation)
  | .complete id generation _ => some (id, generation)
  | .deliver id generation => some (id, generation)
  | .staleCallback id generation => some (id, generation)
  | .settle id generation => some (id, generation)
  | .cleanup _ => none
  | .finish id generation => some (id, generation)
  | .wake id generation => some (id, generation)
  | .takeWake id generation => some (id, generation)

/-- The handle an admission issues when the table accepts it. -/
def Action.issues : Action → Option (TaskId × Nat)
  | .acquire id generation => some (id, generation)
  | _ => none

/-- Whether the client holds what a call names: the named handle, or for cleanup some handle on the
slot. Admission needs nothing. -/
def holds (held : List (TaskId × Nat)) (action : Action) : Bool :=
  match action.handle with
  | some handle => decide (handle ∈ held)
  | none =>
      match action with
      | .cleanup id => decide (∃ handle ∈ held, handle.1 = id)
      | _ => true

/-- The handles held after a call: an accepted admission adds the handle it issued. -/
def issue (held : List (TaskId × Nat)) (action : Action) (table : Table) : List (TaskId × Nat) :=
  if accepts action table then action.issues.toList ++ held else held

/-- Whether every call of a client history names only what the client holds when it makes the call.
A rejected call does not end the history, and a rejected admission issues no handle. -/
def validFrom : List (TaskId × Nat) → Table → List Action → Bool
  | _, _, [] => true
  | held, table, action :: rest =>
      holds held action && validFrom (issue held action table) (advance action table) rest

/-- The handles a start table has issued: the generation of each record it holds, live or retired. -/
def issuedHandles (table : Table) : List (TaskId × Nat) :=
  ((generationOf table.left.lifecycle).map (fun generation => (TaskId.left, generation))).toList ++
    ((generationOf table.right.lifecycle).map (fun generation => (TaskId.right, generation))).toList

/-- **The valid-client predicate**: from a start table, the client names only handles it holds. -/
def ValidClient (start : Table) (history : List Action) : Bool :=
  validFrom (issuedHandles start) start history

/-- On a history with no rejected call, the client semantics reaches what `reaches` reaches. -/
theorem afterHistory_of_reaches (history : List Action) (table last : Table)
    (h : reaches history table = some last) : afterHistory history table = last := by
  induction history generalizing table with
  | nil =>
      simp only [reaches, Option.some.injEq] at h
      exact h
  | cons action rest ih =>
      simp only [reaches] at h
      cases hStep : step action table with
      | none => simp [hStep] at h
      | some next =>
          simp only [hStep, Option.bind_some] at h
          simp only [afterHistory, advance, hStep, Option.getD_some]
          exact ih next h

/-- An admission issues a handle on its own id. -/
theorem issues_names_its_id (action : Action) (handle : TaskId × Nat)
    (h : action.issues = some handle) : handle.1 = action.id := by
  cases action <;> simp only [Action.issues, reduceCtorEq] at h
  simp only [Option.some.injEq] at h
  subst h
  rfl

/-- Holding is decided by membership alone. -/
theorem holds_congr (held other : List (TaskId × Nat)) (action : Action)
    (h : ∀ handle, handle ∈ held ↔ handle ∈ other) : holds held action = holds other action := by
  unfold holds
  cases action.handle with
  | some handle => simp [h handle]
  | none => cases action <;> simp [h]

theorem mem_issue_congr (held other : List (TaskId × Nat)) (action : Action) (table : Table)
    (h : ∀ handle, handle ∈ held ↔ handle ∈ other) (handle : TaskId × Nat) :
    handle ∈ issue held action table ↔ handle ∈ issue other action table := by
  unfold issue
  split <;> simp [h handle]

/-- Validity is decided by which handles are held, not by their order. -/
theorem validFrom_congr (history : List Action) :
    ∀ (held other : List (TaskId × Nat)) (table : Table),
      (∀ handle, handle ∈ held ↔ handle ∈ other) →
        validFrom held table history = validFrom other table history := by
  induction history with
  | nil => intro _ _ _ _; rfl
  | cons action rest ih =>
      intro held other table h
      simp only [validFrom]
      rw [holds_congr held other action h,
        ih (issue held action table) (issue other action table) (advance action table)
          (mem_issue_congr held other action table h)]

/-- A call's handle check ignores what an admission on another id issued. -/
theorem holds_after_other_issue (held : List (TaskId × Nat)) (first second : Action) (table : Table)
    (hIds : first.id ≠ second.id) : holds (issue held first table) second = holds held second := by
  unfold issue
  split
  · cases hIssue : first.issues with
    | none => rfl
    | some issued =>
        obtain ⟨issuedId, issuedGeneration⟩ := issued
        have hIssued : issuedId = first.id := issues_names_its_id first _ hIssue
        have hNe : issuedId ≠ second.id := by
          rw [hIssued]
          exact hIds
        cases second <;> simp only [Action.id] at hNe <;>
          simp [holds, Action.handle, hNe, Ne.symm hNe]
  · rfl

/-- Two admissions on different ids issue the same handles in either order. -/
theorem mem_issue_swap (held : List (TaskId × Nat)) (first second : Action) (table : Table)
    (hFirstKept : accepts first (advance second table) = accepts first table)
    (hSecondKept : accepts second (advance first table) = accepts second table)
    (handle : TaskId × Nat) :
    handle ∈ issue (issue held first table) second (advance first table) ↔
      handle ∈ issue (issue held second table) first (advance second table) := by
  unfold issue
  rw [hFirstKept, hSecondKept]
  by_cases hFirst : accepts first table <;> by_cases hSecond : accepts second table <;>
    simp [hFirst, hSecond, or_left_comm]

theorem validFrom_swap (related : Action → Action → Bool) (hSound : ClientSound related)
    (first second : Action) (hRelated : related first second = true) (rest : List Action)
    (held : List (TaskId × Nat)) (table : Table) :
    validFrom held table (first :: second :: rest) = validFrom held table (second :: first :: rest) := by
  obtain ⟨hIds, hTable⟩ := hSound first second hRelated
  obtain ⟨hFirstKept, hSecondKept, hCommute⟩ := hTable table
  simp only [validFrom]
  rw [holds_after_other_issue held first second table hIds,
    holds_after_other_issue held second first table (Ne.symm hIds), hCommute,
    validFrom_congr rest (issue (issue held first table) second (advance first table))
      (issue (issue held second table) first (advance second table))
      (advance second (advance first table))
      (mem_issue_swap held first second table hFirstKept hSecondKept)]
  cases holds held first <;> cases holds held second <;> rfl

theorem callsOf_swap (related : Action → Action → Bool) (hSound : ClientSound related)
    (first second : Action) (hRelated : related first second = true) (rest : List Action)
    (id : TaskId) (table : Table) :
    callsOf id (first :: second :: rest) table = callsOf id (second :: first :: rest) table := by
  obtain ⟨hIds, hTable⟩ := hSound first second hRelated
  obtain ⟨hFirstKept, hSecondKept, hCommute⟩ := hTable table
  simp only [callsOf, respond, hFirstKept, hSecondKept, hCommute]
  by_cases hFirstId : first.id = id <;> by_cases hSecondId : second.id = id
  · exact absurd (hFirstId.trans hSecondId.symm) hIds
  · simp [hFirstId, hSecondId]
  · simp [hFirstId, hSecondId]
  · simp [hFirstId, hSecondId]

theorem afterHistory_swap (related : Action → Action → Bool) (hSound : ClientSound related)
    (first second : Action) (hRelated : related first second = true) (rest : List Action)
    (table : Table) :
    afterHistory (first :: second :: rest) table = afterHistory (second :: first :: rest) table := by
  obtain ⟨_, hTable⟩ := hSound first second hRelated
  simp only [afterHistory, (hTable table).2.2]

theorem validFrom_insertCanonical (related : Action → Action → Bool) (hSound : ClientSound related)
    (action : Action) (rest : List Action) :
    ∀ held table, validFrom held table (insertCanonical related action rest) =
      validFrom held table (action :: rest) := by
  induction rest with
  | nil => intro _ _; rfl
  | cons head tail ih =>
      intro held table
      simp only [insertCanonical]
      split
      · rename_i hOut
        have hRelated : related action head = true := by
          simp only [outOfOrder, Bool.and_eq_true] at hOut
          exact hOut.1.1
        rw [validFrom_swap related hSound action head hRelated tail held table]
        simp only [validFrom, ih]
      · rfl

theorem callsOf_insertCanonical (related : Action → Action → Bool) (hSound : ClientSound related)
    (action : Action) (rest : List Action) (id : TaskId) :
    ∀ table, callsOf id (insertCanonical related action rest) table =
      callsOf id (action :: rest) table := by
  induction rest with
  | nil => intro _; rfl
  | cons head tail ih =>
      intro table
      simp only [insertCanonical]
      split
      · rename_i hOut
        have hRelated : related action head = true := by
          simp only [outOfOrder, Bool.and_eq_true] at hOut
          exact hOut.1.1
        rw [callsOf_swap related hSound action head hRelated tail id table]
        simp only [callsOf, ih]
      · rfl

theorem afterHistory_insertCanonical (related : Action → Action → Bool)
    (hSound : ClientSound related) (action : Action) (rest : List Action) :
    ∀ table, afterHistory (insertCanonical related action rest) table =
      afterHistory (action :: rest) table := by
  induction rest with
  | nil => intro _; rfl
  | cons head tail ih =>
      intro table
      simp only [insertCanonical]
      split
      · rename_i hOut
        have hRelated : related action head = true := by
          simp only [outOfOrder, Bool.and_eq_true] at hOut
          exact hOut.1.1
        rw [afterHistory_swap related hSound action head hRelated tail table]
        simp only [afterHistory, ih]
      · rfl

theorem validFrom_canonicalize (related : Action → Action → Bool) (hSound : ClientSound related)
    (history : List Action) :
    ∀ held table, validFrom held table (canonicalize related history) =
      validFrom held table history := by
  induction history with
  | nil => intro _ _; rfl
  | cons action rest ih =>
      intro held table
      simp only [canonicalize]
      rw [validFrom_insertCanonical related hSound action (canonicalize related rest) held table]
      simp only [validFrom, ih]

theorem callsOf_canonicalize (related : Action → Action → Bool) (hSound : ClientSound related)
    (history : List Action) (id : TaskId) :
    ∀ table, callsOf id (canonicalize related history) table = callsOf id history table := by
  induction history with
  | nil => intro _; rfl
  | cons action rest ih =>
      intro table
      simp only [canonicalize]
      rw [callsOf_insertCanonical related hSound action (canonicalize related rest) id table]
      simp only [callsOf, ih]

theorem afterHistory_canonicalize (related : Action → Action → Bool)
    (hSound : ClientSound related) (history : List Action) :
    ∀ table, afterHistory (canonicalize related history) table = afterHistory history table := by
  induction history with
  | nil => intro _; rfl
  | cons action rest ih =>
      intro table
      simp only [canonicalize]
      rw [afterHistory_insertCanonical related hSound action (canonicalize related rest) table]
      simp only [afterHistory, ih]

/-- **The canonical representative of a client history is valid exactly when the history is, and
has the same documented client projection.** -/
theorem client_canonicalize (related : Action → Action → Bool) (hSound : ClientSound related)
    (start : Table) (history : List Action) :
    ValidClient start (canonicalize related history) = ValidClient start history ∧
      projectClient start (canonicalize related history) = projectClient start history :=
  ⟨validFrom_canonicalize related hSound history _ _, by
    simp only [projectClient, callsOf_canonicalize related hSound history,
      afterHistory_canonicalize related hSound history]⟩

/-- The documented client projections of the valid client histories among a sequence list. -/
def clientObservations (start : Table) (sequences : List (List Action)) : List ClientObservation :=
  (sequences.filter (ValidClient start)).map (projectClient start)

/-- **The reduced search agrees with the full search on valid client histories**: for every
client-sound relation, start table, alphabet and depth, the two searches reach the same set of
documented client projections. -/
theorem reduced_client_search_agrees (related : Action → Action → Bool)
    (hSound : ClientSound related) (start : Table) (alphabet : List Action) (depth : Nat)
    (observation : ClientObservation) :
    observation ∈ clientObservations start (sequencesUpTo alphabet depth) ↔
      observation ∈ clientObservations start (reducedUpTo related alphabet depth) := by
  simp only [clientObservations, List.mem_map, List.mem_filter]
  constructor
  · rintro ⟨history, ⟨hMember, hValid⟩, rfl⟩
    obtain ⟨hLength, hAll⟩ := (mem_sequencesUpTo alphabet depth history).1 hMember
    have hPerm := canonicalize_perm related history
    obtain ⟨hValidSame, hProjected⟩ := client_canonicalize related hSound start history
    refine ⟨canonicalize related history, ⟨(mem_reducedUpTo related alphabet depth _).2
      ⟨?_, ?_, canonicalize_canonical related history⟩, by rw [hValidSame]; exact hValid⟩,
      hProjected⟩
    · rw [hPerm.length_eq]
      exact hLength
    · intro action hAction
      exact hAll action (hPerm.mem_iff.1 hAction)
  · rintro ⟨history, ⟨hMember, hValid⟩, rfl⟩
    obtain ⟨hLength, hAll, _⟩ := (mem_reducedUpTo related alphabet depth history).1 hMember
    exact ⟨history, ⟨(mem_sequencesUpTo alphabet depth history).2 ⟨hLength, hAll⟩, hValid⟩, rfl⟩

/-- The declared client baselines: valid client histories over the declared bound, full and reduced. -/
def declaredClientBaseline : List ClientObservation :=
  declaredStarts.flatMap (fun start => clientObservations start (sequencesUpTo exploreAlphabet 3))

def declaredReducedClientBaseline : List ClientObservation :=
  declaredStarts.flatMap (fun start =>
    clientObservations start (reducedUpTo independent exploreAlphabet 3))

/-- Full and reduced agreement on valid client histories at the declared bound. -/
theorem the_declared_client_reduction_agrees (observation : ClientObservation) :
    observation ∈ declaredClientBaseline ↔ observation ∈ declaredReducedClientBaseline := by
  simp only [declaredClientBaseline, declaredReducedClientBaseline, List.mem_flatMap]
  constructor <;> rintro ⟨start, hStart, hMember⟩ <;> refine ⟨start, hStart, ?_⟩
  · exact (reduced_client_search_agrees independent independent_is_client_sound start
      exploreAlphabet 3 observation).1 hMember
  · exact (reduced_client_search_agrees independent independent_is_client_sound start
      exploreAlphabet 3 observation).2 hMember

/-- The valid-client predicate on concrete histories: a handle the start never issued is forged, a
rejected admission issues nothing, an accepted one issues its handle, and a stale handle stays held
so its rejected call is still a valid call. -/
theorem valid_client_examples :
    ValidClient baseline [.deliver .right 1] = false ∧
      ValidClient baseline [.acquire .right 1, .deliver .right 1] = false ∧
      ValidClient baseline [.finish .left 1, .acquire .right 1, .deliver .right 1] = true ∧
      ValidClient baseline [.finish .left 1, .acquire .left 2, .deliver .left 1] = true ∧
      respond (.deliver .left 1) (afterHistory [.finish .left 1, .acquire .left 2] baseline)
        = .rejected ∧
      ValidClient paired [.deliver .right 1, .cleanup .left] = true := by
  decide

/-- Every coverage witness is a valid client history. -/
theorem the_coverage_witnesses_are_valid_clients :
    coverageWitnesses.all (fun witness => ValidClient witness.1 witness.2) = true := by
  decide

/-- **Positive histories**: each pair ends at the same table but differs in a rejected call — a
delivery after cancellation, an old-generation delivery after reuse, a settlement after cleanup —
and the documented client projection distinguishes it. The two cancel/deliver orders are
distinguished by the delivery's response. -/
theorem the_client_projection_discriminates :
    afterHistory [.cancel .left 1, .deliver .left 1] baseline
        = afterHistory [.cancel .left 1] baseline ∧
      projectClient baseline [.cancel .left 1, .deliver .left 1]
        ≠ projectClient baseline [.cancel .left 1] ∧
      afterHistory [.finish .left 1, .acquire .left 2, .deliver .left 1] baseline
        = afterHistory [.finish .left 1, .acquire .left 2] baseline ∧
      projectClient baseline [.finish .left 1, .acquire .left 2, .deliver .left 1]
        ≠ projectClient baseline [.finish .left 1, .acquire .left 2] ∧
      afterHistory [.deliver .left 1, .cleanup .left, .settle .left 1] baseline
        = afterHistory [.deliver .left 1, .cleanup .left] baseline ∧
      projectClient baseline [.deliver .left 1, .cleanup .left, .settle .left 1]
        ≠ projectClient baseline [.deliver .left 1, .cleanup .left] ∧
      (projectClient baseline [.deliver .left 1, .cancel .left 1]).leftCalls
        ≠ (projectClient baseline [.cancel .left 1, .deliver .left 1]).leftCalls := by
  decide

/-- **Negative histories**: the lossy projection, which drops rejected calls, identifies each pair
above — the false equivalence that the documented projection exposes. -/
theorem the_lossy_client_projection_hides_rejected_calls :
    lossyClient baseline [.cancel .left 1, .deliver .left 1]
        = lossyClient baseline [.cancel .left 1] ∧
      lossyClient baseline [.finish .left 1, .acquire .left 2, .deliver .left 1]
        = lossyClient baseline [.finish .left 1, .acquire .left 2] ∧
      lossyClient baseline [.deliver .left 1, .cleanup .left, .settle .left 1]
        = lossyClient baseline [.deliver .left 1, .cleanup .left] := by
  decide

/-- Deterministic counts of the valid client histories per declared start, kernel-checked. From
`baseline` a client cannot name the right id until an accepted admission issues it a handle; from
`paired` every sequence over the alphabet is a valid client history. -/
theorem the_declared_client_history_counts :
    ((sequencesUpTo exploreAlphabet 3).filter (ValidClient baseline)).length = 2384 ∧
      ((reducedUpTo independent exploreAlphabet 3).filter (ValidClient baseline)).length = 2168 ∧
      ((sequencesUpTo exploreAlphabet 3).filter (ValidClient paired)).length = 3616 ∧
      ((reducedUpTo independent exploreAlphabet 3).filter (ValidClient paired)).length = 2717 := by
  decide +kernel

/-- **The client comparison detects the unsound rule from both starts**: it loses the right-first
delivery order from `paired`, and from `baseline` it loses a rejected admission followed by the
finish that would have enabled it, which the proved relation keeps. -/
theorem the_unsound_client_reduction_loses_projections :
    projectClient paired [.deliver .right 1, .deliver .left 1]
        ∈ clientObservations paired (sequencesUpTo exploreAlphabet 3) ∧
      projectClient paired [.deliver .right 1, .deliver .left 1]
        ∉ clientObservations paired (reducedUpTo differentIds exploreAlphabet 3) ∧
      projectClient baseline [.acquire .right 1, .finish .left 1]
        ∈ clientObservations baseline (sequencesUpTo exploreAlphabet 3) ∧
      projectClient baseline [.acquire .right 1, .finish .left 1]
        ∉ clientObservations baseline (reducedUpTo differentIds exploreAlphabet 3) ∧
      projectClient baseline [.acquire .right 1, .finish .left 1]
        ∈ clientObservations baseline (reducedUpTo independent exploreAlphabet 3) := by
  decide +kernel

end NobleFoundation.TaskInterleaving
