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
    exact identity the root's pinned map selected, and an emission records which
    slot generation performed the distinguished event. -/
inductive GuestEvent where
  | dispatch (root : Nat) (key : Nat) (generation : Nat)
  | emitted (root : Nat) (key : Nat) (generation : Nat)
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

/-! ## Event relocation under root pinning (VC-LIVE-03, model half)

The theorems above hold by construction: `trace` reads only the pinned map. This section gives the
model something a pin could get wrong. Programs dispatch to other slots and emit a distinguished
event. Every dispatch consults a map *view* chosen per dispatch, and a per-dispatch policy decides
authority independently.

* **Pinned selection** reads the root's pinned map at every dispatch, so a publication during the
  run cannot reach it.
* **Current-map selection**, the deliberately broken control, reads the registry as published at
  each dispatch.

The relocation fixture moves the event from caller to callee between versions, and its reverse
moves it back:
* `A-old` emits, then calls `B`;
* `A-new` only calls `B`;
* `B-old` is inert;
* `B-new` emits.

Coherent all-old and all-new pins emit exactly once. When `B-new` is published between the two hops,
current-map selection duplicates the event under the relocation and omits it under the reverse,
while a root pinned before the publication still emits it once.
A root pinned between two *sequential* single-slot publications selects a mixed pair from its one
map, and that duplicates or omits too: a limit of the coherence premise, not a root-snapshot bug.
A denial stops the run and keeps the committed prefix (`denials_only_truncate`).

What this is not: no correspondence relates these definitions to `slot-registry.mjs`, and the
implementation-side broken control is not constructible from the runtime's authority shape (see the
issue). -/

/-- One program step: emit the distinguished event, or dispatch to a slot through the root. -/
inductive Instr where
  | emit
  | call (key : Nat)
deriving DecidableEq, Repr

/-- The body each slot generation runs. -/
abbrev Programs := Nat → Nat → List Instr

/-- One activation: the slot and generation running, and the rest of its body. -/
structure Frame where
  key : Nat
  generation : Nat
  rest : List Instr
deriving DecidableEq, Repr

/-- A run's observable state: the ordered guest events, how many dispatches were attempted (which
indexes the map view and the policy), and whether a denial stopped the run. -/
structure RunState where
  events : List GuestEvent
  dispatches : Nat
  stopped : Bool
deriving DecidableEq, Repr

/-- Run at most `fuel` steps of an activation stack. A dispatch selects from `view` at its own index
and asks `allowed` at the same index; a denial stops the run with the events committed so far, and an
unselectable key dispatches nothing. Returning from a body resumes its caller. -/
def run (programs : Programs) (epoch : Nat) (view : Nat → EntryMap) (allowed : Nat → Bool) :
    Nat → List Frame → RunState → RunState
  | 0, _, state => state
  | _ + 1, [], state => state
  | fuel + 1, frame :: stack, state =>
      if state.stopped then state
      else
        match frame.rest with
        | [] => run programs epoch view allowed fuel stack state
        | .emit :: rest =>
            run programs epoch view allowed fuel (⟨frame.key, frame.generation, rest⟩ :: stack)
              ⟨state.events ++ [.emitted epoch frame.key frame.generation], state.dispatches, false⟩
        | .call callee :: rest =>
            if allowed state.dispatches then
              match select (view state.dispatches) callee with
              | none =>
                  run programs epoch view allowed fuel (⟨frame.key, frame.generation, rest⟩ :: stack)
                    ⟨state.events, state.dispatches + 1, false⟩
              | some generation =>
                  run programs epoch view allowed fuel
                    (⟨callee, generation, programs callee generation⟩ ::
                      ⟨frame.key, frame.generation, rest⟩ :: stack)
                    ⟨state.events ++ [.dispatch epoch callee generation], state.dispatches + 1, false⟩
            else ⟨state.events, state.dispatches, true⟩

/-- A request through a root enters one slot. -/
def request (programs : Programs) (epoch : Nat) (view : Nat → EntryMap) (allowed : Nat → Bool)
    (fuel entry : Nat) : RunState :=
  run programs epoch view allowed fuel [⟨entry, 0, [.call entry]⟩] ⟨[], 0, false⟩

/-- Root-pinned selection: every dispatch reads the root's pinned map. -/
def pinnedRequest (programs : Programs) (root : Root) (allowed : Nat → Bool) (fuel entry : Nat) :
    RunState :=
  request programs root.epoch (fun _ => root.pinned) allowed fuel entry

/-- The deliberately broken control: every dispatch reads the registry as published at its own
index, so a publication between two hops changes the second hop's selection. -/
def currentMapRequest (programs : Programs) (epoch : Nat) (registryAt : Nat → EntryMap)
    (allowed : Nat → Bool) (fuel entry : Nat) : RunState :=
  request programs epoch registryAt allowed fuel entry

/-- How many times the distinguished event was emitted. -/
def emissions (events : List GuestEvent) : Nat :=
  (events.filter fun event =>
    match event with
    | .emitted _ _ _ => true
    | _ => false).length

/-- A run only adds events: whatever it does, the events it started with stay a prefix. -/
theorem run_extends_the_events (programs : Programs) (epoch : Nat) (view : Nat → EntryMap)
    (allowed : Nat → Bool) (fuel : Nat) :
    ∀ (stack : List Frame) (state : RunState),
      state.events <+: (run programs epoch view allowed fuel stack state).events := by
  induction fuel with
  | zero => intro stack state; exact List.prefix_refl _
  | succ fuel ih =>
      intro stack state
      cases stack with
      | nil => exact List.prefix_refl _
      | cons frame stack =>
          obtain ⟨key, generation, rest⟩ := frame
          by_cases hStopped : state.stopped = true
          · simp only [run, hStopped, ↓reduceIte]
            exact List.prefix_refl _
          · have hRunning : state.stopped = false := by simpa using hStopped
            cases rest with
            | nil =>
                simp only [run, hRunning, Bool.false_eq_true, ↓reduceIte]
                exact ih _ _
            | cons instr rest =>
                cases instr with
                | emit =>
                    simp only [run, hRunning, Bool.false_eq_true, ↓reduceIte]
                    exact (List.prefix_append _ _).trans (ih _ ⟨_, _, _⟩)
                | call callee =>
                    by_cases hAllowed : allowed state.dispatches = true
                    · cases hSelect : select (view state.dispatches) callee with
                      | none =>
                          simp only [run, hRunning, hAllowed, hSelect, Bool.false_eq_true, ↓reduceIte]
                          exact ih _ ⟨state.events, state.dispatches + 1, false⟩
                      | some selected =>
                          simp only [run, hRunning, hAllowed, hSelect, Bool.false_eq_true, ↓reduceIte]
                          exact (List.prefix_append _ _).trans (ih _ ⟨_, _, _⟩)
                    · have hDenied : allowed state.dispatches = false := by simpa using hAllowed
                      simp only [run, hRunning, hDenied, Bool.false_eq_true, ↓reduceIte]
                      exact List.prefix_refl _

/-- **Denials only truncate**: a run whose policy denies some dispatches commits a prefix of the
events the fully authorised run commits from the same state. A denial stops execution; it never
rolls back a committed event and never commits one the authorised run would not. -/
theorem denials_only_truncate (programs : Programs) (epoch : Nat) (view : Nat → EntryMap)
    (allowed : Nat → Bool) (fuel : Nat) :
    ∀ (stack : List Frame) (state : RunState), state.stopped = false →
      (run programs epoch view allowed fuel stack state).events <+:
        (run programs epoch view (fun _ => true) fuel stack state).events := by
  induction fuel with
  | zero => intro stack state _; exact List.prefix_refl _
  | succ fuel ih =>
      intro stack state hRunning
      cases stack with
      | nil => exact List.prefix_refl _
      | cons frame stack =>
          obtain ⟨key, generation, rest⟩ := frame
          cases rest with
          | nil =>
              simp only [run, hRunning, Bool.false_eq_true, ↓reduceIte]
              exact ih _ _ hRunning
          | cons instr rest =>
              cases instr with
              | emit =>
                  simp only [run, hRunning, Bool.false_eq_true, ↓reduceIte]
                  exact ih _ _ rfl
              | call callee =>
                  by_cases hAllowed : allowed state.dispatches = true
                  · cases hSelect : select (view state.dispatches) callee with
                    | none =>
                        simp only [run, hRunning, hAllowed, hSelect, Bool.false_eq_true, ↓reduceIte]
                        exact ih _ _ rfl
                    | some selected =>
                        simp only [run, hRunning, hAllowed, hSelect, Bool.false_eq_true, ↓reduceIte]
                        exact ih _ _ rfl
                  · have hDenied : allowed state.dispatches = false := by simpa using hAllowed
                    cases hSelect : select (view state.dispatches) callee with
                    | none =>
                        simp only [run, hRunning, hDenied, hSelect, Bool.false_eq_true, ↓reduceIte]
                        exact run_extends_the_events programs epoch view (fun _ => true) fuel _
                          ⟨state.events, state.dispatches + 1, false⟩
                    | some selected =>
                        simp only [run, hRunning, hDenied, hSelect, Bool.false_eq_true, ↓reduceIte]
                        exact (List.prefix_append _ _).trans
                          (run_extends_the_events programs epoch view (fun _ => true) fuel _ ⟨_, _, _⟩)

/-- The relocation fixture: slot `0` is the caller `A`, slot `1` the callee `B`. Generation `1` is the
old version and `2` the new one; the event moves from `A-old` to `B-new`. -/
def relocation : Programs := fun key generation =>
  if key = 0 ∧ generation = 1 then [.emit, .call 1]
  else if key = 0 ∧ generation = 2 then [.call 1]
  else if key = 1 ∧ generation = 2 then [.emit]
  else []

/-- The reverse relocation: the event moves from the callee `B-old` into the caller `A-new`. -/
def reverseRelocation : Programs := fun key generation =>
  if key = 0 ∧ generation = 1 then [.call 1]
  else if key = 0 ∧ generation = 2 then [.emit, .call 1]
  else if key = 1 ∧ generation = 1 then [.emit]
  else []

/-- The coherent maps and the two mixed maps sequential publication passes through. -/
def allOld : EntryMap := [⟨0, 1⟩, ⟨1, 1⟩]
def allNew : EntryMap := [⟨0, 2⟩, ⟨1, 2⟩]
def newCallerOldCallee : EntryMap := publish allOld 0 2
def oldCallerNewCallee : EntryMap := publish allOld 1 2

/-- Every dispatch is authorised. -/
def everyDispatch : Nat → Bool := fun _ => true

/-- The registry as each dispatch of one request sees it when `B-new` is published between the
caller's dispatch and the callee's. -/
def calleePublishedBetweenHops : Nat → EntryMap := fun index =>
  if index = 0 then allOld else oldCallerNewCallee

/-- **Coherent pins emit exactly once**: an all-old or an all-new pin performs the event once, for
both fixtures, wherever each version places it. -/
theorem coherent_pins_emit_exactly_once :
    emissions (pinnedRequest relocation ⟨1, allOld⟩ everyDispatch 16 0).events = 1 ∧
      emissions (pinnedRequest relocation ⟨2, allNew⟩ everyDispatch 16 0).events = 1 ∧
      emissions (pinnedRequest reverseRelocation ⟨1, allOld⟩ everyDispatch 16 0).events = 1 ∧
      emissions (pinnedRequest reverseRelocation ⟨2, allNew⟩ everyDispatch 16 0).events = 1 := by
  decide

/-- **The broken current-map control duplicates and omits; the pin does not.** With `B-new`
published between the hops:
* under the relocation, current-map selection runs the old caller's emission and then the new
  callee's, duplicating the event;
* under the reverse, the old caller reaches the now inert new callee, omitting it;
* a root pinned to the old map before the publication emits exactly once in both cases. -/
theorem current_map_selection_duplicates_and_omits :
    emissions (currentMapRequest relocation 1 calleePublishedBetweenHops everyDispatch 16 0).events
        = 2 ∧
      emissions (currentMapRequest reverseRelocation 1 calleePublishedBetweenHops everyDispatch 16
        0).events = 0 ∧
      emissions (pinnedRequest relocation ⟨1, allOld⟩ everyDispatch 16 0).events = 1 ∧
      emissions (pinnedRequest reverseRelocation ⟨1, allOld⟩ everyDispatch 16 0).events = 1 := by
  decide

/-- **Sequential publication is not atomic**: a fresh root pinned between two single-slot publications
still selects from one map, but that map is a mixed pair. Publishing the caller first gives a pin that
omits the event; publishing the callee first gives one that duplicates it. -/
theorem a_root_pinned_between_sequential_publications_can_mix :
    select newCallerOldCallee 0 = some 2 ∧ select newCallerOldCallee 1 = some 1 ∧
      emissions (pinnedRequest relocation ⟨1, newCallerOldCallee⟩ everyDispatch 16 0).events = 0 ∧
      select oldCallerNewCallee 0 = some 1 ∧ select oldCallerNewCallee 1 = some 2 ∧
      emissions (pinnedRequest relocation ⟨1, oldCallerNewCallee⟩ everyDispatch 16 0).events = 2 := by
  decide

/-- **A denial before the emitting hop emits nothing and keeps the committed prefix.**
* Under the new pin, the callee's dispatch is denied: the caller's dispatch stays committed and the
  event never happens.
* Under the old pin, denying the caller's own dispatch commits nothing at all. -/
theorem a_denial_before_the_emitting_hop_keeps_the_prefix :
    (pinnedRequest relocation ⟨2, allNew⟩ (fun index => index = 0) 16 0).events
        = [.dispatch 2 0 2] ∧
      (pinnedRequest relocation ⟨1, allOld⟩ (fun _ => false) 16 0).events = [] := by
  decide

end NobleFoundation.LiveDispatch
