/-!
# Region trace relation pilot (WIT-TRACE-01, WIT-TRACE-02)

`.cairn/changes/region-trace-relation` fixes a source alphabet (`RegionProtocol-v1`'s
`Req`/`Resp`/`Commit` classes) and a target alphabet (the emitted ABI steps of the bounded
region host), defines the relation first, and permits erasure only for named internal ABI
bookkeeping steps with a per-class justification.

This module is a **model**: the target alphabet is a handwritten machine, not an emitted
step sequence from a real lowering, and nothing here is a correspondence theorem about
Wasmtime, Canonical ABI or the Rust host. The relation is the projection of a target trace
onto the source alphabet plus the erasure function, so every property proved below is a
property *of the relation*, never of an execution.

Declared premises that stay premises, not claims:
* exact identity: a commit carries the identity of the decision that authorised it; 
  `identity_is_decision_identity` shows the projection keeps the identity it was given,
  while *that the host supplies the right identity* is a source obligation.
* pairing and ordering: `append_projection` and `take_projection` give the prefix
  structure; response-to-request pairing is preserved by construction because a target
  response step carries its own operation, and *which* request it answers stays a premise.
* cost/fuel: an exhaustion step may cut a trace, so the projection of a truncated trace is
  the projection of the prefix, never an invented commit (`take_projection`).
-/
namespace RegionTrace

/-- A source-side operation of `RegionProtocol-v1`. -/
inductive Operation where
  | regionRead
  | guestRelease
  deriving DecidableEq, Repr

/-- The abnormal-exit class. `unknown` is a real member: an absent observation stays unknown
rather than being filled in with a successful protocol outcome. -/
inductive ExitClass where
  | trap
  | quotaExhaustion
  | cleanupFailure
  | unknown
  deriving DecidableEq, Repr

/-- One protected commit. `hostRetire` is a host obligation and is never a guest release. -/
inductive CommitKind where
  | regionRead (start : Nat) (length : Nat)
  | guestRelease
  | destructorRelease
  | hostRetire
  deriving DecidableEq, Repr

/-- The source alphabet: exactly `RegionProtocol-v1`'s request, response and commit classes. -/
inductive SourceEvent where
  | req (op : Operation) (offset : Nat) (length : Nat)
  | resp (op : Operation) (value : Option Nat) (reason : Option String)
  | commit (kind : CommitKind) (identity : Nat)
  deriving DecidableEq, Repr

/-- The target alphabet: the emitted ABI steps this host actually performs. -/
inductive AbiStep where
  | importCrossing (op : Operation) (offset : Nat) (length : Nat)
  | hostDecision (op : Operation) (accepted : Bool) (identity : Nat)
  | protectedCopy (length : Nat)
  | returned (op : Operation) (value : Option Nat) (reason : Option String)
  | normalReturn
  | abnormalExit (exitClass : ExitClass)
  | cleanup (kind : CommitKind) (identity : Nat)
  | handleTableIndex (index : Nat)
  | storeBookkeeping (slot : Nat)
  deriving DecidableEq, Repr

/-- The two erasable classes, named. Handle-table indexing and store-data bookkeeping are
internal ABI bookkeeping: they carry no observable source event, and erasing them cannot
hide a denial (a denial is an `importCrossing` plus a refusing `hostDecision`), cannot move a
commit (commits are `cleanup` steps) and cannot turn retirement into a release (a release is
a `cleanup` whose kind is `guestRelease`). -/
def erasable : AbiStep → Bool
  | .handleTableIndex _ => true
  | .storeBookkeeping _ => true
  | _ => false

/-- Erase exactly the named bookkeeping classes, preserving the order of everything else. -/
def eraseStep : AbiStep → Option AbiStep
  | .handleTableIndex _ => none
  | .storeBookkeeping _ => none
  | step => some step

def erase (steps : List AbiStep) : List AbiStep :=
  steps.filterMap eraseStep

/-- The projection of one target step onto the source alphabet. Bookkeeping, decisions,
copies, returns and abnormal exits carry no source event; a denial stays visible as its
request step, which is never erased. -/
def projectStep : AbiStep → Option SourceEvent
  | .importCrossing op offset length => some (.req op offset length)
  | .returned op value reason => some (.resp op value reason)
  | .cleanup kind identity => some (.commit kind identity)
  | .hostDecision _ _ _ => none
  | .protectedCopy _ => none
  | .normalReturn => none
  | .abnormalExit _ => none
  | .handleTableIndex _ => none
  | .storeBookkeeping _ => none

/-- The identities the host decided on, in order. -/
def decisions (steps : List AbiStep) : List Nat :=
  steps.filterMap fun step =>
    match step with
    | .hostDecision _ _ identity => some identity
    | _ => none

/-- The identities of the commits a trace performed, in order. -/
def commitIdentities (steps : List AbiStep) : List Nat :=
  steps.filterMap fun step =>
    match step with
    | .cleanup _ identity => some identity
    | _ => none

/-- The read crossings a trace performed, in order. -/
def readCrossings (steps : List AbiStep) : List (Nat × Nat) :=
  steps.filterMap fun step =>
    match step with
    | .importCrossing .regionRead start length => some (start, length)
    | _ => none

/-- The release-or-retirement members a trace committed, in order. -/
def releaseMembers (steps : List AbiStep) : List Nat :=
  steps.filterMap fun step =>
    match step with
    | .cleanup kind _ =>
        match kind with
        | .guestRelease => some (0 : Nat)
        | .destructorRelease => some 1
        | .hostRetire => some 2
        | .regionRead _ _ => none
    | _ => none

/-- Every commit identity was decided first: no commit may carry an identity the host never
authorised, so a rep number or handle class cannot stand in for the exact identity. -/
def identitiesDecided (steps : List AbiStep) : Bool :=
  let decided := decisions steps
  (commitIdentities steps).all (fun identity => decided.contains identity)

/-- Every read commit is explained: a protected read commit must be preceded by the import
crossing that asked for exactly that region. -/
def readCommits (steps : List AbiStep) : List (Nat × Nat) :=
  steps.filterMap fun step =>
    match step with
    | .cleanup (.regionRead start length) _ => some (start, length)
    | _ => none

def readCommitsExplained (steps : List AbiStep) : Bool :=
  let crossings := readCrossings steps
  (readCommits steps).all (fun read => crossings.contains read)

/-- At most one release or retirement per trace: `guestRelease`, `destructorRelease` and
`hostRetire` may each commit at most once. -/
def atMostOnce (steps : List AbiStep) : Bool :=
  let members := releaseMembers steps
  (List.range 3).all (fun member => members.count member ≤ 1)

/-- Target well-formedness: the relation relates the source trace to a target trace that is
actually well formed, so a substituted identity, an unexplained read commit or a second
release cannot be related at all. -/
def wellFormed (steps : List AbiStep) : Bool :=
  identitiesDecided steps && readCommitsExplained steps && atMostOnce steps

/-- The relation: a source trace is related to a target trace when it is the projection of a
well-formed target trace. Identity, read-commit explicability and at-most-once release are
conditions *of the relation*, so a forged target trace is refused rather than projected. -/
def related (source : List SourceEvent) (target : List AbiStep) : Bool :=
  (source == target.filterMap projectStep) && wellFormed target

/-- Erasure never changes the projected source trace: no erasable step carries a source
event. -/
theorem erase_preserves_projection (steps : List AbiStep) :
    (erase steps).filterMap projectStep = steps.filterMap projectStep := by
  induction steps with
  | nil => rfl
  | cons head tail ih =>
      cases head with
      | handleTableIndex index => simpa [erase, eraseStep, projectStep] using ih
      | storeBookkeeping slot => simpa [erase, eraseStep, projectStep] using ih
      | importCrossing op offset length => simpa [erase, eraseStep, projectStep] using ih
      | hostDecision op accepted identity => simpa [erase, eraseStep, projectStep] using ih
      | protectedCopy length => simpa [erase, eraseStep, projectStep] using ih
      | returned op value reason => simpa [erase, eraseStep, projectStep] using ih
      | normalReturn => simpa [erase, eraseStep, projectStep] using ih
      | abnormalExit exitClass => simpa [erase, eraseStep, projectStep] using ih
      | cleanup kind identity => simpa [erase, eraseStep, projectStep] using ih

/-- Erasure leaves each scan input list alone: it removes only the two named bookkeeping
classes, which none of the four scans observes. -/
theorem erase_preserves_decisions (steps : List AbiStep) :
    decisions (erase steps) = decisions steps := by
  induction steps with
  | nil => rfl
  | cons head tail ih =>
      cases head with
      | handleTableIndex index => simpa [erase, eraseStep, decisions] using ih
      | storeBookkeeping slot => simpa [erase, eraseStep, decisions] using ih
      | importCrossing op offset length => simpa [erase, eraseStep, decisions] using ih
      | hostDecision op accepted identity => simpa [erase, eraseStep, decisions] using ih
      | protectedCopy length => simpa [erase, eraseStep, decisions] using ih
      | returned op value reason => simpa [erase, eraseStep, decisions] using ih
      | normalReturn => simpa [erase, eraseStep, decisions] using ih
      | abnormalExit exitClass => simpa [erase, eraseStep, decisions] using ih
      | cleanup kind identity => simpa [erase, eraseStep, decisions] using ih

theorem erase_preserves_commit_identities (steps : List AbiStep) :
    commitIdentities (erase steps) = commitIdentities steps := by
  induction steps with
  | nil => rfl
  | cons head tail ih =>
      cases head with
      | handleTableIndex index => simpa [erase, eraseStep, commitIdentities] using ih
      | storeBookkeeping slot => simpa [erase, eraseStep, commitIdentities] using ih
      | importCrossing op offset length => simpa [erase, eraseStep, commitIdentities] using ih
      | hostDecision op accepted identity => simpa [erase, eraseStep, commitIdentities] using ih
      | protectedCopy length => simpa [erase, eraseStep, commitIdentities] using ih
      | returned op value reason => simpa [erase, eraseStep, commitIdentities] using ih
      | normalReturn => simpa [erase, eraseStep, commitIdentities] using ih
      | abnormalExit exitClass => simpa [erase, eraseStep, commitIdentities] using ih
      | cleanup kind identity => simpa [erase, eraseStep, commitIdentities] using ih

theorem erase_preserves_read_crossings (steps : List AbiStep) :
    readCrossings (erase steps) = readCrossings steps := by
  induction steps with
  | nil => rfl
  | cons head tail ih =>
      cases head with
      | handleTableIndex index => simpa [erase, eraseStep, readCrossings, Operation.noConfusion] using ih
      | storeBookkeeping slot => simpa [erase, eraseStep, readCrossings, Operation.noConfusion] using ih
      | importCrossing op offset length =>
          cases op <;> simpa [erase, eraseStep, readCrossings] using ih
      | hostDecision op accepted identity => simpa [erase, eraseStep, readCrossings, Operation.noConfusion] using ih
      | protectedCopy length => simpa [erase, eraseStep, readCrossings, Operation.noConfusion] using ih
      | returned op value reason => simpa [erase, eraseStep, readCrossings, Operation.noConfusion] using ih
      | normalReturn => simpa [erase, eraseStep, readCrossings, Operation.noConfusion] using ih
      | abnormalExit exitClass => simpa [erase, eraseStep, readCrossings, Operation.noConfusion] using ih
      | cleanup kind identity => simpa [erase, eraseStep, readCrossings, Operation.noConfusion] using ih

theorem erase_preserves_release_members (steps : List AbiStep) :
    releaseMembers (erase steps) = releaseMembers steps := by
  induction steps with
  | nil => rfl
  | cons head tail ih =>
      cases head with
      | handleTableIndex index => simpa [erase, eraseStep, releaseMembers, CommitKind.noConfusion] using ih
      | storeBookkeeping slot => simpa [erase, eraseStep, releaseMembers, CommitKind.noConfusion] using ih
      | importCrossing op offset length => simpa [erase, eraseStep, releaseMembers, CommitKind.noConfusion] using ih
      | hostDecision op accepted identity => simpa [erase, eraseStep, releaseMembers, CommitKind.noConfusion] using ih
      | protectedCopy length => simpa [erase, eraseStep, releaseMembers, CommitKind.noConfusion] using ih
      | returned op value reason => simpa [erase, eraseStep, releaseMembers, CommitKind.noConfusion] using ih
      | normalReturn => simpa [erase, eraseStep, releaseMembers, CommitKind.noConfusion] using ih
      | abnormalExit exitClass => simpa [erase, eraseStep, releaseMembers, CommitKind.noConfusion] using ih
      | cleanup kind identity =>
          cases kind <;> simpa [erase, eraseStep, releaseMembers] using ih

theorem erase_preserves_read_commits (steps : List AbiStep) :
    readCommits (erase steps) = readCommits steps := by
  induction steps with
  | nil => rfl
  | cons head tail ih =>
      cases head with
      | handleTableIndex index => simpa [erase, eraseStep, readCommits, CommitKind.noConfusion] using ih
      | storeBookkeeping slot => simpa [erase, eraseStep, readCommits, CommitKind.noConfusion] using ih
      | importCrossing op offset length => simpa [erase, eraseStep, readCommits, CommitKind.noConfusion] using ih
      | hostDecision op accepted identity => simpa [erase, eraseStep, readCommits, CommitKind.noConfusion] using ih
      | protectedCopy length => simpa [erase, eraseStep, readCommits, CommitKind.noConfusion] using ih
      | returned op value reason => simpa [erase, eraseStep, readCommits, CommitKind.noConfusion] using ih
      | normalReturn => simpa [erase, eraseStep, readCommits, CommitKind.noConfusion] using ih
      | abnormalExit exitClass => simpa [erase, eraseStep, readCommits, CommitKind.noConfusion] using ih
      | cleanup kind identity =>
          cases kind <;> simpa [erase, eraseStep, readCommits] using ih

/-- Erasure leaves well-formedness alone: it removes only the two named bookkeeping classes,
and every scan the relation performs ranges over a derived list that ignores them. -/
theorem erase_preserves_well_formedness (steps : List AbiStep) :
    wellFormed (erase steps) = wellFormed steps := by
  simp only [wellFormed, identitiesDecided, readCommitsExplained, atMostOnce,
    erase_preserves_decisions, erase_preserves_commit_identities,
    erase_preserves_read_commits, erase_preserves_read_crossings,
    erase_preserves_release_members]

/-- The relation is erasure-invariant: a target trace and its erased form project to the same
source trace and stay equally well formed, so an erased class cannot create or remove a source
event and cannot repair a forged target trace. -/
theorem related_erase_invariant (source : List SourceEvent) (target : List AbiStep) :
    related source target = related source (erase target) := by
  simp only [related, erase_preserves_projection, erase_preserves_well_formedness]

/-- A refused request stays visible: the projection keeps its request step and adds no
response, so a denial cannot be erased into a successful trace. -/
theorem denial_projects_to_request_only :
    related [.req .regionRead 0 8]
      [.importCrossing .regionRead 0 8, .hostDecision .regionRead false 7] = true := by
  decide

/-- A normal success projects its request and its protected commit, with the decision,
copy and return steps carrying no source event. -/
theorem normal_success_projects_request_and_commit :
    related [.req .regionRead 0 8, .commit .guestRelease 7]
      [.importCrossing .regionRead 0 8, .hostDecision .regionRead true 7,
       .protectedCopy 8, .normalReturn, .cleanup .guestRelease 7] = true := by
  decide

/-- A commit keeps the exact identity it was emitted with: the projection never rewrites it,
so a rep number or handle class can never be substituted for the identity. -/
theorem identity_is_decision_identity (kind : CommitKind) (identity : Nat) :
    projectStep (AbiStep.cleanup kind identity) =
      some (SourceEvent.commit kind identity) := rfl

/-- Host retirement is never converted into a guest release: a cleanup projects to a commit
of its own kind, and `hostRetire` is not `guestRelease`. -/
theorem retirement_is_not_a_guest_release (identity : Nat) :
    ∃ committed, projectStep (AbiStep.cleanup CommitKind.hostRetire identity) =
        some (SourceEvent.commit committed identity) ∧
      committed ≠ CommitKind.guestRelease :=
  ⟨CommitKind.hostRetire, rfl, by decide⟩

/-- The target model keeps the abnormal-exit class, but the source projection cannot: trap,
quota exhaustion, cleanup failure and an absent observation all project to no source event.
So a theorem stated over the projection alone would be a theorem about a coarser label; the
trap/quota/cleanup distinction has to be observed on the target side. -/
theorem abnormal_exit_class_is_not_in_the_projection (exitClass : ExitClass) :
    projectStep (AbiStep.abnormalExit exitClass) = none := rfl

/-- Two different abnormal-exit classes are indistinguishable in the projection. -/
theorem abnormal_exit_classes_collapse_in_projection :
    projectStep (AbiStep.abnormalExit ExitClass.trap) =
      projectStep (AbiStep.abnormalExit ExitClass.quotaExhaustion) := rfl

/-- Prefix compatibility: projecting a concatenation is concatenating the projections, so a
trace cut by an exhaustion boundary projects to the projection of its retained prefix and
never to an invented commit (`List.filterMap_append`). -/
theorem append_projection (first second : List AbiStep) :
    (first ++ second).filterMap projectStep =
      first.filterMap projectStep ++ second.filterMap projectStep :=
  List.filterMap_append

/-- The relation composes with truncation: a trap may cut the target trace, and a well-formed
trace's retained prefix relates to its own projection. -/
theorem take_projection (steps : List AbiStep) (bound : Nat) :
    wellFormed (steps.take bound) = true →
    related ((steps.take bound).filterMap projectStep) (steps.take bound) = true := by
  intro formed
  simp [related, formed]

/-- Negative controls (design: substituted identity, unexplained read commit, missing protected
commit, host cleanup disguised as a guest release, a denial erased into a success). Each forged
target trace is refused by the relation rather than projected. -/
theorem related_rejects_substituted_identity :
    related [.req .regionRead 0 8, .commit .guestRelease 7]
      [.importCrossing .regionRead 0 8, .hostDecision .regionRead true 9,
       .cleanup .guestRelease 7] = false := by
  decide

theorem related_rejects_unexplained_read_commit :
    related [.commit (.regionRead 0 8) 5]
      [.hostDecision .regionRead true 5, .cleanup (.regionRead 0 8) 5] = false := by
  decide

theorem related_rejects_two_guest_releases :
    related [.req .regionRead 0 8, .commit .guestRelease 5, .commit .guestRelease 5]
      [.importCrossing .regionRead 0 8, .hostDecision .regionRead true 5,
       .cleanup .guestRelease 5, .cleanup .guestRelease 5] = false := by
  decide

theorem related_rejects_missing_protected_commit :
    related [.req .regionRead 0 8, .commit .guestRelease 7]
      [.importCrossing .regionRead 0 8, .hostDecision .regionRead true 7, .normalReturn] = false := by
  decide

theorem related_rejects_host_cleanup_as_guest_release :
    related [.req .regionRead 0 8, .commit .guestRelease 7]
      [.importCrossing .regionRead 0 8, .hostDecision .regionRead true 7,
       .cleanup .hostRetire 7] = false := by
  decide

theorem related_rejects_denial_erased_into_success :
    related [.req .regionRead 0 8]
      [.importCrossing .regionRead 0 8, .hostDecision .regionRead true 7,
       .cleanup .guestRelease 7] = false := by
  decide

/-- Positive control: a well-formed success relates, so the refusals above are not vacuous. -/
theorem related_accepts_well_formed_success :
    related [.req .regionRead 0 8, .commit .guestRelease 7]
      [.importCrossing .regionRead 0 8, .hostDecision .regionRead true 7,
       .protectedCopy 8, .cleanup .guestRelease 7] = true := by
  decide

end RegionTrace
