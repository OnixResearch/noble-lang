import M5Resources
import NobleFoundation.RegionProtocol

open Aeneas Aeneas.Std
open noble_kernel
open NobleFoundation

set_option maxHeartbeats 2000000
set_option maxRecDepth 4096

namespace RegionTable

/-!
# T4: the `ownership::Table` transitions of the bounded-region adapter refine `RegionProtocol-v1`

Obligation T4 of `.cairn/changes/behavioral-effect-protocol`, through the M5 lane. The subject is the
pinned M5 extraction of `noble_kernel::resources`, the crate `bounded_region.rs` imports as
`ownership`; the lane binds its six decision source files byte for byte, and
`M5Resources.transition_refines` and `M5Resources.table_decide_refines` make `M5Resources.step` its
decision function.

The adapter creates a one-slot table (`Table::new`, then `register`) and calls four table methods: a
borrowed read is `begin`, then `native_access`, then `complete` with the outcome `native_access`
selects; a guest release, a destructor and the host's cleanup are all `release`. On the extracted
methods:

* `registration_represents`: the adapter's construction yields a table that represents the model's
  initial state (`Represents`): one well-formed record standing for the live owner, the scope counter
  at zero with the model's scope limit as its ceiling, and no pin outstanding.
* `begin_admits`, `begin_exhausted`, `open_borrow_returns`, `release_table`, `begin_retired_table`:
  below the ceiling, `begin` admits the borrow under the next serial and stores the opened record; at
  the ceiling it refuses with `ScopeExhausted`. `native_access` authorizes the open borrow and
  `complete` returns the owner for either outcome, so the model's `complete` oracle (A4) is `true` for
  the table and its `lost` state is unreachable. `release` retires a live owner and is refused on a
  retired one.
* `calls_refine`, `cleanup_refines`, `exec_refines`, `run_refines`: under the host the table induces
  (`tableHost`), the adapter's table calls for every guest request, every execution and every run with
  the host's cleanup keep the table representing the model state. The scope counter stays the model's
  `granted`, and the table's own `local_releases` accounting (`accounting_local_releases`) equals the
  model's release commits.
* `table_retires_exactly_once`: P1 and the cleanup claim therefore hold for the table itself. From a
  live owner, every run ends with the record retired, no pin outstanding and exactly one local release
  accounted.

Premises that stay premises:
* The per-request call sequence (`tableCalls`) is read off `bounded_region.rs`. That the Rust host
  follows it and performs the identity and bounds checks the model states is T5, which stays open.
* The `lower` oracle is the guest lowering, which belongs to T6 and stays open.
* Wasmtime, the Canonical ABI and the CLI wrapper are not modeled.

This is a theorem about the pinned extracted table code and the model only.
-/

/-- The requirement the adapter presents with every claim: the one it registered the owner under,
which is the owner handle's own context, kind and rights (`registration_represents`). -/
def requirementFor (handle : resources.Handle) : resources.Requirement :=
  ⟨handle.context, handle.kind, handle.rights⟩

/-- The record while the borrow with scope serial `serial` is open. -/
def opened (record : resources.Snapshot) (serial : U64) : resources.Snapshot :=
  { record with state := .Busy serial, last_scope := some serial }

/-- The record after the borrow `serial` returned its owner. -/
def returned (record : resources.Snapshot) (serial : U64) : resources.Snapshot :=
  { record with last_scope := some serial }

/-- The record after a release. -/
def released (record : resources.Snapshot) : resources.Snapshot :=
  { record with state := .Retired }

@[simp] theorem released_state (record : resources.Snapshot) : (released record).state = .Retired := rfl

@[simp] theorem opened_handle (record : resources.Snapshot) (serial : U64) :
    (opened record serial).handle = record.handle := rfl

@[simp] theorem returned_handle (record : resources.Snapshot) (serial : U64) :
    (returned record serial).handle = record.handle := rfl

@[simp] theorem released_handle (record : resources.Snapshot) : (released record).handle = record.handle := rfl

/-! ## The decisions -/

theorem opened_valid (record : resources.Snapshot) (serial : U64)
    (wellFormed : M5Resources.valid record = true) (live : record.state = .Live)
    (nonzero : serial ≠ 0#u64) :
    M5Resources.valid (opened record serial) = true := by
  simp [M5Resources.valid, live] at wellFormed
  simp [M5Resources.valid, opened, wellFormed, nonzero]

theorem access_opened (record : resources.Snapshot) (serial : U64)
    (wellFormed : M5Resources.valid record = true) (live : record.state = .Live)
    (nonzero : serial ≠ 0#u64) :
    M5Resources.step (opened record serial) record.handle (.Access serial) =
      .Ok ⟨opened record serial, .Unchanged⟩ := by
  have openValid := opened_valid record serial wellFormed live nonzero
  simp only [M5Resources.step, openValid, ite_true]
  simp [opened, M5Resources.afterCheck, M5Resources.applyEvent, M5Resources.access,
    M5Resources.scopeCheck, nonzero, M5Resources.unchanged]

theorem complete_opened (record : resources.Snapshot) (serial : U64) (outcome : resources.Completion)
    (wellFormed : M5Resources.valid record = true) (live : record.state = .Live)
    (nonzero : serial ≠ 0#u64) :
    M5Resources.step (opened record serial) record.handle (.Complete serial outcome) =
      .Ok ⟨returned record serial, .OwnerReturned outcome⟩ := by
  have openValid := opened_valid record serial wellFormed live nonzero
  simp only [M5Resources.step, openValid, ite_true]
  cases record
  simp_all [opened, returned, M5Resources.afterCheck, M5Resources.applyEvent, M5Resources.complete,
    M5Resources.scopeCheck]

theorem returned_valid (record : resources.Snapshot) (serial : U64)
    (wellFormed : M5Resources.valid record = true) (live : record.state = .Live)
    (nonzero : serial ≠ 0#u64) :
    M5Resources.valid (returned record serial) = true := by
  simp [M5Resources.valid, live] at wellFormed
  simp [M5Resources.valid, returned, wellFormed, nonzero, live]

theorem release_live (record : resources.Snapshot)
    (wellFormed : M5Resources.valid record = true) (live : record.state = .Live) :
    M5Resources.step record record.handle (.Release (requirementFor record.handle)) =
      .Ok ⟨released record, .LocalRelease⟩ := by
  simp [M5Resources.step, wellFormed, M5Resources.afterCheck, M5Resources.applyEvent,
    M5Resources.release, M5Resources.liveCheck, requirementFor, live, released]

theorem release_retired (record : resources.Snapshot)
    (wellFormed : M5Resources.valid record = true) (retired : record.state = .Retired) :
    M5Resources.step record record.handle (.Release (requirementFor record.handle)) = .Err .Retired := by
  simp [M5Resources.step, wellFormed, M5Resources.afterCheck, M5Resources.applyEvent,
    M5Resources.release, M5Resources.liveCheck, requirementFor, retired]

theorem begin_retired (record : resources.Snapshot) (serial : U64)
    (wellFormed : M5Resources.valid record = true) (retired : record.state = .Retired) :
    M5Resources.step record record.handle (.Begin (requirementFor record.handle) serial) =
      .Err .Retired := by
  simp [M5Resources.step, wellFormed, M5Resources.afterCheck, M5Resources.applyEvent,
    M5Resources.begin, M5Resources.liveCheck, requirementFor, retired]

theorem released_valid (record : resources.Snapshot)
    (wellFormed : M5Resources.valid record = true) :
    M5Resources.valid (released record) = true := by
  unfold M5Resources.valid at wellFormed ⊢
  simp only [released]
  split at wellFormed
  · simp at wellFormed
  · rename_i generation
    simp only [generation, ite_false]
    split at wellFormed
    · simp at wellFormed
    · rename_i scope
      simp [scope]

/-- A scope serial the record has not used: nonzero and above every serial it has seen. -/
def Fresh (record : resources.Snapshot) (serial : U64) : Prop :=
  serial ≠ 0#u64 ∧ ∀ previous, record.last_scope = some previous → previous.val < serial.val

theorem begin_fresh (record : resources.Snapshot) (serial : U64)
    (wellFormed : M5Resources.valid record = true) (live : record.state = .Live)
    (fresh : Fresh record serial) :
    M5Resources.step record record.handle (.Begin (requirementFor record.handle) serial) =
      .Ok ⟨opened record serial, .BorrowAdmitted⟩ := by
  obtain ⟨nonzero, above⟩ := fresh
  simp [M5Resources.step, wellFormed, M5Resources.afterCheck, M5Resources.applyEvent,
    M5Resources.begin, M5Resources.liveCheck, requirementFor, live, nonzero, opened]
  cases scope : record.last_scope with
  | none => simp
  | some previous =>
      have below := above previous scope
      simp
      omega

/-! ## The adapter's one-slot table -/

/-- The protocol owner a table record state stands for at a guest boundary. A guest boundary never
observes `busy` (`RegionProtocol.step_not_busy`); the table never refuses the completion of an admitted
borrow (`open_borrow_returns`), so `lost` has no table counterpart; and the table does not record who
released. -/
def corresponds : RegionProtocol.Owner → resources.State → Bool
  | .live, .Live => true
  | .released _, .Retired => true
  | _, _ => false

/-- The adapter's one-slot table in the protocol state `s`, for the registered owner `handle`: its
only record carries that handle, is well formed and stands for the owner; the scope counter is the
number of granted borrows, its ceiling is the model's scope limit, and no serial the record has seen
exceeds it; no pin is outstanding and the pin limit admits one. -/
structure Represents (handle : resources.Handle) (s : RegionProtocol.State)
    (table : resources.table.Table) (record : resources.Snapshot) : Prop where
  records : table.records.val = [record]
  owned : record.handle = handle
  slot : handle.slot.val = 0
  wellFormed : M5Resources.valid record = true
  owner : corresponds s.owner record.state = true
  scope : table.scope.val = s.granted
  ceiling : table.limits.scopes.val = RegionProtocol.scopeLimit
  unpinned : table.pins.val = 0
  pinRoom : 0 < table.limits.pins.val
  seen : ∀ previous, record.last_scope = some previous → previous.val ≤ table.scope.val

theorem selected_of_records (table : resources.table.Table) (record : resources.Snapshot)
    (records : table.records.val = [record]) (slot : record.handle.slot.val = 0) :
    table.records.val[record.handle.slot.val]? = some record := by
  simp [records, slot]

theorem commit_admitted (table : resources.table.Table) (decision : resources.Decision)
    (record : resources.Snapshot) (records : table.records.val = [record])
    (slot : decision.record.handle.slot.val = 0) (admitted : decision.action = .BorrowAdmitted)
    (room : table.pins.val < table.limits.pins.val) :
    ∃ committed, resources.table.Table.commit table decision = Result.ok (.Ok (), committed) ∧
      committed.records.val = [decision.record] ∧ committed.pins.val = table.pins.val + 1 ∧
      committed.scope = table.scope ∧ committed.limits = table.limits := by
  obtain ⟨decided, action⟩ := decision
  simp only at admitted slot
  subst admitted
  have notFull : ¬ (table.pins >= table.limits.pins) := by simp; omega
  have spec := Usize.checked_add_bv_spec table.pins 1#usize
  have sliceVal : table.records.slice.val = [record] := records
  simp only [resources.table.Table.commit, notFull, ite_false]
  split at spec
  · rename_i pins added
    simp [added, alloc.vec.Vec.deref_mut, core.slice.Slice.get_mut, lift, sliceVal, slot]
    refine ⟨by simp [alloc.vec.Vec.val, sliceVal, slot], ?_⟩
    simp at spec
    omega
  · have : table.limits.pins.val ≤ Usize.max := by scalar_tac
    simp at spec
    omega

theorem commit_returned (table : resources.table.Table) (decision : resources.Decision)
    (record : resources.Snapshot) (outcome : resources.Completion)
    (records : table.records.val = [record]) (slot : decision.record.handle.slot.val = 0)
    (returnedOwner : decision.action = .OwnerReturned outcome) (pinned : 0 < table.pins.val) :
    ∃ committed, resources.table.Table.commit table decision = Result.ok (.Ok (), committed) ∧
      committed.records.val = [decision.record] ∧ committed.pins.val = table.pins.val - 1 ∧
      committed.scope = table.scope ∧ committed.limits = table.limits := by
  obtain ⟨decided, action⟩ := decision
  simp only at returnedOwner slot
  subst returnedOwner
  have spec := Usize.checked_sub_bv_spec table.pins 1#usize
  have sliceVal : table.records.slice.val = [record] := records
  simp only [resources.table.Table.commit]
  split at spec
  · rename_i pins subtracted
    simp [subtracted, alloc.vec.Vec.deref_mut, core.slice.Slice.get_mut, lift, sliceVal, slot]
    refine ⟨by simp [alloc.vec.Vec.val, sliceVal, slot], ?_⟩
    simp at spec
    omega
  · simp at spec
    omega

theorem commit_released (table : resources.table.Table) (decision : resources.Decision)
    (record : resources.Snapshot) (records : table.records.val = [record])
    (slot : decision.record.handle.slot.val = 0) (localRelease : decision.action = .LocalRelease) :
    ∃ committed, resources.table.Table.commit table decision = Result.ok (.Ok (), committed) ∧
      committed.records.val = [decision.record] ∧ committed.pins = table.pins ∧
      committed.scope = table.scope ∧ committed.limits = table.limits := by
  obtain ⟨decided, action⟩ := decision
  simp only at localRelease slot
  subst localRelease
  have sliceVal : table.records.slice.val = [record] := records
  simp [resources.table.Table.commit, alloc.vec.Vec.deref_mut, core.slice.Slice.get_mut, lift,
    sliceVal, slot]
  simp [alloc.vec.Vec.val, sliceVal, slot]

/-- **The adapter's table.** `Table.new` with the adapter's limits (one slot, one pin, one owner per
context, one generation, and the model's scope limit as the scope ceiling), then `register`, yields the
owner and a table that represents the model's initial state. The requirement the adapter presents
afterwards, its registration requirement, is `requirementFor` of the owner's handle. -/
theorem registration_represents (id : resources.TableId) (required : resources.Requirement) :
    ∃ table, resources.table.Table.new id ⟨1#usize, 1#usize, 1#usize, 1#u64, 64#u64⟩ =
      Result.ok (.Ok table) ∧
    ∃ owner registered record,
      resources.table.Table.register table required = Result.ok (.Ok owner, registered) ∧
      Represents owner.handle RegionProtocol.initial registered record ∧
      requirementFor owner.handle = required := by
  refine ⟨_, by simp [resources.table.Table.new, resources.MAX_SLOTS]; rfl, ?_⟩
  simp only [resources.table.Table.register, resources.table.inspection.Table.free_slot,
    resources.table.inspection.Table.free_slot_loop, resources.table.inspection.Table.context_count,
    resources.table.inspection.Table.context_count_loop]
  rw [loop, loop]
  simp [resources.table.inspection.Table.free_slot_loop.body,
    resources.table.inspection.Table.context_count_loop.body, alloc.vec.Vec.with_capacity,
    resources.table.next]
  have gen := U64.checked_add_bv_spec 0#u64 1#u64
  split at gen
  · rename_i generation added
    have generationVal : generation.val = 1 := by simp at gen; omega
    have nonzero : generation ≠ 0#u64 := by intro zero; simp [zero] at generationVal
    have zero : (0#usize) = Usize.ofNatCore 0 (by simp) := rfl
    have room : 1 ≤ Usize.max := by scalar_tac
    simp [added, alloc.vec.Vec.push, ← zero, lift, room]
    refine ⟨⟨(⟨⟨id, 0#usize, generation, required.context, required.kind, required.rights⟩, .Live, none,
      none⟩ : resources.Snapshot), {
      records := by simp
      owned := rfl
      slot := by simp
      wellFormed := by simp [M5Resources.valid, nonzero]
      owner := by simp [corresponds, RegionProtocol.initial]
      scope := by simp [RegionProtocol.initial]
      ceiling := by simp [RegionProtocol.scopeLimit]
      unpinned := by simp
      pinRoom := by simp
      seen := by simp }⟩, ?_⟩
    cases required
    rfl
  · have : 0 < U64.max := by simp [U64.max, U64.numBits]
    simp at gen
    omega

/-- At the scope ceiling, `begin` refuses with `ScopeExhausted` and leaves the table. -/
theorem begin_exhausted (handle : resources.Handle) (s : RegionProtocol.State)
    (table : resources.table.Table) (record : resources.Snapshot)
    (represents : Represents handle s table record) (full : RegionProtocol.scopeLimit ≤ s.granted) :
    resources.table.access.Table.begin table ⟨handle⟩ (requirementFor handle) =
      Result.ok (.Err ⟨.ScopeExhausted, ⟨handle⟩⟩, table) := by
  have exhausted : table.scope >= table.limits.scopes := by
    simp only [ge_iff_le, UScalar.le_equiv, represents.scope, represents.ceiling]
    exact full
  simp [resources.table.access.Table.begin, resources.table.next, exhausted]

/-- Below the ceiling, `begin` on a live owner allocates the next serial, admits the borrow and
stores the opened record with one pin outstanding. -/
theorem begin_admits (handle : resources.Handle) (granted : Nat)
    (table : resources.table.Table) (record : resources.Snapshot)
    (represents : Represents handle ⟨.live, granted⟩ table record)
    (room : granted < RegionProtocol.scopeLimit) :
    ∃ serial opening,
      resources.table.access.Table.begin table ⟨handle⟩ (requirementFor handle) =
        Result.ok (.Ok ⟨⟨⟨handle, serial⟩⟩, ⟨opened record serial, .BorrowAdmitted⟩⟩, opening) ∧
      serial.val = granted + 1 ∧ opening.records.val = [opened record serial] ∧ opening.scope = serial ∧
      opening.pins.val = 1 ∧ opening.limits = table.limits := by
  have live : record.state = .Live := by
    have := represents.owner
    cases state : record.state <;> simp_all [corresponds]
  have below : ¬ (table.scope >= table.limits.scopes) := by
    simp only [ge_iff_le, UScalar.le_equiv, represents.scope, represents.ceiling]
    omega
  have scopeVal : table.scope.val = granted := represents.scope
  have ceilingVal : table.limits.scopes.val = 64 := represents.ceiling
  have below64 : granted < 64 := room
  have oneVal : (1#u64).val = 1 := rfl
  have spec := U64.checked_add_bv_spec table.scope 1#u64
  split at spec
  · rename_i serial added
    obtain ⟨-, serialVal, -⟩ := spec
    rw [oneVal] at serialVal
    have fresh : Fresh record serial := by
      refine ⟨?_, fun previous scope => ?_⟩
      · intro zero
        simp [zero] at serialVal
      · have := represents.seen previous scope
        omega
    have decided : resources.table.Table.decide table handle
        (.Begin (requirementFor handle) serial) = Result.ok (.Ok ⟨opened record serial, .BorrowAdmitted⟩) := by
      rw [M5Resources.table_decide_refines, ← represents.owned]
      simp only [M5Resources.tableStep,
        selected_of_records table record represents.records (represents.owned ▸ represents.slot)]
      exact congrArg Result.ok (begin_fresh record serial represents.wellFormed live fresh)
    obtain ⟨committed, commit, committedRecords, committedPins, committedScope, committedLimits⟩ :=
      commit_admitted table ⟨opened record serial, .BorrowAdmitted⟩ record represents.records
        (by simp [opened, represents.owned, represents.slot]) rfl
        (by rw [represents.unpinned]; exact represents.pinRoom)
    refine ⟨serial, { committed with scope := serial }, ?_, by omega, committedRecords, rfl, ?_,
      committedLimits⟩
    · simp [resources.table.access.Table.begin, resources.table.next, below, added, lift, decided,
        commit]
    · simp [committedPins, represents.unpinned]
  · have bound : table.scope.val + 1 ≤ U64.max := by
      have : (64 : Nat) < U64.max := by simp [U64.max, U64.numBits]
      omega
    rw [oneVal] at spec
    omega

/-- **A4 for the table.** A table holding the opened record: `native_access` authorizes the borrow,
and `complete` returns the owner for either outcome, stores the returned record and releases the
pin. -/
theorem open_borrow_returns (handle : resources.Handle) (opening : resources.table.Table)
    (record : resources.Snapshot) (serial : U64) (outcome : resources.Completion)
    (wellFormed : M5Resources.valid record = true) (live : record.state = .Live)
    (owned : record.handle = handle) (slot : handle.slot.val = 0) (nonzero : serial ≠ 0#u64)
    (records : opening.records.val = [opened record serial]) (pinned : opening.pins.val = 1) :
    resources.table.access.Table.native_access opening ⟨⟨handle, serial⟩⟩ =
        Result.ok (.Ok (opened record serial)) ∧
    ∃ closing,
      resources.table.access.Table.complete opening ⟨handle, serial⟩ outcome =
        Result.ok (.Ok ⟨some ⟨handle⟩, ⟨returned record serial, .OwnerReturned outcome⟩⟩, closing) ∧
      closing.records.val = [returned record serial] ∧ closing.pins.val = 0 ∧
      closing.scope = opening.scope ∧ closing.limits = opening.limits := by
  have selected : opening.records.val[handle.slot.val]? = some (opened record serial) := by
    simp [records, slot]
  have accessed : resources.table.Table.decide opening handle (.Access serial) =
      Result.ok (.Ok ⟨opened record serial, .Unchanged⟩) := by
    rw [M5Resources.table_decide_refines]
    simp only [M5Resources.tableStep, selected]
    exact congrArg Result.ok (owned ▸ access_opened record serial wellFormed live nonzero)
  have completed : resources.table.Table.decide opening handle (.Complete serial outcome) =
      Result.ok (.Ok ⟨returned record serial, .OwnerReturned outcome⟩) := by
    rw [M5Resources.table_decide_refines]
    simp only [M5Resources.tableStep, selected]
    exact congrArg Result.ok (owned ▸ complete_opened record serial outcome wellFormed live nonzero)
  obtain ⟨closing, commit, closingRecords, closingPins, closingScope, closingLimits⟩ :=
    commit_returned opening ⟨returned record serial, .OwnerReturned outcome⟩ (opened record serial)
      outcome records (by simp [returned, owned, slot]) rfl (by omega)
  refine ⟨?_, closing, ?_, closingRecords, by omega, closingScope, closingLimits⟩
  · simp [resources.table.access.Table.native_access, accessed]
  · simp [resources.table.access.Table.complete, completed, commit, owned]

/-- `release` on a live owner retires it and stores the retired record; on a retired record it is
refused and leaves the table. -/
theorem release_table (handle : resources.Handle) (s : RegionProtocol.State)
    (table : resources.table.Table) (record : resources.Snapshot)
    (represents : Represents handle s table record) :
    (record.state = .Live → ∃ retired,
      resources.table.access.Table.release table ⟨handle⟩ (requirementFor handle) =
        Result.ok (.Ok ⟨released record, .LocalRelease⟩, retired) ∧
      retired.records.val = [released record] ∧ retired.pins = table.pins ∧
      retired.scope = table.scope ∧ retired.limits = table.limits) ∧
    (record.state = .Retired →
      resources.table.access.Table.release table ⟨handle⟩ (requirementFor handle) =
        Result.ok (.Err ⟨.Retired, ⟨handle⟩⟩, table)) := by
  have selected := selected_of_records table record represents.records
    (represents.owned ▸ represents.slot)
  rw [represents.owned] at selected
  refine ⟨fun live => ?_, fun retired => ?_⟩
  · have decided : resources.table.Table.decide table handle (.Release (requirementFor handle)) =
        Result.ok (.Ok ⟨released record, .LocalRelease⟩) := by
      rw [M5Resources.table_decide_refines]
      simp only [M5Resources.tableStep, selected]
      exact congrArg Result.ok (represents.owned ▸ release_live record represents.wellFormed live)
    obtain ⟨retiredTable, commit, retiredRecords, retiredPins, retiredScope, retiredLimits⟩ :=
      commit_released table ⟨released record, .LocalRelease⟩ record represents.records
        (by simp [released, represents.owned, represents.slot]) rfl
    refine ⟨retiredTable, ?_, retiredRecords, retiredPins, retiredScope, retiredLimits⟩
    simp [resources.table.access.Table.release, resources.table.Table.apply, decided, commit]
  · have decided : resources.table.Table.decide table handle (.Release (requirementFor handle)) =
        Result.ok (.Err .Retired) := by
      rw [M5Resources.table_decide_refines]
      simp only [M5Resources.tableStep, selected]
      exact congrArg Result.ok (represents.owned ▸ release_retired record represents.wellFormed retired)
    simp [resources.table.access.Table.release, resources.table.Table.apply, decided]

/-- `begin` on a retired record is refused (at the ceiling or by the table) and leaves the table. -/
theorem begin_retired_table (handle : resources.Handle) (s : RegionProtocol.State)
    (table : resources.table.Table) (record : resources.Snapshot)
    (represents : Represents handle s table record) (retired : record.state = .Retired) :
    ∃ error, resources.table.access.Table.begin table ⟨handle⟩ (requirementFor handle) =
      Result.ok (.Err ⟨error, ⟨handle⟩⟩, table) := by
  by_cases full : RegionProtocol.scopeLimit ≤ s.granted
  · exact ⟨_, begin_exhausted handle s table record represents full⟩
  · have below : ¬ (table.scope >= table.limits.scopes) := by
      simp only [ge_iff_le, UScalar.le_equiv, represents.scope, represents.ceiling]
      omega
    have selected := selected_of_records table record represents.records
      (represents.owned ▸ represents.slot)
    rw [represents.owned] at selected
    have spec := U64.checked_add_bv_spec table.scope 1#u64
    split at spec
    · rename_i serial added
      have decided : resources.table.Table.decide table handle (.Begin (requirementFor handle) serial) =
          Result.ok (.Err .Retired) := by
        rw [M5Resources.table_decide_refines]
        simp only [M5Resources.tableStep, selected]
        exact congrArg Result.ok (represents.owned ▸ begin_retired record serial represents.wellFormed retired)
      exact ⟨.Retired, by simp [resources.table.access.Table.begin, resources.table.next, below, added,
        lift, decided]⟩
    · rename_i none
      exact ⟨.ScopeExhausted, by simp [resources.table.access.Table.begin, resources.table.next, below,
        none, lift]⟩

/-! ## The model link -/

/-- The model host the table induces: `access` and `complete` answer `true`. Below the scope limit,
which the model checks first and which is the table's scope ceiling (`Represents.ceiling`), `begin`
admits every borrow (`begin_admits`), and `complete` returns the owner of every admitted borrow
(`open_borrow_returns`). The region, the owner identity and the `lower` oracle stay the host's. -/
def tableHost (host : RegionProtocol.Host) : RegionProtocol.Host :=
  { host with access := fun _ => true, complete := fun _ => true }

/-- The local releases the table's accounting assigns to an action (`accounting_local_releases`). -/
def actionReleases : resources.Action → Nat
  | .LocalRelease => 1
  | .RetirementCompleted => 1
  | _ => 0

/-- `actionReleases` is the `local_releases` field of the extracted `Decision.accounting`. -/
theorem accounting_local_releases (decision : resources.Decision) :
    ∃ accounting, resources.Decision.accounting decision = Result.ok accounting ∧
      accounting.local_releases.val = actionReleases decision.action := by
  obtain ⟨record, action⟩ := decision
  cases action <;> exact ⟨_, rfl, by simp [actionReleases]⟩

/-- The completion outcome the adapter derives from `native_access`: `Success` when it authorized. -/
def authorizedOutcome : core.result.Result resources.Snapshot resources.Error → resources.Completion
  | .Ok _ => .Success
  | .Err _ => .DomainError

/-- The adapter's table calls for one borrowed read that passed the host's identity and bounds checks
(`read_region`): `begin`; once admitted, `native_access`, then `complete` with the outcome
`native_access` selects. The result is the table after the calls and the local releases their
decisions account. -/
def borrowedRead (table : resources.table.Table) (handle : resources.Handle) :
    Result (resources.table.Table × Nat) := do
  let (begun, opening) ← resources.table.access.Table.begin table ⟨handle⟩ (requirementFor handle)
  match begun with
  | .Err _ => Result.ok (opening, 0)
  | .Ok admitted =>
      let authorized ← resources.table.access.Table.native_access opening admitted.borrow
      let (completed, closing) ← resources.table.access.Table.complete opening admitted.borrow.scope
        (authorizedOutcome authorized)
      match completed with
      | .Ok value =>
          Result.ok (closing, actionReleases admitted.decision.action + actionReleases value.decision.action)
      | .Err _ => Result.ok (closing, actionReleases admitted.decision.action)

/-- The adapter's table call for a guest release, a destructor and the host's cleanup alike:
`release`. A release the host refuses before the table, because it no longer holds the owner, leaves
the same table as the refusal here. -/
def releaseCall (table : resources.table.Table) (handle : resources.Handle) :
    Result (resources.table.Table × Nat) := do
  let (result, after) ← resources.table.access.Table.release table ⟨handle⟩ (requirementFor handle)
  match result with
  | .Ok decision => Result.ok (after, actionReleases decision.action)
  | .Err _ => Result.ok (after, 0)

/-- The adapter's table calls for one guest request. A read reaches the table only after the host's
identity and bounds checks, and an owned receiver traps on its ABI shape before the host. -/
def tableCalls (host : RegionProtocol.Host) (handle : resources.Handle) (table : resources.table.Table) :
    RegionProtocol.Request → Result (resources.table.Table × Nat)
  | .read claim off len =>
      if claim = host.owner ∧ RegionProtocol.inBounds host.region.length off len = true then
        borrowedRead table handle
      else Result.ok (table, 0)
  | .readOwned => Result.ok (table, 0)
  | .release => releaseCall table handle
  | .drop => releaseCall table handle

/-- A read on a live owner under the table's host keeps the owner live, grants a borrow exactly when
the claim is the owner's, the range is in bounds and the scope limit is not reached, and commits no
release. -/
theorem read_step (host : RegionProtocol.Host) (granted claim : Nat) (off len : Int) :
    (RegionProtocol.step (tableHost host) ⟨.live, granted⟩ (.read claim off len)).next =
      ⟨.live, if claim = host.owner ∧ RegionProtocol.inBounds host.region.length off len = true ∧
        granted < RegionProtocol.scopeLimit then granted + 1 else granted⟩ ∧
    (RegionProtocol.step (tableHost host) ⟨.live, granted⟩ (.read claim off len)).events.countP
      RegionProtocol.isRelease = 0 := by
  simp only [RegionProtocol.step]
  unfold RegionProtocol.readStep
  (repeat' split) <;> simp_all [tableHost, RegionProtocol.isRelease]
  all_goals omega

/-- **T4, one request.** From a table that represents the model state, the adapter's table calls for
one guest request return a table that represents the model's next state under the table's host, and
the table's own accounting counts exactly the model's release commits. -/
theorem calls_refine (host : RegionProtocol.Host) (handle : resources.Handle) (s : RegionProtocol.State)
    (table : resources.table.Table) (record : resources.Snapshot) (request : RegionProtocol.Request)
    (represents : Represents handle s table record) :
    ∃ table' record',
      tableCalls host handle table request = Result.ok (table',
        (RegionProtocol.step (tableHost host) s request).events.countP RegionProtocol.isRelease) ∧
      Represents handle (RegionProtocol.step (tableHost host) s request).next table' record' := by
  obtain ⟨owner, granted⟩ := s
  have related := represents.owner
  cases owner <;> cases state : record.state <;> simp [corresponds, state] at related
  · cases request with
    | read claim off len =>
        obtain ⟨next, none⟩ := read_step host granted claim off len
        rw [next, none]
        by_cases checks : claim = host.owner ∧ RegionProtocol.inBounds host.region.length off len = true
        · by_cases room : granted < RegionProtocol.scopeLimit
          · obtain ⟨serial, opening, begun, serialVal, openRecords, openScope, openPins, openLimits⟩ :=
              begin_admits handle granted table record represents room
            have nonzero : serial ≠ 0#u64 := by intro zero; simp [zero] at serialVal
            obtain ⟨accessed, closing, completed, closingRecords, closingPins, closingScope,
              closingLimits⟩ := open_borrow_returns handle opening record serial .Success
                represents.wellFormed state represents.owned represents.slot nonzero openRecords openPins
            refine ⟨closing, returned record serial, ?_, ?_⟩
            · simp [tableCalls, checks, borrowedRead, begun, accessed, authorizedOutcome, completed,
                actionReleases]
            · simp only [checks, room, and_self, ite_true]
              exact {
                records := closingRecords
                owned := by simp [represents.owned]
                slot := represents.slot
                wellFormed := returned_valid record serial represents.wellFormed state nonzero
                owner := by simp [corresponds, returned, state]
                scope := by rw [closingScope, openScope]; exact serialVal
                ceiling := by rw [closingLimits, openLimits]; exact represents.ceiling
                unpinned := closingPins
                pinRoom := by rw [closingLimits, openLimits]; exact represents.pinRoom
                seen := by
                  intro previous scope
                  simp only [returned, Option.some.injEq] at scope
                  rw [closingScope, openScope, scope] }
          · have full : RegionProtocol.scopeLimit ≤ granted := by omega
            refine ⟨table, record, ?_, ?_⟩
            · simp [tableCalls, checks, borrowedRead,
                begin_exhausted handle ⟨.live, granted⟩ table record represents full]
            · simp only [checks, room, and_false, ite_false]
              exact represents
        · refine ⟨table, record, ?_, ?_⟩
          · simp [tableCalls, checks]
          · have unchecked : ¬ (claim = host.owner ∧ RegionProtocol.inBounds host.region.length off len = true ∧
                granted < RegionProtocol.scopeLimit) := fun all => checks ⟨all.1, all.2.1⟩
            simp only [unchecked, ite_false]
            exact represents
    | readOwned =>
        exact ⟨table, record, by simp [tableCalls, RegionProtocol.step, RegionProtocol.isRelease],
          by simpa [RegionProtocol.step] using represents⟩
    | release =>
        obtain ⟨retired, released', retiredRecords, retiredPins, retiredScope, retiredLimits⟩ :=
          (release_table handle _ table record represents).1 state
        refine ⟨retired, released record, ?_, ?_⟩
        · simp [tableCalls, releaseCall, released', RegionProtocol.step, RegionProtocol.isRelease,
            actionReleases]
        · exact {
            records := retiredRecords
            owned := by simp [represents.owned]
            slot := represents.slot
            wellFormed := released_valid record represents.wellFormed
            owner := by simp [RegionProtocol.step, corresponds]
            scope := by rw [retiredScope]; exact represents.scope
            ceiling := by rw [retiredLimits]; exact represents.ceiling
            unpinned := by rw [retiredPins]; exact represents.unpinned
            pinRoom := by rw [retiredLimits]; exact represents.pinRoom
            seen := by rw [retiredScope]; exact represents.seen }
    | drop =>
        obtain ⟨retired, released', retiredRecords, retiredPins, retiredScope, retiredLimits⟩ :=
          (release_table handle _ table record represents).1 state
        refine ⟨retired, released record, ?_, ?_⟩
        · simp [tableCalls, releaseCall, released', RegionProtocol.step, RegionProtocol.isRelease,
            actionReleases]
        · exact {
            records := retiredRecords
            owned := by simp [represents.owned]
            slot := represents.slot
            wellFormed := released_valid record represents.wellFormed
            owner := by simp [RegionProtocol.step, corresponds]
            scope := by rw [retiredScope]; exact represents.scope
            ceiling := by rw [retiredLimits]; exact represents.ceiling
            unpinned := by rw [retiredPins]; exact represents.unpinned
            pinRoom := by rw [retiredLimits]; exact represents.pinRoom
            seen := by rw [retiredScope]; exact represents.seen }
  · cases request with
    | read claim off len =>
        refine ⟨table, record, ?_, by simpa [RegionProtocol.step] using represents⟩
        by_cases checks : claim = host.owner ∧ RegionProtocol.inBounds host.region.length off len = true
        · obtain ⟨error, refused⟩ := begin_retired_table handle _ table record represents state
          simp [tableCalls, checks, borrowedRead, refused, RegionProtocol.step, RegionProtocol.isRelease]
        · simp [tableCalls, checks, RegionProtocol.step, RegionProtocol.isRelease]
    | readOwned =>
        exact ⟨table, record, by simp [tableCalls, RegionProtocol.step, RegionProtocol.isRelease],
          by simpa [RegionProtocol.step] using represents⟩
    | release =>
        exact ⟨table, record, by simp [tableCalls, releaseCall,
          (release_table handle _ table record represents).2 state, RegionProtocol.step,
          RegionProtocol.isRelease], by simpa [RegionProtocol.step] using represents⟩
    | drop =>
        exact ⟨table, record, by simp [tableCalls, releaseCall,
          (release_table handle _ table record represents).2 state, RegionProtocol.step,
          RegionProtocol.isRelease], by simpa [RegionProtocol.step] using represents⟩

/-- The host's cleanup, which the adapter performs through the same `release`. -/
theorem cleanup_refines (handle : resources.Handle) (s : RegionProtocol.State)
    (table : resources.table.Table) (record : resources.Snapshot)
    (represents : Represents handle s table record) :
    ∃ table' record',
      releaseCall table handle = Result.ok (table', (RegionProtocol.retire s).1.countP RegionProtocol.isRelease) ∧
      Represents handle (RegionProtocol.retire s).2 table' record' := by
  obtain ⟨owner, granted⟩ := s
  have related := represents.owner
  cases owner <;> cases state : record.state <;> simp [corresponds, state] at related
  · obtain ⟨retired, released', retiredRecords, retiredPins, retiredScope, retiredLimits⟩ :=
      (release_table handle _ table record represents).1 state
    refine ⟨retired, released record, ?_, ?_⟩
    · simp [releaseCall, released', RegionProtocol.retire, RegionProtocol.isRelease, actionReleases]
    · exact {
        records := retiredRecords
        owned := by simp [represents.owned]
        slot := represents.slot
        wellFormed := released_valid record represents.wellFormed
        owner := by simp [RegionProtocol.retire, corresponds]
        scope := by rw [retiredScope]; exact represents.scope
        ceiling := by rw [retiredLimits]; exact represents.ceiling
        unpinned := by rw [retiredPins]; exact represents.unpinned
        pinRoom := by rw [retiredLimits]; exact represents.pinRoom
        seen := by rw [retiredScope]; exact represents.seen }
  · exact ⟨table, record, by simp [releaseCall, (release_table handle _ table record represents).2 state,
      RegionProtocol.retire], by simpa [RegionProtocol.retire] using represents⟩

/-- The table side of `RegionProtocol.exec`: the adapter's table calls for the same requests in the
same order, ending where the guest's execution ends. -/
def tableExec (host : RegionProtocol.Host) (handle : resources.Handle) :
    RegionProtocol.State → resources.table.Table → List RegionProtocol.Action →
      Result (resources.table.Table × Nat)
  | _, table, [] => Result.ok (table, 0)
  | _, table, .stop _ :: _ => Result.ok (table, 0)
  | s, table, .call request :: rest => do
      let (called, here) ← tableCalls host handle table request
      if (RegionProtocol.step (tableHost host) s request).trapped then Result.ok (called, here)
      else
        let (after, later) ←
          tableExec host handle (RegionProtocol.step (tableHost host) s request).next called rest
        Result.ok (after, here + later)

/-- The table side of `RegionProtocol.run`: the execution, then the host's cleanup. -/
def tableRun (host : RegionProtocol.Host) (handle : resources.Handle) (s : RegionProtocol.State)
    (table : resources.table.Table) (program : List RegionProtocol.Action) :
    Result (resources.table.Table × Nat) := do
  let (executed, releases) ← tableExec host handle s table program
  let (cleaned, cleanup) ← releaseCall executed handle
  Result.ok (cleaned, releases + cleanup)

theorem exec_refines (host : RegionProtocol.Host) (handle : resources.Handle) :
    ∀ (program : List RegionProtocol.Action) (s : RegionProtocol.State) (table : resources.table.Table)
      (record : resources.Snapshot), Represents handle s table record →
      ∃ table' record',
        tableExec host handle s table program = Result.ok (table',
          (RegionProtocol.exec (tableHost host) s program).events.countP RegionProtocol.isRelease) ∧
        Represents handle (RegionProtocol.exec (tableHost host) s program).state table' record' := by
  intro program
  induction program with
  | nil =>
      intro s table record represents
      exact ⟨table, record, by simp [tableExec, RegionProtocol.exec],
        by simpa [RegionProtocol.exec] using represents⟩
  | cons action rest ih =>
      intro s table record represents
      cases action with
      | stop reason =>
          exact ⟨table, record, by simp [tableExec, RegionProtocol.exec],
            by simpa [RegionProtocol.exec] using represents⟩
      | call request =>
          obtain ⟨called, calledRecord, calls, calledRepresents⟩ :=
            calls_refine host handle s table record request represents
          by_cases trapped : (RegionProtocol.step (tableHost host) s request).trapped
          · refine ⟨called, calledRecord, ?_, ?_⟩
            · simp [tableExec, calls, trapped, RegionProtocol.exec]
            · simpa [RegionProtocol.exec, trapped] using calledRepresents
          · obtain ⟨after, afterRecord, later, afterRepresents⟩ :=
              ih _ called calledRecord calledRepresents
            refine ⟨after, afterRecord, ?_, ?_⟩
            · simp [tableExec, calls, trapped, later, RegionProtocol.exec, List.countP_append]
            · simpa [RegionProtocol.exec, trapped] using afterRepresents

/-- **T4, whole runs.** From a table that represents the model state, the adapter's table calls for a
whole program and the host's cleanup return a table that represents the model's final state under the
table's host, and the table's own accounting counts exactly the model's release commits. -/
theorem run_refines (host : RegionProtocol.Host) (handle : resources.Handle) (s : RegionProtocol.State)
    (table : resources.table.Table) (record : resources.Snapshot) (program : List RegionProtocol.Action)
    (represents : Represents handle s table record) :
    ∃ table' record',
      tableRun host handle s table program = Result.ok (table',
        (RegionProtocol.run (tableHost host) s program).events.countP RegionProtocol.isRelease) ∧
      Represents handle (RegionProtocol.run (tableHost host) s program).final table' record' := by
  obtain ⟨executed, executedRecord, exec, executedRepresents⟩ :=
    exec_refines host handle program s table record represents
  obtain ⟨cleaned, cleanedRecord, cleanup, cleanedRepresents⟩ :=
    cleanup_refines handle _ executed executedRecord executedRepresents
  refine ⟨cleaned, cleanedRecord, ?_, cleanedRepresents⟩
  simp [tableRun, exec, cleanup, RegionProtocol.run, List.countP_append]

/-- **P1 and the cleanup claim for the table.** From a table that represents a live owner, for every
program and every `lower` oracle, the adapter's table calls end with the record retired and no pin
outstanding, and the table's own accounting counts exactly one local release. -/
theorem table_retires_exactly_once (host : RegionProtocol.Host) (handle : resources.Handle)
    (granted : Nat) (table : resources.table.Table) (record : resources.Snapshot)
    (program : List RegionProtocol.Action) (represents : Represents handle ⟨.live, granted⟩ table record) :
    ∃ final retiredRecord,
      tableRun host handle ⟨.live, granted⟩ table program = Result.ok (final, 1) ∧
      final.records.val = [retiredRecord] ∧ retiredRecord.state = .Retired ∧ final.pins.val = 0 := by
  obtain ⟨final, finalRecord, ran, finalRepresents⟩ :=
    run_refines host handle ⟨.live, granted⟩ table record program represents
  have once := RegionProtocol.exactly_one_release_commit (tableHost host) granted program
  have ended := RegionProtocol.run_final_owner (tableHost host) ⟨.live, granted⟩ program (by simp)
  have related := finalRepresents.owner
  generalize (RegionProtocol.run (tableHost host) ⟨.live, granted⟩ program).final.owner = owner
    at related ended once
  cases owner with
  | live => exact absurd rfl ended.1
  | busy => exact absurd rfl ended.2
  | lost => simp [corresponds] at related
  | released who =>
      refine ⟨final, finalRecord, by rw [ran, once]; rfl, finalRepresents.records, ?_,
        finalRepresents.unpinned⟩
      cases state : finalRecord.state <;> simp_all [corresponds]

end RegionTable
