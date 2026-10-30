/-
  `RegionProtocol-v1`: the protocol model of `.cairn/changes/behavioral-effect-protocol` (issue #9),
  obligations T1–T3.

  The subject world is the synchronous bounded-region adapter: one host-injected owner, finitely many
  borrowed reads, at most one release. A trace has three event classes, kept separate: guest
  requests (`req`), host responses (`resp`) and externally committed effects (`commit`).

  - `step` is the design's transition table, row by row and with the host checks in its order. A
    guest boundary never observes `busy`, because one host call runs from `begin` to `complete`.
  - `run` adds the host's post-return and abort cleanup: a live owner is retired once.
  - T2, for the selected subject `region.read swap regions.release`: every normal return is the
    success or the ordinary-error postcondition (`subject_normal_return`), and the error is
    `bounds-reject` exactly when the in-bounds condition fails (`subject_rejects_exactly_out_of_bounds`).
  - Prefix safety for every program and host: exactly one release commit unless a completion failed
    (P1, `exactly_one_release_commit`); the monitor's rules are P2–P5, and it accepts every model run
    (`run_is_accepted`); an abort discards the rest of the program and adds at most the host's
    retirement (P6, `abort_tail`, `stop_discards_the_rest`).
  - T3: sequential composition over identical intermediate states, counters included (`seq`). A
    counter-free rule is unsound (`counters_do_not_commute`), and no set-union rule is derivable
    (`no_set_union_rule`).

  Host assumptions enter as follows. A1 (immutable `R`), A2 (one fresh owner identity), A3 (the
  result schema) and A6 (the only environment is `R`, the table and the limits) are the shape of
  `Host` and `Reply`. A4 enters as the `complete` oracle, assumed `true` where a claim needs it, and
  A5 as `run`'s unconditional cleanup. This is a model only: no Wasmtime, Canonical ABI, host-Rust
  or compiled-component correspondence is claimed (T4–T6 stay open).
-/

namespace NobleFoundation.RegionProtocol

/-- Who committed the release. -/
inductive By where
  | guest
  | destructor
  | host
  deriving DecidableEq, Repr

/-- The owner's protocol state. `busy` exists only inside one host call; `lost` is the
host-obligation violation of a failed completion (F7). -/
inductive Owner where
  | live
  | busy
  | released (who : By)
  | lost
  deriving DecidableEq, Repr

/-- Externally committed effects. A region read is the host-side protected copy. -/
inductive Commit where
  | regionRead (off len : Nat)
  | guestRelease
  | destructorRelease
  | hostRetire
  deriving DecidableEq, Repr

/-- The host replies: exactly the result schema (A3). An unmodeled reply is not representable. -/
inductive Reply where
  | ok (bytes : List UInt8)
  | boundsReject
  | invalidHandle
  deriving DecidableEq, Repr

/-- Guest requests crossing the import. A borrowed read carries the claim it presents. -/
inductive Request where
  | read (claim : Nat) (off len : Int)
  | readOwned
  | release
  | drop
  deriving DecidableEq, Repr

inductive Event where
  | req (request : Request)
  | resp (reply : Reply)
  | commit (kind : Commit)
  deriving DecidableEq, Repr

/-- Why a run stopped abnormally. `cancel` is the future host interruption control. -/
inductive Abort where
  | trap
  | fuel
  | memory
  | cancel
  deriving DecidableEq, Repr

/-- One guest action: a request, or an abnormal stop the environment imposes. -/
inductive Action where
  | call (request : Request)
  | stop (reason : Abort)
  deriving DecidableEq, Repr

/-- The host side of one invocation: the immutable region (A1), the injected owner's identity (A2),
and per-borrow oracles, indexed by the number of borrows already granted, for `begin` and
`native_access`, for `complete` (A4) and for lowering the bytes into guest memory. -/
structure Host where
  region : List UInt8
  owner : Nat
  access : Nat → Bool
  complete : Nat → Bool
  lower : Nat → Bool

/-- The protocol state at a guest boundary: the owner and the number of granted borrows. -/
structure State where
  owner : Owner
  granted : Nat
  deriving DecidableEq, Repr

def initial : State := ⟨.live, 0⟩

def maxI64 : Int := 9223372036854775807

/-- The scope limit on granted borrows. -/
def scopeLimit : Nat := 64

/-- The in-bounds condition, as the host checks it: nonnegative arguments, no `i64` overflow, inside
the region, and at most 4096 bytes. -/
def inBounds (size : Nat) (off len : Int) : Bool :=
  decide (0 ≤ off) && decide (0 ≤ len) && decide (off + len ≤ maxI64) && decide (off + len ≤ (size : Int)) &&
    decide (len ≤ 4096)

/-- `R[off..off+len)`. -/
def slice (region : List UInt8) (off len : Int) : List UInt8 := (region.drop off.toNat).take len.toNat

/-- The events of one host call, the next state, and whether the call trapped. -/
structure Result where
  events : List Event
  next : State
  trapped : Bool
  deriving DecidableEq, Repr

/-- A borrowed read on a live owner, with the host checks in the design's order: identity, bounds,
the scope limit and `begin`/`native_access`, `complete`, then the protected copy and lowering. -/
def readStep (host : Host) (s : State) (claim : Nat) (off len : Int) : Result :=
  if claim ≠ host.owner then ⟨[.req (.read claim off len), .resp .invalidHandle], s, false⟩
  else if inBounds host.region.length off len = false then
    ⟨[.req (.read claim off len), .resp .boundsReject], s, false⟩
  else if scopeLimit ≤ s.granted ∨ host.access s.granted = false then
    ⟨[.req (.read claim off len), .resp .invalidHandle], s, false⟩
  else if host.complete s.granted = false then
    ⟨[.req (.read claim off len), .resp .invalidHandle], { s with owner := .lost }, false⟩
  else if host.lower s.granted = false then
    ⟨[.req (.read claim off len), .commit (.regionRead off.toNat len.toNat)],
      { s with granted := s.granted + 1 }, true⟩
  else
    ⟨[.req (.read claim off len), .commit (.regionRead off.toNat len.toNat),
      .resp (.ok (slice host.region off len))], { s with granted := s.granted + 1 }, false⟩

/-- **The transition table.** A read or release without a live owner traps, an owned receiver
traps on its ABI shape, and a drop without an owner is a no-op. -/
def step (host : Host) (s : State) : Request → Result
  | .read claim off len =>
      match s.owner with
      | .live => readStep host s claim off len
      | _ => ⟨[.req (.read claim off len)], s, true⟩
  | .readOwned => ⟨[.req .readOwned], s, true⟩
  | .release =>
      match s.owner with
      | .live => ⟨[.req .release, .commit .guestRelease], { s with owner := .released .guest }, false⟩
      | _ => ⟨[.req .release], s, true⟩
  | .drop =>
      match s.owner with
      | .live =>
          ⟨[.req .drop, .commit .destructorRelease], { s with owner := .released .destructor }, false⟩
      | _ => ⟨[.req .drop], s, false⟩

/-- The host's cleanup after a normal return or an abort: a live owner is retired (A5). -/
def retire (s : State) : List Event × State :=
  match s.owner with
  | .live => ([.commit .hostRetire], { s with owner := .released .host })
  | _ => ([], s)

/-- An execution prefix: its events, the state reached, and the abort that ended it, if any. -/
structure Exec where
  events : List Event
  state : State
  stopped : Option Abort
  deriving DecidableEq, Repr

/-- Execute actions until the program ends or the run stops. Cleanup is not applied here. -/
def exec (host : Host) : State → List Action → Exec
  | s, [] => ⟨[], s, none⟩
  | s, .stop reason :: _ => ⟨[], s, some reason⟩
  | s, .call request :: rest =>
      if (step host s request).trapped then
        ⟨(step host s request).events, (step host s request).next, some .trap⟩
      else
        ⟨(step host s request).events ++ (exec host (step host s request).next rest).events,
          (exec host (step host s request).next rest).state,
          (exec host (step host s request).next rest).stopped⟩

inductive Outcome where
  | returned
  | aborted (reason : Abort)
  deriving DecidableEq, Repr

structure Run where
  events : List Event
  final : State
  outcome : Outcome
  deriving DecidableEq, Repr

/-- A complete run: the execution, then the host's cleanup. -/
def run (host : Host) (s : State) (program : List Action) : Run :=
  ⟨(exec host s program).events ++ (retire (exec host s program).state).1,
    (retire (exec host s program).state).2,
    match (exec host s program).stopped with
    | none => .returned
    | some reason => .aborted reason⟩

/-! ## The guest boundary never observes `busy` -/

theorem step_not_busy (host : Host) (s : State) (request : Request) (notBusy : s.owner ≠ .busy) :
    (step host s request).next.owner ≠ .busy := by
  cases request with
  | read claim off len =>
      unfold step
      cases owner : s.owner with
      | live =>
          unfold readStep
          (repeat' split) <;> simp_all
      | busy => exact absurd owner notBusy
      | released who => simp [owner]
      | lost => simp [owner]
  | readOwned => simpa [step] using notBusy
  | release => cases owner : s.owner <;> simp_all [step]
  | drop => cases owner : s.owner <;> simp_all [step]

theorem exec_not_busy (host : Host) :
    ∀ (program : List Action) (s : State), s.owner ≠ .busy → (exec host s program).state.owner ≠ .busy := by
  intro program
  induction program with
  | nil => intro s notBusy; simpa [exec] using notBusy
  | cons action rest ih =>
      intro s notBusy
      cases action with
      | stop reason => simpa [exec] using notBusy
      | call request =>
          have next := step_not_busy host s request notBusy
          unfold exec
          split
          · exact next
          · exact ih _ next

/-- **No live owner and no `busy` state after any run**: cleanup leaves the owner released or lost. -/
theorem run_final_owner (host : Host) (s : State) (program : List Action) (notBusy : s.owner ≠ .busy) :
    (run host s program).final.owner ≠ .live ∧ (run host s program).final.owner ≠ .busy := by
  have reached := exec_not_busy host program s notBusy
  unfold run retire
  dsimp only
  split
  · simp
  · rename_i notLive
    exact ⟨notLive, reached⟩

/-! ## T2: the selected subject

`region.read swap regions.release` issues a borrowed read with the injected owner's claim, then
releases the owner. `swap` is a stack operation and crosses no import. -/

def subject (claim : Nat) (off len : Int) : List Action := [.call (.read claim off len), .call .release]

/-- **Success**: in bounds, under the scope limit, with access granted and completion and lowering
succeeding, the subject reads `R[off..off+len)`, commits the read, and releases once. -/
theorem subject_success (host : Host) (granted : Nat) (off len : Int)
    (bounds : inBounds host.region.length off len = true) (scope : granted < scopeLimit)
    (access : host.access granted = true) (complete : host.complete granted = true)
    (lower : host.lower granted = true) :
    run host ⟨.live, granted⟩ (subject host.owner off len) =
      ⟨[.req (.read host.owner off len), .commit (.regionRead off.toNat len.toNat),
        .resp (.ok (slice host.region off len)), .req .release, .commit .guestRelease],
       ⟨.released .guest, granted + 1⟩, .returned⟩ := by
  have notLimit : ¬ scopeLimit ≤ granted := Nat.not_le.mpr scope
  simp [run, exec, step, readStep, subject, retire, bounds, access, complete, lower, notLimit]

/-- **Ordinary error, bounds**: outside the in-bounds condition the read is refused, nothing is
committed for it, and the owner is still released once by the guest. -/
theorem subject_bounds_reject (host : Host) (granted : Nat) (off len : Int)
    (outside : inBounds host.region.length off len = false) :
    run host ⟨.live, granted⟩ (subject host.owner off len) =
      ⟨[.req (.read host.owner off len), .resp .boundsReject, .req .release, .commit .guestRelease],
       ⟨.released .guest, granted⟩, .returned⟩ := by
  simp [run, exec, step, readStep, subject, retire, outside]

/-- **Ordinary error, invalid handle**: in bounds, but past the scope limit or refused by
`begin`/`native_access`. -/
theorem subject_invalid_handle (host : Host) (granted : Nat) (off len : Int)
    (bounds : inBounds host.region.length off len = true)
    (refused : scopeLimit ≤ granted ∨ host.access granted = false) :
    run host ⟨.live, granted⟩ (subject host.owner off len) =
      ⟨[.req (.read host.owner off len), .resp .invalidHandle, .req .release, .commit .guestRelease],
       ⟨.released .guest, granted⟩, .returned⟩ := by
  simp [run, exec, step, readStep, subject, retire, bounds, refused]

/-- **Every normal return is exactly one of the two postconditions**: the success trace, or an
ordinary error whose trace has no region-read commit. A failed completion or lowering aborts the
run instead of returning. -/
theorem subject_normal_return (host : Host) (granted : Nat) (off len : Int)
    (normal : (run host ⟨.live, granted⟩ (subject host.owner off len)).outcome = .returned) :
    (inBounds host.region.length off len = true ∧
      run host ⟨.live, granted⟩ (subject host.owner off len) =
        ⟨[.req (.read host.owner off len), .commit (.regionRead off.toNat len.toNat),
          .resp (.ok (slice host.region off len)), .req .release, .commit .guestRelease],
         ⟨.released .guest, granted + 1⟩, .returned⟩) ∨
    (∃ reply, (reply = .boundsReject ∨ reply = .invalidHandle) ∧
      run host ⟨.live, granted⟩ (subject host.owner off len) =
        ⟨[.req (.read host.owner off len), .resp reply, .req .release, .commit .guestRelease],
         ⟨.released .guest, granted⟩, .returned⟩) := by
  cases bounds : inBounds host.region.length off len with
  | false => exact Or.inr ⟨.boundsReject, Or.inl rfl, subject_bounds_reject host granted off len bounds⟩
  | true =>
      by_cases refused : scopeLimit ≤ granted ∨ host.access granted = false
      · exact Or.inr ⟨.invalidHandle, Or.inr rfl,
          subject_invalid_handle host granted off len bounds refused⟩
      · have scope : granted < scopeLimit := by omega
        have access : host.access granted = true := by
          cases h : host.access granted <;> simp_all
        cases complete : host.complete granted with
        | false =>
            have notLimit : ¬ scopeLimit ≤ granted := Nat.not_le.mpr scope
            simp [run, exec, step, readStep, subject, retire, bounds, access, complete, notLimit] at normal
        | true =>
            cases lower : host.lower granted with
            | false =>
                have notLimit : ¬ scopeLimit ≤ granted := Nat.not_le.mpr scope
                simp [run, exec, step, readStep, subject, retire, bounds, access, complete, lower,
                  notLimit] at normal
            | true =>
                exact Or.inl ⟨rfl, subject_success host granted off len bounds scope access complete lower⟩

/-- **The error is `bounds-reject` exactly when the in-bounds condition fails**, for the authentic
owner under the scope limit with access granted. -/
theorem subject_rejects_exactly_out_of_bounds (host : Host) (granted : Nat) (off len : Int)
    (scope : granted < scopeLimit) (access : host.access granted = true) :
    (run host ⟨.live, granted⟩ (subject host.owner off len)).events =
        [.req (.read host.owner off len), .resp .boundsReject, .req .release, .commit .guestRelease] ↔
      inBounds host.region.length off len = false := by
  constructor
  · intro shape
    cases bounds : inBounds host.region.length off len with
    | false => rfl
    | true =>
        have notLimit : ¬ scopeLimit ≤ granted := Nat.not_le.mpr scope
        exfalso
        cases complete : host.complete granted <;> cases lower : host.lower granted <;>
          simp [run, exec, step, readStep, subject, retire, bounds, access, complete, lower,
            notLimit] at shape
  · intro outside
    rw [subject_bounds_reject host granted off len outside]

/-- The success bytes have exactly the requested length. -/
theorem slice_length (region : List UInt8) (off len : Int)
    (bounds : inBounds region.length off len = true) : (slice region off len).length = len.toNat := by
  simp only [inBounds, Bool.and_eq_true, decide_eq_true_eq] at bounds
  obtain ⟨⟨⟨⟨offNonneg, lenNonneg⟩, _⟩, inside⟩, _⟩ := bounds
  unfold slice
  rw [List.length_take, List.length_drop]
  omega

/-! ## P1: one release commit per owner -/

/-- A release-class commit. -/
def isRelease : Event → Bool
  | .commit .guestRelease => true
  | .commit .destructorRelease => true
  | .commit .hostRetire => true
  | _ => false

def releasedCount : Owner → Nat
  | .released _ => 1
  | _ => 0

theorem step_release_count (host : Host) (s : State) (request : Request) :
    (step host s request).events.countP isRelease + releasedCount s.owner =
      releasedCount (step host s request).next.owner := by
  cases request with
  | read claim off len =>
      unfold step
      cases owner : s.owner with
      | live =>
          unfold readStep
          (repeat' split) <;> simp_all [isRelease, releasedCount]
      | _ => simp [owner, isRelease]
  | readOwned => simp [step, isRelease]
  | release => cases owner : s.owner <;> simp [step, owner, isRelease, releasedCount]
  | drop => cases owner : s.owner <;> simp [step, owner, isRelease, releasedCount]

theorem exec_release_count (host : Host) :
    ∀ (program : List Action) (s : State),
      (exec host s program).events.countP isRelease + releasedCount s.owner =
        releasedCount (exec host s program).state.owner := by
  intro program
  induction program with
  | nil => intro s; simp [exec]
  | cons action rest ih =>
      intro s
      cases action with
      | stop reason => simp [exec]
      | call request =>
          have stepped := step_release_count host s request
          unfold exec
          split
          · exact stepped
          · dsimp only
            rw [List.countP_append, Nat.add_comm (List.countP isRelease (step host s request).events),
              Nat.add_assoc, stepped]
            exact ih _

theorem retire_release_count (s : State) :
    (retire s).1.countP isRelease + releasedCount s.owner = releasedCount (retire s).2.owner := by
  unfold retire
  split <;> simp_all [isRelease, releasedCount]

/-- **P1 and the cleanup claim**: a run from a live owner commits exactly one release (guest,
destructor or host retirement), unless a failed completion lost the owner, in which case it commits
none. -/
theorem exactly_one_release_commit (host : Host) (granted : Nat) (program : List Action) :
    (run host ⟨.live, granted⟩ program).events.countP isRelease =
      if (run host ⟨.live, granted⟩ program).final.owner = .lost then 0 else 1 := by
  have executed := exec_release_count host program ⟨.live, granted⟩
  have final := run_final_owner host ⟨.live, granted⟩ program (by simp)
  have retired := retire_release_count (exec host ⟨.live, granted⟩ program).state
  rw [show releasedCount (State.owner ⟨.live, granted⟩) = 0 from rfl, Nat.add_zero] at executed
  unfold run at final ⊢
  dsimp only at final ⊢
  rw [List.countP_append, executed,
    Nat.add_comm (releasedCount (exec host ⟨.live, granted⟩ program).state.owner), retired]
  generalize (retire (exec host ⟨.live, granted⟩ program).state).2.owner = owner at final ⊢
  cases owner with
  | released who => simp [releasedCount]
  | lost => simp [releasedCount]
  | live => exact absurd rfl final.1
  | busy => exact absurd rfl final.2

theorem step_keeps_owner_without_loss (host : Host) (s : State) (request : Request)
    (completes : ∀ n, host.complete n = true) (notLost : s.owner ≠ .lost) :
    (step host s request).next.owner ≠ .lost := by
  cases request with
  | read claim off len =>
      unfold step
      cases owner : s.owner with
      | live =>
          unfold readStep
          (repeat' split) <;> simp_all
      | lost => exact absurd owner notLost
      | _ => simp [owner]
  | readOwned => simpa [step] using notLost
  | release => cases owner : s.owner <;> simp_all [step]
  | drop => cases owner : s.owner <;> simp_all [step]

/-- **Under A4 the release is exactly once**: when every completion succeeds, every run from a live
owner commits exactly one release, whatever the program and wherever it stops. -/
theorem exactly_one_release_under_completion (host : Host) (granted : Nat) (program : List Action)
    (completes : ∀ n, host.complete n = true) :
    (run host ⟨.live, granted⟩ program).events.countP isRelease = 1 := by
  rw [exactly_one_release_commit]
  have kept : ∀ (program : List Action) (s : State), s.owner ≠ .lost →
      (exec host s program).state.owner ≠ .lost := by
    intro program
    induction program with
    | nil => intro s notLost; simpa [exec] using notLost
    | cons action rest ih =>
        intro s notLost
        cases action with
        | stop reason => simpa [exec] using notLost
        | call request =>
            have next := step_keeps_owner_without_loss host s request completes notLost
            unfold exec
            split
            · exact next
            · exact ih _ next
  have reached := kept program ⟨.live, granted⟩ (by simp)
  have notLost : (run host ⟨.live, granted⟩ program).final.owner ≠ .lost := by
    unfold run retire
    dsimp only
    split <;> simp_all
  simp [notLost]

/-! ## P2–P5: the monitor

The design's monitor replays a recorded trace against the transition table. Its rules are the
prefix-safety properties:
- **P2**: no `ok` response once a release has been committed.
- **P3**: a region-read commit directly follows a read request on the authentic owner that satisfies
  the in-bounds condition.
- **P4**: every response answers the latest unanswered read request, and only once.
- **P5**: every response is in the schema. An `ok` carries exactly the requested length and follows
  its commit; an error follows no commit.

The monitor also refuses a second release commit (P1). -/

structure Mon where
  released : Bool
  pending : Option Request
  committed : Bool
  deriving DecidableEq, Repr

def Mon.start : Mon := ⟨false, none, false⟩

def watch (host : Host) (m : Mon) : Event → Option Mon
  | .req request => some { m with pending := some request, committed := false }
  | .commit (.regionRead off len) =>
      match m.pending with
      | some (.read claim o l) =>
          if claim = host.owner ∧ inBounds host.region.length o l = true ∧ o.toNat = off ∧ l.toNat = len ∧
              m.committed = false ∧ m.released = false then
            some { m with committed := true }
          else none
      | _ => none
  | .commit _ => if m.released then none else some { m with released := true }
  | .resp reply =>
      match m.pending with
      | some (.read _ _ len) =>
          match reply with
          | .ok bytes =>
              if m.released = false ∧ m.committed = true ∧ bytes.length = len.toNat then
                some { m with pending := none, committed := false }
              else none
          | _ => if m.committed = false then some { m with pending := none } else none
      | _ => none

def monitor (host : Host) : Mon → List Event → Option Mon
  | m, [] => some m
  | m, event :: rest =>
      match watch host m event with
      | some next => monitor host next rest
      | none => none

theorem monitor_append (host : Host) :
    ∀ (m : Mon) (first second : List Event),
      monitor host m (first ++ second) = (monitor host m first).bind (fun next => monitor host next second) := by
  intro m first
  induction first generalizing m with
  | nil => intro second; rfl
  | cons event rest ih =>
      intro second
      simp only [List.cons_append, monitor]
      split
      · exact ih _ second
      · rfl

/-- The monitor's view agrees with the protocol state: it has seen a release exactly when the owner
is released. -/
def Agrees (s : State) (m : Mon) : Prop :=
  (m.released = true ↔ ∃ who, s.owner = .released who) ∧ s.owner ≠ .busy

theorem step_accepted (host : Host) (s : State) (m : Mon) (request : Request) (agrees : Agrees s m) :
    ∃ next, monitor host m (step host s request).events = some next ∧ Agrees (step host s request).next next := by
  obtain ⟨releasedIff, notBusy⟩ := agrees
  cases request with
  | read claim off len =>
      unfold step
      cases owner : s.owner with
      | live =>
          have fresh : m.released = false := by
            cases flag : m.released with
            | false => rfl
            | true =>
                obtain ⟨who, released⟩ := releasedIff.mp flag
                rw [owner] at released
                cases released
          unfold readStep
          (repeat' split) <;>
            simp_all [monitor, watch, Agrees, slice_length]
      | busy => exact absurd owner notBusy
      | released who =>
          exact ⟨{ m with pending := some (.read claim off len), committed := false },
            by simp [monitor, watch], by simp_all [Agrees]⟩
      | lost =>
          exact ⟨{ m with pending := some (.read claim off len), committed := false },
            by simp [monitor, watch], by simp_all [Agrees]⟩
  | readOwned =>
      exact ⟨{ m with pending := some .readOwned, committed := false }, by simp [step, monitor, watch],
        by simp_all [step, Agrees]⟩
  | release =>
      unfold step
      cases owner : s.owner with
      | live =>
          have fresh : m.released = false := by
            cases flag : m.released with
            | false => rfl
            | true =>
                obtain ⟨who, released⟩ := releasedIff.mp flag
                rw [owner] at released
                cases released
          exact ⟨{ m with pending := some .release, committed := false, released := true },
            by simp [monitor, watch, fresh], by simp [Agrees]⟩
      | busy => exact absurd owner notBusy
      | released who =>
          exact ⟨{ m with pending := some .release, committed := false }, by simp [monitor, watch],
            by simp_all [Agrees]⟩
      | lost =>
          exact ⟨{ m with pending := some .release, committed := false }, by simp [monitor, watch],
            by simp_all [Agrees]⟩
  | drop =>
      unfold step
      cases owner : s.owner with
      | live =>
          have fresh : m.released = false := by
            cases flag : m.released with
            | false => rfl
            | true =>
                obtain ⟨who, released⟩ := releasedIff.mp flag
                rw [owner] at released
                cases released
          exact ⟨{ m with pending := some .drop, committed := false, released := true },
            by simp [monitor, watch, fresh], by simp [Agrees]⟩
      | busy => exact absurd owner notBusy
      | released who =>
          exact ⟨{ m with pending := some .drop, committed := false }, by simp [monitor, watch],
            by simp_all [Agrees]⟩
      | lost =>
          exact ⟨{ m with pending := some .drop, committed := false }, by simp [monitor, watch],
            by simp_all [Agrees]⟩

theorem exec_accepted (host : Host) :
    ∀ (program : List Action) (s : State) (m : Mon), Agrees s m →
      ∃ next, monitor host m (exec host s program).events = some next ∧ Agrees (exec host s program).state next := by
  intro program
  induction program with
  | nil => intro s m agrees; exact ⟨m, rfl, agrees⟩
  | cons action rest ih =>
      intro s m agrees
      cases action with
      | stop reason => exact ⟨m, rfl, agrees⟩
      | call request =>
          obtain ⟨middle, stepped, middleAgrees⟩ := step_accepted host s m request agrees
          unfold exec
          split
          · exact ⟨middle, stepped, middleAgrees⟩
          · obtain ⟨last, rested, lastAgrees⟩ := ih _ middle middleAgrees
            refine ⟨last, ?_, lastAgrees⟩
            rw [monitor_append, stepped]
            exact rested

theorem retire_accepted (host : Host) (s : State) (m : Mon) (agrees : Agrees s m) :
    ∃ next, monitor host m (retire s).1 = some next := by
  obtain ⟨releasedIff, _⟩ := agrees
  unfold retire
  split
  · rename_i owner
    have fresh : m.released = false := by
      cases flag : m.released with
      | false => rfl
      | true =>
          obtain ⟨who, released⟩ := releasedIff.mp flag
          rw [owner] at released
          cases released
    exact ⟨{ m with released := true }, by simp [monitor, watch, fresh]⟩
  · exact ⟨m, rfl⟩

/-- **P2–P5 hold on every run**: the monitor accepts the events of every run from a live owner, for
every host and every program, including runs that trap, exhaust a quota or are cancelled. -/
theorem run_is_accepted (host : Host) (granted : Nat) (program : List Action) :
    ∃ last, monitor host Mon.start (run host ⟨.live, granted⟩ program).events = some last := by
  have start : Agrees ⟨.live, granted⟩ Mon.start := by simp [Agrees, Mon.start]
  obtain ⟨middle, executed, middleAgrees⟩ := exec_accepted host program _ _ start
  obtain ⟨last, retired⟩ := retire_accepted host _ middle middleAgrees
  refine ⟨last, ?_⟩
  unfold run
  dsimp only
  rw [monitor_append, executed]
  exact retired

/-- The monitor's P2 rule: once a release is committed, no `ok` response passes. -/
theorem monitor_refuses_ok_after_release (host : Host) (m : Mon) (bytes : List UInt8)
    (released : m.released = true) : watch host m (.resp (.ok bytes)) = none := by
  simp only [watch]
  split
  · simp [released]
  · rfl

/-- The monitor's P3 rule: a region-read commit passes only directly after an authentic in-bounds
read request whose arguments it records. -/
theorem monitor_commit_needs_its_request (host : Host) (m next : Mon) (off len : Nat)
    (passed : watch host m (.commit (.regionRead off len)) = some next) :
    ∃ o l, m.pending = some (.read host.owner o l) ∧ inBounds host.region.length o l = true ∧
      o.toNat = off ∧ l.toNat = len ∧ m.committed = false ∧ m.released = false := by
  simp only [watch] at passed
  cases pending : m.pending with
  | none => simp [pending] at passed
  | some request =>
      cases request with
      | read claim o l =>
          rw [pending] at passed
          dsimp only at passed
          split at passed
          · rename_i checks
            obtain ⟨sameClaim, bounds, offEq, lenEq, notCommitted, notReleased⟩ := checks
            exact ⟨o, l, by rw [sameClaim], bounds, offEq, lenEq, notCommitted, notReleased⟩
          · cases passed
      | readOwned => simp [pending] at passed
      | release => simp [pending] at passed
      | drop => simp [pending] at passed

/-- The monitor's P4 and P5 rules: a response passes only as the answer to a pending read, and an
`ok` only with its commit and exactly the requested length. -/
theorem monitor_response_answers_a_read (host : Host) (m next : Mon) (reply : Reply)
    (passed : watch host m (.resp reply) = some next) :
    ∃ claim off len, m.pending = some (.read claim off len) ∧ next.pending = none ∧
      (∀ bytes, reply = .ok bytes → m.committed = true ∧ m.released = false ∧ bytes.length = len.toNat) := by
  simp only [watch] at passed
  cases pending : m.pending with
  | none => simp [pending] at passed
  | some request =>
      cases request with
      | read claim off len =>
          rw [pending] at passed
          dsimp only at passed
          refine ⟨claim, off, len, rfl, ?_, ?_⟩
          · cases reply with
            | ok bytes =>
                dsimp only at passed
                split at passed
                · cases passed
                  rfl
                · cases passed
            | boundsReject =>
                dsimp only at passed
                split at passed
                · cases passed
                  rfl
                · cases passed
            | invalidHandle =>
                dsimp only at passed
                split at passed
                · cases passed
                  rfl
                · cases passed
          · intro bytes isOk
            subst isOk
            dsimp only at passed
            split at passed
            · rename_i checks
              exact ⟨checks.2.1, checks.1, checks.2.2⟩
            · cases passed
      | readOwned => simp [pending] at passed
      | release => simp [pending] at passed
      | drop => simp [pending] at passed

/-! ## P6: nothing after an abort point -/

theorem exec_append (host : Host) :
    ∀ (first second : List Action) (s : State),
      exec host s (first ++ second) =
        match (exec host s first).stopped with
        | some _ => exec host s first
        | none =>
            ⟨(exec host s first).events ++ (exec host (exec host s first).state second).events,
              (exec host (exec host s first).state second).state,
              (exec host (exec host s first).state second).stopped⟩ := by
  intro first
  induction first with
  | nil => intro second s; simp [exec]
  | cons action rest ih =>
      intro second s
      cases action with
      | stop reason => simp [exec]
      | call request =>
          simp only [List.cons_append, exec]
          cases trapped : (step host s request).trapped with
          | true => simp
          | false =>
              simp only [Bool.false_eq_true, if_false]
              rw [ih second]
              cases stopped : (exec host (step host s request).next rest).stopped with
              | none => simp
              | some reason => simp [stopped]

/-- **An abort discards the rest of the program**: once a run stops, later actions never execute. -/
theorem stop_discards_the_rest (host : Host) (s : State) (first second : List Action)
    (stopped : (exec host s first).stopped ≠ none) : exec host s (first ++ second) = exec host s first := by
  rw [exec_append]
  cases flag : (exec host s first).stopped with
  | none => exact absurd flag stopped
  | some reason => rfl

/-- **P6**: an aborted run's events are its execution prefix followed by at most the host's
retirement. -/
theorem abort_tail (host : Host) (s : State) (program : List Action) :
    ∃ tail, (run host s program).events = (exec host s program).events ++ tail ∧
      (tail = [] ∨ tail = [.commit .hostRetire]) := by
  refine ⟨(retire (exec host s program).state).1, rfl, ?_⟩
  unfold retire
  split
  · exact Or.inr rfl
  · exact Or.inl rfl

/-! ## T3: sequential composition

A triple is an execution that reaches an identical intermediate state, counters included, and
continues normally. Traces concatenate in program order. -/

def Triple (host : Host) (s : State) (program : List Action) (s' : State) (trace : List Event) : Prop :=
  exec host s program = ⟨trace, s', none⟩

/-- **Seq is sound** over identical intermediate states. -/
theorem seq (host : Host) {s s' s'' : State} {first second : List Action} {firstTrace secondTrace : List Event}
    (firstTriple : Triple host s first s' firstTrace) (secondTriple : Triple host s' second s'' secondTrace) :
    Triple host s (first ++ second) s'' (firstTrace ++ secondTrace) := by
  unfold Triple at firstTriple secondTriple ⊢
  rw [exec_append, firstTriple]
  dsimp only
  rw [secondTriple]

/-- The read's triple from a live owner. -/
theorem read_triple (host : Host) (granted : Nat) (off len : Int)
    (bounds : inBounds host.region.length off len = true) (scope : granted < scopeLimit)
    (access : host.access granted = true) (complete : host.complete granted = true)
    (lower : host.lower granted = true) :
    Triple host ⟨.live, granted⟩ [.call (.read host.owner off len)] ⟨.live, granted + 1⟩
      [.req (.read host.owner off len), .commit (.regionRead off.toNat len.toNat),
        .resp (.ok (slice host.region off len))] := by
  have notLimit : ¬ scopeLimit ≤ granted := Nat.not_le.mpr scope
  simp [Triple, exec, step, readStep, bounds, access, complete, lower, notLimit]

/-- The release's triple from a live owner. -/
theorem release_triple (host : Host) (granted : Nat) :
    Triple host ⟨.live, granted⟩ [.call .release] ⟨.released .guest, granted⟩
      [.req .release, .commit .guestRelease] := by
  simp [Triple, exec, step]

/-- **The worked composition**: the subject's success trace is the read's trace followed by the
release's, by `seq`. -/
theorem subject_by_seq (host : Host) (granted : Nat) (off len : Int)
    (bounds : inBounds host.region.length off len = true) (scope : granted < scopeLimit)
    (access : host.access granted = true) (complete : host.complete granted = true)
    (lower : host.lower granted = true) :
    Triple host ⟨.live, granted⟩ (subject host.owner off len) ⟨.released .guest, granted + 1⟩
      ([.req (.read host.owner off len), .commit (.regionRead off.toNat len.toNat),
        .resp (.ok (slice host.region off len))] ++ [.req .release, .commit .guestRelease]) :=
  seq host (read_triple host granted off len bounds scope access complete lower)
    (release_triple host (granted + 1))

/-! ## Fixtures and controls

The fixtures use the design's region `R = 000102` and an injected owner `7`, with every host oracle
succeeding unless a fixture says otherwise. -/

def fixtureHost : Host := ⟨[0, 1, 2], 7, fun _ => true, fun _ => true, fun _ => true⟩

/-- EPROTO-CASE-01: `(1, 2)` reads `0102`, and `(3, 0)` reads the empty slice at the end. -/
theorem subject_reads_the_slice :
    (run fixtureHost initial (subject 7 1 2)).events =
        [.req (.read 7 1 2), .commit (.regionRead 1 2), .resp (.ok [1, 2]), .req .release,
          .commit .guestRelease] ∧
      (run fixtureHost initial (subject 7 3 0)).events =
        [.req (.read 7 3 0), .commit (.regionRead 3 0), .resp (.ok []), .req .release,
          .commit .guestRelease] := by
  decide

/-- EPROTO-CASE-02: `(3, 1)`, `(2, 2)`, `(-1, 1)` and `(i64::MAX, 1)` are refused as `bounds-reject`;
the last one overflows. -/
theorem out_of_bounds_reads_are_rejected :
    inBounds 3 3 1 = false ∧ inBounds 3 2 2 = false ∧ inBounds 3 (-1) 1 = false ∧
      inBounds 3 maxI64 1 = false ∧
      (run fixtureHost initial (subject 7 (-1) 1)).events =
        [.req (.read 7 (-1) 1), .resp .boundsReject, .req .release, .commit .guestRelease] := by
  decide

/-- EPROTO-CASE-03 (i): sixty-four reads pass and the sixty-fifth is refused as `invalid-handle`
with no commit. -/
theorem the_sixty_fifth_read_is_refused :
    (run fixtureHost initial (List.replicate 65 (.call (.read 7 0 1)))).events.countP
        (· == .resp (.ok [0])) = 64 ∧
      (run fixtureHost initial (List.replicate 65 (.call (.read 7 0 1)))).events.getLast? =
        some (.commit .hostRetire) ∧
      (exec fixtureHost ⟨.live, 64⟩ [.call (.read 7 0 1)]).events =
        [.req (.read 7 0 1), .resp .invalidHandle] := by
  decide +kernel

/-- EPROTO-CASE-03 (ii): a forged claim is refused as `invalid-handle`, with nothing committed and
the owner unchanged. -/
theorem forged_claim_is_refused :
    exec fixtureHost initial [.call (.read 8 0 1)] =
      ⟨[.req (.read 8 0 1), .resp .invalidHandle], initial, none⟩ := by
  decide

/-- EPROTO-CASE-04: a trap after the protected copy keeps the committed prefix, and the host retires
the owner once. -/
theorem trap_after_commit :
    run fixtureHost initial [.call (.read 7 1 2), .stop .trap] =
      ⟨[.req (.read 7 1 2), .commit (.regionRead 1 2), .resp (.ok [1, 2]), .commit .hostRetire],
        ⟨.released .host, 1⟩, .aborted .trap⟩ := by
  decide

/-- A failed lowering of the bytes into guest memory traps after the commit and before any
response. -/
theorem lowering_failure_traps_after_commit :
    run { fixtureHost with lower := fun _ => false } initial (subject 7 1 2) =
      ⟨[.req (.read 7 1 2), .commit (.regionRead 1 2), .commit .hostRetire], ⟨.released .host, 1⟩,
        .aborted .trap⟩ := by
  decide

/-- EPROTO-CASE-05 and -06: a trap before any request leaves only the retirement, and a fuel
exhaustion is classified apart from a trap. -/
theorem stops_are_classified :
    run fixtureHost initial [.stop .trap] = ⟨[.commit .hostRetire], ⟨.released .host, 0⟩, .aborted .trap⟩ ∧
      (run fixtureHost initial [.call (.read 7 1 2), .stop .fuel]).outcome = .aborted .fuel := by
  decide

/-- EPROTO-CASE-08: a guest that returns without releasing is retired by the host, not credited
with a release. -/
theorem omitted_release_is_retired :
    run fixtureHost initial [.call (.read 7 1 2)] =
      ⟨[.req (.read 7 1 2), .commit (.regionRead 1 2), .resp (.ok [1, 2]), .commit .hostRetire],
        ⟨.released .host, 1⟩, .returned⟩ := by
  decide

/-- EPROTO-CASE-09: a second release and a read after release both trap, with exactly one release
commit. -/
theorem double_release_and_use_after_release_trap :
    run fixtureHost initial [.call .release, .call .release] =
      ⟨[.req .release, .commit .guestRelease, .req .release], ⟨.released .guest, 0⟩, .aborted .trap⟩ ∧
      run fixtureHost initial [.call .release, .call (.read 7 0 1)] =
        ⟨[.req .release, .commit .guestRelease, .req (.read 7 0 1)], ⟨.released .guest, 0⟩,
          .aborted .trap⟩ := by
  decide

/-- **A counter-free composition rule is unsound** (the design's "counters do not commute"). One read
returns the owner to `live`, and sixty-four reads from a fresh `live` owner all succeed. Composed,
the last of them meets the scope limit, so the composite trace is not the concatenation. Seq needs
the identical intermediate state, counters included. -/
theorem counters_do_not_commute :
    exec fixtureHost initial [.call (.read 7 0 1)] =
        ⟨[.req (.read 7 0 1), .commit (.regionRead 0 1), .resp (.ok [0])], ⟨.live, 1⟩, none⟩ ∧
      (exec fixtureHost initial (List.replicate 64 (.call (.read 7 0 1)))).state = ⟨.live, 64⟩ ∧
      (exec fixtureHost initial ([.call (.read 7 0 1)] ++ List.replicate 64 (.call (.read 7 0 1)))).events ≠
        (exec fixtureHost initial [.call (.read 7 0 1)]).events ++
          (exec fixtureHost initial (List.replicate 64 (.call (.read 7 0 1)))).events := by
  decide +kernel

/-- **No set-union rule is derivable**: the read and the release each execute normally from a live
owner, but in the other order the read traps after the release. The two composite traces are not
permutations of each other, so a composite trace is not determined by its parts' traces as a set. -/
theorem no_set_union_rule :
    (exec fixtureHost initial [.call (.read 7 0 1)]).stopped = none ∧
      (exec fixtureHost initial [.call .release]).stopped = none ∧
      (run fixtureHost initial [.call (.read 7 0 1), .call .release]).outcome = .returned ∧
      (run fixtureHost initial [.call .release, .call (.read 7 0 1)]).outcome = .aborted .trap ∧
      ¬ (run fixtureHost initial [.call .release, .call (.read 7 0 1)]).events.Perm
        (run fixtureHost initial [.call (.read 7 0 1), .call .release]).events := by
  decide

/-- **The monitor rejects the violations it names**: a double release, an `ok` after release, an
unsolicited response, a commit without its request, a commit for a forged or out-of-bounds read, an
`ok` of the wrong length, an `ok` without its commit and a second response. -/
theorem monitor_rejects_violations :
    monitor fixtureHost Mon.start [.req .release, .commit .guestRelease, .req .release, .commit .guestRelease] = none ∧
      monitor fixtureHost Mon.start [.req .release, .commit .guestRelease, .req (.read 7 0 1), .resp (.ok [0])] = none ∧
      monitor fixtureHost Mon.start [.resp .boundsReject] = none ∧
      monitor fixtureHost Mon.start [.commit (.regionRead 0 1)] = none ∧
      monitor fixtureHost Mon.start [.req (.read 8 0 1), .commit (.regionRead 0 1)] = none ∧
      monitor fixtureHost Mon.start [.req (.read 7 3 1), .commit (.regionRead 3 1)] = none ∧
      monitor fixtureHost Mon.start [.req (.read 7 0 1), .commit (.regionRead 0 1), .resp (.ok [0, 1])] = none ∧
      monitor fixtureHost Mon.start [.req (.read 7 0 1), .resp (.ok [0])] = none ∧
      monitor fixtureHost Mon.start
        [.req (.read 7 0 1), .commit (.regionRead 0 1), .resp (.ok [0]), .resp (.ok [0])] = none := by
  decide

end NobleFoundation.RegionProtocol
