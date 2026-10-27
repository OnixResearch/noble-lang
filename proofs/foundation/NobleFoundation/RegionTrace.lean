/-!
  A prefix-safe source/target trace relation for the bounded RegionProtocol subject (issue #50).

  The relation is defined *first*, and every property below is derived from it: non-identical traces
  do not preserve arbitrary properties by themselves. The source alphabet is RegionProtocol-v1's
  request/response/commit classes for the exact resource identity the pilot uses. The target alphabet
  is the ABI step sequence the bounded-region host performs, with one named internal bookkeeping
  class that may stutter.

  Two erasures would make a broken execution look correct, so the projection refuses them outright:
  a **host retirement is never rewritten into a guest release**, and a **trap exit is never dropped**
  to leave a successful-looking prefix. Denials are not erasable either; only `bookkeeping` steps
  vanish, and only internal steps with an identified tag can stutter.

  Not claimed: that the emitted ABI steps equal these events (no correspondence to
  `crates/noble-cli/src/component/bounded_region.rs` with pinned revisions), that a real Wasmtime
  instance behaves this way, eventual return, successful cleanup, liveness, rollback or cancellation
  support. Prefix safety implies none of those.
-/
namespace NobleFoundation.RegionTrace

/-- The exact resource identity of the bounded pilot. -/
inductive ResourceId where
  | swap
deriving DecidableEq, Repr

/-- The two guest operations the pilot exercises. -/
inductive Operation where
  | read
  | release
deriving DecidableEq, Repr

/-- RegionProtocol-v1's guest-visible classes. There is deliberately no `retire` class: host
    retirement is not a guest release. -/
inductive SourceEvent where
  | request (operation : Operation) (resource : ResourceId)
  | response (accepted : Bool) (code : Option Nat)
  | commit (operation : Operation) (resource : ResourceId)
deriving DecidableEq, Repr

/-- The target alphabet: guest-visible calls and acknowledgements, the protected store, the named
    internal bookkeeping class, host retirement and the trap exit. -/
inductive TargetEvent where
  | call (operation : Operation) (resource : ResourceId)
  | ack (accepted : Bool) (code : Option Nat)
  | store (operation : Operation) (resource : ResourceId)
  | bookkeeping (tag : Nat)
  | retire (resource : ResourceId)
  | trapExit (code : Nat)
deriving DecidableEq, Repr

/-- The only class that may stutter: one named internal bookkeeping class, each step tagged. -/
def erasable (event : TargetEvent) : Bool :=
  match event with
  | .bookkeeping _ => true
  | _ => false

/-- Events the relation refuses to translate at all. They end the related prefix rather than being
    erased inside it. -/
def halted (event : TargetEvent) : Bool :=
  match event with
  | .retire _ => true
  | .trapExit _ => true
  | _ => false

/-- How a target event would be translated, if at all. `none` is the internal bookkeeping class. -/
def visibleSource (event : TargetEvent) : Option SourceEvent :=
  match event with
  | .call operation resource => some (.request operation resource)
  | .ack accepted code => some (.response accepted code)
  | .store operation resource => some (.commit operation resource)
  | .bookkeeping _ => none
  | .retire _ => none
  | .trapExit _ => none

/-- The projection: the source trace a target trace relates to, or `none` when the target contains an
    event the relation refuses to translate. -/
def project : List TargetEvent → Option (List SourceEvent)
  | [] => some []
  | event :: rest =>
      if halted event then none
      else
        (project rest).map (fun source =>
          match visibleSource event with
          | none => source
          | some mapped => mapped :: source)

/-- The relation itself: a target trace relates to exactly its projection. -/
def Relates (target : List TargetEvent) (source : List SourceEvent) : Prop :=
  project target = some source

/-! ### The two refusals -/

/-- A host retirement has no source translation, so it cannot be presented as a guest release. -/
theorem project_refuses_retirement (resource : ResourceId) (rest : List TargetEvent) :
    project (.retire resource :: rest) = none := by
  simp [project, halted]

/-- A trap exit is refused rather than dropped, so a trap cannot leave a successful-looking prefix
    behind by itself. -/
theorem project_refuses_trap_exit (code : Nat) (rest : List TargetEvent) :
    project (.trapExit code :: rest) = none := by
  simp [project, halted]

/-- Retirement alone relates to no source trace: the control against disguising host cleanup as a
    guest release. -/
theorem retirement_is_not_a_guest_release (resource : ResourceId) (source : List SourceEvent) :
    ¬ Relates [.retire resource] source := by
  intro related
  simp [Relates, project, halted] at related

/-- Only internal bookkeeping stutters: an event has no source translation exactly when it is the
    bookkeeping class or one of the refused terminators. -/
theorem only_bookkeeping_and_terminators_stutter (event : TargetEvent) :
    visibleSource event = none ↔ erasable event = true ∨ halted event = true := by
  cases event <;> simp [visibleSource, erasable, halted]

/-! ### The projection keeps the protected observations -/

/-- The main characterization: a target trace with no refused terminator projects to exactly its
    visible events, in order. -/
theorem project_is_the_visible_projection (target : List TargetEvent)
    (clean : ∀ (event : TargetEvent), event ∈ target → halted event = false) :
    project target = some (target.filterMap visibleSource) := by
  induction target with
  | nil => rfl
  | cons event rest fresh =>
      have tailClean : ∀ (later : TargetEvent), later ∈ rest → halted later = false := by
        intro later inside
        exact clean later (List.mem_cons_of_mem event inside)
      have headClean : halted event = false := clean event List.mem_cons_self
      have tailProjection := fresh tailClean
      simp only [project, headClean, Bool.false_eq_true, if_false, tailProjection,
        Option.map_some, List.filterMap_cons]
      cases mapped : visibleSource event with
      | none => rfl
      | some source => rfl

/-- Every protected store appears as its own commit, in both directions. -/
theorem project_keeps_every_commit (target : List TargetEvent) (source : List SourceEvent)
    (related : project target = some source)
    (clean : ∀ (event : TargetEvent), event ∈ target → halted event = false)
    (operation : Operation) (resource : ResourceId) :
    (.store operation resource) ∈ target ↔ (.commit operation resource) ∈ source := by
  have characterized := project_is_the_visible_projection target clean
  rw [characterized, Option.some.injEq] at related
  subst related
  rw [List.mem_filterMap]
  constructor
  · intro inside
    exact ⟨.store operation resource, inside, rfl⟩
  · intro inside
    obtain ⟨event, eventInside, mapped⟩ := inside
    cases event with
    | call operation resource => simp [visibleSource] at mapped
    | ack accepted code' =>
        simp only [visibleSource] at mapped
        cases mapped
    | store storedOperation storedResource =>
        simp only [visibleSource] at mapped
        cases mapped
        exact eventInside
    | bookkeeping tag => simp [visibleSource] at mapped
    | retire resource => simp [visibleSource] at mapped
    | trapExit code' => simp [visibleSource] at mapped

/-- Denials are not erasable either: a refused acknowledgement appears as a refused response. -/
theorem project_keeps_denials (target : List TargetEvent) (source : List SourceEvent)
    (related : project target = some source)
    (clean : ∀ (event : TargetEvent), event ∈ target → halted event = false)
    (code : Option Nat) :
    (.ack false code) ∈ target ↔ (.response false code) ∈ source := by
  have characterized := project_is_the_visible_projection target clean
  rw [characterized, Option.some.injEq] at related
  subst related
  rw [List.mem_filterMap]
  constructor
  · intro inside
    exact ⟨.ack false code, inside, rfl⟩
  · intro inside
    obtain ⟨event, eventInside, mapped⟩ := inside
    cases event with
    | call operation resource => simp [visibleSource] at mapped
    | ack accepted acknowledged =>
        simp only [visibleSource] at mapped
        cases mapped
        exact eventInside
    | store operation resource => simp [visibleSource] at mapped
    | bookkeeping tag => simp [visibleSource] at mapped
    | retire resource => simp [visibleSource] at mapped
    | trapExit code' => simp [visibleSource] at mapped

/-- Requests and their acknowledgements keep their pairing and their order: the source trace is the
    target's visible subsequence, not a permutation of it. -/
theorem project_keeps_the_order (target : List TargetEvent) (source : List SourceEvent)
    (related : project target = some source)
    (clean : ∀ (event : TargetEvent), event ∈ target → halted event = false) :
    source = target.filterMap visibleSource := by
  have characterized := project_is_the_visible_projection target clean
  rw [characterized, Option.some.injEq] at related
  exact related.symm

/-! ### Prefix compatibility and the cost boundary -/

/-- Prefix of a trace, as its own inductive so prefix claims stay about the trace rather than about
    list indexing. -/
inductive Prefix {α : Type} : List α → List α → Prop where
  | nil (trace : List α) : Prefix [] trace
  | cons {first : α} {shorter longer : List α} : Prefix shorter longer →
      Prefix (first :: shorter) (first :: longer)

/-- Any target prefix of a related trace relates to a source prefix of the related source trace. -/
theorem prefix_of_projection (longer : List TargetEvent) (source : List SourceEvent)
    (related : project longer = some source) :
    ∀ (shortTrace : List TargetEvent), Prefix shortTrace longer →
      ∃ sourcePrefix : List SourceEvent, project shortTrace = some sourcePrefix ∧
        Prefix sourcePrefix source := by
  induction longer generalizing source with
  | nil =>
      intro shortTrace shortPrefix
      cases shortPrefix
      exact ⟨[], rfl, Prefix.nil source⟩
  | cons event rest fresh =>
      intro shortTrace shortPrefix
      cases shortPrefix with
      | nil => exact ⟨[], rfl, Prefix.nil source⟩
      | cons innerPrefix =>
          rename_i shortTail
          cases head : halted event with
          | true => simp [project, head] at related
          | false =>
              simp only [project, head, Bool.false_eq_true, if_false] at related
              obtain ⟨tail, tailProjection, tailSource⟩ :=
                Option.map_eq_some_iff.mp related
              subst tailSource
              obtain ⟨sourcePrefix, shorterProjection, sourcePrefixIsPrefix⟩ :=
                fresh tail tailProjection shortTail innerPrefix
              cases mapped : visibleSource event with
              | none =>
                  exact ⟨sourcePrefix, by simp [project, head, shorterProjection, mapped],
                    sourcePrefixIsPrefix⟩
              | some translated =>
                  exact ⟨translated :: sourcePrefix,
                    by simp [project, head, shorterProjection, mapped],
                    Prefix.cons sourcePrefixIsPrefix⟩

/-- Truncating a trace gives a prefix of it. -/
theorem take_is_a_prefix (target : List TargetEvent) :
    ∀ (budget : Nat), Prefix (target.take budget) target := by
  induction target with
  | nil =>
      intro budget
      cases budget <;> exact Prefix.nil []
  | cons event rest fresh =>
      intro budget
      cases budget with
      | zero => exact Prefix.nil _
      | succ spent => exact Prefix.cons (fresh spent)

/-- A fuel boundary truncates the target trace; what it truncates still relates to a source prefix
    instead of inventing a completed source run. -/
theorem excerpt_keeps_the_committed_prefix (budget : Nat) (target : List TargetEvent)
    (source : List SourceEvent) (related : project target = some source) :
    ∃ sourcePrefix : List SourceEvent, project (target.take budget) = some sourcePrefix ∧
      Prefix sourcePrefix source :=
  prefix_of_projection target source related (target.take budget) (take_is_a_prefix target budget)

/-- Two target traces that agree on their visible events project to the same source trace: this is
    the whole justification for the one class that may stutter, and it says nothing about invisible
    steps changing fuel cost. -/
theorem internal_stutter_is_invisible (shorter longer : List TargetEvent)
    (shorterClean : ∀ (event : TargetEvent), event ∈ shorter → halted event = false)
    (longerClean : ∀ (event : TargetEvent), event ∈ longer → halted event = false)
    (sameVisible : shorter.filterMap visibleSource = longer.filterMap visibleSource) :
    project shorter = project longer := by
  rw [project_is_the_visible_projection shorter shorterClean,
    project_is_the_visible_projection longer longerClean, sameVisible]

end NobleFoundation.RegionTrace
