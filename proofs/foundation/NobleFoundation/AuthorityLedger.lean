/-
  Checkpoint-scoped M5 authority-ledger model (issue #66, phase 1).

  The existing kernel already makes permits move-only and revalidates exact
  plan/grant facts before a serialized commit charges quota, consumes the
  witness and registers the attempt. This file names the bounded ledger
  invariants those steps are supposed to keep, so a later extracted-function
  binding has statements to bind to. It makes no claim about the Rust
  implementation's correspondence to this model, and no claim about host
  quota replenishment beyond the stated carry-forward premise.

  The model has bounded witnesses, a bounded attempt ledger, single-slot-free
  checkpoint charges and a finite serialized request prefix. Denials and
  preflight refusals leave the ledger unchanged; their changed request/effect
  counters are deliberately outside the ledger state. Admission compares the
  requested cost with the *cumulative* charge at the checkpoint against the
  independently supplied snapshot, so the snapshot bound is enforced at the
  transition rather than assumed as a separate premise.
-/
namespace NobleFoundation.AuthorityLedger

/-- A registered committed attempt. `started` records the at-most-once start. -/
structure Attempt where
  witness : Nat
  checkpoint : Nat
  cost : Nat
  started : Bool
deriving DecidableEq, Repr

/-- The bounded ledger state. `consumed` holds exactly the witnesses committed in
    this prefix, `attempts` the registered attempts, and `charges` the
    checkpoint-local quota charges. -/
structure Ledger where
  consumed : List Nat
  attempts : List Attempt
  charges : List (Nat × Nat)
deriving DecidableEq, Repr

/-- The initial state before any serialized request. -/
def initial : Ledger := ⟨[], [], []⟩

/-- Quota charged at one checkpoint. -/
def chargedFor (checkpoint : Nat) (charges : List (Nat × Nat)) : Nat :=
  ((charges.filter fun charge => charge.1 = checkpoint).map fun charge => charge.2).sum

/-- A fresh root/checkpoint snapshot is supplied independently of the ledger.
    The carry-forward premise is a *host* obligation; without it this model
    makes no cross-checkpoint conservation claim. -/
abbrev Snapshot := Nat → Nat

/-- One independently supplied authorization fact bundle. Exact equality of
    plan and grant is what admission checks; a model input that names only a
    witness identity is not a capability. -/
structure Facts where
  plan : Nat
  grant : Nat
  requestedPlan : Nat
  requestedGrant : Nat
  sourceAuthentic : Bool
  checkpointValid : Bool
  policyLive : Bool
  credentialsValid : Bool
  revoked : Bool
  cost : Nat
deriving DecidableEq, Repr

/-- Tie the two requested identities to the supplied ones: a claim is committed
    only when the plan and grant facts equal what the request asked for. -/
def FactsExact (facts : Facts) : Bool :=
  facts.plan == facts.requestedPlan && facts.grant == facts.requestedGrant

/-- The cumulative quota component of admissibility, readable on its own. -/
def WithinSnapshot (ledger : Ledger) (snapshot : Snapshot) (facts : Facts)
    (checkpoint : Nat) : Bool :=
  decide (chargedFor checkpoint ledger.charges + facts.cost ≤ snapshot checkpoint)

/-- Every independently checked precondition admission requires. The quota
    comparison is cumulative: the charges already recorded at the checkpoint
    plus the requested cost must fit the independently supplied snapshot. A
    free per-request `available` field would let two requests each fit it and
    jointly exceed the snapshot. -/
def FactsAdmissible (ledger : Ledger) (snapshot : Snapshot) (facts : Facts)
    (checkpoint : Nat) : Bool :=
  FactsExact facts
    && facts.sourceAuthentic
    && facts.checkpointValid
    && facts.policyLive
    && facts.credentialsValid
    && !facts.revoked
    && WithinSnapshot ledger snapshot facts checkpoint

theorem chargedFor_cons_same (checkpoint : Nat) (charges : List (Nat × Nat)) (cost : Nat) :
    chargedFor checkpoint ((checkpoint, cost) :: charges) = cost + chargedFor checkpoint charges := by
  simp [chargedFor]

theorem chargedFor_cons_other (checkpoint other : Nat) (charges : List (Nat × Nat))
    (cost : Nat) (distinct : other ≠ checkpoint) :
    chargedFor checkpoint ((other, cost) :: charges) = chargedFor checkpoint charges := by
  simp [chargedFor, distinct]

/-- Consuming one witness: the registry records it once and charges its cost at
    the request's checkpoint. -/
def consume (ledger : Ledger) (facts : Facts) (witness checkpoint : Nat) : Ledger :=
  ⟨witness :: ledger.consumed,
   ⟨witness, checkpoint, facts.cost, false⟩ :: ledger.attempts,
   (checkpoint, facts.cost) :: ledger.charges⟩

/-- A denied or preflight-refused request leaves the ledger unchanged. Only the
    separately recorded request/effect counters move. -/
def refuse (ledger : Ledger) (_facts : Facts) : Ledger := ledger

/-- One serialized request. Admission checks the exact facts, including the
    cumulative checkpoint charge against the supplied snapshot, *before*
    consuming the witness; nothing here can start execution directly. -/
def accept (ledger : Ledger) (snapshot : Snapshot) (facts : Facts)
    (witness checkpoint : Nat) : Ledger :=
  if witness ∈ ledger.consumed then refuse ledger facts
  else if FactsAdmissible ledger snapshot facts checkpoint then
    consume ledger facts witness checkpoint
  else refuse ledger facts

/-- Starting a registered attempt. `start` refuses an unregistered or already
    started attempt rather than minting a second execution permit. -/
def start (ledger : Ledger) (witness : Nat) : Option Ledger :=
  match ledger.attempts.find? (fun attempt => attempt.witness = witness) with
  | none => none
  | some attempt =>
      if attempt.started then none
      else some ⟨ledger.consumed,
        ledger.attempts.map fun other =>
          if other.witness = witness then ⟨other.witness, other.checkpoint, other.cost, true⟩
          else other,
        ledger.charges⟩

/-- The witness sets stay aligned: every consumed witness names exactly one
    registered attempt and vice versa. -/
def WitnessAlignment (ledger : Ledger) : Prop :=
  ledger.consumed.length = ledger.attempts.length
    ∧ ∀ witness ∈ ledger.consumed,
        ∃ attempt ∈ ledger.attempts, attempt.witness = witness

/-- No witness is consumed twice. -/
def WitnessUniqueness (ledger : Ledger) : Prop :=
  ∀ witness ∈ ledger.consumed,
    (ledger.consumed.filter fun other => other = witness).length = 1

/-- Checkpoint-local charges never exceed the independently supplied snapshot for
    that checkpoint. -/
def WithinQuota (ledger : Ledger) (snapshot : Snapshot) : Prop :=
  ∀ checkpoint, chargedFor checkpoint ledger.charges ≤ snapshot checkpoint

/-- Attempts start at most once. -/
def StartedAtMostOnce (ledger : Ledger) : Prop :=
  ∀ attempt ∈ ledger.attempts, attempt.started = true →
    (ledger.attempts.filter fun other => other.witness = attempt.witness ∧ other.started).length = 1

theorem initial_alignment : WitnessAlignment initial := by
  refine ⟨rfl, ?_⟩
  intro witness member
  exact absurd member (by simp [initial])

theorem initial_uniqueness : WitnessUniqueness initial := by
  intro witness member
  exact absurd member (by simp [initial])

theorem initial_within_quota (snapshot : Snapshot) : WithinQuota initial snapshot := by
  intro checkpoint
  simp [initial, chargedFor]

theorem initial_started_at_most_once : StartedAtMostOnce initial := by
  intro attempt member
  exact absurd member (by simp [initial])

/-- A successful admission consumes its witness and charges the checkpoint, so a
    denied or refused request cannot resurrect consumed authority: its ledger is
    definitionally unchanged. -/
theorem refused_request_preserves_ledger (ledger : Ledger) (facts : Facts) :
    refuse ledger facts = ledger := by
  rfl

/-- The cumulative quota comparison is exactly the last admissibility conjunct. -/
theorem admissible_implies_within_snapshot (ledger : Ledger) (snapshot : Snapshot)
    (facts : Facts) (checkpoint : Nat)
    (admissible : FactsAdmissible ledger snapshot facts checkpoint = true) :
    chargedFor checkpoint ledger.charges + facts.cost ≤ snapshot checkpoint := by
  unfold FactsAdmissible WithinSnapshot at admissible
  by_cases within : chargedFor checkpoint ledger.charges + facts.cost ≤ snapshot checkpoint
  · exact within
  · exfalso
    have failed : decide (chargedFor checkpoint ledger.charges + facts.cost
        ≤ snapshot checkpoint) = false := by
      simp [within]
    simp [failed] at admissible

theorem denied_admission_preserves_ledger (ledger : Ledger) (snapshot : Snapshot)
    (facts : Facts) (witness checkpoint : Nat) (already : witness ∈ ledger.consumed) :
    accept ledger snapshot facts witness checkpoint = ledger := by
  simp [accept, already, refuse]

/-- Admission of a fresh witness under admissible facts grows the consumed set by
    exactly that witness: one committed attempt per consumed witness. -/
theorem admission_consumes_exactly_one_witness (ledger : Ledger) (snapshot : Snapshot)
    (facts : Facts) (witness checkpoint : Nat) (fresh : witness ∉ ledger.consumed)
    (admissible : FactsAdmissible ledger snapshot facts checkpoint = true) :
    (accept ledger snapshot facts witness checkpoint).consumed = witness :: ledger.consumed := by
  simp [accept, fresh, admissible, consume]

theorem admission_registers_one_attempt (ledger : Ledger) (snapshot : Snapshot)
    (facts : Facts) (witness checkpoint : Nat) (fresh : witness ∉ ledger.consumed)
    (admissible : FactsAdmissible ledger snapshot facts checkpoint = true) :
    (accept ledger snapshot facts witness checkpoint).attempts
      = ⟨witness, checkpoint, facts.cost, false⟩ :: ledger.attempts := by
  simp [accept, fresh, admissible, consume]

/-- The exact-plan comparison is not decorative: an altered requested plan fails
    admissibility and mints nothing. -/
theorem altered_plan_cannot_mint (facts : Facts) (snapshot : Snapshot)
    (witness checkpoint : Nat) (ledger : Ledger) (fresh : witness ∉ ledger.consumed)
    (altered : facts.plan ≠ facts.requestedPlan) :
    accept ledger snapshot facts witness checkpoint = ledger := by
  have inexact : FactsExact facts = false := by
    simp [FactsExact, altered]
  simp [accept, fresh, FactsAdmissible, inexact, refuse]

/-- A stale or revoked fact bundle likewise mints nothing. -/
theorem stale_facts_cannot_mint (facts : Facts) (snapshot : Snapshot)
    (witness checkpoint : Nat) (ledger : Ledger) (fresh : witness ∉ ledger.consumed)
    (stale : facts.revoked = true ∨ facts.checkpointValid = false
      ∨ facts.policyLive = false ∨ facts.sourceAuthentic = false) :
    accept ledger snapshot facts witness checkpoint = ledger := by
  have inadmissible : FactsAdmissible ledger snapshot facts checkpoint = false := by
    rcases stale with broken | broken | broken | broken <;> simp [FactsAdmissible, broken]
  simp [accept, fresh, inadmissible, refuse]

/-- A fresh witness under admissible facts reaches the consuming transition. -/
theorem admission_is_consumption (ledger : Ledger) (snapshot : Snapshot) (facts : Facts)
    (witness checkpoint : Nat) (fresh : witness ∉ ledger.consumed)
    (admissible : FactsAdmissible ledger snapshot facts checkpoint = true) :
    accept ledger snapshot facts witness checkpoint = consume ledger facts witness checkpoint := by
  simp [accept, fresh, admissible]

/-- One admission preserves the witness alignment: the consumed list and the
    attempt ledger grow together by the same witness. -/
theorem admission_preserves_alignment (ledger : Ledger) (snapshot : Snapshot) (facts : Facts)
    (witness checkpoint : Nat) (fresh : witness ∉ ledger.consumed)
    (admissible : FactsAdmissible ledger snapshot facts checkpoint = true)
    (aligned : WitnessAlignment ledger) :
    WitnessAlignment (accept ledger snapshot facts witness checkpoint) := by
  rcases aligned with ⟨lengths, members⟩
  rw [admission_is_consumption ledger snapshot facts witness checkpoint fresh admissible]
  refine ⟨by simp [consume, lengths], ?_⟩
  intro other member
  rcases List.mem_cons.mp member with added | old
  · rw [added]
    exact ⟨⟨witness, checkpoint, facts.cost, false⟩, List.mem_cons_self .., rfl⟩
  · rcases members other old with ⟨attempt, registered, identity⟩
    exact ⟨attempt, List.mem_cons_of_mem _ registered, identity⟩

/-- One admission preserves witness uniqueness because it only adds a witness the
    ledger did not already consume. -/
theorem admission_preserves_uniqueness (ledger : Ledger) (snapshot : Snapshot) (facts : Facts)
    (witness checkpoint : Nat) (fresh : witness ∉ ledger.consumed)
    (admissible : FactsAdmissible ledger snapshot facts checkpoint = true)
    (unique : WitnessUniqueness ledger) :
    WitnessUniqueness (accept ledger snapshot facts witness checkpoint) := by
  rw [admission_is_consumption ledger snapshot facts witness checkpoint fresh admissible]
  intro other member
  rcases List.mem_cons.mp member with added | old
  · rw [added]
    have noPrior : (ledger.consumed.filter fun prior => decide (prior = witness)) = [] := by
      apply List.filter_eq_nil_iff.mpr
      intro prior present same
      have equal : prior = witness := of_decide_eq_true same
      exact fresh (equal ▸ present)
    simp [consume, noPrior]
  · have head : decide (witness = other) = false := by
      have distinct : other ≠ witness := by
        intro equal
        exact fresh (equal ▸ old)
      simp [Ne.symm distinct]
    rw [show (consume ledger facts witness checkpoint).consumed
      = witness :: ledger.consumed from rfl]
    simp [head]
    exact unique other old

/-- One admission preserves the checkpoint quota bound. The accepted checkpoint's
    bound is no longer a separate premise: it is the cumulative comparison that
    admissibility enforces, so an admission cannot push a checkpoint above its
    snapshot. -/
theorem admission_preserves_quota (ledger : Ledger) (facts : Facts) (snapshot : Snapshot)
    (witness checkpoint : Nat) (fresh : witness ∉ ledger.consumed)
    (admissible : FactsAdmissible ledger snapshot facts checkpoint = true)
    (within : WithinQuota ledger snapshot) :
    WithinQuota (accept ledger snapshot facts witness checkpoint) snapshot := by
  rw [admission_is_consumption ledger snapshot facts witness checkpoint fresh admissible]
  intro other
  by_cases same : other = checkpoint
  · subst same
    simp [consume, chargedFor_cons_same]
    have fits := admissible_implies_within_snapshot ledger snapshot facts other admissible
    simpa [Nat.add_comm] using fits
  · rw [show (consume ledger facts witness checkpoint).charges
      = (checkpoint, facts.cost) :: ledger.charges from rfl]
    rw [chargedFor_cons_other other checkpoint ledger.charges facts.cost (Ne.symm same)]
    exact within other

/-- A ledger whose checkpoint 0 is already charged to its snapshot. -/
def chargedLedger : Ledger :=
  { consumed := [7], attempts := [⟨7, 0, 5, false⟩], charges := [(0, 5)] }

/-- The checkpoint-0 snapshot of five. -/
def fiveSnapshot : Snapshot := fun _ => 5

/-- Otherwise-admissible facts costing one unit. -/
def oneMoreFacts : Facts :=
  { plan := 1, grant := 2, requestedPlan := 1, requestedGrant := 2, sourceAuthentic := true,
    checkpointValid := true, policyLive := true, credentialsValid := true, revoked := false,
    cost := 1 }

/-- **The cumulative quota is enforced at admission**: five units are already
    charged at checkpoint 0 against a snapshot of five, so a fresh witness costing
    one more unit is refused and the ledger is unchanged. Under the old
    per-request `cost ≤ available` test this admission succeeded and the charge
    reached six. -/
theorem exhausted_checkpoint_quota_refuses :
    accept chargedLedger fiveSnapshot oneMoreFacts 8 0 = chargedLedger := by
  decide

/-- Starting an already started attempt is refused rather than counted twice. -/
theorem started_attempt_cannot_start_again (ledger : Ledger) (attempt : Attempt)
    (registered : ledger.attempts.find? (fun other => other.witness = attempt.witness)
      = some attempt) (started : attempt.started = true) :
    start ledger attempt.witness = none := by
  simp [start, registered, started]

/-- Every registered attempt's cost is present in the checkpoint charges: quota
    accounting and the attempt ledger cannot disagree. -/
def AttemptsCharged (ledger : Ledger) : Prop :=
  ∀ attempt ∈ ledger.attempts, (attempt.checkpoint, attempt.cost) ∈ ledger.charges

theorem initial_attempts_charged : AttemptsCharged initial := by
  intro attempt member
  exact absurd member (by simp [initial])

theorem admission_preserves_attempts_charged (ledger : Ledger) (snapshot : Snapshot)
    (facts : Facts) (witness checkpoint : Nat) (fresh : witness ∉ ledger.consumed)
    (admissible : FactsAdmissible ledger snapshot facts checkpoint = true)
    (charged : AttemptsCharged ledger) :
    AttemptsCharged (accept ledger snapshot facts witness checkpoint) := by
  rw [admission_is_consumption ledger snapshot facts witness checkpoint fresh admissible]
  intro attempt member
  rcases List.mem_cons.mp member with added | old
  · rw [added]
    exact List.mem_cons_self ..
  · exact List.mem_cons_of_mem _ (charged attempt old)

/-- Mutation control: a consuming transition that forgets the quota charge breaks
    the accounting invariant, so the charge is load-bearing rather than
    decorative. -/
def consumeUncharged (ledger : Ledger) (facts : Facts) (witness checkpoint : Nat) : Ledger :=
  ⟨witness :: ledger.consumed,
   ⟨witness, checkpoint, facts.cost, false⟩ :: ledger.attempts,
   ledger.charges⟩

theorem omitted_quota_charge_breaks_accounting (facts : Facts) (witness checkpoint : Nat) :
    ¬ AttemptsCharged (consumeUncharged initial facts witness checkpoint) := by
  intro charged
  have present := charged ⟨witness, checkpoint, facts.cost, false⟩ (by simp [consumeUncharged])
  simp [consumeUncharged, initial] at present

/-- Mutation control: a transition that skips the freshness check consumes a
    witness twice and breaks uniqueness. -/
def consumeUnchecked (ledger : Ledger) (facts : Facts) (witness checkpoint : Nat) : Ledger :=
  ⟨witness :: ledger.consumed,
   ⟨witness, checkpoint, facts.cost, false⟩ :: ledger.attempts,
   (checkpoint, facts.cost) :: ledger.charges⟩

theorem unchecked_reuse_breaks_uniqueness (ledger : Ledger) (facts : Facts)
    (witness checkpoint : Nat) (present : witness ∈ ledger.consumed) :
    ¬ WitnessUniqueness (consumeUnchecked ledger facts witness checkpoint) := by
  intro unique
  have kept : witness ∈ List.filter (fun other => decide (other = witness)) ledger.consumed := by
    apply List.mem_filter.mpr
    exact ⟨present, by simp⟩
  have nonempty : List.filter (fun other => decide (other = witness)) ledger.consumed ≠ [] := by
    intro empty
    rw [empty] at kept
    exact nomatch kept
  have positive : 0 < (List.filter (fun other => decide (other = witness))
      ledger.consumed).length := List.length_pos_iff.mpr nonempty
  have counted := unique witness (by simp [consumeUnchecked])
  have lengths : (List.filter (fun other => decide (other = witness))
      (consumeUnchecked ledger facts witness checkpoint).consumed).length
      = (List.filter (fun other => decide (other = witness)) ledger.consumed).length + 1 := by
    simp [consumeUnchecked]
  rw [lengths] at counted
  omega

/-- Admissibility with the exact-plan comparison removed: the plan-blind mutant's
    own precondition. -/
def FactsAdmissibleIgnoringPlan (ledger : Ledger) (snapshot : Snapshot) (facts : Facts)
    (checkpoint : Nat) : Bool :=
  facts.sourceAuthentic && facts.checkpointValid && facts.policyLive
    && facts.credentialsValid && !facts.revoked
    && decide (chargedFor checkpoint ledger.charges + facts.cost ≤ snapshot checkpoint)

/-- Mutation control: an admission that ignores the exact plan comparison mints an
    attempt for an altered requested plan, so the comparison is load-bearing. -/
def acceptIgnoringPlan (ledger : Ledger) (snapshot : Snapshot) (facts : Facts)
    (witness checkpoint : Nat) : Ledger :=
  if witness ∈ ledger.consumed then refuse ledger facts
  else if FactsAdmissibleIgnoringPlan ledger snapshot facts checkpoint then
    consume ledger facts witness checkpoint
  else refuse ledger facts

/-- Admissibility with checkpoint validity removed: the checkpoint-blind mutant's
    own precondition. -/
def FactsAdmissibleIgnoringCheckpoint (ledger : Ledger) (snapshot : Snapshot) (facts : Facts)
    (checkpoint : Nat) : Bool :=
  FactsExact facts && facts.sourceAuthentic && facts.policyLive
    && facts.credentialsValid && !facts.revoked
    && decide (chargedFor checkpoint ledger.charges + facts.cost ≤ snapshot checkpoint)

/-- Mutation control: an admission that ignores checkpoint validity charges stale
    facts, so checkpoint validation is load-bearing. -/
def acceptIgnoringCheckpoint (ledger : Ledger) (snapshot : Snapshot) (facts : Facts)
    (witness checkpoint : Nat) : Ledger :=
  if witness ∈ ledger.consumed then refuse ledger facts
  else if FactsAdmissibleIgnoringCheckpoint ledger snapshot facts checkpoint then
    consume ledger facts witness checkpoint
  else refuse ledger facts

/-- The plan-blind mutant mints exactly where the real admission refuses: with the
    plan altered but every other fact and the cumulative quota in order, it
    consumes the witness while `accept` leaves the ledger unchanged. `altered` is
    used in the proof, so the theorem is not true for arbitrary facts. -/
theorem plan_blind_admission_mints_altered_plan (ledger : Ledger) (snapshot : Snapshot)
    (facts : Facts) (witness checkpoint : Nat) (fresh : witness ∉ ledger.consumed)
    (altered : facts.plan ≠ facts.requestedPlan)
    (authentic : facts.sourceAuthentic = true) (checkpointValid : facts.checkpointValid = true)
    (live : facts.policyLive = true) (credentials : facts.credentialsValid = true)
    (revoked : facts.revoked = false)
    (within : chargedFor checkpoint ledger.charges + facts.cost ≤ snapshot checkpoint) :
    (acceptIgnoringPlan ledger snapshot facts witness checkpoint).consumed
        = witness :: ledger.consumed
      ∧ accept ledger snapshot facts witness checkpoint = ledger := by
  have exactFalse : FactsExact facts = false := by
    simp [FactsExact, altered]
  have mutantAdmissible : FactsAdmissibleIgnoringPlan ledger snapshot facts checkpoint = true := by
    simp [FactsAdmissibleIgnoringPlan, authentic, checkpointValid, live, credentials, revoked,
      within]
  have realInadmissible : FactsAdmissible ledger snapshot facts checkpoint = false := by
    simp [FactsAdmissible, exactFalse]
  constructor
  · simp [acceptIgnoringPlan, fresh, mutantAdmissible, consume]
  · simp [accept, fresh, realInadmissible, refuse]

/-- The checkpoint-blind mutant mints exactly where the real admission refuses:
    with checkpoint validity false but the plan exact and the other facts in
    order, it consumes the witness while `accept` leaves the ledger unchanged. -/
theorem checkpoint_blind_admission_mints_stale_checkpoint (ledger : Ledger) (snapshot : Snapshot)
    (facts : Facts) (witness checkpoint : Nat) (fresh : witness ∉ ledger.consumed)
    (stale : facts.checkpointValid = false)
    (exact : FactsExact facts = true)
    (authentic : facts.sourceAuthentic = true)
    (live : facts.policyLive = true) (credentials : facts.credentialsValid = true)
    (revoked : facts.revoked = false)
    (within : chargedFor checkpoint ledger.charges + facts.cost ≤ snapshot checkpoint) :
    (acceptIgnoringCheckpoint ledger snapshot facts witness checkpoint).consumed
        = witness :: ledger.consumed
      ∧ accept ledger snapshot facts witness checkpoint = ledger := by
  have mutantAdmissible : FactsAdmissibleIgnoringCheckpoint ledger snapshot facts checkpoint = true := by
    simp [FactsAdmissibleIgnoringCheckpoint, exact, authentic, live, credentials, revoked, within]
  have realInadmissible : FactsAdmissible ledger snapshot facts checkpoint = false := by
    simp [FactsAdmissible, stale]
  constructor
  · simp [acceptIgnoringCheckpoint, fresh, mutantAdmissible, consume]
  · simp [accept, fresh, realInadmissible, refuse]

/-- A finite serialized prefix: admission, then one start of the last committed
    witness. Every step either preserves the ledger (refusal) or adds exactly one
    attempt (admission). -/
def acceptPrefix (ledger : Ledger) (snapshot : Snapshot) (requests : List Facts)
    (checkpoint : Nat) : Ledger :=
  requests.foldl (fun state facts =>
    accept state snapshot facts state.consumed.length checkpoint) ledger

/-- Preservation over any finite serialized prefix: the consumed set length equals
    the attempt ledger length and every consumed witness names a registered
    attempt. -/
theorem prefix_preserves_alignment (snapshot : Snapshot) (requests : List Facts)
    (checkpoint : Nat) :
    ∀ ledger : Ledger, WitnessAlignment ledger →
      WitnessAlignment (acceptPrefix ledger snapshot requests checkpoint) := by
  induction requests with
  | nil => intro ledger aligned; exact aligned
  | cons facts rest induction =>
      intro ledger aligned
      unfold acceptPrefix
      rw [List.foldl_cons]
      refine induction (accept ledger snapshot facts ledger.consumed.length checkpoint) ?_
      by_cases present : ledger.consumed.length ∈ ledger.consumed
      · simp [accept, present]
        exact aligned
      · by_cases admissible : FactsAdmissible ledger snapshot facts checkpoint = true
        · exact admission_preserves_alignment ledger snapshot facts _ checkpoint present
            admissible aligned
        · simp [accept, present, admissible, refuse]
          exact aligned

/-- Checkpoint renewal is monotone: a ledger within one snapshot stays within every
    snapshot that is at least as large at every checkpoint. This uses the existing
    bound rather than restating it; host replenishment that lowers a snapshot
    below the charges is outside this premise rather than kernel-enforced safety. -/
theorem renewed_snapshot_keeps_quota (ledger : Ledger) (snapshot renewed : Snapshot)
    (within : WithinQuota ledger snapshot)
    (carried : ∀ checkpoint, snapshot checkpoint ≤ renewed checkpoint) :
    WithinQuota ledger renewed :=
  fun checkpoint => Nat.le_trans (within checkpoint) (carried checkpoint)

end NobleFoundation.AuthorityLedger
