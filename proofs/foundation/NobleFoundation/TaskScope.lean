/-
  Bounded host-owned M6 task scope over the #46 interleaving model (issue #115).

  A scope composes *existing* M6 task records. It adds no kernel state and changes no per-task
  transition: every scope step that has a table counterpart is taken only when the imported
  `TaskInterleaving.step` takes it, and the scope's own bookkeeping is a host-side policy record.

  The scope is bounded at two children. Membership is an ordered list of child references (slot,
  generation) appended by `attach`, which is admitted only for a closed=false, not-yet-membered
  child while the list is shorter than two. `results` is derived from membership by filtering the
  attachment-ordered list, so completion order cannot reorder results.

  Settlement is deliberately stricter than the imported table's `finish`, which does not carry the
  kernel's native-stop, pin-settlement and cleanup guards: the scope records the three host
  acknowledgements in order (`stopped`, `pinsSettled`, `cleaned`) and refuses `finish` before all
  three, so an outstanding native operation, pin or cleanup obligation prevents a claim of final
  settlement. The scope is not `settled` until every member has finalized and been released once;
  `close` is refused until then and is idempotent afterwards.

  The primary failure is the single recorded scope failure, set by the first failing completion or
  wake-budget exhaustion in the host's serialized order and never replaced. Cancellation is retained
  separately (`cancelled`): it revokes guest access and keeps the admitted pin, and this model's
  imported table does not set a failure for it. The Rust table records `Failure::Cancelled` on the
  same retirement, so the adapter asserts that record and the receipt labels it adapter evidence.

  Not modelled, therefore not claimed: nesting, more than two children, native completion after
  host death, fairness or liveness, the kernel's internal transition rules, or any correspondence
  between this model and the Rust table API. The generated corpus compares declared traces; it is
  finite agreement, not a refinement theorem.
-/
import NobleFoundation.TaskInterleaving

namespace NobleFoundation.TaskScope

open NobleFoundation.TaskInterleaving

/-- A retained child reference is bound to the host scope and table that issued it, as well as the
    slot and generation. The two identities are descriptive bindings, not task capabilities. -/
structure Member where
  /-- The host-issued scope identity that minted this member. -/
  scopeIdentity : Nat
  /-- The host-assigned table identity containing this member. -/
  tableIdentity : Nat
  /-- The declared table slot. -/
  id : TaskId
  /-- The generation the child was admitted at. -/
  generation : Nat
deriving DecidableEq, Repr

/-- One host-owned scope: a host-issued identity and the policy bookkeeping the scope reports.

    `members` is the ordered attachment list; every other member list is an acknowledgement list
    whose guards make the policy order explicit. `failure` is the single primary failure. -/
structure Scope where
  /-- The host-issued scope identity; no task handle, slot or generation substitutes for it. -/
  identity : Nat
  /-- The one host table to which this scope is bound. -/
  tableIdentity : Nat
  /-- Latest table generation issued through this scope; admission must advance it exactly once. -/
  lastGeneration : Nat := 0
  /-- Unattached move-only admitted values minted by this scope. -/
  issued : List Member := []
  members : List Member := []
  delivered : List Member := []
  cancelled : List Member := []
  stopped : List Member := []
  pinsSettled : List Member := []
  cleaned : List Member := []
  finalized : List Member := []
  released : List Member := []
  failure : Option Member := none
  closed : Bool := false
deriving DecidableEq, Repr

/-- An empty scope at a host-issued identity. -/
def emptyScope (identity tableIdentity : Nat) : Scope := { identity, tableIdentity }

/-- The declared left child at generation one. -/
def left1 : Member := { scopeIdentity := 7, tableIdentity := 3, id := .left, generation := 1 }

/-- The declared right child at generation two after the left child was admitted first. -/
def right2 : Member := { scopeIdentity := 7, tableIdentity := 3, id := .right, generation := 2 }

/-- The declared left child at the second generation, for same-slot reuse. -/
def left2 : Member := { scopeIdentity := 7, tableIdentity := 3, id := .left, generation := 2 }

/-- A foreign-scope child, used only by a refusal control. -/
def foreignScopeLeft1 : Member := { scopeIdentity := 8, tableIdentity := 3, id := .left, generation := 1 }

/-- A foreign-table child, used only by a refusal control. -/
def foreignTableLeft1 : Member := { scopeIdentity := 7, tableIdentity := 4, id := .left, generation := 1 }

/-- Results are the members in attachment order that the table delivered: the filter keeps the
    membership order, so no completion order can reorder them. -/
def Scope.results (scope : Scope) : List Member :=
  scope.members.filter (fun member => decide (member ∈ scope.delivered))

/-- The earliest attached child whose result has neither been delivered nor cancelled. A later
    result cannot be transferred to the guest ahead of this member. -/
def Scope.nextUndelivered (scope : Scope) : Option Member :=
  scope.members.find? (fun member =>
    !(decide (member ∈ scope.delivered)) && !(decide (member ∈ scope.cancelled)))

/-- The recorded primary failure is retained: a later abnormal outcome does not replace it. -/
def noteFailure (scope : Scope) (member : Member) : Scope :=
  match scope.failure with
  | none => { scope with failure := some member }
  | some _ => scope

/-- Membership admission: the scope is open, no other scope-issued value is awaiting attachment,
    and the two-child bound holds. The table's own admission is required in addition to this. -/
def canAdmit (scope : Scope) (member : Member) : Prop :=
  scope.closed = false ∧ scope.issued.isEmpty ∧
    scope.members.length + scope.issued.length < 2 ∧
    member.scopeIdentity = scope.identity ∧ member.tableIdentity = scope.tableIdentity ∧
    member.generation = scope.lastGeneration + 1

/-- Attachment consumes only a live value that this scope's own successful admission issued. -/
def accepts (scope : Scope) (member : Member) : Prop :=
  scope.closed = false ∧ member ∈ scope.issued ∧ member ∉ scope.members ∧ scope.members.length < 2

/-- Membership release: the child was admitted, finalized, and not already released. -/
def releasable (scope : Scope) (member : Member) : Prop :=
  member ∈ scope.members ∧ member ∈ scope.finalized ∧ member ∉ scope.released

/-- Final settlement: every member finalized (`finish` succeeded after the three host
    acknowledgements) and every member's membership was released exactly once. Stated as a check
    over the retained membership list so `close` refuses on a retained fact, not on a snapshot. -/
def settled (scope : Scope) : Bool :=
  scope.issued.isEmpty && scope.members.all (fun member =>
    decide (member ∈ scope.finalized) && decide (member ∈ scope.released))

/-- Close is allowed once the scope has settled; an already closed scope stays closable so a
    repeated close is idempotent rather than a second release. -/
def closable (scope : Scope) : Bool :=
  scope.closed || settled scope

instance decidableAccepts (scope : Scope) (member : Member) : Decidable (accepts scope member) := by
  unfold accepts
  infer_instance

instance decidableCanAdmit (scope : Scope) (member : Member) : Decidable (canAdmit scope member) := by
  unfold canAdmit
  infer_instance

instance decidableReleasable (scope : Scope) (member : Member) : Decidable (releasable scope member) := by
  unfold releasable
  infer_instance

/-- The documented scope actions. `refuse` records a refused admission and retains nothing;
    `stopped`, `settlePins` and `clean` are host observations, matching the kernel's separate
    native-stop, pin-settlement and cleanup acknowledgements. -/
inductive Step where
  | admission (member : Member)
  | attach (member : Member)
  | refuse (member : Member)
  | succeed (member : Member)
  | fail (member : Member)
  | deliver (member : Member)
  | cancel (member : Member)
  | wake (member : Member)
  | takeWake (member : Member)
  | stopped (member : Member)
  | settlePins (member : Member)
  | clean (member : Member)
  | finish (member : Member)
  | release (member : Member)
  | close
deriving DecidableEq, Repr

/-- The table action a scope step carries, if any. Host observations and bookkeeping steps carry
    none: they change no retained table state. -/
def tableAction : Step → Option Action
  | .admission member => some (.acquire member.id member.generation)
  | .attach _ => none
  | .refuse _ => none
  | .succeed member => some (.complete member.id member.generation true)
  | .fail member => some (.complete member.id member.generation false)
  | .deliver member => some (.deliver member.id member.generation)
  | .cancel member => some (.cancel member.id member.generation)
  | .wake member => some (.wake member.id member.generation)
  | .takeWake member => some (.takeWake member.id member.generation)
  | .stopped _ => none
  | .settlePins _ => none
  | .clean _ => none
  | .finish member => some (.finish member.id member.generation)
  | .release _ => none
  | .close => none

/-- One scope step: the scope guard, the imported table transition, and the policy bookkeeping.
    A refused guard or a refused table transition yields `none` and retains nothing. -/
def step (action : Step) (table : Table) (scope : Scope) : Option (Table × Scope) :=
  match action with
  | .admission member =>
      if canAdmit scope member then
        (NobleFoundation.TaskInterleaving.step (.acquire member.id member.generation) table).map
          (fun next => (next, { scope with
            lastGeneration := member.generation
            issued := scope.issued ++ [member] }))
      else none
  | .attach member =>
      if accepts scope member then
        some (table, { scope with
          issued := scope.issued.filter (fun issued => decide (issued ≠ member))
          members := scope.members ++ [member] })
      else none
  | .refuse _ => some (table, scope)
  | .succeed member =>
      if member ∈ scope.members then
        (NobleFoundation.TaskInterleaving.step (.complete member.id member.generation true) table).map
          (fun next => (next, scope))
      else none
  | .fail member =>
      if member ∈ scope.members then
        (NobleFoundation.TaskInterleaving.step (.complete member.id member.generation false) table).map
          (fun next => (next, noteFailure scope member))
      else none
  | .deliver member =>
      if member ∈ scope.members ∧ scope.nextUndelivered = some member then
        (NobleFoundation.TaskInterleaving.step (.deliver member.id member.generation) table).map
          (fun next => (next, { scope with delivered := scope.delivered ++ [member] }))
      else none
  | .cancel member =>
      if member ∈ scope.members ∧ member ∉ scope.cancelled then
        (NobleFoundation.TaskInterleaving.step (.cancel member.id member.generation) table).map
          (fun next => (next, noteFailure
            { scope with cancelled := scope.cancelled ++ [member] } member))
      else none
  | .wake member =>
      if member ∈ scope.members then
        (NobleFoundation.TaskInterleaving.step (.wake member.id member.generation) table).map
          (fun next =>
            (next, if table.failure.isNone && next.failure.isSome then noteFailure scope member
              else scope))
      else none
  | .takeWake member =>
      if member ∈ scope.members then
        (NobleFoundation.TaskInterleaving.step (.takeWake member.id member.generation) table).map
          (fun next => (next, scope))
      else none
  | .stopped member =>
      if member ∈ scope.members ∧ member ∉ scope.stopped then
        some (table, { scope with stopped := scope.stopped ++ [member] })
      else none
  | .settlePins member =>
      if member ∈ scope.stopped ∧ member ∉ scope.pinsSettled then
        some (table, { scope with pinsSettled := scope.pinsSettled ++ [member] })
      else none
  | .clean member =>
      if member ∈ scope.pinsSettled ∧ member ∉ scope.cleaned then
        some (table, { scope with cleaned := scope.cleaned ++ [member] })
      else none
  | .finish member =>
      if member ∈ scope.members ∧ member ∈ scope.cleaned ∧ member ∉ scope.finalized then
        (NobleFoundation.TaskInterleaving.step (.finish member.id member.generation) table).map
          (fun next => (next, { scope with finalized := scope.finalized ++ [member] }))
      else none
  | .release member =>
      if releasable scope member then
        some (table, { scope with released := scope.released ++ [member] })
      else none
  | .close =>
      if closable scope then some (table, { scope with closed := true }) else none

/-- A step list applied to a table and a scope; a refused step refuses the whole list. -/
def stepThrough : List Step → Table → Scope → Option (Table × Scope)
  | [], table, scope => some (table, scope)
  | action :: rest, table, scope =>
      (step action table scope).bind (fun next => stepThrough rest next.1 next.2)

/-- An empty #46 table with the scope study's declared task capacity. -/
def emptyTable (capacity : Nat) : Table :=
  { capacity, left := absentSlot, right := absentSlot, failure := none, deliveries := [], settled := [] }

/-- The scope pilot's explicit reference alphabet: one scope, two possible child identities, table
    admission/completion/delivery/cancellation/wakeup, and separate host native-stop/pin inputs. -/
def scopeAlphabet : List Step :=
  [ .admission left1, .attach left1, .admission right2, .attach right2,
    .succeed left1, .fail left1, .deliver left1, .cancel left1,
    .succeed right2, .deliver right2, .cancel right2,
    .wake left1, .takeWake left1, .stopped left1, .settlePins left1 ]

/-- The scope-policy search is deliberately bounded to three actions from the empty one-capacity
    table. Longer lifecycle obligations are kernel-checked as named traces below. -/
def scopeDepth : Nat := 3

def scopeSequencesOfLength (alphabet : List Step) : Nat → List (List Step)
  | 0 => [[]]
  | depth + 1 =>
      (scopeSequencesOfLength alphabet depth).flatMap
        (fun lead => alphabet.map (fun action => action :: lead))

def scopeSequencesUpTo (alphabet : List Step) : Nat → List (List Step)
  | 0 => [[]]
  | depth + 1 =>
      scopeSequencesOfLength alphabet (depth + 1) ++ scopeSequencesUpTo alphabet depth

/-- Every declared bounded sequence has one reached scope/table state or is refused. -/
def scopeBoundedHistories : List (Table × Scope) :=
  (scopeSequencesUpTo scopeAlphabet scopeDepth).filterMap
    (fun actions => stepThrough actions (emptyTable 1) (emptyScope 7 3))

/-- Counts every declared scope sequence before refusals; no reduction is used. -/
def scopeBoundedSequenceCount : Nat := (scopeSequencesUpTo scopeAlphabet scopeDepth).length

/-- Counts the traces that reach a state under the scope and #46 table guards. -/
def scopeBoundedHistoryCount : Nat := scopeBoundedHistories.length

/-- The exact bounded completion-order controls: only completion order changes; result delivery is
    serialized in attachment order. -/
def resultTraceLeftFirst : List Step :=
  [ .admission left1, .attach left1, .admission right2, .attach right2,
    .succeed left1, .succeed right2, .deliver left1, .deliver right2 ]

def resultTraceRightFirst : List Step :=
  [ .admission left1, .attach left1, .admission right2, .attach right2,
    .succeed right2, .succeed left1, .deliver left1, .deliver right2 ]

/-- A cancelled child's late success cannot create a result or replace the primary failure. -/
def cancelledLateSuccessTrace : List Step :=
  [ .admission left1, .attach left1, .cancel left1, .succeed left1 ]

/-- A completed sibling remains reportable while a cancelled child's late result stays retired. -/
def completedSiblingCancelledTrace : List Step :=
  [ .admission left1, .attach left1, .admission right2, .attach right2,
    .succeed left1, .cancel right2, .fail right2, .deliver left1 ]

/-- Cancellation may precede the first sibling's completion without losing its later result. -/
def cancelledSiblingBeforeCompletionTrace : List Step :=
  [ .admission left1, .attach left1, .admission right2, .attach right2,
    .cancel right2, .succeed left1, .fail right2, .deliver left1 ]

/-- A completed member keeps the scope open until native stop, pins, cleanup, finish and release. -/
def fullySettledTrace : List Step :=
  [ .admission left1, .attach left1, .succeed left1, .deliver left1,
    .stopped left1, .settlePins left1, .clean left1, .finish left1,
    .release left1, .close ]

/-- A live child's first wake consumes one bounded reservation without ending its task. -/
def wakeReservationTrace : List Step :=
  [ .admission left1, .attach left1, .wake left1 ]

/-- Cancellation releases the queued wake reservation but leaves the task pinned and retiring. -/
def cancelledWakeReservationTrace : List Step := wakeReservationTrace ++ [.cancel left1]

/-- The #46 model's already-declared sequence enumerator remains the table-level reference. -/
def taskInterleavingReferenceSequences : List (List Action) :=
  TaskInterleaving.sequencesUpTo TaskInterleaving.exploreAlphabet scopeDepth

/-- The documented report of one scope state: the policy bookkeeping plus the table observables the
    adapter also records. -/
structure Report where
  scopeIdentity : Nat
  tableIdentity : Nat
  issued : List Member
  members : List Member
  results : List Member
  cancelled : List Member
  failure : Option Member
  finalized : List Member
  released : List Member
  closed : Bool
  leftLifecycle : Lifecycle
  rightLifecycle : Lifecycle
  leftPinned : Bool
  rightPinned : Bool
  leftRetiring : Bool
  rightRetiring : Bool
  failures : Option Nat
  deliveries : List (TaskId × Nat)
  settlements : List (TaskId × Nat)
deriving DecidableEq, Repr

/-- The report of one reached state. -/
def report (table : Table) (scope : Scope) : Report :=
  { scopeIdentity := scope.identity
    tableIdentity := scope.tableIdentity
    issued := scope.issued
    members := scope.members
    results := scope.results
    cancelled := scope.cancelled
    failure := scope.failure
    finalized := scope.finalized
    released := scope.released
    closed := scope.closed
    leftLifecycle := table.left.lifecycle
    rightLifecycle := table.right.lifecycle
    leftPinned := table.left.pinHeld
    rightPinned := table.right.pinHeld
    leftRetiring := table.left.retiring
    rightRetiring := table.right.retiring
    failures := table.failure
    deliveries := table.deliveries
    settlements := table.settled }

/-! ## Structural obligations -/

/-- Results are a sublist of the retained membership, so their order is the attachment order and no
    completion order can reorder them. -/
theorem results_sublist_members (scope : Scope) : scope.results.Sublist scope.members :=
  List.filter_sublist

/-- A reported result is a member: nothing outside the retained membership is reported. -/
theorem results_mem_members (scope : Scope) (member : Member) (held : member ∈ scope.results) :
    member ∈ scope.members :=
  (List.mem_filter.1 held).1

/-- A reported result has already passed the table-delivery gate. -/
theorem results_mem_delivered (scope : Scope) (member : Member) (held : member ∈ scope.results) :
    member ∈ scope.delivered :=
  of_decide_eq_true (List.mem_filter.1 held).2

/-- Two children with distinct references report distinct results: the filter cannot duplicate a
    member that membership holds once. -/
theorem results_nodup (scope : Scope) (nodup : scope.members.Nodup) : scope.results.Nodup :=
  (results_sublist_members scope).nodup nodup

/-- **Transition composition**: a scope step that carries a table action takes that exact imported
    transition. The scope adds policy; it never substitutes its own table decision. -/
theorem table_actions_are_composed (action : Step) (carried : Action) (table next : Table)
    (scope scope' : Scope) (hTable : tableAction action = some carried)
    (hStep : step action table scope = some (next, scope')) :
    NobleFoundation.TaskInterleaving.step carried table = some next := by
  cases action <;> simp only [tableAction] at hTable
  all_goals cases hTable
  all_goals
    simp only [step] at hStep
    (repeat' split at hStep)
    all_goals
      try simp at hStep
      all_goals
        rcases hStep with ⟨reached, taken, firstEq, _⟩
        rw [firstEq] at taken
        exact taken

/-- A scope step that carries no table action keeps the table unchanged. -/
theorem host_observations_keep_the_table (action : Step) (table next : Table) (scope scope' : Scope)
    (noAction : tableAction action = none) (hStep : step action table scope = some (next, scope')) :
    next = table := by
  cases action <;> simp only [tableAction] at noAction
  all_goals (try cases noAction)
  all_goals
    simp only [step] at hStep
    (repeat' split at hStep) <;> simp_all

/-- Close is refused while any member is unsettled: final settlement is a retained fact, not a
    report the host may assert. -/
theorem close_requires_settlement (table : Table) (scope : Scope) (unclosed : scope.closed = false)
    (unsettled : settled scope = false) : step .close table scope = none := by
  simp [step, closable, unclosed, unsettled]

/-- A member's membership is released once: a repeated release is refused rather than charged
    again. -/
theorem release_is_released_once (table : Table) (scope : Scope) (member : Member)
    (released : member ∈ scope.released) : step (.release member) table scope = none := by
  simp [step, releasable, released]

/-- Duplicate attachment is refused: a child already attached cannot be attached twice. -/
theorem duplicate_attachment_is_refused (table : Table) (scope : Scope) (member : Member)
    (capabilityConsumed : member ∉ scope.issued) : step (.attach member) table scope = none := by
  simp [step, accepts, capabilityConsumed]

/-- The two-child bound is enforced by the scope, not by the table's capacity. -/
theorem attachment_is_bounded (table : Table) (scope : Scope) (member : Member)
    (full : scope.members.length = 2) : step (.attach member) table scope = none := by
  simp [step, accepts, full]

/-- A closed scope refuses attachment: membership cannot be created after close. -/
theorem closed_scopes_refuse_attachment (table : Table) (scope : Scope) (member : Member)
    (closed : scope.closed = true) : step (.attach member) table scope = none := by
  simp [step, accepts, closed]

/-- A copied descriptor from another scope cannot mint a member in this scope. -/
theorem foreign_scope_admission_is_refused (table : Table) :
    step (.admission foreignScopeLeft1) table (emptyScope 7 3) = none := by
  simp [step, canAdmit, foreignScopeLeft1, emptyScope]

/-- A descriptor bound to another table cannot enter this scope's membership. -/
theorem foreign_table_admission_is_refused (table : Table) :
    step (.admission foreignTableLeft1) table (emptyScope 7 3) = none := by
  simp [step, canAdmit, foreignTableLeft1, emptyScope]

/-- An old slot generation is not a newly issued move-only child value. -/
theorem stale_generation_admission_is_refused (table : Table) :
    step (.admission left1) table { emptyScope 7 3 with lastGeneration := 1 } = none := by
  simp [step, canAdmit, left1, emptyScope]

/-- Fabricated membership is refused: a child that was never attached cannot deliver, finish or
    release. -/
theorem fabricated_membership_is_refused (table : Table) (scope : Scope) (member : Member)
    (notIssued : member ∉ scope.issued) (notMember : member ∉ scope.members) :
    step (.deliver member) table scope = none ∧ step (.finish member) table scope = none ∧
      step (.release member) table scope = none ∧ step (.attach member) table scope = none := by
  simp [step, releasable, accepts, notIssued, notMember]

/-- The recorded primary failure is retained: once a member is the primary failure, no later step
    replaces it, and a later failing completion or wake exhaustion leaves it in place. -/
theorem primary_failure_is_retained (action : Step) (table next : Table) (scope scope' : Scope)
    (member : Member) (recorded : scope.failure = some member)
    (hStep : step action table scope = some (next, scope')) :
    scope'.failure = some member := by
  cases action <;> simp only [step] at hStep
  all_goals
    repeat' split at hStep
    all_goals
      try simp at hStep
      all_goals
        first
        | (rcases hStep with ⟨_, _, _, scopeEq⟩
           rw [← scopeEq]
           simp [noteFailure, recorded])
        | (rcases hStep with ⟨_, scopeEq⟩
           rw [← scopeEq]
           simp [recorded])

set_option maxRecDepth 100000 in
set_option maxHeartbeats 20000000 in
/-- The full scope alphabet up to the declared depth has a kernel-checked finite count. -/
theorem scope_bounded_sequence_count : scopeBoundedSequenceCount = 3616 := by decide

set_option maxRecDepth 100000 in
set_option maxHeartbeats 20000000 in
/-- The one-capacity scope/table search has nine reachable prefixes at the declared bound; refused
    admissions and lifecycle actions do not create phantom states. -/
theorem scope_bounded_history_count : scopeBoundedHistoryCount = 9 := by decide

/-- The imported #46 enumerator remains the table-level reference at the same depth. -/
theorem task_interleaving_reference_sequence_count :
    taskInterleavingReferenceSequences.length = 3616 := by
  simpa [taskInterleavingReferenceSequences, scopeDepth] using
    TaskInterleaving.the_declared_sequence_counts.1

set_option maxRecDepth 100000 in
set_option maxHeartbeats 20000000 in
/-- The in-bound wake trace is part of the declared alphabet and consumes its one reserved wake. -/
theorem wake_reservation_trace_is_in_declared_bound :
    wakeReservationTrace ∈ scopeSequencesUpTo scopeAlphabet scopeDepth := by decide

/-- The bounded wake interleaving retains shared task capacity while reserving one wake. -/
theorem wake_reservation_keeps_the_shared_task_live :
    (stepThrough wakeReservationTrace (emptyTable 1) (emptyScope 7 3)).map
        (fun pair =>
          (pair.1.left.reserved, pair.1.left.wakeups, pair.1.right.reserved,
            TaskInterleaving.liveCount pair.1)) = some (1, 0, 0, 1) := by decide

/-- Cancellation releases that wake reservation without releasing the live task or its pin. -/
theorem cancellation_releases_wakeup_reservation_but_not_retirement_capacity :
    (stepThrough cancelledWakeReservationTrace (emptyTable 1) (emptyScope 7 3)).map
        (fun pair =>
          (pair.1.left.reserved, pair.1.left.wakeups, pair.1.left.pinHeld,
            pair.1.left.retiring, TaskInterleaving.liveCount pair.1)) =
      some (0, 0, true, true, 1) := by decide

/-- Distinct child identities do not make a shared-capacity admission and slot release independent.
    This reuses the #46 relation rather than postulating a scope-specific reduction. -/
theorem distinct_child_capacity_actions_are_not_independent :
    TaskInterleaving.independent (.acquire .right 2) (.finish .left 1) = false := by decide

/-- The #46 shared-capacity witness is retained verbatim as the dependency control for the scope
    layer. -/
theorem imported_distinct_id_control_is_retained :
    (TaskInterleaving.step (.acquire .right 1) TaskInterleaving.baseline).bind
        (TaskInterleaving.step (.finish .left 1)) = none ∧
      ((TaskInterleaving.step (.finish .left 1) TaskInterleaving.baseline).bind
        (TaskInterleaving.step (.acquire .right 1))).isSome = true :=
  TaskInterleaving.different_ids_are_not_independent

/-- Either normal child-completion order yields the same attachment-ordered result and table
    delivery record. A later ready child waits for the earlier member's delivery. -/
theorem normal_completion_orders_report_attachment_order :
    (stepThrough resultTraceLeftFirst (emptyTable 2) (emptyScope 7 3)).map
        (fun pair => (pair.2.results, pair.1.deliveries)) =
      some ([left1, right2], [(.left, 1), (.right, 2)]) ∧
    (stepThrough resultTraceRightFirst (emptyTable 2) (emptyScope 7 3)).map
        (fun pair => (pair.2.results, pair.1.deliveries)) =
      some ([left1, right2], [(.left, 1), (.right, 2)]) := by decide

/-- Normal scope traces project to the existing #46 table-level reachability function. -/
theorem normal_completion_traces_reach_the_imported_table :
    (stepThrough resultTraceLeftFirst (emptyTable 2) (emptyScope 7 3)).map Prod.fst =
        TaskInterleaving.reaches (resultTraceLeftFirst.filterMap tableAction) (emptyTable 2) ∧
      (stepThrough resultTraceRightFirst (emptyTable 2) (emptyScope 7 3)).map Prod.fst =
        TaskInterleaving.reaches (resultTraceRightFirst.filterMap tableAction) (emptyTable 2) := by
  decide

/-- A later ready child cannot be delivered while the earliest attached child remains pending. -/
theorem later_ready_member_delivery_is_refused :
    (stepThrough [.admission left1, .attach left1, .admission right2, .attach right2,
      .succeed right2] (emptyTable 2) (emptyScope 7 3)).bind
      (fun pair => step (.deliver right2) pair.1 pair.2) = none := by decide

/-- Completion alone is not delivery: the scope reports no result until the imported table accepts
    the member's delivery action. -/
theorem completion_does_not_report_before_delivery :
    (stepThrough [.admission left1, .attach left1, .succeed left1]
      (emptyTable 1) (emptyScope 7 3)).map (fun pair => pair.2.results) = some [] ∧
    (stepThrough [.admission left1, .attach left1, .succeed left1, .deliver left1]
      (emptyTable 1) (emptyScope 7 3)).map (fun pair => pair.2.results) = some [left1] := by decide

/-- Cancellation acknowledges revocation without becoming a result or final settlement; a late
    successful completion leaves the primary failure and the pin-retaining retiring state intact. -/
theorem late_success_after_cancellation_is_not_a_result :
    (stepThrough cancelledLateSuccessTrace (emptyTable 1) (emptyScope 7 3)).map
      (fun pair =>
        (pair.2.failure, pair.2.results, pair.2.cancelled,
          pair.1.left.pinHeld, pair.1.left.retiring, settled pair.2)) =
      some (some left1, [], [left1], true, true, false) := by decide

/-- A completed first child is still delivered in order; a cancelled second child's late failure
    remains the first group failure and cannot become a result. -/
theorem completed_sibling_survives_cancelled_late_completion :
    (stepThrough completedSiblingCancelledTrace (emptyTable 2) (emptyScope 7 3)).map
      (fun pair =>
        (pair.2.results, pair.2.cancelled, pair.2.failure,
          pair.1.right.pinHeld, pair.1.right.retiring)) =
      some ([left1], [right2], some right2, true, true) := by decide

/-- The same attachment result and retired ownership hold when cancellation precedes the other
    child's completion. -/
theorem cancellation_before_sibling_completion_preserves_result_and_retirement :
    (stepThrough cancelledSiblingBeforeCompletionTrace (emptyTable 2) (emptyScope 7 3)).map
      (fun pair =>
        (pair.2.results, pair.2.cancelled, pair.2.failure,
          pair.1.right.pinHeld, pair.1.right.retiring)) =
      some ([left1], [right2], some right2, true, true) := by decide

/-- A capacity refusal of the second child does not alter the already attached first child. The
    table's `none` is a refusal, not a rollback of the retained first member. -/
theorem refused_second_admission_keeps_first_member :
    (stepThrough [.admission left1, .attach left1]
      (emptyTable 1) (emptyScope 7 3)).map (fun pair => pair.2.members) = some [left1] ∧
    (stepThrough [.admission left1, .attach left1]
      (emptyTable 1) (emptyScope 7 3)).bind
        (fun pair => step (.admission right2) pair.1 pair.2) = none := by decide

/-- Only one scope-issued admission may be outstanding before attachment or abandonment. -/
theorem second_unattached_admission_is_refused :
    (stepThrough [.admission left1] (emptyTable 2) (emptyScope 7 3)).bind
        (fun pair => step (.admission right2) pair.1 pair.2) = none := by decide

/-- A refusal receipt is observationally a no-op for both table and scope; the runtime adapter also
    checks that its generic caller input is returned unchanged. -/
theorem explicit_refusal_keeps_scope_state (table : Table) (scope : Scope) (member : Member) :
    step (.refuse member) table scope = some (table, scope) := rfl

/-- The move-only admitted value cannot be attached twice after its first attachment consumed it. -/
theorem duplicate_attachment_control :
    stepThrough [.admission left1, .attach left1, .attach left1]
      (emptyTable 1) (emptyScope 7 3) = none := by decide

/-- A fabricated descriptor without a successful scope admission cannot be attached. -/
theorem fabricated_attachment_control :
    step (.attach left1) (emptyTable 1) (emptyScope 7 3) = none := by decide

/-- Post-close admission is refused even if another slot and generation would otherwise fit. -/
theorem post_close_admission_control :
    stepThrough (fullySettledTrace ++ [.admission right2])
      (emptyTable 2) (emptyScope 7 3) = none := by decide

/-- Close is idempotent: the completed trace and a repeated close retain one release. -/
theorem repeated_close_is_idempotent :
    (stepThrough (fullySettledTrace ++ [.close])
      (emptyTable 2) (emptyScope 7 3)).map
        (fun pair => (pair.2.closed, pair.2.released)) = some (true, [left1]) := by decide

/-- An early close is refused while a child has not passed finish and membership release. -/
theorem early_close_is_refused :
    (stepThrough [.admission left1, .attach left1]
      (emptyTable 1) (emptyScope 7 3)).bind
        (fun pair => step .close pair.1 pair.2) = none := by decide

end NobleFoundation.TaskScope
