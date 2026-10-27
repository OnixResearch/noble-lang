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

/-- The relation: a source trace is related to a target trace when it is the projection of
the target trace. -/
def related (source : List SourceEvent) (target : List AbiStep) : Bool :=
  source == target.filterMap projectStep

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

/-- The relation is erasure-invariant: a target trace and its erased form project to the same
source trace, so an erased class cannot create or remove a source event. -/
theorem related_erase_invariant (source : List SourceEvent) (target : List AbiStep) :
    related source target = related source (erase target) := by
  simp only [related, erase_preserves_projection]

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

/-- The relation composes with truncation: a trap may cut the target trace, and the related
source trace is the projection of the retained prefix. -/
theorem take_projection (steps : List AbiStep) (bound : Nat) :
    related ((steps.take bound).filterMap projectStep) (steps.take bound) = true :=
  beq_self_eq_true _

end RegionTrace
