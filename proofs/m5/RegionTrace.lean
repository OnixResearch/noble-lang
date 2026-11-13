/-!
# Region trace relation pilot (WIT-TRACE-01, WIT-TRACE-02)

`.cairn/changes/region-trace-relation` fixes a source alphabet (`RegionProtocol-v1`'s
`Req`/`Resp`/`Commit` classes) and a target alphabet (the emitted ABI steps of the bounded
region host, abstracted as a handwritten model), defines the relation first, and permits
erasure only for named internal ABI bookkeeping steps with a per-class justification.

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
  structure; `responsesPaired` makes each projected response answer the preceding request
  of the same operation, while permitting an unmatched final request at a truncated prefix.
* cost/fuel: an exhaustion step may cut a trace, so the projection of a truncated trace is
  the projection of the prefix, never an invented commit (`take_projection`).
-/
namespace RegionTrace

/-- A source-side operation of `RegionProtocol-v1`. -/
inductive Operation where
  | regionRead
  | guestRelease
  deriving DecidableEq, Repr

/-- The abnormal-exit class, matching the classes the host itself reports for a run that did not end
in a conformant protocol return: a guest trap, the host's fuel boundary, a cancellation the host
performed, an adapter fault that is not a guest trap, a result outside the result schema, and a
refused post-exit retirement. `normal` and `rejected` are conformant returns whose outcome is
already in the responses, so they add no exit step. `unknown` is a real member: an absent
observation stays unknown rather than being filled in with a successful protocol outcome. -/
inductive ExitClass where
  | trap
  | quotaExhaustion
  | cancelled
  | adapterInvalid
  | outsideModel
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

/-- The source alphabet: exactly `RegionProtocol-v1`'s request, response and commit classes.
A request carries the guest's signed (`s64`) arguments as `Int`, so a refused negative argument is
represented as itself (#90). A release request has no arguments, and the model writes them `0 0`.
A commit's region stays a `Nat` pair: the host commits only a region it admitted. -/
inductive SourceEvent where
  | req (op : Operation) (offset : Int) (length : Int)
  | resp (op : Operation) (value : Option Nat) (reason : Option String)
  | commit (kind : CommitKind) (identity : Nat)
  deriving DecidableEq, Repr

/-- A handwritten target model of the bounded host's ABI steps, not an emitted component trace. -/
inductive AbiStep where
  | importCrossing (op : Operation) (offset : Int) (length : Int)
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

/-! Every response must follow a request of its own operation, with only that request's own
protected commits in between. Prefixes may end with that request unanswered, but a response before
any request, a response for another operation, a differently-kinded commit, or another request
before the answer is not paired. -/
def commitMatchesRequest (op : Operation) (offset length : Int) : CommitKind → Bool
  | .regionRead start size =>
      decide (op = .regionRead ∧ offset = (start : Int) ∧ length = (size : Int))
  | .guestRelease => decide (op = .guestRelease ∧ offset = 0 ∧ length = 0)
  | .destructorRelease | .hostRetire => false

mutual
/-- A projected prefix is paired when every response answers the preceding request. -/
def responsesPaired : List SourceEvent → Bool
  | [] => true
  | .resp _ _ _ :: _ => false
  | .req op offset length :: rest => responseFollows op offset length rest
  | .commit kind _ :: rest =>
      match kind with
      | .regionRead _ _ | .guestRelease => false
      | .destructorRelease | .hostRetire => responsesPaired rest

/-- The response a request expects, after any number of that request's protected commits. -/
def responseFollows (op : Operation) (offset length : Int) : List SourceEvent → Bool
  | [] => true
  | .commit kind _ :: rest =>
      commitMatchesRequest op offset length kind && responseFollows op offset length rest
  | .resp other _ _ :: more => decide (op = other) && responsesPaired more
  | .req _ _ _ :: _ => false
end

/-- The identities from accepted host decisions, in order. -/
def decisionStep : AbiStep → Option Nat
  | .hostDecision _ true identity => some identity
  | _ => none

def decisions (steps : List AbiStep) : List Nat :=
  steps.filterMap decisionStep

/-- The identities of the commits a trace performed, in order. -/
def commitStep : AbiStep → Option Nat
  | .cleanup _ identity => some identity
  | _ => none

def commitIdentities (steps : List AbiStep) : List Nat :=
  steps.filterMap commitStep

/-- The read crossings a trace performed, in order, with their signed arguments. -/
def readCrossings (steps : List AbiStep) : List (Int × Int) :=
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

/-- Every commit identity occurs among the accepted host decisions; ordering is checked separately
by `commitAuthorityInOrder`, and read/release commits are additionally paired to their own request. -/
def identitiesDecided (steps : List AbiStep) : Bool :=
  let decided := decisions steps
  (commitIdentities steps).all (fun identity => decided.contains identity)

/-! A decision can authorize only commits that follow it. Registration is a separate premise and
authorizes host retirement only; it is not folded into the decision list. -/
def commitAuthorityInOrder (registered : List Nat) : List Nat → List AbiStep → Bool
  | _, [] => true
  | decided, .hostDecision _ accepted identity :: rest =>
      commitAuthorityInOrder registered (if accepted then identity :: decided else decided) rest
  | decided, .cleanup .hostRetire identity :: rest =>
      (decided.contains identity || registered.contains identity) &&
        commitAuthorityInOrder registered decided rest
  | decided, .cleanup _ identity :: rest =>
      decided.contains identity && commitAuthorityInOrder registered decided rest
  | decided, _ :: rest => commitAuthorityInOrder registered decided rest

/-- Every read commit is explained: a protected read commit must be preceded by the import
crossing that asked for exactly that region. -/
def readCommits (steps : List AbiStep) : List (Nat × Nat) :=
  steps.filterMap fun step =>
    match step with
    | .cleanup (.regionRead start length) _ => some (start, length)
    | _ => none

/-! A read-side witness stream makes request/decision/copy/commit/response order explicit while
ignoring unrelated target events. The erasure scan removes no member of this stream. -/
inductive ReadTraceEvent where
  | request (offset length : Int)
  | decision (accepted : Bool) (identity : Nat)
  | copy (length : Nat)
  | commit (start length identity : Nat)
  | response (value : Option Nat) (reason : Option String)
  deriving DecidableEq, Repr

def readTraceStep : AbiStep → Option ReadTraceEvent
  | .importCrossing .regionRead offset length => some (.request offset length)
  | .hostDecision .regionRead accepted identity => some (.decision accepted identity)
  | .protectedCopy length => some (.copy length)
  | .cleanup (.regionRead start length) identity => some (.commit start length identity)
  | .returned .regionRead value reason => some (.response value reason)
  | _ => none

def readTraceEvents (steps : List AbiStep) : List ReadTraceEvent :=
  steps.filterMap readTraceStep

mutual
def readTracePaired : List ReadTraceEvent → Bool
  | [] => true
  | .request offset length :: rest =>
      readTraceResponseFollows offset length none false false rest
  | _ :: _ => false

def readTraceResponseFollows (offset length : Int) (decision : Option (Bool × Nat))
    (copied committed : Bool) : List ReadTraceEvent → Bool
  | [] => true
  | .request _ _ :: _ => false
  | .decision accepted identity :: rest =>
      match decision with
      | none => readTraceResponseFollows offset length (some (accepted, identity)) copied committed rest
      | some _ => false
  | .copy copyLength :: rest =>
      match decision with
      | some (true, _) =>
          decide (length = (copyLength : Int)) && !copied &&
            readTraceResponseFollows offset length decision true committed rest
      | _ => false
  | .commit start size identity :: rest =>
      match decision with
      | some (true, decisionIdentity) =>
          decide (offset = (start : Int) ∧ length = (size : Int) ∧ identity = decisionIdentity) &&
            copied && !committed &&
            readTraceResponseFollows offset length decision copied true rest
      | _ => false
  | .response value reason :: rest =>
      decide (value.isSome = committed ∧ value.isSome = reason.isNone) && readTracePaired rest
end

def readCommitsExplained (steps : List AbiStep) : Bool :=
  readTracePaired (readTraceEvents steps)

/-! Guest-release commits use their own request and accepted identity. A registered owner may
authorize `hostRetire`, but it cannot supply a guest-release request or commit. -/
inductive ReleaseTraceEvent where
  | request (offset length : Int)
  | decision (accepted : Bool) (identity : Nat)
  | commit (identity : Nat)
  | response (value : Option Nat) (reason : Option String)
  deriving DecidableEq, Repr

def releaseTraceStep : AbiStep → Option ReleaseTraceEvent
  | .importCrossing .guestRelease offset length => some (.request offset length)
  | .hostDecision .guestRelease accepted identity => some (.decision accepted identity)
  | .cleanup .guestRelease identity => some (.commit identity)
  | .returned .guestRelease value reason => some (.response value reason)
  | _ => none

def releaseTraceEvents (steps : List AbiStep) : List ReleaseTraceEvent :=
  steps.filterMap releaseTraceStep

mutual
def releaseTracePaired : List ReleaseTraceEvent → Bool
  | [] => true
  | .request offset length :: rest =>
      decide (offset = 0 ∧ length = 0) && releaseTraceResponseFollows none false rest
  | _ :: _ => false

def releaseTraceResponseFollows (decision : Option (Bool × Nat)) (committed : Bool) :
    List ReleaseTraceEvent → Bool
  | [] => true
  | .request _ _ :: _ => false
  | .decision accepted identity :: rest =>
      match decision with
      | none => releaseTraceResponseFollows (some (accepted, identity)) committed rest
      | some _ => false
  | .commit identity :: rest =>
      match decision with
      | some (true, decisionIdentity) =>
          decide (identity = decisionIdentity) && !committed &&
            releaseTraceResponseFollows decision true rest
      | _ => false
  | .response value reason :: rest =>
      decide (value.isNone && (!committed || reason.isNone)) && releaseTracePaired rest
end

def releaseCommitsExplained (steps : List AbiStep) : Bool :=
  releaseTracePaired (releaseTraceEvents steps)

/-! A successful read response cannot erase its protected commit, and a refused read cannot invent
one. Every commit is paired with the exact preceding signed request, accepted decision identity and
copy length; a prefix may stop before the response without changing the committed prefix. -/

/-- At most one release or retirement per trace: of `guestRelease`, `destructorRelease` and
`hostRetire`, a trace commits at most one, so a guest release followed by a host retirement of
the same region is a second release. (The earlier definition bounded each kind separately and
accepted that pair.) -/
def atMostOnce (steps : List AbiStep) : Bool :=
  decide ((releaseMembers steps).length ≤ 1)

/-- Target well-formedness: accepted and ordered identities, request-bound read/release commits,
release count and projected response pairing are conditions of the relation. An unmatched final
request is allowed as a truncated prefix. -/
def wellFormed (steps : List AbiStep) : Bool :=
  identitiesDecided steps && commitAuthorityInOrder [] [] steps &&
    readCommitsExplained steps && releaseCommitsExplained steps && atMostOnce steps &&
    responsesPaired (steps.filterMap projectStep)

/-- The relation: a source trace is related to a target trace when it is the projection of a
well-formed target trace. Identity, read-commit explicability, at-most-once release and response
pairing are conditions *of the relation*, so a forged target trace is refused rather than projected. -/
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
      | handleTableIndex index => simpa [erase, eraseStep, projectStep, List.filterMap_cons] using ih
      | storeBookkeeping slot => simpa [erase, eraseStep, projectStep, List.filterMap_cons] using ih
      | importCrossing op offset length => simpa [erase, eraseStep, projectStep] using ih
      | hostDecision op accepted identity => simpa [erase, eraseStep, projectStep, List.filterMap_cons] using ih
      | protectedCopy length => simpa [erase, eraseStep, projectStep, List.filterMap_cons] using ih
      | returned op value reason => simpa [erase, eraseStep, projectStep] using ih
      | normalReturn => simpa [erase, eraseStep, projectStep, List.filterMap_cons] using ih
      | abnormalExit exitClass => simpa [erase, eraseStep, projectStep, List.filterMap_cons] using ih
      | cleanup kind identity => simpa [erase, eraseStep, projectStep] using ih

/-- Erasure leaves each scan input list alone: it removes only the two named bookkeeping
classes, which none of the four scans observes. -/
theorem erase_preserves_decisions (steps : List AbiStep) :
    decisions (erase steps) = decisions steps := by
  induction steps with
  | nil => rfl
  | cons head tail ih =>
      cases head with
      | handleTableIndex index => simpa [erase, eraseStep, decisions, decisionStep, List.filterMap_cons] using ih
      | storeBookkeeping slot => simpa [erase, eraseStep, decisions, decisionStep, List.filterMap_cons] using ih
      | importCrossing op offset length => simpa [erase, eraseStep, decisions, decisionStep, List.filterMap_cons] using ih
      | hostDecision op accepted identity =>
          cases accepted <;>
            simpa [erase, eraseStep, decisions, decisionStep, List.filterMap_cons] using ih
      | protectedCopy length => simpa [erase, eraseStep, decisions, decisionStep, List.filterMap_cons] using ih
      | returned op value reason => simpa [erase, eraseStep, decisions, decisionStep, List.filterMap_cons] using ih
      | normalReturn => simpa [erase, eraseStep, decisions, decisionStep, List.filterMap_cons] using ih
      | abnormalExit exitClass => simpa [erase, eraseStep, decisions, decisionStep, List.filterMap_cons] using ih
      | cleanup kind identity => simpa [erase, eraseStep, decisions, decisionStep, List.filterMap_cons] using ih

theorem erase_preserves_commit_authority (registered decided : List Nat) (steps : List AbiStep) :
    commitAuthorityInOrder registered decided (erase steps) =
      commitAuthorityInOrder registered decided steps := by
  induction steps generalizing registered decided with
  | nil => rfl
  | cons head tail ih =>
      have tailAuthority := ih
      simp only [erase] at tailAuthority
      cases head with
      | handleTableIndex index =>
          simp [erase, eraseStep, commitAuthorityInOrder, List.filterMap_cons,
            tailAuthority registered decided]
      | storeBookkeeping slot =>
          simp [erase, eraseStep, commitAuthorityInOrder, List.filterMap_cons,
            tailAuthority registered decided]
      | importCrossing op offset length =>
          simp [erase, eraseStep, commitAuthorityInOrder, List.filterMap_cons,
            tailAuthority registered decided]
      | hostDecision op accepted identity =>
          simp [erase, eraseStep, commitAuthorityInOrder, List.filterMap_cons,
            tailAuthority registered (if accepted then identity :: decided else decided)]
      | protectedCopy length =>
          simp [erase, eraseStep, commitAuthorityInOrder, List.filterMap_cons,
            tailAuthority registered decided]
      | returned op value reason =>
          simp [erase, eraseStep, commitAuthorityInOrder, List.filterMap_cons,
            tailAuthority registered decided]
      | normalReturn =>
          simp [erase, eraseStep, commitAuthorityInOrder, List.filterMap_cons,
            tailAuthority registered decided]
      | abnormalExit exitClass =>
          simp [erase, eraseStep, commitAuthorityInOrder, List.filterMap_cons,
            tailAuthority registered decided]
      | cleanup kind identity =>
          cases kind <;>
            simp [erase, eraseStep, commitAuthorityInOrder, List.filterMap_cons,
              tailAuthority registered decided]

theorem commit_authority_mono (registered decided : List Nat) (steps : List AbiStep)
    (h : commitAuthorityInOrder [] decided steps = true) :
    commitAuthorityInOrder registered decided steps = true := by
  induction steps generalizing decided with
  | nil => rfl
  | cons head tail ih =>
      cases head with
      | hostDecision op accepted identity =>
          simp only [commitAuthorityInOrder] at h ⊢
          exact ih (if accepted then identity :: decided else decided) h
      | cleanup kind identity =>
          cases kind with
          | hostRetire =>
              simp only [commitAuthorityInOrder, Bool.and_eq_true] at h ⊢
              simp at h
              exact ⟨by simp [h.1], ih decided h.2⟩
          | regionRead start length =>
              simp only [commitAuthorityInOrder, Bool.and_eq_true] at h ⊢
              exact ⟨h.1, ih decided h.2⟩
          | guestRelease =>
              simp only [commitAuthorityInOrder, Bool.and_eq_true] at h ⊢
              exact ⟨h.1, ih decided h.2⟩
          | destructorRelease =>
              simp only [commitAuthorityInOrder, Bool.and_eq_true] at h ⊢
              exact ⟨h.1, ih decided h.2⟩
      | importCrossing op offset length =>
          simp only [commitAuthorityInOrder] at h ⊢
          exact ih decided h
      | protectedCopy length =>
          simp only [commitAuthorityInOrder] at h ⊢
          exact ih decided h
      | returned op value reason =>
          simp only [commitAuthorityInOrder] at h ⊢
          exact ih decided h
      | normalReturn =>
          simp only [commitAuthorityInOrder] at h ⊢
          exact ih decided h
      | abnormalExit exitClass =>
          simp only [commitAuthorityInOrder] at h ⊢
          exact ih decided h
      | handleTableIndex index =>
          simp only [commitAuthorityInOrder] at h ⊢
          exact ih decided h
      | storeBookkeeping slot =>
          simp only [commitAuthorityInOrder] at h ⊢
          exact ih decided h

theorem erase_preserves_commit_identities (steps : List AbiStep) :
    commitIdentities (erase steps) = commitIdentities steps := by
  induction steps with
  | nil => rfl
  | cons head tail ih =>
      cases head with
      | handleTableIndex index => simpa [erase, eraseStep, commitIdentities, commitStep, List.filterMap_cons] using ih
      | storeBookkeeping slot => simpa [erase, eraseStep, commitIdentities, commitStep, List.filterMap_cons] using ih
      | importCrossing op offset length => simpa [erase, eraseStep, commitIdentities, commitStep, List.filterMap_cons] using ih
      | hostDecision op accepted identity => simpa [erase, eraseStep, commitIdentities, commitStep, List.filterMap_cons] using ih
      | protectedCopy length => simpa [erase, eraseStep, commitIdentities, commitStep, List.filterMap_cons] using ih
      | returned op value reason => simpa [erase, eraseStep, commitIdentities, commitStep, List.filterMap_cons] using ih
      | normalReturn => simpa [erase, eraseStep, commitIdentities, commitStep, List.filterMap_cons] using ih
      | abnormalExit exitClass => simpa [erase, eraseStep, commitIdentities, commitStep, List.filterMap_cons] using ih
      | cleanup kind identity => simpa [erase, eraseStep, commitIdentities, commitStep, List.filterMap_cons] using ih

theorem erase_preserves_read_crossings (steps : List AbiStep) :
    readCrossings (erase steps) = readCrossings steps := by
  induction steps with
  | nil => rfl
  | cons head tail ih =>
      cases head with
      | handleTableIndex index => simpa [erase, eraseStep, readCrossings, Operation.noConfusion, List.filterMap_cons] using ih
      | storeBookkeeping slot => simpa [erase, eraseStep, readCrossings, Operation.noConfusion, List.filterMap_cons] using ih
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
      | handleTableIndex index => simpa [erase, eraseStep, releaseMembers, CommitKind.noConfusion, List.filterMap_cons] using ih
      | storeBookkeeping slot => simpa [erase, eraseStep, releaseMembers, CommitKind.noConfusion, List.filterMap_cons] using ih
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
      | handleTableIndex index => simpa [erase, eraseStep, readCommits, CommitKind.noConfusion, List.filterMap_cons] using ih
      | storeBookkeeping slot => simpa [erase, eraseStep, readCommits, CommitKind.noConfusion, List.filterMap_cons] using ih
      | importCrossing op offset length => simpa [erase, eraseStep, readCommits, CommitKind.noConfusion] using ih
      | hostDecision op accepted identity => simpa [erase, eraseStep, readCommits, CommitKind.noConfusion] using ih
      | protectedCopy length => simpa [erase, eraseStep, readCommits, CommitKind.noConfusion] using ih
      | returned op value reason => simpa [erase, eraseStep, readCommits, CommitKind.noConfusion] using ih
      | normalReturn => simpa [erase, eraseStep, readCommits, CommitKind.noConfusion] using ih
      | abnormalExit exitClass => simpa [erase, eraseStep, readCommits, CommitKind.noConfusion] using ih
      | cleanup kind identity =>
          cases kind <;> simpa [erase, eraseStep, readCommits] using ih

theorem erase_preserves_read_trace_events (steps : List AbiStep) :
    readTraceEvents (erase steps) = readTraceEvents steps := by
  induction steps with
  | nil => rfl
  | cons head tail ih =>
      cases head with
      | handleTableIndex index =>
          simpa [erase, eraseStep, readTraceEvents, readTraceStep, List.filterMap_cons] using ih
      | storeBookkeeping slot =>
          simpa [erase, eraseStep, readTraceEvents, readTraceStep, List.filterMap_cons] using ih
      | importCrossing op offset length =>
          cases op <;>
            simpa [erase, eraseStep, readTraceEvents, readTraceStep, List.filterMap_cons] using ih
      | hostDecision op accepted identity =>
          cases op <;>
            simpa [erase, eraseStep, readTraceEvents, readTraceStep, List.filterMap_cons] using ih
      | protectedCopy length =>
          simpa [erase, eraseStep, readTraceEvents, readTraceStep, List.filterMap_cons] using ih
      | returned op value reason =>
          cases op <;>
            simpa [erase, eraseStep, readTraceEvents, readTraceStep, List.filterMap_cons] using ih
      | normalReturn =>
          simpa [erase, eraseStep, readTraceEvents, readTraceStep, List.filterMap_cons] using ih
      | abnormalExit exitClass =>
          simpa [erase, eraseStep, readTraceEvents, readTraceStep, List.filterMap_cons] using ih
      | cleanup kind identity =>
          cases kind <;>
            simpa [erase, eraseStep, readTraceEvents, readTraceStep, List.filterMap_cons] using ih

theorem erase_preserves_release_trace_events (steps : List AbiStep) :
    releaseTraceEvents (erase steps) = releaseTraceEvents steps := by
  induction steps with
  | nil => rfl
  | cons head tail ih =>
      cases head with
      | handleTableIndex index =>
          simpa [erase, eraseStep, releaseTraceEvents, releaseTraceStep, List.filterMap_cons] using ih
      | storeBookkeeping slot =>
          simpa [erase, eraseStep, releaseTraceEvents, releaseTraceStep, List.filterMap_cons] using ih
      | importCrossing op offset length =>
          cases op <;>
            simpa [erase, eraseStep, releaseTraceEvents, releaseTraceStep, List.filterMap_cons] using ih
      | hostDecision op accepted identity =>
          cases op <;>
            simpa [erase, eraseStep, releaseTraceEvents, releaseTraceStep, List.filterMap_cons] using ih
      | protectedCopy length =>
          simpa [erase, eraseStep, releaseTraceEvents, releaseTraceStep, List.filterMap_cons] using ih
      | returned op value reason =>
          cases op <;>
            simpa [erase, eraseStep, releaseTraceEvents, releaseTraceStep, List.filterMap_cons] using ih
      | normalReturn =>
          simpa [erase, eraseStep, releaseTraceEvents, releaseTraceStep, List.filterMap_cons] using ih
      | abnormalExit exitClass =>
          simpa [erase, eraseStep, releaseTraceEvents, releaseTraceStep, List.filterMap_cons] using ih
      | cleanup kind identity =>
          cases kind <;>
            simpa [erase, eraseStep, releaseTraceEvents, releaseTraceStep, List.filterMap_cons] using ih

/-- Erasure leaves well-formedness alone: it removes only the two named bookkeeping classes, and
every relation scan ranges over a derived list that ignores them or over the unchanged decisions. -/
theorem erase_preserves_well_formedness (steps : List AbiStep) :
    wellFormed (erase steps) = wellFormed steps := by
  simp only [wellFormed, identitiesDecided, readCommitsExplained, releaseCommitsExplained, atMostOnce,
    erase_preserves_decisions, erase_preserves_commit_identities,
    erase_preserves_read_trace_events, erase_preserves_commit_authority,
    erase_preserves_release_trace_events, erase_preserves_release_members,
    erase_preserves_projection]

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
    related [.req .regionRead 0 8, .commit (.regionRead 0 8) 7,
      .resp .regionRead (some 8) none]
      [.importCrossing .regionRead 0 8, .hostDecision .regionRead true 7,
       .protectedCopy 8, .cleanup (.regionRead 0 8) 7,
       .returned .regionRead (some 8) none, .normalReturn] = true := by
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
    related [.req .regionRead 0 8, .commit (.regionRead 0 8) 7]
      [.importCrossing .regionRead 0 8, .hostDecision .regionRead true 9,
       .protectedCopy 8, .cleanup (.regionRead 0 8) 7] = false := by
  decide

theorem related_rejects_unexplained_read_commit :
    related [.commit (.regionRead 0 8) 5]
      [.hostDecision .regionRead true 5, .cleanup (.regionRead 0 8) 5] = false := by
  decide

/-- A guest-release commit cannot be paired with a read request merely because both are commits. -/
theorem related_rejects_read_as_guest_release_commit :
    related [.req .regionRead 0 1, .commit .guestRelease 5]
      [.importCrossing .regionRead 0 1, .hostDecision .regionRead true 5,
       .cleanup .guestRelease 5] = false := by
  decide

/-- A later accepted identity cannot authorize an earlier read commit, even when the source and
target projections otherwise agree. -/
theorem related_rejects_read_commit_before_its_decision :
    related [.req .regionRead 0 1, .commit (.regionRead 0 1) 5]
      [.importCrossing .regionRead 0 1, .protectedCopy 1,
       .cleanup (.regionRead 0 1) 5, .hostDecision .regionRead true 5] = false := by
  decide

/-- Two identities that are both decided are still not interchangeable between read requests; the
adjacent matched trace is accepted. -/
theorem related_rejects_crossed_read_commit_identities :
    related
        [.req .regionRead 0 1, .commit (.regionRead 0 1) 4, .resp .regionRead (some 1) none,
         .req .regionRead 1 1, .commit (.regionRead 1 1) 4, .resp .regionRead (some 1) none]
        [.importCrossing .regionRead 0 1, .hostDecision .regionRead true 4,
         .protectedCopy 1, .cleanup (.regionRead 0 1) 4,
         .returned .regionRead (some 1) none,
         .importCrossing .regionRead 1 1, .hostDecision .regionRead true 5,
         .protectedCopy 1, .cleanup (.regionRead 1 1) 4,
         .returned .regionRead (some 1) none] = false ∧
      related
        [.req .regionRead 0 1, .commit (.regionRead 0 1) 4, .resp .regionRead (some 1) none,
         .req .regionRead 1 1, .commit (.regionRead 1 1) 5, .resp .regionRead (some 1) none]
        [.importCrossing .regionRead 0 1, .hostDecision .regionRead true 4,
         .protectedCopy 1, .cleanup (.regionRead 0 1) 4,
         .returned .regionRead (some 1) none,
         .importCrossing .regionRead 1 1, .hostDecision .regionRead true 5,
         .protectedCopy 1, .cleanup (.regionRead 1 1) 5,
         .returned .regionRead (some 1) none] = true := by
  decide

theorem related_rejects_two_guest_releases :
    related [.req .guestRelease 0 0, .commit .guestRelease 5, .commit .guestRelease 5]
      [.importCrossing .guestRelease 0 0, .hostDecision .guestRelease true 5,
       .cleanup .guestRelease 5, .cleanup .guestRelease 5] = false := by
  decide

/-- A guest-release commit keeps the exact identity from that release request's decision. -/
theorem related_accepts_guest_release_commit :
    related [.req .guestRelease 0 0, .commit .guestRelease 7,
      .resp .guestRelease none none]
      [.importCrossing .guestRelease 0 0, .hostDecision .guestRelease true 7,
       .cleanup .guestRelease 7, .returned .guestRelease none none] = true := by
  decide

theorem related_rejects_crossed_guest_release_identity :
    related
        [.req .regionRead 0 1, .commit (.regionRead 0 1) 5,
         .resp .regionRead (some 1) none,
         .req .guestRelease 0 0, .commit .guestRelease 5]
        [.importCrossing .regionRead 0 1, .hostDecision .regionRead true 5,
         .protectedCopy 1, .cleanup (.regionRead 0 1) 5,
         .returned .regionRead (some 1) none,
         .importCrossing .guestRelease 0 0, .hostDecision .guestRelease true 4,
         .cleanup .guestRelease 5] = false := by
  decide

/-- A guest release followed by a host retirement of the same region is a second release of
one region, and the relation refuses it even though each kind commits only once. -/
theorem related_rejects_release_then_retirement :
    related [.req .guestRelease 0 0, .commit .guestRelease 7,
      .resp .guestRelease none none, .commit .hostRetire 7]
      [.importCrossing .guestRelease 0 0, .hostDecision .guestRelease true 7,
       .cleanup .guestRelease 7, .returned .guestRelease none none,
       .abnormalExit .trap, .cleanup .hostRetire 7] = false := by
  decide

theorem related_rejects_missing_protected_commit :
    related [.req .regionRead 0 8, .commit (.regionRead 0 8) 7,
      .resp .regionRead (some 8) none]
      [.importCrossing .regionRead 0 8, .hostDecision .regionRead true 7, .normalReturn] = false := by
  decide

theorem related_rejects_host_cleanup_as_guest_release :
    related [.req .guestRelease 0 0, .commit .guestRelease 7]
      [.importCrossing .guestRelease 0 0, .hostDecision .guestRelease true 7,
       .cleanup .hostRetire 7] = false := by
  decide

theorem related_rejects_denial_erased_into_success :
    related [.req .regionRead 0 8]
      [.importCrossing .regionRead 0 8, .hostDecision .regionRead true 7,
       .protectedCopy 8, .cleanup (.regionRead 0 8) 7] = false := by
  decide

/-- Positive control: a well-formed success relates, so the refusals above are not vacuous. -/
theorem related_accepts_well_formed_success :
    related [.req .regionRead 0 8, .commit (.regionRead 0 8) 7,
      .resp .regionRead (some 8) none]
      [.importCrossing .regionRead 0 8, .hostDecision .regionRead true 7,
       .protectedCopy 8, .cleanup (.regionRead 0 8) 7,
       .returned .regionRead (some 8) none] = true := by
  decide

/-! ## Cost and the exhaustion boundary (WIT-TRACE-04)

Fuel is an explicit accounting relation, never a free "the target may trap anywhere" clause: each
emitted step charges a declared cost, a trace retained before fuel exhausts is a prefix of the full
trace, its own cost stays inside the budget, and the commits it projects are a prefix of the commits
the full trace projects. So exhaustion may cut a trace but cannot invent or drop a commit. -/

/-- The declared cost of one emitted step. A protected copy charges its byte length; every other
step charges one unit. This is an admission estimate, not a claim about allocator behaviour. -/
def stepCost : AbiStep → Nat
  | .protectedCopy length => length
  | .importCrossing _ _ _ => 1
  | .hostDecision _ _ _ => 1
  | .returned _ _ _ => 1
  | .normalReturn => 1
  | .abnormalExit _ => 1
  | .cleanup _ _ => 1
  | .handleTableIndex _ => 1
  | .storeBookkeeping _ => 1

def traceCost (steps : List AbiStep) : Nat :=
  (steps.map stepCost).sum

/-- The steps the host can still afford: a greedy prefix that stops before the first step whose
cost exceeds what remains. A step is emitted whole or not at all, so a partly charged step never
appears. -/
def retainFrom (fuel : Nat) : List AbiStep → List AbiStep
  | [] => []
  | step :: rest =>
      if fuel < stepCost step then [] else step :: retainFrom (fuel - stepCost step) rest

/-- A retained trace is a prefix of the full trace, so exhaustion cannot reorder or invent steps. -/
theorem retainFrom_prefix (fuel : Nat) (steps : List AbiStep) : retainFrom fuel steps <+: steps := by
  induction steps generalizing fuel with
  | nil => exact List.nil_prefix
  | cons head tail ih =>
      by_cases affordable : stepCost head ≤ fuel
      · rw [retainFrom, ite_eq_right (Nat.not_lt.mpr affordable)]
        exact (List.cons_prefix_cons.mpr ⟨rfl, ih (fuel - stepCost head)⟩)
      · rw [retainFrom, ite_eq_left (Nat.lt_of_not_le affordable)]
        exact List.nil_prefix

/-- A projected prefix is a prefix of the projection, so the relation composes with exhaustion. -/
theorem filterMap_prefix {alpha beta : Type} (project : alpha → Option beta)
    {left right : List alpha} (pref : left <+: right) :
    left.filterMap project <+: right.filterMap project := by
  induction left generalizing right with
  | nil => exact List.nil_prefix
  | cons head tail ih =>
      cases right with
      | nil =>
          obtain ⟨rest, impossible⟩ := pref
          exact absurd impossible (by simp)
      | cons step rest =>
          obtain ⟨same, restPrefix⟩ := List.cons_prefix_cons.mp pref
          have sameProjection : project head = project step := by rw [same]
          simp only [List.filterMap_cons, sameProjection]
          cases projected : project step with
          | none => exact ih restPrefix
          | some event => exact List.cons_prefix_cons.mpr ⟨rfl, ih restPrefix⟩

/-- The committed prefix is preserved across the exhaustion boundary: the commits a retained trace
projects are a prefix of the commits the full trace projects. -/
theorem exhaustion_keeps_committed_prefix (fuel : Nat) (steps : List AbiStep) :
    (retainFrom fuel steps).filterMap projectStep <+:
      steps.filterMap projectStep :=
  filterMap_prefix projectStep (retainFrom_prefix fuel steps)

/-- The retained trace's own cost stays inside the fuel budget it was retained under. -/
theorem retainFrom_cost_bounded (fuel : Nat) (steps : List AbiStep) :
    traceCost (retainFrom fuel steps) ≤ fuel := by
  induction steps generalizing fuel with
  | nil => exact Nat.zero_le fuel
  | cons head tail ih =>
      by_cases affordable : stepCost head ≤ fuel
      · rw [retainFrom, ite_eq_right (Nat.not_lt.mpr affordable)]
        have bounded : traceCost (retainFrom (fuel - stepCost head) tail) ≤ fuel - stepCost head :=
          ih (fuel - stepCost head)
        calc traceCost (head :: retainFrom (fuel - stepCost head) tail)
            = stepCost head + traceCost (retainFrom (fuel - stepCost head) tail) := by
                simp only [traceCost, List.map_cons, List.sum_cons]
          _ ≤ stepCost head + (fuel - stepCost head) := Nat.add_le_add_left bounded _
          _ = fuel := by omega
      · rw [retainFrom, ite_eq_left (Nat.lt_of_not_le affordable)]
        simp only [traceCost, List.map_nil, List.sum_nil]
        exact Nat.zero_le fuel

/-- The commit identities a retained trace projects are a prefix of the full trace's commit
identities: an exhaustion boundary cannot drop an already committed identity. -/
theorem exhaustion_keeps_commit_identities (fuel : Nat) (steps : List AbiStep) :
    commitIdentities (retainFrom fuel steps) <+: commitIdentities steps :=
  filterMap_prefix commitStep (retainFrom_prefix fuel steps)

/-- The exhaustion rule bites: a source trace claiming a commit that the retained prefix could not
afford does not relate to that target under the fuel budget. -/
theorem exhaustion_cannot_relate_an_unaffordable_commit :
    related [.req .regionRead 0 8, .commit (.regionRead 0 8) 7]
      (retainFrom 1
        [.importCrossing .regionRead 0 8, .hostDecision .regionRead true 7,
         .protectedCopy 8, .cleanup (.regionRead 0 8) 7]) = false := by
  decide

/-- The same trace relates under a budget that affords every step, so the refusal above is not an
artefact of the fixture. -/
theorem exhaustion_relates_when_affordable :
    related [.req .regionRead 0 8, .commit (.regionRead 0 8) 7]
      (retainFrom 11
        [.importCrossing .regionRead 0 8, .hostDecision .regionRead true 7,
         .protectedCopy 8, .cleanup (.regionRead 0 8) 7]) = true := by
  decide

/-! ## RegionProtocol corollaries (WIT-TRACE-03)

Derived from the relation, never stated alongside it: an ordered permitted prefix, an exact identity
that came from a decision, an explained read commit and an at-most-once release. The premises the
corollaries still need — the host's own identity supply and response-to-request pairing — stay
premises and are not silently discharged here. -/

/-- A permitted ordered prefix of a source trace: the retained trace's projection is a prefix of the
related source trace itself, not merely of the full target trace. So an exhaustion boundary composes
with the relation at the source level. -/
theorem retained_projection_is_permitted_prefix (fuel : Nat) (source : List SourceEvent)
    (steps : List AbiStep) (relatedTrace : related source steps = true) :
    (retainFrom fuel steps).filterMap projectStep <+: source := by
  have both := relatedTrace
  rw [related, Bool.and_eq_true, beq_iff_eq] at both
  rw [both.1]
  exact exhaustion_keeps_committed_prefix fuel steps

/-- No forged identity survives at the source level: every identity the related source trace commits
was decided by the target trace it relates to. -/
theorem source_commits_are_decided (source : List SourceEvent) (steps : List AbiStep)
    (relatedTrace : related source steps = true) :
    (commitIdentities steps).all (fun identity => (decisions steps).contains identity) = true := by
  have relationParts : (source == steps.filterMap projectStep) = true ∧ wellFormed steps = true := by
    simpa only [related, Bool.and_eq_true] using relatedTrace
  have formed := relationParts.2
  simp only [wellFormed, Bool.and_eq_true] at formed
  simpa [identitiesDecided] using formed.1.1.1.1.1

/-- A related source trace commits at most one release or retirement: at-most-once release is
inherited by the source alphabet rather than asserted about it. -/
theorem source_release_is_at_most_once (source : List SourceEvent) (steps : List AbiStep)
    (relatedTrace : related source steps = true) :
    (releaseMembers steps).length ≤ 1 := by
  have relationParts : (source == steps.filterMap projectStep) = true ∧ wellFormed steps = true := by
    simpa only [related, Bool.and_eq_true] using relatedTrace
  have formed := relationParts.2
  simp only [wellFormed, atMostOnce, Bool.and_eq_true] at formed
  exact of_decide_eq_true formed.1.2

/-! ## Fixtures (WIT-TRACE-05, model level)

The design lists five observable executions. Each is a handwritten target trace here — the host-side
instrumentation that would record them from a real execution is exactly what this package does not
add, so these are fixtures of the *relation*, not observations of an execution. Cancellation stays
unsupported and unmodelled: the alphabets have no cancellation event. -/

/-- Fixture: ordinary error — a rejected response carrying a reason. -/
theorem fixture_ordinary_error :
    related [.req .regionRead 0 8, .resp .regionRead none (some "bounds-reject")]
      [.importCrossing .regionRead 0 8, .hostDecision .regionRead false 7,
       .returned .regionRead none (some "bounds-reject")] = true := by
  decide

/-- Fixture: host refusal on bounds — the request is visible and nothing was copied. -/
theorem fixture_host_bounds_refusal :
    related [.req .regionRead 0 8]
      [.importCrossing .regionRead 0 8, .hostDecision .regionRead false 7] = true := by
  decide

/-- Fixture: trap after a protected copy. The copy carries no source event and the trap is a
target-side class, so the source trace is still the request. -/
theorem fixture_trap_after_protected_copy :
    related [.req .regionRead 0 8]
      [.importCrossing .regionRead 0 8, .hostDecision .regionRead true 7,
       .protectedCopy 8, .abnormalExit (ExitClass.trap)] = true := by
  decide

/-- Fixture: cleanup failure, kept distinct from trap and from quota exhaustion on the target side. -/
theorem fixture_cleanup_failure_after_protected_copy :
    related [.req .regionRead 0 8]
      [.importCrossing .regionRead 0 8, .hostDecision .regionRead true 7,
       .protectedCopy 8, .abnormalExit (ExitClass.cleanupFailure)] = true := by
  decide

/-- Fixture: quota prefix — the budget affords the request and the decision but not the copy, so the
retained trace is the request alone and it relates to exactly that source trace. -/
theorem fixture_quota_prefix_stops_before_the_copy :
    related [.req .regionRead 0 8]
      (retainFrom 2
        [.importCrossing .regionRead 0 8, .hostDecision .regionRead true 7,
         .protectedCopy 8, .cleanup (.regionRead 0 8) 7]) = true := by
  decide

/-- The same budget stops before a commit when the copy is affordable but the commit is not, so the
quota boundary is measured rather than assumed. -/
theorem fixture_quota_prefix_keeps_the_affordable_commit :
    related [.req .regionRead 0 8]
      (retainFrom 10
        [.importCrossing .regionRead 0 8, .hostDecision .regionRead true 7, .protectedCopy 8]) = true := by
  decide


/-! ## Ordered pairing, identity retention and the observed host trace (WIT-TRACE-03/05)

`crates/noble-cli/src/component/bounded_region.rs` now records an ordered trace of what the host
actually observed: a request with its exact operation, exact table identity and arguments, the
protected read commit it authorised, the response, the guest release and, on an abnormal exit, its
own retirement. The lowering below maps that observed shape onto the target alphabet without
inventing a step; it is a lowering of the observation, not a correspondence theorem about the
emitted component. -/

/-- One observed host event, as the adapter records it. A read request carries the guest's signed
arguments exactly. A release request records none (the adapter writes `null`), and the model writes
them `0 0`. `exit` is the class the host reported for the invocation. `retirementRefused` is a
retirement the host attempted and the ownership table refused: the reason is retained, and the
refusal commits nothing. -/
inductive ObservedEvent where
  | request (op : Operation) (identity : Nat) (offset : Int) (length : Int)
  | protectedReadCommit (identity : Nat) (start : Nat) (length : Nat)
  | response (op : Operation) (value : Option Nat) (reason : Option String)
  | hostRetirement (identity : Nat)
  | exit (exitClass : ExitClass)
  | retirementRefused (identity : Nat) (reason : Option String)
  deriving DecidableEq, Repr

/-- Lower the observed trace onto the target alphabet, preserving its order. A protected read
commit becomes the copy and the commit of the exact identity that was authorized; a host
retirement becomes a `hostRetire` commit and never a guest release. The report does not expose a
separate admission-decision event, so `lower` labels a request's identity witness accepted and the
model only uses that label to check a commit; a response refusal with no commit stays visible. This
is an explicit host-observation premise, not a measured decision. The reported exit class lowers to
the target's abnormal-exit step, so a trap, a quota exhaustion and a cancellation stay distinct
target steps even though the projection erases all three. A refused retirement lowers to the
cleanup-failure exit class and commits nothing: the target alphabet has no step for a retirement
that did not happen. -/
def lower : List ObservedEvent → List AbiStep
  | [] => []
  | .request op identity offset length :: rest =>
      .importCrossing op offset length :: .hostDecision op true identity :: lower rest
  | .protectedReadCommit identity start length :: rest =>
      .protectedCopy length :: .cleanup (.regionRead start length) identity :: lower rest
  | .response op value reason :: rest => .returned op value reason :: lower rest
  | .hostRetirement identity :: rest => .cleanup .hostRetire identity :: lower rest
  | .exit exitClass :: rest => .abnormalExit exitClass :: lower rest
  | .retirementRefused _ _ :: rest => .abnormalExit .cleanupFailure :: lower rest

/-- The observed trace of an in-bounds read and a consuming guest release, in the order the host
really records it (`region.read`, commit, response, `region.release`, response). -/
def observedReadAndRelease : List ObservedEvent :=
  [.request .regionRead 2 0 1, .protectedReadCommit 2 0 1, .response .regionRead (some 1) none,
   .request .guestRelease 2 0 0, .response .guestRelease none none]

/-- Fixture: the observed read-and-release trace lowers to a trace the relation accepts. -/
theorem observed_read_and_release_lowers_to_a_related_trace :
    related
      [.req .regionRead 0 1, .commit (.regionRead 0 1) 2, .resp .regionRead (some 1) none,
       .req .guestRelease 0 0, .resp .guestRelease none none]
      (lower observedReadAndRelease) = true := by
  decide

/-- The read requests an observed trace records, with their signed arguments, in order. -/
def observedReadArguments (events : List ObservedEvent) : List (Int × Int) :=
  events.filterMap fun event =>
    match event with
    | .request .regionRead _ offset length => some (offset, length)
    | _ => none

/-- The lowering keeps every observed read request's arguments, in order: a negative argument
stays negative and no argument is reinterpreted (#90). -/
theorem lower_keeps_read_arguments (events : List ObservedEvent) :
    readCrossings (lower events) = observedReadArguments events := by
  induction events with
  | nil => rfl
  | cons head tail ih =>
      cases head with
      | request op identity offset length =>
          cases op <;> simpa [lower, readCrossings, observedReadArguments, List.filterMap_cons] using ih
      | protectedReadCommit identity start length =>
          simpa [lower, readCrossings, observedReadArguments, List.filterMap_cons] using ih
      | response op value reason =>
          simpa [lower, readCrossings, observedReadArguments, List.filterMap_cons] using ih
      | hostRetirement identity =>
          simpa [lower, readCrossings, observedReadArguments, List.filterMap_cons] using ih
      | exit exitClass =>
          simpa [lower, readCrossings, observedReadArguments, List.filterMap_cons] using ih
      | retirementRefused identity reason =>
          simpa [lower, readCrossings, observedReadArguments, List.filterMap_cons] using ih

/-- Fixture: the host's reported exit class lowers to the target's own abnormal-exit step, so the
target trace keeps the class even though the projection erases it. -/
theorem observed_exit_lowers_to_its_abnormal_exit_class :
    lower [.request .regionRead 2 0 1, .protectedReadCommit 2 0 1, .exit .trap] =
        [.importCrossing .regionRead 0 1, .hostDecision .regionRead true 2, .protectedCopy 1,
         .cleanup (.regionRead 0 1) 2, .abnormalExit .trap] ∧
      lower [.exit .quotaExhaustion] = [.abnormalExit .quotaExhaustion] ∧
      lower [.exit .cancelled] = [.abnormalExit .cancelled] ∧
      lower [.exit .adapterInvalid] = [.abnormalExit .adapterInvalid] ∧
      lower [.exit .outsideModel] = [.abnormalExit .outsideModel] := by
  decide

/-- Control: a trap, a quota exhaustion, a cancellation and an adapter fault stay distinct target
steps after lowering. They are not collapsed into one abnormal exit, so the host's own class is
recoverable from the lowered trace. -/
theorem exit_classes_stay_distinct_after_lowering :
    lower [.exit .trap] ≠ lower [.exit .quotaExhaustion] ∧
      lower [.exit .quotaExhaustion] ≠ lower [.exit .cancelled] ∧
      lower [.exit .trap] ≠ lower [.exit .cancelled] ∧
      lower [.exit .cancelled] ≠ lower [.exit .adapterInvalid] ∧
      lower [.exit .adapterInvalid] ≠ lower [.exit .trap] ∧
      lower [.exit .outsideModel] ≠ lower [.exit .adapterInvalid] := by
  decide

/-- The projection erases the exit class, so two observations that differ only in their exit class
project to the same source trace. This is why the target step carries the class, and why a
cancellation cannot be read out of a source trace. -/
theorem exit_class_is_not_in_the_projection :
    (lower [.exit .trap]).filterMap projectStep = (lower [.exit .cancelled]).filterMap projectStep := by
  decide

/-- An abnormal exit with no other event relates to the empty source trace: the projection is
right, and it says nothing about which abnormal exit it was unless the target step is read. -/
theorem exit_without_events_relates_vacuously :
    related [] (lower [.exit .cancelled]) = true := by
  decide

/-- A refused retirement is a cleanup failure, not a retirement: it lowers to the cleanup-failure
exit class and commits nothing. -/
theorem retirement_refused_is_a_cleanup_failure :
    lower [.retirementRefused 2 (some "already-retired")] = [.abnormalExit .cleanupFailure] ∧
      lower [.retirementRefused 2 none] = [.abnormalExit .cleanupFailure] := by
  decide

/-- Control: a source trace that claims the host retired the owner while the host's retirement was
refused does not relate, and the trace that claims nothing relates. A refused retirement is never
read as a retirement. -/
theorem a_refused_retirement_is_not_a_retirement :
    related [.commit .hostRetire 2] (lower [.retirementRefused 2 (some "already-retired")]) = false ∧
      related [] (lower [.retirementRefused 2 (some "already-retired")]) = true := by
  decide

/-- A refused retirement does not drop or invent the protected commit the run already made: the
committed read survives, and no retirement appears. -/
theorem refused_retirement_keeps_the_committed_prefix :
    related [.req .regionRead 0 1, .commit (.regionRead 0 1) 2]
        (lower [.request .regionRead 2 0 1, .protectedReadCommit 2 0 1,
                .retirementRefused 2 (some "already-retired")]) = true ∧
      (lower [.request .regionRead 2 0 1, .protectedReadCommit 2 0 1,
              .retirementRefused 2 (some "already-retired")]).filterMap projectStep =
        [.req .regionRead 0 1, .commit (.regionRead 0 1) 2] := by
  decide

/-- Fixture (#90): a refused read with a negative offset lowers and projects with that offset,
not with its unsigned 64-bit reinterpretation. -/
theorem observed_negative_request_projects_with_its_sign :
    (lower [.request .regionRead 2 (-1) 1, .response .regionRead none (some "bounds-reject")]).filterMap
        projectStep =
      [.req .regionRead (-1) 1, .resp .regionRead none (some "bounds-reject")] := by
  decide

/-- The relation tells a signed argument from its unsigned reinterpretation: a source trace that
reports the refused `-1` offset as `2^64 - 1` does not relate to the observed trace (#90). -/
theorem related_rejects_unsigned_reinterpretation :
    related [.req .regionRead 18446744073709551615 1, .resp .regionRead none (some "bounds-reject")]
      (lower [.request .regionRead 2 (-1) 1, .response .regionRead none (some "bounds-reject")]) = false := by
  decide

/-- Positive control: the same observed trace relates to the source trace that keeps the sign, so
the refusal above comes from the reinterpreted argument and is not vacuous. -/
theorem related_accepts_the_signed_refused_request :
    related [.req .regionRead (-1) 1, .resp .regionRead none (some "bounds-reject")]
      (lower [.request .regionRead 2 (-1) 1, .response .regionRead none (some "bounds-reject")]) = true := by
  decide

/-- A commit of the reinterpreted region is not explained by the signed crossing that asked for
`-1`, so the reinterpretation cannot launder a protected read commit. -/
theorem related_rejects_commit_of_reinterpreted_region :
    related [.req .regionRead (-1) 1, .commit (.regionRead 18446744073709551615 1) 2]
      [.importCrossing .regionRead (-1) 1, .hostDecision .regionRead true 2,
       .protectedCopy 1, .cleanup (.regionRead 18446744073709551615 1) 2] = false := by
  decide

/-- Fixture: an abnormal exit after the read records the host's own retirement, and the relation
accepts it as a retirement rather than as a guest release. -/
theorem observed_abnormal_exit_lowers_to_a_related_trace :
    related
      [.req .regionRead 0 1, .commit (.regionRead 0 1) 2, .resp .regionRead (some 1) none,
       .commit .hostRetire 2]
      (lower [.request .regionRead 2 0 1, .protectedReadCommit 2 0 1,
              .response .regionRead (some 1) none, .hostRetirement 2]) = true := by
  decide

/-- The lowering keeps the read request at the head of the projection: a request is never moved
behind its own response. -/
theorem lowered_projection_starts_with_the_request :
    (lower observedReadAndRelease).filterMap projectStep =
      [.req .regionRead 0 1, .commit (.regionRead 0 1) 2, .resp .regionRead (some 1) none,
       .req .guestRelease 0 0, .resp .guestRelease none none] := by
  decide

/-- The projected trace the observed fixtures lower to is paired: every response follows a request
of its own operation. -/
theorem observed_traces_are_paired :
    responsesPaired
      [.req .regionRead 0 1, .commit (.regionRead 0 1) 2, .resp .regionRead (some 1) none,
       .req .guestRelease 0 0, .resp .guestRelease none none] = true := by
  decide

/-! General corollary the pilot was missing: every read commit a related trace projects is
explained by an import crossing that asked for that exact region. -/
theorem related_implies_read_commits_explained (source : List SourceEvent) (target : List AbiStep) :
    related source target = true → readCommitsExplained target = true := by
  intro relatedTrace
  have formed : wellFormed target = true := by
    simp only [related, Bool.and_eq_true] at relatedTrace
    exact relatedTrace.2
  simp only [wellFormed, Bool.and_eq_true] at formed
  exact formed.1.1.1.2

/-- Every trace related by the model has paired projected responses; an unanswered final request is
permitted so an abnormal exit may cut a call at a prefix boundary. -/
theorem related_projects_only_paired_traces (source : List SourceEvent) (target : List AbiStep)
    (relatedTrace : related source target = true) : responsesPaired source = true := by
  have relationParts :
      (source == target.filterMap projectStep) = true ∧ wellFormed target = true := by
    simpa only [related, Bool.and_eq_true] using relatedTrace
  have projected := beq_iff_eq.mp relationParts.1
  have paired : responsesPaired (target.filterMap projectStep) = true := by
    have formed := relationParts.2
    simp only [wellFormed, Bool.and_eq_true] at formed
    exact formed.2
  simpa [projected] using paired

/-- Prefix pairing accepts a request whose response lies beyond the retained boundary. -/
theorem paired_prefix_may_end_with_an_unanswered_request :
    responsesPaired [.req .regionRead 0 1] = true := by
  decide

/-- A swapped response is unpaired: the response answers an operation its request never asked for. -/
theorem swapped_response_is_unpaired :
    responsesPaired [.req .regionRead 0 1, .resp .guestRelease none none] = false := by
  decide

/-- And the relation refuses a target trace whose response answers the wrong operation. -/
theorem related_rejects_swapped_response :
    related [.req .regionRead 0 1, .commit (.regionRead 0 1) 2,
      .resp .guestRelease none none]
      [.importCrossing .regionRead 0 1, .hostDecision .regionRead true 2, .protectedCopy 1,
       .cleanup (.regionRead 0 1) 2, .returned .guestRelease none none] = false := by
  decide

/-- Positive control: the same target trace relates when its returned operation answers the read
request that preceded it. -/
theorem related_accepts_paired_response :
    related [.req .regionRead 0 1, .commit (.regionRead 0 1) 2,
      .resp .regionRead (some 1) none]
      [.importCrossing .regionRead 0 1, .hostDecision .regionRead true 2, .protectedCopy 1,
       .cleanup (.regionRead 0 1) 2, .returned .regionRead (some 1) none] = true := by
  decide

/-- A substituted resource identity is refused: a commit under an identity the host never decided
on is not related. -/
theorem related_rejects_substituted_resource_identity :
    related
      [.req .regionRead 0 1, .commit (.regionRead 0 1) 2, .resp .regionRead (some 1) none]
      (lower [.request .regionRead 2 0 1, .protectedReadCommit 3 0 1,
              .response .regionRead (some 1) none]) = false := by
  decide

/-- A host retirement disguised as a guest release is refused: the observed retirement lowers to a
`hostRetire` commit, so claiming the guest released the same region does not relate. -/
theorem related_rejects_observed_retirement_as_guest_release :
    related
      [.req .regionRead 0 1, .commit (.regionRead 0 1) 2, .resp .regionRead (some 1) none,
       .commit .guestRelease 2]
      (lower [.request .regionRead 2 0 1, .protectedReadCommit 2 0 1,
              .response .regionRead (some 1) none, .hostRetirement 2]) = false := by
  decide

/-! ## Langlib-addendum controls, swapped requests and substituted responses (#50)

The adoption review of `Langlib/Common/Compilation.lean` named four ways a trace contract can pass
while saying too little:
* a semantics that is right at one fuel value only;
* a trace that omits a behaviourally inert read;
* an erroring run that drops an earlier commit;
* a contract that holds for the empty input only.

Each is a control here, paired with the property that refuses it and a positive case that relates, so
no refusal is vacuous. Swapped requests and a substituted response value are refused alongside them. -/

/-- A budget that affords the whole trace cuts nothing. -/
theorem retainFrom_complete (fuel : Nat) (steps : List AbiStep)
    (affordable : traceCost steps ≤ fuel) : retainFrom fuel steps = steps := by
  induction steps generalizing fuel with
  | nil => rfl
  | cons head tail ih =>
      simp only [traceCost, List.map_cons, List.sum_cons] at affordable
      have headFits : stepCost head ≤ fuel := by omega
      have tailFits : traceCost tail ≤ fuel - stepCost head := by
        simp only [traceCost]
        omega
      rw [retainFrom, ite_eq_right (Nat.not_lt.mpr headFits), ih (fuel - stepCost head) tailFits]

/-- Fuel stability as a property of any budgeted semantics (Langlib's `halted_stable`): a run that
completed under one budget returns the same trace under every larger budget. -/
def FuelStable (run : Nat → List AbiStep → List AbiStep) : Prop :=
  ∀ (steps : List AbiStep) (fuel more : Nat),
    traceCost steps ≤ fuel → fuel ≤ more → run more steps = run fuel steps

/-- The model's budgeted semantics is fuel stable. -/
theorem retainFrom_fuel_stable : FuelStable retainFrom := by
  intro steps fuel more completes larger
  rw [retainFrom_complete more steps (Nat.le_trans completes larger),
    retainFrom_complete fuel steps completes]

/-- So the relation cannot hold at one magic fuel value only: once a budget completes the trace, a
source trace related under it stays related under every larger budget. -/
theorem related_stable_under_more_fuel (source : List SourceEvent) (steps : List AbiStep)
    (fuel more : Nat) (completes : traceCost steps ≤ fuel) (larger : fuel ≤ more)
    (relatedAt : related source (retainFrom fuel steps) = true) :
    related source (retainFrom more steps) = true := by
  rw [retainFrom_fuel_stable steps fuel more completes larger]
  exact relatedAt

/-- Control semantics: right at the completing budget, but returns nothing once the budget is
larger. -/
def magicFuelRetain (fuel : Nat) (steps : List AbiStep) : List AbiStep :=
  if fuel ≤ traceCost steps then retainFrom fuel steps else []

/-- A successful protected read: request, decision, eight-byte copy and commit. -/
def protectedReadTarget : List AbiStep :=
  [.importCrossing .regionRead 0 8, .hostDecision .regionRead true 7,
   .protectedCopy 8, .cleanup (.regionRead 0 8) 7]

/-- Control: at the completing budget the control semantics relates exactly as the model does, but
one unit later it does not, so a contract checked at a single fuel value would accept it. -/
theorem magic_fuel_relates_only_at_one_budget :
    related [.req .regionRead 0 8, .commit (.regionRead 0 8) 7]
        (magicFuelRetain 11 protectedReadTarget) = true ∧
      related [.req .regionRead 0 8, .commit (.regionRead 0 8) 7]
        (magicFuelRetain 12 protectedReadTarget) = false := by
  decide

/-- Fuel stability refuses the control semantics. -/
theorem magic_fuel_is_not_fuel_stable : ¬ FuelStable magicFuelRetain := by
  intro stable
  have same := stable protectedReadTarget 11 12 (by decide) (by decide)
  revert same
  decide

/-- The last source event a target trace projects: its final observable answer. -/
def finalResponse (steps : List AbiStep) : Option SourceEvent :=
  (steps.filterMap projectStep).getLast?

/-- The observed trace of the same release with the read left out. The read's value was never used,
so the final answer does not change. -/
def observedReleaseWithoutRead : List ObservedEvent :=
  [.request .guestRelease 2 0 0, .response .guestRelease none none]

/-- Control: Langlib's `trace_faithful` permits omitting a behaviourally inert read. Both traces end
in the same answer, yet the relation refuses the one that skipped the protected read: a protected
read matters even when its value is discarded. The trace with the read relates
(`observed_read_and_release_lowers_to_a_related_trace`). -/
theorem related_rejects_omitted_unused_read :
    finalResponse (lower observedReleaseWithoutRead) = finalResponse (lower observedReadAndRelease) ∧
      related
        [.req .regionRead 0 1, .commit (.regionRead 0 1) 2, .resp .regionRead (some 1) none,
         .req .guestRelease 0 0, .resp .guestRelease none none]
        (lower observedReleaseWithoutRead) = false := by
  decide

/-- The read requests a source trace claims, with their arguments, in order. -/
def sourceReads (source : List SourceEvent) : List (Int × Int) :=
  source.filterMap fun event =>
    match event with
    | .req .regionRead offset length => some (offset, length)
    | _ => none

/-- In general, a related source trace claims exactly the read crossings the target performed, in
order, whether or not each read's value is later used: no protected read is dropped or invented. -/
theorem related_source_reads_are_target_reads (source : List SourceEvent) (target : List AbiStep)
    (relatedTrace : related source target = true) : sourceReads source = readCrossings target := by
  have both := relatedTrace
  rw [related, Bool.and_eq_true, beq_iff_eq] at both
  rw [both.1, sourceReads, readCrossings, List.filterMap_filterMap]
  congr 1
  funext step
  cases step with
  | importCrossing op offset length => cases op <;> rfl
  | hostDecision op accepted identity => rfl
  | protectedCopy length => rfl
  | returned op value reason => rfl
  | normalReturn => rfl
  | abnormalExit exitClass => rfl
  | cleanup kind identity => rfl
  | handleTableIndex index => rfl
  | storeBookkeeping slot => rfl

/-- An abnormal exit projects to nothing, so a trap cannot remove from the projection a commit the
trace already made. -/
theorem abnormal_exit_keeps_commits (steps : List AbiStep) (exitClass : ExitClass) :
    (steps ++ [AbiStep.abnormalExit exitClass]).filterMap projectStep =
      steps.filterMap projectStep := by
  rw [append_projection]
  simp [projectStep]

/-- A read committed before a trap. -/
def trappedAfterCommit : List AbiStep :=
  lower [.request .regionRead 2 0 1, .protectedReadCommit 2 0 1] ++ [.abnormalExit .trap]

/-- Control: Langlib's laws leave erroring runs unconstrained. A source trace that drops the
committed read because the run later trapped is refused, and the trace that keeps the commit
relates. -/
theorem related_rejects_trap_that_drops_a_commit :
    related [.req .regionRead 0 1] trappedAfterCommit = false ∧
      related [.req .regionRead 0 1, .commit (.regionRead 0 1) 2] trappedAfterCommit = true := by
  decide

/-- The observed trace of an in-bounds read of `length` bytes at `start` under `identity`, its
response and a consuming guest release. -/
def observedRead (identity start length value : Nat) : List ObservedEvent :=
  [.request .regionRead identity start length, .protectedReadCommit identity start length,
   .response .regionRead (some value) none, .request .guestRelease identity 0 0,
   .response .guestRelease none none]

/-- The source trace that input relates to. -/
def readAndReleaseSource (start length identity value : Nat) : List SourceEvent :=
  [.req .regionRead start length, .commit (.regionRead start length) identity,
   .resp .regionRead (some value) none, .req .guestRelease 0 0, .resp .guestRelease none none]

/-- A lowering is correct on every read-and-release input, not on one. -/
def CorrectOnEveryInput (lowering : List ObservedEvent → List AbiStep) : Prop :=
  ∀ (identity start length value : Nat),
    related (readAndReleaseSource start length identity value)
      (lowering (observedRead identity start length value)) = true

/-- The model's lowering is correct on every such input: the statement quantifies over the identity,
the region and the returned value. -/
theorem lower_correct_on_every_input : CorrectOnEveryInput lower := by
  intro identity start length value
  simp [readAndReleaseSource, observedRead, lower, related, wellFormed, identitiesDecided,
    commitAuthorityInOrder, readCommitsExplained, readTracePaired, readTraceResponseFollows,
    readTraceEvents, readTraceStep, releaseCommitsExplained, releaseTracePaired,
    releaseTraceResponseFollows, releaseTraceEvents, releaseTraceStep, atMostOnce,
    decisions, commitIdentities, releaseMembers, decisionStep, commitStep, projectStep,
    commitMatchesRequest, responsesPaired, responseFollows, List.filterMap_cons]

/-- Control lowering: right for the empty input only. -/
def lowerEmptyOnly (_events : List ObservedEvent) : List AbiStep := []

/-- Control: the empty-input-only lowering agrees with the model on the empty input, and the empty
traces relate there. -/
theorem empty_only_lowering_matches_on_the_empty_input :
    lowerEmptyOnly [] = lower [] ∧ related [] (lowerEmptyOnly []) = true := by
  decide

/-- The universally quantified statement refuses the empty-input-only lowering. -/
theorem empty_only_lowering_is_not_correct_on_every_input :
    ¬ CorrectOnEveryInput lowerEmptyOnly := by
  intro correct
  have refused := correct 2 0 1 1
  revert refused
  decide

/-- A swapped request order is refused: the source trace claims the release before the read, while
the host performed the read first. The trace in the host's order relates
(`observed_read_and_release_lowers_to_a_related_trace`). -/
theorem related_rejects_swapped_requests :
    related
      [.req .guestRelease 0 0, .resp .guestRelease none none, .req .regionRead 0 1,
       .commit (.regionRead 0 1) 2, .resp .regionRead (some 1) none]
      (lower observedReadAndRelease) = false := by
  decide

/-- A substituted response value is refused: the source trace claims the read returned 2 where the
host returned 1. -/
theorem related_rejects_substituted_response_value :
    related
      [.req .regionRead 0 1, .commit (.regionRead 0 1) 2, .resp .regionRead (some 2) none,
       .req .guestRelease 0 0, .resp .guestRelease none none]
      (lower observedReadAndRelease) = false := by
  decide

/-! ## Registration as a premise (#50)

The host registers one owner before guest start (A1). On an abnormal exit it retires that owner, and
the retirement is authorised by the registration, not by a guest request. A trace in which the guest
made no request, such as a trap before use or a quota exhausted before the read, therefore has no
decision for its retired identity, and `related` refuses it. Taking that identity from the retirement
itself would let a forged retirement relate.

The v2 report names the registered identity (`registration.identity`). `relatedFrom registered` takes
it as a premise and lets it authorise a host retirement only. A read commit, a guest release and a
destructor release still need a decision, so a registration cannot stand in for one. `related` is
unchanged and implies `relatedFrom` under every registration. -/

/-- Commit authority under a registration premise: a decision or a registration authorises a host
retirement, and only a decision authorises any other commit. -/
def authorisedFrom (registered : List Nat) (steps : List AbiStep) : Bool :=
  steps.all fun step =>
    match step with
    | .cleanup .hostRetire identity =>
        (decisions steps).contains identity || registered.contains identity
    | .cleanup _ identity => (decisions steps).contains identity
    | _ => true

/-- Target well-formedness under a registration premise. -/
def wellFormedFrom (registered : List Nat) (steps : List AbiStep) : Bool :=
  authorisedFrom registered steps && commitAuthorityInOrder registered [] steps &&
    readCommitsExplained steps && releaseCommitsExplained steps && atMostOnce steps &&
    responsesPaired (steps.filterMap projectStep)

/-- The relation under a registration premise. -/
def relatedFrom (registered : List Nat) (source : List SourceEvent) (target : List AbiStep) : Bool :=
  (source == target.filterMap projectStep) && wellFormedFrom registered target

/-- Every commit a decision authorises stays authorised under any registration. -/
theorem authorisedFrom_of_identitiesDecided (registered : List Nat) (steps : List AbiStep)
    (hdecided : identitiesDecided steps = true) : authorisedFrom registered steps = true := by
  simp only [identitiesDecided, List.all_eq_true] at hdecided
  simp only [authorisedFrom, List.all_eq_true]
  intro step hmem
  cases step with
  | cleanup kind identity =>
      have hcommit : identity ∈ commitIdentities steps :=
        List.mem_filterMap.mpr ⟨.cleanup kind identity, hmem, rfl⟩
      have hcontains := hdecided identity hcommit
      have hin : identity ∈ decisions steps := by simpa using hcontains
      cases kind <;> simp [hin]
  | importCrossing => rfl
  | hostDecision => rfl
  | protectedCopy => rfl
  | returned => rfl
  | normalReturn => rfl
  | abnormalExit => rfl
  | handleTableIndex => rfl
  | storeBookkeeping => rfl

/-- A registration only widens the relation: every trace `related` relates is related under any
registration. -/
theorem relatedFrom_of_related (registered : List Nat) (source : List SourceEvent)
    (target : List AbiStep) (hrelated : related source target = true) :
    relatedFrom registered source target = true := by
  have relationParts : (source == target.filterMap projectStep) = true ∧ wellFormed target = true := by
    simpa only [related, Bool.and_eq_true] using hrelated
  have hprojection := relationParts.1
  have hformed := relationParts.2
  simp only [wellFormed, Bool.and_eq_true] at hformed
  obtain ⟨⟨⟨⟨⟨hdecided, hordered⟩, hexplained⟩, hreleases⟩, honce⟩, hpaired⟩ := hformed
  simp only [relatedFrom, wellFormedFrom, Bool.and_eq_true]
  exact ⟨hprojection,
    ⟨⟨⟨⟨⟨authorisedFrom_of_identitiesDecided registered target hdecided,
      commit_authority_mono registered [] target hordered⟩, hexplained⟩, hreleases⟩,
      honce⟩, hpaired⟩⟩

/-- The gap closed: a host retirement before any guest request relates under the registration the
host reports, and stays refused without it. -/
theorem registration_authorises_an_early_retirement :
    relatedFrom [1] [.commit .hostRetire 1] (lower [.hostRetirement 1]) = true ∧
      related [.commit .hostRetire 1] (lower [.hostRetirement 1]) = false := by
  decide

/-- A retirement of an identity nobody registered or decided is refused: the registration is a
premise, never taken from the retirement. -/
theorem relatedFrom_rejects_a_forged_retirement :
    relatedFrom [1] [.commit .hostRetire 2] (lower [.hostRetirement 2]) = false := by
  decide

/-- A registration does not authorise a read commit: the commit still needs the decision of its own
read, here made for another identity. -/
theorem registration_does_not_authorise_a_read_commit :
    relatedFrom [1] [.req .regionRead 0 1, .commit (.regionRead 0 1) 1]
      [.importCrossing .regionRead 0 1, .hostDecision .regionRead true 5, .protectedCopy 1,
       .cleanup (.regionRead 0 1) 1] = false := by
  decide

/-- Nor does it authorise a guest release. -/
theorem registration_does_not_authorise_a_guest_release :
    relatedFrom [1] [.commit .guestRelease 1] [.cleanup .guestRelease 1] = false := by
  decide

end RegionTrace
