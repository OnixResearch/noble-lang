import NobleKernel

open Aeneas Aeneas.Std
open noble_kernel

set_option maxHeartbeats 2000000
set_option maxRecDepth 4096

/-!
Implementation theorems over the retained Charon/Aeneas translation of the M5 authority ledger
(`crates/noble-kernel/src/authority`). Every statement names the generated functions themselves:
`admission.Authority.admit`, `commit`, `charge_quota`, `start_execution`, the `facts` validators and
the public authorization, checkpoint, observation, retirement and delivery transitions. No handwritten
machine stands in for them. `verification/m5-authority/check.mjs` compiles the retained translation
anew and requires every Rust source file with a generated declaration reachable from these theorems
to keep the bytes the extraction recorded.

Every public state-returning transition is mapped to one `LedgerMove`, from which the reachable
invariants follow: the checkpoint-local quota bound, and the agreement of the witness and attempt
tables with the published counters. Two trace properties are proved over every finite sequence of
public transitions (`Relation.ReflTransGen Step`): a settled witness record is never rewritten,
and a started attempt is never started again.

Scope: serialized calls on one owned authority value. Checkpoint renewal is a host premise, not kernel
safety: quota records are checkpoint-local, so no cross-checkpoint conservation is stated. No
concurrency, crash-recovery, remote-revocation atomicity, exactly-once delivery, Charon/Aeneas
translation or Rust compiler correctness claim.
-/

namespace M5Authority

open authority

/-! ### Result inversion -/

theorem bind_eq_ok {α β : Type} {m : Result α} {f : α → Result β} {v : β}
    (h : Bind.bind m f = Result.ok v) : ∃ a, m = Result.ok a ∧ f a = Result.ok v := by
  cases m with
  | ret a => exact ⟨a, rfl, by simpa using h⟩
  | vis e k => simp [Bind.bind, Aeneas.Std.bind_vis] at h
  | div => simp [Bind.bind] at h

/-! ### Every comparison on the admission path is total and decides equality -/

deriving instance DecidableEq for authority.OwnerContext
deriving instance DecidableEq for authority.Checkpoint
deriving instance DecidableEq for authority.Constraints
deriving instance DecidableEq for authority.OperationContract
deriving instance DecidableEq for authority.PlanDescription
deriving instance DecidableEq for authority.Plan
deriving instance DecidableEq for authority.permit.WitnessClaim
deriving instance DecidableEq for authority.report.InvocationOutcome
deriving instance DecidableEq for authority.permit.WitnessState

@[simp] theorem owner_eq (a b : OwnerContext) :
    OwnerContext.Insts.CoreCmpPartialEqOwnerContext.eq a b = Result.ok (decide (a = b)) := by
  obtain ⟨ai, av, ag⟩ := a
  obtain ⟨bi, bv, bg⟩ := b
  simp only [OwnerContext.Insts.CoreCmpPartialEqOwnerContext.eq, OwnerContext.mk.injEq]
  by_cases h1 : ai = bi <;> by_cases h2 : av = bv <;> simp [h1, h2]

@[simp] theorem checkpoint_eq (a b : Checkpoint) :
    Checkpoint.Insts.CoreCmpPartialEqCheckpoint.eq a b = Result.ok (decide (a = b)) := by
  obtain ⟨aseq, anow⟩ := a
  obtain ⟨bseq, bnow⟩ := b
  simp only [Checkpoint.Insts.CoreCmpPartialEqCheckpoint.eq, Checkpoint.mk.injEq]
  by_cases h1 : aseq = bseq <;> simp [h1]

theorem bytes_allM : ∀ (l0 l1 : List U8), l0.length = l1.length →
    List.allM (fun (x : U8 × U8) => match x with
      | (x0, x1) => core.cmp.PartialEqU8.eq x0 x1) (l0.zip l1) = Result.ok (decide (l0 = l1))
  | [], [], _ => by simp; rfl
  | [], _ :: _, h => by simp at h
  | _ :: _, [], h => by simp at h
  | x :: xs, y :: ys, h => by
    have tail := bytes_allM xs ys (by simpa using h)
    by_cases hxy : x = y
    · subst hxy
      simp [liftFun2, core.cmp.impls.PartialEqU8.eq, tail]
    · simp [liftFun2, core.cmp.impls.PartialEqU8.eq, hxy]; rfl

@[simp] theorem bytes_eq (v0 v1 : alloc.vec.Vec U8) :
    alloc.vec.partial_eq.PartialEqVec.eq core.cmp.PartialEqU8 v0 v1 = Result.ok (decide (v0 = v1)) := by
  unfold alloc.vec.partial_eq.PartialEqVec.eq
  split
  · rename_i hlen
    rw [bytes_allM v0.val v1.val hlen]
    simp only [Result.ok.injEq, decide_eq_decide]
    exact (alloc.vec.Vec.eq_iff v0 v1).symm
  · rename_i hlen
    have : v0 ≠ v1 := fun h => hlen (by rw [h])
    simp [this]

@[simp] theorem constraints_eq (a b : Constraints) :
    Constraints.Insts.CoreCmpPartialEqConstraints.eq a b = Result.ok (decide (a = b)) := by
  obtain ⟨anb, aex, au, asc, aq⟩ := a
  obtain ⟨bnb, bex, bu, bsc, bq⟩ := b
  simp only [Constraints.Insts.CoreCmpPartialEqConstraints.eq, QuotaId.Insts.CoreCmpPartialEqQuotaId.eq,
    Constraints.mk.injEq]
  split_ifs <;> simp_all

@[simp] theorem operation_eq (a b : OperationContract) :
    OperationContract.Insts.CoreCmpPartialEqOperationContract.eq a b = Result.ok (decide (a = b)) := by
  obtain ⟨an, ae⟩ := a
  obtain ⟨bn, be⟩ := b
  simp only [OperationContract.Insts.CoreCmpPartialEqOperationContract.eq, bytes_eq, bind_tc_ok,
    types.EffId.Insts.CoreCmpPartialEqEffId.eq, OperationContract.mk.injEq]
  split_ifs <;> simp_all

@[simp] theorem description_eq (a b : PlanDescription) :
    PlanDescription.Insts.CoreCmpPartialEqPlanDescription.eq a b = Result.ok (decide (a = b)) := by
  obtain ⟨aop, aargs, aact, aown, apol, acon⟩ := a
  obtain ⟨bop, bargs, bact, bown, bpol, bcon⟩ := b
  simp only [PlanDescription.Insts.CoreCmpPartialEqPlanDescription.eq, operation_eq, bytes_eq,
    owner_eq, constraints_eq, ActorId.Insts.CoreCmpPartialEqActorId.eq,
    PolicyRevision.Insts.CoreCmpPartialEqPolicyRevision.eq, bind_tc_ok, PlanDescription.mk.injEq]
  split_ifs <;> simp_all

@[simp] theorem plan_eq (a b : Plan) :
    Plan.Insts.CoreCmpPartialEqPlan.eq a b = Result.ok (decide (a = b)) := by
  obtain ⟨ad⟩ := a
  obtain ⟨bd⟩ := b
  simp [Plan.Insts.CoreCmpPartialEqPlan.eq]

@[simp] theorem claim_eq (a b : permit.WitnessClaim) :
    permit.WitnessClaim.Insts.CoreCmpPartialEqWitnessClaim.eq a b = Result.ok (decide (a = b)) := by
  obtain ⟨ao, aslot, ag, ak, ar, asc⟩ := a
  obtain ⟨bo, bslot, bg, bk, br, bsc⟩ := b
  simp only [permit.WitnessClaim.Insts.CoreCmpPartialEqWitnessClaim.eq, owner_eq,
    types.ResourceKind.Insts.CoreCmpPartialEqResourceKind.eq, bind_tc_ok, permit.WitnessClaim.mk.injEq]
  split_ifs <;> simp_all

/-! ### Cloning a plan returns the same plan -/

theorem clone_bytes (v : alloc.vec.Vec U8) :
    alloc.vec.CloneVec.clone core.clone.CloneU8 v = Result.ok v := by
  obtain ⟨s', hclone, hsame⟩ := WP.spec_imp_exists
    (Slice.clone_spec (clone := core.clone.CloneU8.clone) (s := v.slice) (fun _ _ => rfl))
  simp [alloc.vec.CloneVec.clone, hclone, ← hsame]

@[simp] theorem plan_clone (plan : Plan) : Plan.Insts.CoreCloneClone.clone plan = Result.ok plan := by
  obtain ⟨⟨⟨opName, effect⟩, arguments, actor, owner, policy, constraints⟩⟩ := plan
  simp [Plan.Insts.CoreCloneClone.clone, PlanDescription.Insts.CoreCloneClone.clone,
    OperationContract.Insts.CoreCloneClone.clone, clone_bytes, types.EffId.Insts.CoreCloneClone.clone,
    ActorId.Insts.CoreCloneClone.clone, OwnerContext.Insts.CoreCloneClone.clone,
    PolicyRevision.Insts.CoreCloneClone.clone, Constraints.Insts.CoreCloneClone.clone]

/-! ### The quota scan finds only the plan's quota at the current checkpoint -/

/-- A located quota record belongs to the plan's quota identity at the authority's checkpoint. -/
def QuotaMatch (s : Authority) (plan : Plan) : Option Usize → Prop
  | none => True
  | some j => ∃ r, s.quotas.val[j.val]? = some r ∧
      r.quota = plan.description.constraints.quota ∧ r.checkpoint = s.profile.checkpoint

theorem quota_position_matches (s : Authority) (plan : Plan) :
    ∃ o, facts.Authority.quota_position s plan = Result.ok o ∧ QuotaMatch s plan o := by
  apply WP.spec_imp_exists
  unfold facts.Authority.quota_position facts.Authority.quota_position_loop
  apply loop.spec_decr_nat (fun index : Usize => s.quotas.val.length - index.val) (fun _ => True)
  · intro index _
    rw [WP.spec_equiv_exists]
    unfold facts.Authority.quota_position_loop.body
    by_cases hlt : index.val < s.quotas.val.length
    · have hbound := alloc.vec.Vec.len_ineq s.quotas
      obtain ⟨index1, hadd, hval⟩ := WP.spec_imp_exists
        (UScalar.add_spec (x := index) (y := 1#usize) (by scalar_tac))
      have hget : alloc.vec.Vec.index (core.slice.index.SliceIndexUsizeSlice QuotaRecord) s.quotas index =
          Result.ok (s.quotas.val[index.val]) := by
        simp [alloc.vec.Vec.index_slice_index, alloc.vec.Vec.index_usize, hlt]
      have hlt' : index < alloc.vec.Vec.len s.quotas := by scalar_tac
      simp only [hlt', if_true, hget, bind_tc_ok, QuotaId.Insts.CoreCmpPartialEqQuotaId.eq, checkpoint_eq]
      split_ifs with hquota hcheckpoint
      · refine ⟨_, rfl, ?_⟩
        simp only [QuotaMatch]
        exact ⟨s.quotas.val[index.val], by simp [hlt]; rfl, of_decide_eq_true hquota,
          of_decide_eq_true hcheckpoint⟩
      · refine ⟨ControlFlow.cont index1, by simp [hadd], ?_⟩
        simp only [true_and]
        scalar_tac
      · refine ⟨ControlFlow.cont index1, by simp [hadd], ?_⟩
        simp only [true_and]
        scalar_tac
    · have hge : ¬ index < alloc.vec.Vec.len s.quotas := by scalar_tac
      simp [hge, QuotaMatch]
  · trivial

/-! ### Start: an attempt starts at most once -/

/-- The attempt lookup accepts exactly an owned, present, same-generation key. -/
theorem attempt_record_ok (s : Authority) (key : permit.AttemptKey) (record : AttemptRecord) :
    admission.Authority.attempt_record s key = Result.ok (.Ok record) ↔
      key.owner = s.profile.owner ∧ s.attempts.val[key.slot.val]? = some record ∧
        key.generation = record.key.generation := by
  unfold admission.Authority.attempt_record
  simp only [core.cmp.PartialEq.ne.trait_default, core.cmp.PartialEq.ne.default, owner_eq,
    bind_tc_ok, alloc.vec.Vec.deref, core.slice.Slice.get, core.slice.index.Usize.get]
  by_cases howner : key.owner = s.profile.owner
  · simp only [howner, decide_true, true_and, not_true_eq_false, decide_false, Bool.false_eq_true,
      ↓reduceIte]
    split
    · rename_i hnone
      simp at hnone
      simp [hnone]
    · rename_i found hsome
      simp at hsome
      simp only [hsome, Option.some.injEq]
      by_cases hgen : key.generation = found.key.generation
      · constructor
        · intro hok
          simp [hgen] at hok
          exact ⟨hok, by rw [hgen, hok]⟩
        · rintro ⟨rfl, -⟩
          simp [hgen]
      · constructor
        · intro hok
          simp [hgen] at hok
        · rintro ⟨rfl, hsame⟩
          exact absurd hsame hgen
  · simp [howner]

theorem start_execution_refuses_a_started_attempt
    (self : Authority) (execution : permit.Execution) (record : AttemptRecord)
    (found : admission.Authority.attempt_record self execution.attempt = Result.ok (.Ok record))
    (started : record.started = true) :
    admission.Authority.start_execution self execution =
      Result.ok (.Err report.BoundaryError.AlreadyStarted, self) := by
  simp [admission.Authority.start_execution, found, started]

theorem start_execution_refusal_returns_input
    (self s' : Authority) (execution : permit.Execution) (error : report.BoundaryError)
    (h : admission.Authority.start_execution self execution = Result.ok (.Err error, s')) :
    s' = self := by
  unfold admission.Authority.start_execution at h
  obtain ⟨r, -, h⟩ := bind_eq_ok h
  split at h
  · split at h
    · simp at h
      exact h.2.symm
    · obtain ⟨b, -, h⟩ := bind_eq_ok h
      split at h
      · simp at h
        exact h.2.symm
      · obtain ⟨p, -, h⟩ := bind_eq_ok h
        obtain ⟨⟨ar, back⟩, -, h⟩ := bind_eq_ok h
        obtain ⟨i, -, h⟩ := bind_eq_ok h
        simp at h
  · simp at h
    exact h.2.symm

/-- A successful start marks exactly the presented unstarted attempt and counts one protected
    operation; witnesses, quota records and the other ledger counters are untouched. -/
theorem start_execution_marks_one_unstarted_attempt
    (self s' : Authority) (execution : permit.Execution) (started : permit.Started)
    (h : admission.Authority.start_execution self execution = Result.ok (.Ok started, s')) :
    ∃ record, admission.Authority.attempt_record self execution.attempt = Result.ok (.Ok record) ∧
      record.started = false ∧
      s'.attempts.val = self.attempts.val.set execution.attempt.slot.val { record with started := true } ∧
      s'.counters.protected_operations.val = self.counters.protected_operations.val + 1 ∧
      s'.witnesses = self.witnesses ∧ s'.quotas = self.quotas ∧ s'.profile = self.profile ∧
      started.attempt = record.key ∧ started.plan = record.plan ∧
      s'.counters.witnesses_created = self.counters.witnesses_created ∧
      s'.counters.witness_consumptions = self.counters.witness_consumptions ∧
      s'.counters.attempts_admitted = self.counters.attempts_admitted := by
  unfold admission.Authority.start_execution at h
  obtain ⟨r, hr, h⟩ := bind_eq_ok h
  split at h
  · rename_i record
    split at h
    · simp at h
    · rename_i hfresh
      obtain ⟨b, hb, h⟩ := bind_eq_ok h
      split at h
      · simp at h
      · obtain ⟨p, hp, h⟩ := bind_eq_ok h
        obtain ⟨⟨ar, back⟩, hmut, h⟩ := bind_eq_ok h
        obtain ⟨i, hi, h⟩ := bind_eq_ok h
        simp at h
        obtain ⟨rfl, rfl⟩ := h
        have hfound := (attempt_record_ok _ _ _).1 hr
        simp only [plan_clone, Result.ok.injEq] at hp
        subst hp
        simp only [alloc.vec.Vec.index_mut_slice_index, alloc.vec.Vec.index_mut_usize,
          alloc.vec.Vec.index_usize, alloc.vec.Vec.getElem?_Nat_eq, hfound.2.1, bind_tc_ok,
          Result.ok.injEq, Prod.mk.injEq] at hmut
        obtain ⟨rfl, rfl⟩ := hmut
        obtain ⟨i', hadd, hval⟩ := WP.spec_imp_exists
          (UScalar.add_spec (x := self.counters.protected_operations) (y := 1#u32) (by
            have := (UScalar.add_equiv self.counters.protected_operations 1#u32)
            rw [hi] at this
            simp at this
            scalar_tac))
        rw [hadd] at hi
        simp only [Result.ok.injEq] at hi
        subst hi
        refine ⟨record, hr, by simpa using hfresh, ?_, ?_, rfl, rfl, rfl, rfl, rfl, rfl, rfl, rfl⟩
        · simp [alloc.vec.Vec.set_val_eq]
        · simpa using hval
  · simp at h

/-- Attempts start at most once: replaying the same execution permit on the resulting state is
    refused and changes nothing. -/
theorem started_attempt_cannot_start_again
    (self s' : Authority) (execution : permit.Execution) (started : permit.Started)
    (h : admission.Authority.start_execution self execution = Result.ok (.Ok started, s')) :
    admission.Authority.start_execution s' execution =
      Result.ok (.Err report.BoundaryError.AlreadyStarted, s') := by
  obtain ⟨record, hr, -, hattempts, -, -, -, hprofile, -, -⟩ :=
    start_execution_marks_one_unstarted_attempt self s' execution started h
  have hfound := (attempt_record_ok _ _ _).1 hr
  apply start_execution_refuses_a_started_attempt s' execution { record with started := true }
  · rw [attempt_record_ok]
    refine ⟨by rw [hprofile]; exact hfound.1, ?_, hfound.2.2⟩
    have hlt : execution.attempt.slot.val < self.attempts.val.length := by
      have := hfound.2.1
      exact (List.getElem?_eq_some_iff.1 this).1
    rw [hattempts]
    simp [hlt]
  · rfl

/-! ### Refusal: denial and preflight leave the ledger unchanged -/

/-! ### Per-check facts: each lemma observes exactly one validation step of the extracted code -/

@[simp] theorem invocation_eq (a b : report.InvocationOutcome) :
    report.InvocationOutcome.Insts.CoreCmpPartialEqInvocationOutcome.eq a b = Result.ok (decide (a = b)) := by
  cases a <;> cases b <;>
    simp [report.InvocationOutcome.Insts.CoreCmpPartialEqInvocationOutcome.eq,
      report.InvocationOutcome.read_discriminant]

@[simp] theorem ne_trait_default_owner (a b : OwnerContext) :
    core.cmp.PartialEq.ne.trait_default OwnerContext.Insts.CoreCmpPartialEqOwnerContext a b =
      Result.ok (!decide (a = b)) := by
  simp [core.cmp.PartialEq.ne.trait_default, core.cmp.PartialEq.ne.default]

@[simp] theorem ne_trait_default_checkpoint (a b : Checkpoint) :
    core.cmp.PartialEq.ne.trait_default Checkpoint.Insts.CoreCmpPartialEqCheckpoint a b =
      Result.ok (!decide (a = b)) := by
  simp [core.cmp.PartialEq.ne.trait_default, core.cmp.PartialEq.ne.default]

@[simp] theorem ne_trait_default_plan (a b : Plan) :
    core.cmp.PartialEq.ne.trait_default Plan.Insts.CoreCmpPartialEqPlan a b =
      Result.ok (!decide (a = b)) := by
  simp [core.cmp.PartialEq.ne.trait_default, core.cmp.PartialEq.ne.default]

@[simp] theorem ne_trait_default_claim (a b : permit.WitnessClaim) :
    core.cmp.PartialEq.ne.trait_default permit.WitnessClaim.Insts.CoreCmpPartialEqWitnessClaim a b =
      Result.ok (!decide (a = b)) := by
  simp [core.cmp.PartialEq.ne.trait_default, core.cmp.PartialEq.ne.default]

@[simp] theorem ne_trait_default_invocation (a b : report.InvocationOutcome) :
    core.cmp.PartialEq.ne.trait_default report.InvocationOutcome.Insts.CoreCmpPartialEqInvocationOutcome a b =
      Result.ok (!decide (a = b)) := by
  simp [core.cmp.PartialEq.ne.trait_default, core.cmp.PartialEq.ne.default]

@[simp] theorem ne_trait_default_source (a b : SourceId) :
    core.cmp.PartialEq.ne.trait_default SourceId.Insts.CoreCmpPartialEqSourceId a b =
      Result.ok (!decide (a = b)) := by
  simp [core.cmp.PartialEq.ne.trait_default, core.cmp.PartialEq.ne.default,
    SourceId.Insts.CoreCmpPartialEqSourceId.eq]

@[simp] theorem ne_trait_default_kind (a b : types.ResourceKind) :
    core.cmp.PartialEq.ne.trait_default types.ResourceKind.Insts.CoreCmpPartialEqResourceKind a b =
      Result.ok (!decide (a = b)) := by
  simp [core.cmp.PartialEq.ne.trait_default, core.cmp.PartialEq.ne.default,
    types.ResourceKind.Insts.CoreCmpPartialEqResourceKind.eq]

/-- An accepted witness lookup found the record at the claimed slot. -/
theorem witness_record_found (s : Authority) (claim : permit.WitnessClaim) (record : WitnessRecord)
    (h : admission.Authority.witness_record s claim = Result.ok (.Ok record)) :
    claim.owner = s.profile.owner ∧ s.witnesses.val[claim.slot.val]? = some record := by
  unfold admission.Authority.witness_record at h
  simp only [ne_trait_default_owner, ne_trait_default_kind, bind_tc_ok, lift, alloc.vec.Vec.deref,
    core.slice.Slice.get, core.slice.index.Usize.get] at h
  by_cases howner : claim.owner = s.profile.owner
  · refine ⟨howner, ?_⟩
    simp only [howner, decide_true, Bool.not_true, Bool.false_eq_true, ↓reduceIte] at h
    split at h
    · simp at h
    · rename_i found hfound
      simp at hfound
      split_ifs at h <;> simp at h
      subst h
      exact hfound
  · simp [howner] at h

theorem validate_entry_requires_the_presented_claim
    (s : Authority) (witness : Option permit.Witness) (request : permit.AdmissionRequest)
    (current : CurrentFacts) (available : U64)
    (h : admission.Authority.validate_entry s witness request current = Result.ok (.Ok available)) :
    ∃ presented, witness = some presented ∧ presented.claim = request.claim ∧
      request.claim.owner = s.profile.owner := by
  unfold admission.Authority.validate_entry at h
  split at h
  · simp at h
  · rename_i presented
    obtain ⟨r, hr, h⟩ := bind_eq_ok h
    split at h
    · rename_i retained
      obtain ⟨b, hb, h⟩ := bind_eq_ok h
      simp only [ne_trait_default_claim, Result.ok.injEq] at hb
      subst hb
      split at h
      · simp at h
      · rename_i hsame
        exact ⟨presented, rfl, by simpa using hsame, (witness_record_found _ _ _ hr).1⟩
    · simp at h

theorem validate_entry_requires_a_live_witness
    (s : Authority) (witness : Option permit.Witness) (request : permit.AdmissionRequest)
    (current : CurrentFacts) (available : U64)
    (h : admission.Authority.validate_entry s witness request current = Result.ok (.Ok available)) :
    ∃ retained, s.witnesses.val[request.claim.slot.val]? = some retained ∧
      retained.state = permit.WitnessState.Live := by
  unfold admission.Authority.validate_entry at h
  split at h
  · simp at h
  · obtain ⟨r, hr, h⟩ := bind_eq_ok h
    split at h
    · rename_i retained
      obtain ⟨b, -, h⟩ := bind_eq_ok h
      split at h
      · simp at h
      · split at h
        · rename_i hlive
          exact ⟨retained, (witness_record_found _ _ _ hr).2, hlive⟩
        · simp at h
        · simp at h
    · simp at h

theorem validate_entry_requires_the_exact_plan
    (s : Authority) (witness : Option permit.Witness) (request : permit.AdmissionRequest)
    (current : CurrentFacts) (available : U64)
    (h : admission.Authority.validate_entry s witness request current = Result.ok (.Ok available)) :
    ∃ retained, s.witnesses.val[request.claim.slot.val]? = some retained ∧
      retained.plan = request.plan := by
  unfold admission.Authority.validate_entry at h
  split at h
  · simp at h
  · obtain ⟨r, hr, h⟩ := bind_eq_ok h
    split at h
    · rename_i retained
      obtain ⟨b, -, h⟩ := bind_eq_ok h
      split at h
      · simp at h
      · split at h
        · obtain ⟨b1, hb1, h⟩ := bind_eq_ok h
          simp only [ne_trait_default_plan, Result.ok.injEq] at hb1
          subst hb1
          split at h
          · simp at h
          · rename_i hsame
            exact ⟨retained, (witness_record_found _ _ _ hr).2, by simpa using hsame⟩
        · simp at h
        · simp at h
    · simp at h

theorem validate_entry_requires_valid_facts
    (s : Authority) (witness : Option permit.Witness) (request : permit.AdmissionRequest)
    (current : CurrentFacts) (available : U64)
    (h : admission.Authority.validate_entry s witness request current = Result.ok (.Ok available)) :
    facts.Authority.validate_facts s request.plan current = Result.ok (.Ok ()) ∧
      current.quota_remaining = some available := by
  unfold admission.Authority.validate_entry at h
  split at h
  · simp at h
  · obtain ⟨r, -, h⟩ := bind_eq_ok h
    split at h
    · obtain ⟨b, -, h⟩ := bind_eq_ok h
      split at h
      · simp at h
      · split at h
        · obtain ⟨b1, -, h⟩ := bind_eq_ok h
          split at h
          · simp at h
          · obtain ⟨r1, hr1, h⟩ := bind_eq_ok h
            split at h
            · rename_i unitValue
              split at h
              · simp at h
              · rename_i remaining hremaining
                simp at h
                subst h
                cases unitValue
                exact ⟨hr1, hremaining⟩
            · simp at h
        · simp at h
        · simp at h
    · simp at h

theorem validate_facts_requires_the_current_checkpoint
    (s : Authority) (plan : Plan) (current : CurrentFacts)
    (h : facts.Authority.validate_facts s plan current = Result.ok (.Ok ())) :
    current.checkpoint = s.profile.checkpoint := by
  unfold facts.Authority.validate_facts at h
  obtain ⟨b, -, h⟩ := bind_eq_ok h
  split at h
  · simp at h
  · obtain ⟨b1, -, h⟩ := bind_eq_ok h
    split at h
    · simp at h
    · obtain ⟨b2, -, h⟩ := bind_eq_ok h
      split at h
      · simp at h
      · obtain ⟨b3, hb3, h⟩ := bind_eq_ok h
        simp only [ne_trait_default_checkpoint, Result.ok.injEq] at hb3
        subst hb3
        split at h
        · simp at h
        · rename_i hsame
          simpa using hsame

/-- The other fact checks: pending invocation, owner, authority source, granted exact plan,
    credentials, revocation, policy and validity window. -/
theorem validate_facts_requires_current_authority
    (s : Authority) (plan : Plan) (current : CurrentFacts)
    (h : facts.Authority.validate_facts s plan current = Result.ok (.Ok ())) :
    s.invocation = report.InvocationOutcome.Pending ∧ plan.description.owner = s.profile.owner ∧
      current.source = s.profile.authority_source ∧
      (∃ grant, current.grant = some grant ∧ grant.plan = plan ∧ grant.kind = s.profile.witness_kind) ∧
      current.credentials_valid = some true ∧ current.revoked = some false ∧
      current.policy = some plan.description.policy ∧
      plan.description.constraints.not_before.val ≤ current.checkpoint.now.val ∧
      current.checkpoint.now.val < plan.description.constraints.expires_at.val ∧
      facts.Authority.validate_quota s plan current = Result.ok (.Ok ()) := by
  unfold facts.Authority.validate_facts at h
  obtain ⟨b, hb, h⟩ := bind_eq_ok h
  simp only [ne_trait_default_invocation, Result.ok.injEq] at hb
  subst hb
  split at h
  · simp at h
  · rename_i hpending
    obtain ⟨b1, hb1, h⟩ := bind_eq_ok h
    simp only [ne_trait_default_owner, Result.ok.injEq] at hb1
    subst hb1
    split at h
    · simp at h
    · rename_i howner
      obtain ⟨b2, hb2, h⟩ := bind_eq_ok h
      simp only [ne_trait_default_source, Result.ok.injEq] at hb2
      subst hb2
      split at h
      · simp at h
      · rename_i hsource
        obtain ⟨b3, -, h⟩ := bind_eq_ok h
        split at h
        · simp at h
        · obtain ⟨r, hgrant, h⟩ := bind_eq_ok h
          split at h
          · obtain ⟨r1, hstatus, h⟩ := bind_eq_ok h
            split at h
            · refine ⟨by simpa using hpending, by simpa using howner,
                (UScalar.eq_equiv _ _).2 (by simpa using hsource), ?_, ?_⟩
              · unfold facts.Authority.validate_grant at hgrant
                split at hgrant
                · simp at hgrant
                · rename_i grant hsome
                  simp only [ne_trait_default_kind, ne_trait_default_plan, bind_tc_ok, lift] at hgrant
                  split_ifs at hgrant with hkind hright hplan <;> simp at hgrant
                  exact ⟨grant, hsome, by simpa using hplan, (UScalar.eq_equiv _ _).2 (by simpa using hkind)⟩
              · unfold facts.current_status at hstatus
                split at hstatus
                · simp at hstatus
                · rename_i credentials hcred
                  split at hstatus
                  · rename_i hvalid
                    split at hstatus
                    · simp at hstatus
                    · rename_i revoked hrev
                      split at hstatus
                      · simp at hstatus
                      · rename_i hnotrev
                        split at hstatus
                        · simp at hstatus
                        · rename_i policy hpol
                          dsimp only at hstatus
                          split_ifs at hstatus with hpeq hbefore hexpired
                          all_goals (try simp at hstatus)
                          refine ⟨by rw [hcred, hvalid], by simp [hrev, hnotrev], by rw [hpol, hpeq],
                            by scalar_tac, by scalar_tac, h⟩
                  · simp at hstatus
            · simp at h
          · simp at h

/-- The quota check accepts only a charge that fits the independently supplied snapshot: an existing
    checkpoint-local record must carry that same snapshot and stay within it after the charge. -/
theorem validate_quota_bounds_the_charge
    (s : Authority) (plan : Plan) (current : CurrentFacts)
    (h : facts.Authority.validate_quota s plan current = Result.ok (.Ok ())) :
    ∃ available, current.quota_remaining = some available ∧
      ∀ o, facts.Authority.quota_position s plan = Result.ok o →
        match o with
        | none => plan.description.constraints.units.val ≤ available.val
        | some j => ∃ record, s.quotas.val[j.val]? = some record ∧ record.available = available ∧
            record.spent.val + plan.description.constraints.units.val ≤ available.val := by
  unfold facts.Authority.validate_quota at h
  split at h
  · simp at h
  · rename_i available hsome
    refine ⟨available, hsome, ?_⟩
    intro o ho
    obtain ⟨o', ho', h⟩ := bind_eq_ok h
    rw [ho] at ho'
    simp only [Result.ok.injEq] at ho'
    subst ho'
    split at h
    · obtain ⟨o1, ho1, h⟩ := bind_eq_ok h
      simp only [lift, Result.ok.injEq] at ho1
      split at h
      · simp at h
      · rename_i remaining
        split_ifs at h with henough
        · have := core.num.checked_sub_UScalar_bv_spec available 0#u64
          simp only [U64.checked_sub] at ho1
          rw [ho1] at this
          simp only at this
          scalar_tac
        · simp at h
    · rename_i j
      obtain ⟨record, hrecord, h⟩ := bind_eq_ok h
      simp only [alloc.vec.Vec.index_slice_index, alloc.vec.Vec.index_usize] at hrecord
      split at hrecord
      · simp at hrecord
      · rename_i found hfound
        simp only [Result.ok.injEq] at hrecord
        subst hrecord
        split_ifs at h with hsame
        · simp at h
        · obtain ⟨o1, ho1, h⟩ := bind_eq_ok h
          simp only [lift, Result.ok.injEq] at ho1
          split at h
          · simp at h
          · rename_i remaining
            split_ifs at h with henough
            · have := core.num.checked_sub_UScalar_bv_spec available found.spent
              simp only [U64.checked_sub] at ho1
              rw [ho1] at this
              simp only at this
              refine ⟨found, by simpa using hfound, (UScalar.eq_equiv _ _).2 (by simpa using hsame),
                by scalar_tac⟩
            · simp at h

theorem charge_quota_refusal_returns_input
    (self s' : Authority) (plan : Plan) (available : U64) (denial : permit.Denial)
    (h : admission.Authority.charge_quota self plan available = Result.ok (.Err denial, s')) :
    s' = self := by
  unfold admission.Authority.charge_quota at h
  obtain ⟨o, -, h⟩ := bind_eq_ok h
  split at h
  · obtain ⟨v, -, h⟩ := bind_eq_ok h
    simp at h
  · obtain ⟨qr, -, h⟩ := bind_eq_ok h
    obtain ⟨o1, -, h⟩ := bind_eq_ok h
    split at h
    · simp at h
      exact h.2.symm
    · obtain ⟨⟨qr1, back⟩, -, h⟩ := bind_eq_ok h
      simp at h

theorem commit_refusal_returns_input
    (self s' : Authority) (request : permit.AdmissionRequest) (available : U64) (admitted : U32)
    (denial : permit.Denial)
    (h : admission.Authority.commit self request available admitted = Result.ok (.Err denial, s')) :
    s' = self := by
  unfold admission.Authority.commit at h
  obtain ⟨i1, -, h⟩ := bind_eq_ok h
  obtain ⟨p, -, h⟩ := bind_eq_ok h
  obtain ⟨⟨r, self1⟩, hcharge, h⟩ := bind_eq_ok h
  simp only [uncurry_apply_pair] at h
  split at h
  · obtain ⟨⟨wr, back⟩, -, h⟩ := bind_eq_ok h
    obtain ⟨v, -, h⟩ := bind_eq_ok h
    simp at h
  · simp at h
    obtain ⟨-, rfl⟩ := h
    exact charge_quota_refusal_returns_input _ _ _ _ _ hcharge

/-- The counters that count ledger records (witnesses created and consumed, attempts admitted,
    protected operations) are unchanged. -/
def CountersKept (c c' : Counters) : Prop :=
  c'.witnesses_created = c.witnesses_created ∧ c'.witness_consumptions = c.witness_consumptions ∧
    c'.attempts_admitted = c.attempts_admitted ∧ c'.protected_operations = c.protected_operations

/-- The witness, attempt and quota ledgers change only on a committed admission. Denial and
    preflight still record their request counter and requested effect, which are not ledger state,
    and leave every counter that counts ledger records unchanged. -/
theorem refused_admission_preserves_ledger
    (self s' : Authority) (witness : Option permit.Witness) (request : permit.AdmissionRequest)
    (facts : CurrentFacts) (outcome : permit.Admission)
    (h : admission.Authority.admit self witness request facts = Result.ok (outcome, s'))
    (refused : ∀ execution, outcome ≠ .Committed execution) :
    s'.witnesses = self.witnesses ∧ s'.attempts = self.attempts ∧ s'.quotas = self.quotas ∧
      CountersKept self.counters s'.counters := by
  unfold admission.Authority.admit at h
  obtain ⟨b, -, h⟩ := bind_eq_ok h
  split at h
  · obtain ⟨i, -, h⟩ := bind_eq_ok h
    obtain ⟨v, -, h⟩ := bind_eq_ok h
    obtain ⟨o, -, h⟩ := bind_eq_ok h
    obtain ⟨r, -, h⟩ := bind_eq_ok h
    split at h
    · dsimp only at h
      split at h
      · obtain ⟨p, -, h⟩ := bind_eq_ok h
        obtain ⟨rejection, -, h⟩ := bind_eq_ok h
        simp at h
        obtain ⟨-, rfl⟩ := h
        simp [CountersKept]
      · obtain ⟨o1, -, h⟩ := bind_eq_ok h
        split at h
        · obtain ⟨p, -, h⟩ := bind_eq_ok h
          obtain ⟨rejection, -, h⟩ := bind_eq_ok h
          simp at h
          obtain ⟨-, rfl⟩ := h
          simp [CountersKept]
        · obtain ⟨⟨r1, self1⟩, hcommit, h⟩ := bind_eq_ok h
          simp only [uncurry_apply_pair] at h
          split at h
          · simp at h
            exact absurd h.1.symm (refused _)
          · obtain ⟨p, -, h⟩ := bind_eq_ok h
            obtain ⟨rejection, -, h⟩ := bind_eq_ok h
            simp at h
            obtain ⟨-, rfl⟩ := h
            rw [commit_refusal_returns_input _ _ _ _ _ _ hcommit]
            simp [CountersKept]
    · obtain ⟨p, -, h⟩ := bind_eq_ok h
      obtain ⟨rejection, -, h⟩ := bind_eq_ok h
      simp at h
      obtain ⟨-, rfl⟩ := h
      simp [CountersKept]
  · obtain ⟨p, -, h⟩ := bind_eq_ok h
    obtain ⟨rejection, -, h⟩ := bind_eq_ok h
    simp at h
    obtain ⟨-, rfl⟩ := h
    simp [CountersKept]

/-! ### Commit: one charge, one consumed witness, one registered attempt -/

theorem push_ok {α : Type} {v w : alloc.vec.Vec α} {x : α}
    (h : alloc.vec.Vec.push v x = Result.ok w) : w.val = v.val ++ [x] := by
  unfold alloc.vec.Vec.push at h
  dsimp only at h
  split at h
  · simp only [Result.ok.injEq] at h
    subst h
    simp [alloc.vec.Vec.from, alloc.vec.Vec.val]
  · simp at h

theorem index_mut_ok {α : Type} {v : alloc.vec.Vec α} {i : Usize} {x : α} {back : α → alloc.vec.Vec α}
    (h : alloc.vec.Vec.index_mut (core.slice.index.SliceIndexUsizeSlice α) v i = Result.ok (x, back)) :
    v.val[i.val]? = some x ∧ ∀ y, (back y).val = v.val.set i.val y := by
  simp only [alloc.vec.Vec.index_mut_slice_index, alloc.vec.Vec.index_mut_usize,
    alloc.vec.Vec.index_usize] at h
  split at h
  · simp at h
  · rename_i found hfound
    simp only [bind_tc_ok, Result.ok.injEq, Prod.mk.injEq] at h
    obtain ⟨rfl, rfl⟩ := h
    exact ⟨by simpa using hfound, fun y => by simp [alloc.vec.Vec.set_val_eq]⟩

theorem charge_quota_keeps_the_rest (s s' : Authority) (plan : Plan) (available : U64)
    (h : admission.Authority.charge_quota s plan available = Result.ok (.Ok (), s')) :
    s'.profile = s.profile ∧ s'.witnesses = s.witnesses ∧ s'.attempts = s.attempts ∧
      s'.counters = s.counters ∧ s'.invocation = s.invocation := by
  unfold admission.Authority.charge_quota at h
  obtain ⟨o, -, h⟩ := bind_eq_ok h
  split at h
  · obtain ⟨v, -, h⟩ := bind_eq_ok h
    simp at h
    subst h
    simp
  · obtain ⟨qr, -, h⟩ := bind_eq_ok h
    obtain ⟨o1, -, h⟩ := bind_eq_ok h
    split at h
    · simp at h
    · obtain ⟨⟨qr1, back⟩, -, h⟩ := bind_eq_ok h
      simp at h
      subst h
      simp

/-- The quota charge adds exactly the plan's units to the located checkpoint-local record, or opens
    a record for this checkpoint holding the supplied snapshot and exactly those units. -/
theorem charge_quota_charges_the_plan_units (s s' : Authority) (plan : Plan) (available : U64)
    (h : admission.Authority.charge_quota s plan available = Result.ok (.Ok (), s')) :
    ∀ o, facts.Authority.quota_position s plan = Result.ok o →
      match o with
      | none => s'.quotas.val = s.quotas.val ++
          [({ quota := plan.description.constraints.quota,
              checkpoint := s.profile.checkpoint, available,
              spent := plan.description.constraints.units } : QuotaRecord)]
      | some j => ∃ record spent, s.quotas.val[j.val]? = some record ∧
          spent.val = record.spent.val + plan.description.constraints.units.val ∧
          s'.quotas.val = s.quotas.val.set j.val { record with spent } := by
  intro o ho
  unfold admission.Authority.charge_quota at h
  obtain ⟨o', ho', h⟩ := bind_eq_ok h
  rw [ho] at ho'
  simp only [Result.ok.injEq] at ho'
  subst ho'
  split at h
  · obtain ⟨v, hv, h⟩ := bind_eq_ok h
    simp at h
    subst h
    simpa using push_ok hv
  · rename_i j
    obtain ⟨qr, hqr, h⟩ := bind_eq_ok h
    obtain ⟨o1, ho1, h⟩ := bind_eq_ok h
    simp only [lift, Result.ok.injEq] at ho1
    split at h
    · simp at h
    · rename_i spent
      obtain ⟨⟨qr1, back⟩, hmut, h⟩ := bind_eq_ok h
      simp at h
      subst h
      obtain ⟨hget, hback⟩ := index_mut_ok hmut
      simp only [alloc.vec.Vec.index_slice_index, alloc.vec.Vec.index_usize] at hqr
      split at hqr
      · simp at hqr
      · rename_i found hfound
        simp only [Result.ok.injEq] at hqr
        subst hqr
        have hsame : qr1 = found := by
          simp at hfound
          rw [hfound] at hget
          simpa using hget.symm
        subst hsame
        have := core.num.checked_add_UScalar_bv_spec qr1.spent plan.description.constraints.units
        simp only [U64.checked_add] at ho1
        rw [ho1] at this
        refine ⟨qr1, spent, hget, this.2.1, ?_⟩
        simp [hback]

theorem commit_consumes_the_claimed_witness
    (s s' : Authority) (request : permit.AdmissionRequest) (available : U64) (admitted : U32)
    (execution : permit.Execution)
    (h : admission.Authority.commit s request available admitted = Result.ok (.Ok execution, s')) :
    ∃ retained, s.witnesses.val[request.claim.slot.val]? = some retained ∧
      s'.witnesses.val = s.witnesses.val.set request.claim.slot.val
        { retained with state := permit.WitnessState.Consumed } := by
  unfold admission.Authority.commit at h
  obtain ⟨i1, -, h⟩ := bind_eq_ok h
  obtain ⟨p, -, h⟩ := bind_eq_ok h
  obtain ⟨⟨r, s1⟩, hcharge, h⟩ := bind_eq_ok h
  simp only [uncurry_apply_pair] at h
  split at h
  · obtain ⟨⟨wr, back⟩, hmut, h⟩ := bind_eq_ok h
    obtain ⟨v, -, h⟩ := bind_eq_ok h
    simp at h
    obtain ⟨-, rfl⟩ := h
    obtain ⟨-, hw, -, -, -⟩ := charge_quota_keeps_the_rest s s1 request.plan available hcharge
    obtain ⟨hget, hback⟩ := index_mut_ok hmut
    rw [hw] at hget
    refine ⟨wr, hget, ?_⟩
    simp [hback, hw]
  · simp at h

theorem commit_registers_one_unstarted_attempt
    (s s' : Authority) (request : permit.AdmissionRequest) (available : U64) (admitted : U32)
    (execution : permit.Execution)
    (h : admission.Authority.commit s request available admitted = Result.ok (.Ok execution, s')) :
    s'.attempts.val = s.attempts.val ++
        [({ key := execution.attempt, plan := request.plan,
            started := false, admitted := s.profile.checkpoint,
            unknown := none, terminal := none } : AttemptRecord)] ∧
      execution.attempt.owner = s.profile.owner ∧ execution.attempt.slot.val = s.attempts.val.length := by
  unfold admission.Authority.commit at h
  obtain ⟨i1, -, h⟩ := bind_eq_ok h
  obtain ⟨p, hp, h⟩ := bind_eq_ok h
  simp only [plan_clone, Result.ok.injEq] at hp
  subst hp
  obtain ⟨⟨r, s1⟩, hcharge, h⟩ := bind_eq_ok h
  simp only [uncurry_apply_pair] at h
  split at h
  · obtain ⟨⟨wr, back⟩, -, h⟩ := bind_eq_ok h
    obtain ⟨v, hv, h⟩ := bind_eq_ok h
    simp at h
    obtain ⟨rfl, rfl⟩ := h
    obtain ⟨-, -, ha, -, -⟩ := charge_quota_keeps_the_rest s s1 request.plan available hcharge
    refine ⟨?_, rfl, by simp⟩
    rw [push_ok hv, ha]
  · simp at h

theorem commit_charges_the_plan_quota
    (s s' : Authority) (request : permit.AdmissionRequest) (available : U64) (admitted : U32)
    (execution : permit.Execution)
    (h : admission.Authority.commit s request available admitted = Result.ok (.Ok execution, s')) :
    ∀ o, facts.Authority.quota_position s request.plan = Result.ok o →
      match o with
      | none => s'.quotas.val = s.quotas.val ++
          [({ quota := request.plan.description.constraints.quota,
              checkpoint := s.profile.checkpoint, available,
              spent := request.plan.description.constraints.units } : QuotaRecord)]
      | some j => ∃ record spent, s.quotas.val[j.val]? = some record ∧
          spent.val = record.spent.val + request.plan.description.constraints.units.val ∧
          s'.quotas.val = s.quotas.val.set j.val { record with spent } := by
  unfold admission.Authority.commit at h
  obtain ⟨i1, -, h⟩ := bind_eq_ok h
  obtain ⟨p, -, h⟩ := bind_eq_ok h
  obtain ⟨⟨r, s1⟩, hcharge, h⟩ := bind_eq_ok h
  simp only [uncurry_apply_pair] at h
  split at h
  · obtain ⟨⟨wr, back⟩, -, h⟩ := bind_eq_ok h
    obtain ⟨v, -, h⟩ := bind_eq_ok h
    simp at h
    obtain ⟨-, rfl⟩ := h
    exact charge_quota_charges_the_plan_units s s1 request.plan available hcharge
  · simp at h

/-- A successful commit sets both admission counters to the admitted count and leaves the other
    counters unchanged. -/
theorem commit_sets_admission_counters
    (s s' : Authority) (request : permit.AdmissionRequest) (available : U64) (admitted : U32)
    (execution : permit.Execution)
    (h : admission.Authority.commit s request available admitted = Result.ok (.Ok execution, s')) :
    s'.counters = { s.counters with witness_consumptions := admitted, attempts_admitted := admitted } := by
  unfold admission.Authority.commit at h
  obtain ⟨i1, -, h⟩ := bind_eq_ok h
  obtain ⟨p, -, h⟩ := bind_eq_ok h
  obtain ⟨⟨r, s1⟩, hcharge, h⟩ := bind_eq_ok h
  simp only [uncurry_apply_pair] at h
  split at h
  · obtain ⟨⟨wr, back⟩, -, h⟩ := bind_eq_ok h
    obtain ⟨v, -, h⟩ := bind_eq_ok h
    simp at h
    obtain ⟨-, rfl⟩ := h
    obtain ⟨-, -, -, hc, -⟩ := charge_quota_keeps_the_rest s s1 request.plan available hcharge
    simp only [hc]
  · simp at h

/-! ### Committed admission: the structural path through the extracted `admit` -/

/-- A committed admission validated the entry and committed on the same request-recorded state,
    whose profile, witnesses, attempts, quotas, invocation and creation/protected counters are the
    caller's; the admitted count is one more than the caller's. -/
theorem committed_admission_path
    (self s' : Authority) (witness : Option permit.Witness) (request : permit.AdmissionRequest)
    (current : CurrentFacts) (execution : permit.Execution)
    (h : admission.Authority.admit self witness request current = Result.ok (.Committed execution, s')) :
    ∃ (staged : Authority) (available : U64) (admitted : U32),
      staged.profile = self.profile ∧ staged.witnesses = self.witnesses ∧
      staged.attempts = self.attempts ∧ staged.quotas = self.quotas ∧
      staged.invocation = self.invocation ∧
      admission.Authority.validate_entry staged witness request current = Result.ok (.Ok available) ∧
      admission.Authority.commit staged request available admitted = Result.ok (.Ok execution, s') ∧
      staged.counters.witnesses_created = self.counters.witnesses_created ∧
      staged.counters.protected_operations = self.counters.protected_operations ∧
      admitted.val = self.counters.attempts_admitted.val + 1 := by
  unfold admission.Authority.admit at h
  obtain ⟨b, -, h⟩ := bind_eq_ok h
  split at h
  · obtain ⟨i, -, h⟩ := bind_eq_ok h
    obtain ⟨v, -, h⟩ := bind_eq_ok h
    obtain ⟨o, ho, h⟩ := bind_eq_ok h
    simp only [core.option.Option.as_ref, Result.ok.injEq] at ho
    subst ho
    obtain ⟨r, hr, h⟩ := bind_eq_ok h
    split at h
    · rename_i available
      dsimp only at h
      split at h
      · obtain ⟨p, -, h⟩ := bind_eq_ok h
        obtain ⟨rejection, -, h⟩ := bind_eq_ok h
        simp at h
      · obtain ⟨o1, ho1, h⟩ := bind_eq_ok h
        simp only [lift, Result.ok.injEq] at ho1
        split at h
        · obtain ⟨p, -, h⟩ := bind_eq_ok h
          obtain ⟨rejection, -, h⟩ := bind_eq_ok h
          simp at h
        · rename_i admitted
          have hadmitted := core.num.checked_add_UScalar_bv_spec self.counters.attempts_admitted 1#u32
          simp only [U32.checked_add] at ho1
          rw [ho1] at hadmitted
          obtain ⟨⟨r1, self1⟩, hcommit, h⟩ := bind_eq_ok h
          simp only [uncurry_apply_pair] at h
          split at h
          · simp at h
            obtain ⟨rfl, rfl⟩ := h
            refine ⟨_, available, admitted, ?_, ?_, ?_, ?_, ?_, hr, hcommit, ?_, ?_, ?_⟩
            all_goals first | rfl | simpa using hadmitted.2.1
          · obtain ⟨p, -, h⟩ := bind_eq_ok h
            obtain ⟨rejection, -, h⟩ := bind_eq_ok h
            simp at h
    · obtain ⟨p, -, h⟩ := bind_eq_ok h
      obtain ⟨rejection, -, h⟩ := bind_eq_ok h
      simp at h
  · obtain ⟨p, -, h⟩ := bind_eq_ok h
    obtain ⟨rejection, -, h⟩ := bind_eq_ok h
    simp at h

/-! ### Committed admission: the inventoried guarantees -/

/-- A committed admission required a presented witness carrying exactly the requested claim,
    owned by this authority. -/
theorem committed_admission_presents_the_claim
    (self s' : Authority) (witness : Option permit.Witness) (request : permit.AdmissionRequest)
    (current : CurrentFacts) (execution : permit.Execution)
    (h : admission.Authority.admit self witness request current = Result.ok (.Committed execution, s')) :
    ∃ presented, witness = some presented ∧ presented.claim = request.claim ∧
      request.claim.owner = self.profile.owner := by
  obtain ⟨staged, available, admitted, hprofile, -, -, -, -, hentry, -⟩ :=
    committed_admission_path self s' witness request current execution h
  obtain ⟨presented, hw, hclaim, howner⟩ :=
    validate_entry_requires_the_presented_claim _ _ _ _ _ hentry
  exact ⟨presented, hw, hclaim, hprofile ▸ howner⟩

/-- Admitted execution stays bound to the independently authorized plan: the retained witness
    and the current grant both carry exactly the requested plan. -/
theorem committed_admission_binds_the_exact_plan
    (self s' : Authority) (witness : Option permit.Witness) (request : permit.AdmissionRequest)
    (current : CurrentFacts) (execution : permit.Execution)
    (h : admission.Authority.admit self witness request current = Result.ok (.Committed execution, s')) :
    (∃ retained, self.witnesses.val[request.claim.slot.val]? = some retained ∧
        retained.plan = request.plan) ∧
      ∃ grant, current.grant = some grant ∧ grant.plan = request.plan := by
  obtain ⟨staged, available, admitted, -, hwitnesses, -, -, -, hentry, -⟩ :=
    committed_admission_path self s' witness request current execution h
  obtain ⟨retained, hget, hplan⟩ := validate_entry_requires_the_exact_plan _ _ _ _ _ hentry
  obtain ⟨hfacts, -⟩ := validate_entry_requires_valid_facts _ _ _ _ _ hentry
  obtain ⟨-, -, -, ⟨grant, hgrant, hgrantplan, -⟩, -⟩ :=
    validate_facts_requires_current_authority _ _ _ hfacts
  exact ⟨⟨retained, hwitnesses ▸ hget, hplan⟩, grant, hgrant, hgrantplan⟩

/-- Stale facts cannot mint an execution permit: the facts' checkpoint is the authority's. -/
theorem committed_admission_requires_the_current_checkpoint
    (self s' : Authority) (witness : Option permit.Witness) (request : permit.AdmissionRequest)
    (current : CurrentFacts) (execution : permit.Execution)
    (h : admission.Authority.admit self witness request current = Result.ok (.Committed execution, s')) :
    current.checkpoint = self.profile.checkpoint := by
  obtain ⟨staged, available, admitted, hprofile, -, -, -, -, hentry, -⟩ :=
    committed_admission_path self s' witness request current execution h
  obtain ⟨hfacts, -⟩ := validate_entry_requires_valid_facts _ _ _ _ _ hentry
  exact hprofile ▸ validate_facts_requires_the_current_checkpoint _ _ _ hfacts

/-- Invalid claims and stale or revoked facts cannot mint an execution permit. -/
theorem committed_admission_requires_current_authority
    (self s' : Authority) (witness : Option permit.Witness) (request : permit.AdmissionRequest)
    (current : CurrentFacts) (execution : permit.Execution)
    (h : admission.Authority.admit self witness request current = Result.ok (.Committed execution, s')) :
    self.invocation = report.InvocationOutcome.Pending ∧
      request.plan.description.owner = self.profile.owner ∧
      current.source = self.profile.authority_source ∧
      current.credentials_valid = some true ∧ current.revoked = some false ∧
      current.policy = some request.plan.description.policy ∧
      request.plan.description.constraints.not_before.val ≤ current.checkpoint.now.val ∧
      current.checkpoint.now.val < request.plan.description.constraints.expires_at.val := by
  obtain ⟨staged, available, admitted, hprofile, -, -, -, hinvocation, hentry, -⟩ :=
    committed_admission_path self s' witness request current execution h
  obtain ⟨hfacts, -⟩ := validate_entry_requires_valid_facts _ _ _ _ _ hentry
  obtain ⟨hpending, howner, hsource, -, hcred, hrev, hpol, hbefore, hexp, -⟩ :=
    validate_facts_requires_current_authority _ _ _ hfacts
  exact ⟨hinvocation ▸ hpending, hprofile ▸ howner, hprofile ▸ hsource, hcred, hrev, hpol, hbefore, hexp⟩

/-- Each consumed witness corresponds to one admitted attempt: a committed admission consumes
    exactly the claimed, previously live witness and changes no other witness slot. -/
theorem committed_admission_consumes_one_live_witness
    (self s' : Authority) (witness : Option permit.Witness) (request : permit.AdmissionRequest)
    (current : CurrentFacts) (execution : permit.Execution)
    (h : admission.Authority.admit self witness request current = Result.ok (.Committed execution, s')) :
    ∃ retained, self.witnesses.val[request.claim.slot.val]? = some retained ∧
      retained.state = permit.WitnessState.Live ∧
      s'.witnesses.val = self.witnesses.val.set request.claim.slot.val
        { retained with state := permit.WitnessState.Consumed } := by
  obtain ⟨staged, available, admitted, -, hwitnesses, -, -, -, hentry, hcommit, -⟩ :=
    committed_admission_path self s' witness request current execution h
  obtain ⟨live, hlive, hstate⟩ := validate_entry_requires_a_live_witness _ _ _ _ _ hentry
  obtain ⟨retained, hget, hset⟩ := commit_consumes_the_claimed_witness _ _ _ _ _ _ hcommit
  rw [hlive] at hget
  simp only [Option.some.injEq] at hget
  subst hget
  exact ⟨live, hwitnesses ▸ hlive, hstate, hwitnesses ▸ hset⟩

/-- A committed admission registers exactly one unstarted attempt bound to the requested plan. -/
theorem committed_admission_registers_one_unstarted_attempt
    (self s' : Authority) (witness : Option permit.Witness) (request : permit.AdmissionRequest)
    (current : CurrentFacts) (execution : permit.Execution)
    (h : admission.Authority.admit self witness request current = Result.ok (.Committed execution, s')) :
    s'.attempts.val = self.attempts.val ++
        [({ key := execution.attempt, plan := request.plan,
            started := false, admitted := self.profile.checkpoint,
            unknown := none, terminal := none } : AttemptRecord)] ∧
      execution.attempt.owner = self.profile.owner ∧
      execution.attempt.slot.val = self.attempts.val.length := by
  obtain ⟨staged, available, admitted, hprofile, -, hattempts, -, -, -, hcommit, -⟩ :=
    committed_admission_path self s' witness request current execution h
  obtain ⟨hlist, howner, hslot⟩ := commit_registers_one_unstarted_attempt _ _ _ _ _ _ hcommit
  rw [hattempts, hprofile] at hlist
  exact ⟨hlist, hprofile ▸ howner, hattempts ▸ hslot⟩

/-- Checkpoint-local charges never exceed the independently supplied available quota: a committed
    admission adds exactly the plan's units to this checkpoint's record for the plan's quota, which
    must carry the supplied snapshot, or opens such a record; either way the charge fits. -/
theorem committed_admission_charges_within_the_supplied_quota
    (self s' : Authority) (witness : Option permit.Witness) (request : permit.AdmissionRequest)
    (current : CurrentFacts) (execution : permit.Execution)
    (h : admission.Authority.admit self witness request current = Result.ok (.Committed execution, s')) :
    ∃ available, current.quota_remaining = some available ∧
      ((∃ (j : Usize) (record : QuotaRecord) (spent : U64), self.quotas.val[j.val]? = some record ∧
          record.quota = request.plan.description.constraints.quota ∧
          record.checkpoint = self.profile.checkpoint ∧ record.available = available ∧
          spent.val = record.spent.val + request.plan.description.constraints.units.val ∧
          spent.val ≤ available.val ∧
          s'.quotas.val = self.quotas.val.set j.val { record with spent }) ∨
        (request.plan.description.constraints.units.val ≤ available.val ∧
          s'.quotas.val = self.quotas.val ++
            [({ quota := request.plan.description.constraints.quota,
                checkpoint := self.profile.checkpoint, available,
                spent := request.plan.description.constraints.units } : QuotaRecord)])) := by
  obtain ⟨staged, available, admitted, hprofile, -, -, hquotas, -, hentry, hcommit, -⟩ :=
    committed_admission_path self s' witness request current execution h
  obtain ⟨hfacts, havailable⟩ := validate_entry_requires_valid_facts _ _ _ _ _ hentry
  obtain ⟨-, -, -, -, -, -, -, -, -, hquota⟩ := validate_facts_requires_current_authority _ _ _ hfacts
  obtain ⟨available', havailable', hbound⟩ := validate_quota_bounds_the_charge _ _ _ hquota
  rw [havailable] at havailable'
  simp only [Option.some.injEq] at havailable'
  subst havailable'
  refine ⟨available, havailable, ?_⟩
  obtain ⟨o, ho, hmatch⟩ := quota_position_matches staged request.plan
  have hb := hbound o ho
  have hc := commit_charges_the_plan_quota _ _ _ _ _ _ hcommit o ho
  cases o with
  | none =>
    right
    exact ⟨hb, by rw [hc, hquotas, hprofile]⟩
  | some j =>
    left
    obtain ⟨record, spent, hget, hspent, hset⟩ := hc
    obtain ⟨record', hget', havail, hfits⟩ := hb
    obtain ⟨record'', hget'', hq, hcp⟩ := hmatch
    rw [hget] at hget' hget''
    simp only [Option.some.injEq] at hget' hget''
    subst hget' hget''
    exact ⟨j, record, spent, hquotas ▸ hget, hq, hprofile ▸ hcp, havail, hspent, by omega,
      hquotas ▸ hset⟩

/-! ### Quota invariant -/

/-- Every checkpoint-local quota record is within its own supplied snapshot. -/
def QuotaWithin (s : Authority) : Prop :=
  ∀ record ∈ s.quotas.val, record.spent.val ≤ record.available.val

/-- Admission preserves the quota invariant, whether it commits, denies or refuses in preflight. -/
theorem admission_preserves_quota_within
    (self s' : Authority) (witness : Option permit.Witness) (request : permit.AdmissionRequest)
    (current : CurrentFacts) (outcome : permit.Admission)
    (hinv : QuotaWithin self)
    (h : admission.Authority.admit self witness request current = Result.ok (outcome, s')) :
    QuotaWithin s' := by
  by_cases hcommitted : ∃ execution, outcome = .Committed execution
  · obtain ⟨execution, rfl⟩ := hcommitted
    obtain ⟨available, -, hcharge⟩ :=
      committed_admission_charges_within_the_supplied_quota self s' witness request current execution h
    intro record hmem
    rcases hcharge with ⟨j, old, spent, -, -, -, havail, -, hfits, hset⟩ | ⟨hfits, happend⟩
    · rw [hset] at hmem
      rcases List.mem_or_eq_of_mem_set hmem with hold | hnew
      · exact hinv record hold
      · subst hnew
        simpa [havail] using hfits
    · rw [happend] at hmem
      rcases List.mem_append.1 hmem with hold | hnew
      · exact hinv record hold
      · simp only [List.mem_singleton] at hnew
        subst hnew
        exact hfits
  · have hrefused : ∀ execution, outcome ≠ .Committed execution := fun execution heq =>
      hcommitted ⟨execution, heq⟩
    obtain ⟨-, -, hquotas, -⟩ := refused_admission_preserves_ledger self s' witness request current outcome h hrefused
    intro record hmem
    rw [hquotas] at hmem
    exact hinv record hmem

/-! ### Quota frames: no other public transition writes a quota record -/

/-- Peel non-tuple binds and branches off `h : program = ok (x, s')`, then close every leaf whose
    returned state has the caller's quota records by definition. -/
macro "quota_frame " h:ident : tactic => `(tactic| (
  repeat' (first
    | (obtain ⟨_, _, $h:ident⟩ := bind_eq_ok $h:ident)
    | (split at $h:ident)
    | (dsimp only at $h:ident; split at $h:ident))
  all_goals (simp only [Result.ok.injEq, Prod.mk.injEq] at $h:ident
             obtain ⟨_, hs⟩ := $h:ident
             subst hs
             rfl)))

theorem authorize_keeps_quotas (s s' : Authority) (plan : Plan) (current : CurrentFacts)
    (outcome : permit.Authorization)
    (h : facts.Authority.authorize_from_trusted_host s plan current = Result.ok (outcome, s')) :
    s'.quotas = s.quotas := by
  unfold facts.Authority.authorize_from_trusted_host at h
  quota_frame h

theorem advance_keeps_quotas (s s' : Authority) (checkpoint : Checkpoint) (outcome : core.result.Result Unit SetupError)
    (h : facts.Authority.advance_from_trusted_host s checkpoint = Result.ok (outcome, s')) :
    s'.quotas = s.quotas := by
  unfold facts.Authority.advance_from_trusted_host at h
  quota_frame h

theorem start_keeps_quotas (s s' : Authority) (execution : permit.Execution)
    (outcome : core.result.Result permit.Started report.BoundaryError)
    (h : admission.Authority.start_execution s execution = Result.ok (outcome, s')) :
    s'.quotas = s.quotas := by
  unfold admission.Authority.start_execution at h
  obtain ⟨r, -, h⟩ := bind_eq_ok h
  split at h
  · split at h
    · quota_frame h
    · obtain ⟨b, -, h⟩ := bind_eq_ok h
      split at h
      · quota_frame h
      · obtain ⟨p, -, h⟩ := bind_eq_ok h
        obtain ⟨⟨ar, back⟩, -, h⟩ := bind_eq_ok h
        simp only [uncurry_apply_pair] at h
        quota_frame h
  · quota_frame h

theorem observe_keeps_quotas (s s' : Authority) (description : report.ObservationDescription)
    (outcome : core.result.Result report.Observation report.BoundaryError)
    (h : observation.Authority.observe_from_trusted_host s description = Result.ok (outcome, s')) :
    s'.quotas = s.quotas := by
  unfold observation.Authority.observe_from_trusted_host at h
  obtain ⟨r, -, h⟩ := bind_eq_ok h
  split at h
  · obtain ⟨⟨record, back⟩, -, h⟩ := bind_eq_ok h
    simp only [uncurry_apply_pair] at h
    quota_frame h
  · quota_frame h

theorem retire_witness_keeps_quotas (s s' : Authority) (witness : permit.Witness)
    (outcome : core.result.Result Unit permit.Denial)
    (h : observation.Authority.retire_witness s witness = Result.ok (outcome, s')) :
    s'.quotas = s.quotas := by
  unfold observation.Authority.retire_witness at h
  obtain ⟨r, -, h⟩ := bind_eq_ok h
  split at h
  · split at h
    · obtain ⟨⟨wr, back⟩, -, h⟩ := bind_eq_ok h
      simp only [uncurry_apply_pair] at h
      quota_frame h
    · quota_frame h
    · quota_frame h
  · quota_frame h

/-- Retiring every live witness walks the witness table and writes only witness states. -/
theorem retire_live_witnesses_keeps_quotas (s s' : Authority)
    (h : observation.Authority.retire_live_witnesses s = Result.ok s') : s'.quotas = s.quotas := by
  have hspec : observation.Authority.retire_live_witnesses_loop s 0#usize ⦃ r => r.2.2.2.1 = s.quotas ⦄ := by
    unfold observation.Authority.retire_live_witnesses_loop
    apply loop.spec_decr_nat (fun st : Authority × Usize => st.1.witnesses.val.length - st.2.val)
      (fun st => st.1.quotas = s.quotas)
    · rintro ⟨st, index⟩ hq
      dsimp only
      unfold observation.Authority.retire_live_witnesses_loop.body
      by_cases hlt : index.val < st.witnesses.val.length
      · have hbound := alloc.vec.Vec.len_ineq st.witnesses
        obtain ⟨index1, hadd, hval⟩ := WP.spec_imp_exists
          (UScalar.add_spec (x := index) (y := 1#usize) (by scalar_tac))
        have hlt' : index < alloc.vec.Vec.len st.witnesses := by scalar_tac
        have hget : alloc.vec.Vec.index (core.slice.index.SliceIndexUsizeSlice WitnessRecord) st.witnesses index =
            Result.ok (st.witnesses.val[index.val]) := by
          simp [alloc.vec.Vec.index_slice_index, alloc.vec.Vec.index_usize, hlt]
          rfl
        have hmut : alloc.vec.Vec.index_mut (core.slice.index.SliceIndexUsizeSlice WitnessRecord) st.witnesses index =
            Result.ok (st.witnesses.val[index.val], alloc.vec.Vec.set st.witnesses index) := by
          simp [alloc.vec.Vec.index_mut_slice_index, alloc.vec.Vec.index_mut_usize,
            alloc.vec.Vec.index_usize, hlt]
          rfl
        simp only [hlt', if_true, hget, bind_tc_ok]
        split
        · simp only [hmut, hadd, bind_tc_ok, uncurry_apply_pair, WP.spec_ok]
          refine ⟨hq, ?_⟩
          simp only [alloc.vec.Vec.set_val_eq, List.length_set]
          scalar_tac
        · simp only [hadd, bind_tc_ok, WP.spec_ok]
          exact ⟨hq, by scalar_tac⟩
        · simp only [hadd, bind_tc_ok, WP.spec_ok]
          exact ⟨hq, by scalar_tac⟩
      · have hge : ¬ index < alloc.vec.Vec.len st.witnesses := by scalar_tac
        simp only [hge, if_false, WP.spec_ok]
        exact hq
    · rfl
  obtain ⟨r, hr, hpost⟩ := WP.spec_imp_exists hspec
  obtain ⟨p, v, v1, v2, v3, c, io, o⟩ := r
  unfold observation.Authority.retire_live_witnesses at h
  rw [hr] at h
  simp only [bind_tc_ok, uncurry_apply_pair, Result.ok.injEq] at h
  subst h
  exact hpost

theorem cancel_keeps_quotas (s s' : Authority)
    (outcome : core.result.Result report.InvocationOutcome report.BoundaryError)
    (h : observation.Authority.cancel_invocation s = Result.ok (outcome, s')) :
    s'.quotas = s.quotas := by
  unfold observation.Authority.cancel_invocation at h
  split at h
  · obtain ⟨self1, hretire, h⟩ := bind_eq_ok h
    simp only [Result.ok.injEq, Prod.mk.injEq] at h
    obtain ⟨-, rfl⟩ := h
    exact retire_live_witnesses_keeps_quotas
      { s with invocation := report.InvocationOutcome.Cancelled } self1 hretire
  all_goals (simp only [Result.ok.injEq, Prod.mk.injEq] at h; obtain ⟨-, rfl⟩ := h; rfl)

theorem fail_keeps_quotas (s s' : Authority)
    (outcome : core.result.Result report.InvocationOutcome report.BoundaryError)
    (h : observation.Authority.fail_invocation s = Result.ok (outcome, s')) :
    s'.quotas = s.quotas := by
  unfold observation.Authority.fail_invocation at h
  split at h
  · obtain ⟨self1, hretire, h⟩ := bind_eq_ok h
    simp only [Result.ok.injEq, Prod.mk.injEq] at h
    obtain ⟨-, rfl⟩ := h
    exact retire_live_witnesses_keeps_quotas
      { s with invocation := report.InvocationOutcome.Failed } self1 hretire
  all_goals (simp only [Result.ok.injEq, Prod.mk.injEq] at h; obtain ⟨-, rfl⟩ := h; rfl)

theorem deliver_keeps_quotas (s s' : Authority) (attempt : permit.AttemptKey)
    (observed : report.Observation) (outcome : core.result.Result report.Receipt report.BoundaryError)
    (h : observation.Authority.deliver_success s attempt observed = Result.ok (outcome, s')) :
    s'.quotas = s.quotas := by
  unfold observation.Authority.deliver_success at h
  obtain ⟨r, -, h⟩ := bind_eq_ok h
  split at h
  · obtain ⟨b, -, h⟩ := bind_eq_ok h
    split at h
    · simp only [Result.ok.injEq, Prod.mk.injEq] at h
      obtain ⟨-, rfl⟩ := h
      rfl
    · obtain ⟨b1, -, h⟩ := bind_eq_ok h
      split at h
      · simp only [Result.ok.injEq, Prod.mk.injEq] at h
        obtain ⟨-, rfl⟩ := h
        rfl
      · obtain ⟨i, -, h⟩ := bind_eq_ok h
        obtain ⟨self1, hretire, h⟩ := bind_eq_ok h
        obtain ⟨r1, -, h⟩ := bind_eq_ok h
        simp only [Result.ok.injEq, Prod.mk.injEq] at h
        obtain ⟨-, rfl⟩ := h
        exact (retire_live_witnesses_keeps_quotas _ self1 hretire).trans rfl
  · simp only [Result.ok.injEq, Prod.mk.injEq] at h
    obtain ⟨-, rfl⟩ := h
    rfl

theorem new_opens_no_quota (profile : Profile) (s : Authority)
    (h : facts.Authority.new profile = Result.ok (.Ok s)) : s.quotas.val = [] := by
  unfold facts.Authority.new at h
  split_ifs at h <;> try simp at h
  obtain ⟨r, -, h⟩ := bind_eq_ok h
  split at h
  · simp only [Result.ok.injEq, core.result.Result.Ok.injEq] at h
    subst h
    simp [alloc.vec.Vec.with_capacity]
  · simp at h

/-! ### Reachable authority states -/

/-- One serialized call of a public, state-returning authority transition. Receipt and snapshot
    functions return no authority state and are not steps. -/
inductive Step : Authority → Authority → Prop
  | authorize (s s' : Authority) (plan : Plan) (current : CurrentFacts) (outcome : permit.Authorization) :
      facts.Authority.authorize_from_trusted_host s plan current = Result.ok (outcome, s') → Step s s'
  | advance (s s' : Authority) (checkpoint : Checkpoint) (outcome : core.result.Result Unit SetupError) :
      facts.Authority.advance_from_trusted_host s checkpoint = Result.ok (outcome, s') → Step s s'
  | admission (s s' : Authority) (witness : Option permit.Witness) (request : permit.AdmissionRequest)
      (current : CurrentFacts) (outcome : permit.Admission) :
      admission.Authority.admit s witness request current = Result.ok (outcome, s') → Step s s'
  | start (s s' : Authority) (execution : permit.Execution)
      (outcome : core.result.Result permit.Started report.BoundaryError) :
      admission.Authority.start_execution s execution = Result.ok (outcome, s') → Step s s'
  | observe (s s' : Authority) (description : report.ObservationDescription)
      (outcome : core.result.Result report.Observation report.BoundaryError) :
      observation.Authority.observe_from_trusted_host s description = Result.ok (outcome, s') → Step s s'
  | retire (s s' : Authority) (witness : permit.Witness) (outcome : core.result.Result Unit permit.Denial) :
      observation.Authority.retire_witness s witness = Result.ok (outcome, s') → Step s s'
  | cancel (s s' : Authority) (outcome : core.result.Result report.InvocationOutcome report.BoundaryError) :
      observation.Authority.cancel_invocation s = Result.ok (outcome, s') → Step s s'
  | fail (s s' : Authority) (outcome : core.result.Result report.InvocationOutcome report.BoundaryError) :
      observation.Authority.fail_invocation s = Result.ok (outcome, s') → Step s s'
  | deliver (s s' : Authority) (attempt : permit.AttemptKey) (observed : report.Observation)
      (outcome : core.result.Result report.Receipt report.BoundaryError) :
      observation.Authority.deliver_success s attempt observed = Result.ok (outcome, s') → Step s s'

/-- States reached from `Authority.new` by any finite serialized prefix of public transitions. -/
inductive Reachable : Authority → Prop
  | init (profile : Profile) (s : Authority) : facts.Authority.new profile = Result.ok (.Ok s) → Reachable s
  | step (s s' : Authority) : Reachable s → Step s s' → Reachable s'

/-- Initialization: a new authority starts with no quota record, so the invariant holds. -/
theorem new_starts_within_quota (profile : Profile) (s : Authority)
    (h : facts.Authority.new profile = Result.ok (.Ok s)) : QuotaWithin s := by
  intro record hmem
  rw [new_opens_no_quota profile s h] at hmem
  simp at hmem

/-- Preservation: every public transition keeps each checkpoint-local quota record within its
    supplied snapshot. -/
theorem step_preserves_quota_within (s s' : Authority) (hstep : Step s s') (hinv : QuotaWithin s) :
    QuotaWithin s' := by
  have frame : s'.quotas = s.quotas → QuotaWithin s' := fun hq record hmem => by
    rw [hq] at hmem
    exact hinv record hmem
  cases hstep with
  | authorize plan current outcome h => exact frame (authorize_keeps_quotas _ _ _ _ _ h)
  | advance checkpoint outcome h => exact frame (advance_keeps_quotas _ _ _ _ h)
  | admission witness request current outcome h =>
    exact admission_preserves_quota_within _ _ _ _ _ _ hinv h
  | start execution outcome h => exact frame (start_keeps_quotas _ _ _ _ h)
  | observe description outcome h => exact frame (observe_keeps_quotas _ _ _ _ h)
  | retire witness outcome h => exact frame (retire_witness_keeps_quotas _ _ _ _ h)
  | cancel outcome h => exact frame (cancel_keeps_quotas _ _ _ h)
  | fail outcome h => exact frame (fail_keeps_quotas _ _ _ h)
  | deliver attempt observed outcome h => exact frame (deliver_keeps_quotas _ _ _ _ _ h)

/-- Over every finite serialized prefix of public transitions from `Authority.new`, every
    checkpoint-local quota record stays within the snapshot the host supplied for it. Cross-checkpoint
    conservation is not claimed: a fresh checkpoint opens fresh records from fresh host snapshots. -/
theorem reachable_quota_within (s : Authority) (h : Reachable s) : QuotaWithin s := by
  induction h with
  | init profile s hnew => exact new_starts_within_quota profile s hnew
  | step s s' _ hstep ih => exact step_preserves_quota_within s s' hstep ih

/-! ### Ledger moves: what each public transition does to the witness and attempt tables -/

/-- A witness counts as consumed exactly when its state is `Consumed`. -/
def isConsumed (record : WitnessRecord) : Bool := decide (record.state = permit.WitnessState.Consumed)

/-- What `retire_live_witnesses` leaves in one slot: a live witness is retired, any other is kept. -/
def retireIfLive (record : WitnessRecord) : WitnessRecord :=
  if record.state = permit.WitnessState.Live then { record with state := permit.WitnessState.Retired }
  else record

/-- An observation rewrites an attempt record but keeps its key, plan and start flag. -/
def SameAttempt (record record' : AttemptRecord) : Prop :=
  record'.key = record.key ∧ record'.plan = record.plan ∧ record'.started = record.started

/-- The effect of one public transition on the witness table, the attempt table and the counters
    that count them. Each constructor is realized by the extracted transitions below. -/
inductive LedgerMove (s s' : Authority) : Prop
  | unchanged :
      s'.witnesses = s.witnesses → s'.attempts = s.attempts → CountersKept s.counters s'.counters →
      LedgerMove s s'
  | authorized (record : WitnessRecord) :
      record.state = permit.WitnessState.Live → s'.witnesses.val = s.witnesses.val ++ [record] →
      s'.attempts = s.attempts →
      s'.counters.witnesses_created.val = s.counters.witnesses_created.val + 1 →
      s'.counters.witness_consumptions = s.counters.witness_consumptions →
      s'.counters.attempts_admitted = s.counters.attempts_admitted →
      s'.counters.protected_operations = s.counters.protected_operations → LedgerMove s s'
  | admitted (slot : Nat) (retained : WitnessRecord) (record : AttemptRecord) :
      s.witnesses.val[slot]? = some retained → retained.state = permit.WitnessState.Live →
      s'.witnesses.val =
        s.witnesses.val.set slot { retained with state := permit.WitnessState.Consumed } →
      record.started = false → s'.attempts.val = s.attempts.val ++ [record] →
      s'.counters.witnesses_created = s.counters.witnesses_created →
      s'.counters.attempts_admitted.val = s.counters.attempts_admitted.val + 1 →
      s'.counters.witness_consumptions = s'.counters.attempts_admitted →
      s'.counters.protected_operations = s.counters.protected_operations → LedgerMove s s'
  | started (slot : Nat) (record : AttemptRecord) :
      s.attempts.val[slot]? = some record → record.started = false →
      s'.attempts.val = s.attempts.val.set slot { record with started := true } →
      s'.witnesses = s.witnesses →
      s'.counters.witnesses_created = s.counters.witnesses_created →
      s'.counters.witness_consumptions = s.counters.witness_consumptions →
      s'.counters.attempts_admitted = s.counters.attempts_admitted →
      s'.counters.protected_operations.val = s.counters.protected_operations.val + 1 →
      LedgerMove s s'
  | observed (slot : Nat) (record record' : AttemptRecord) :
      s.attempts.val[slot]? = some record → SameAttempt record record' →
      s'.attempts.val = s.attempts.val.set slot record' → s'.witnesses = s.witnesses →
      CountersKept s.counters s'.counters → LedgerMove s s'
  | retired (slot : Nat) (retained : WitnessRecord) :
      s.witnesses.val[slot]? = some retained → retained.state = permit.WitnessState.Live →
      s'.witnesses.val =
        s.witnesses.val.set slot { retained with state := permit.WitnessState.Retired } →
      s'.attempts = s.attempts → CountersKept s.counters s'.counters → LedgerMove s s'
  | retiredAll :
      s'.witnesses.val = s.witnesses.val.map retireIfLive → s'.attempts = s.attempts →
      CountersKept s.counters s'.counters → LedgerMove s s'

/-- Peel binds and branches off `h : program = ok (x, s')` and close every leaf whose returned
    state keeps both tables and the ledger counters. -/
macro "ledger_frame " h:ident : tactic => `(tactic| (
  repeat' (first
    | (obtain ⟨_, _, $h:ident⟩ := bind_eq_ok $h:ident)
    | (split at $h:ident)
    | (dsimp only at $h:ident; split at $h:ident))
  all_goals (simp only [Result.ok.injEq, Prod.mk.injEq] at $h:ident
             obtain ⟨_, hs⟩ := $h:ident
             subst hs
             exact LedgerMove.unchanged rfl rfl ⟨rfl, rfl, rfl, rfl⟩)))

/-- Peel the observation branches after the attempt slot is borrowed and close every leaf, each
    of which writes back one record with the same key, plan and start flag. -/
macro "observed_leaf " h:ident hget:ident hback:ident : tactic => `(tactic| (
  repeat' (first
    | (obtain ⟨_, _, $h:ident⟩ := bind_eq_ok $h:ident)
    | (split at $h:ident)
    | (dsimp only at $h:ident; split at $h:ident))
  all_goals (simp only [Result.ok.injEq, Prod.mk.injEq] at $h:ident
             obtain ⟨_, hs⟩ := $h:ident
             subst hs
             exact LedgerMove.observed _ _ _ $hget:ident (by exact ⟨rfl, rfl, rfl⟩) ($hback:ident _) rfl
               ⟨rfl, rfl, rfl, rfl⟩)))

theorem advance_moves_ledger (s s' : Authority) (checkpoint : Checkpoint)
    (outcome : core.result.Result Unit SetupError)
    (h : facts.Authority.advance_from_trusted_host s checkpoint = Result.ok (outcome, s')) :
    LedgerMove s s' := by
  unfold facts.Authority.advance_from_trusted_host at h
  ledger_frame h

/-- Authorization appends exactly one live witness and counts it, or changes neither table. -/
theorem authorize_moves_ledger (s s' : Authority) (plan : Plan) (current : CurrentFacts)
    (outcome : permit.Authorization)
    (h : facts.Authority.authorize_from_trusted_host s plan current = Result.ok (outcome, s')) :
    LedgerMove s s' := by
  unfold facts.Authority.authorize_from_trusted_host at h
  obtain ⟨b, -, h⟩ := bind_eq_ok h
  split at h
  · obtain ⟨i, -, h⟩ := bind_eq_ok h
    obtain ⟨r, -, h⟩ := bind_eq_ok h
    split at h
    · dsimp only at h
      split at h
      · ledger_frame h
      · obtain ⟨o, ho, h⟩ := bind_eq_ok h
        simp only [lift, Result.ok.injEq] at ho
        split at h
        · ledger_frame h
        · rename_i created
          obtain ⟨i3, -, h⟩ := bind_eq_ok h
          obtain ⟨v, hv, h⟩ := bind_eq_ok h
          simp only [Result.ok.injEq, Prod.mk.injEq] at h
          obtain ⟨-, rfl⟩ := h
          have hcreated := core.num.checked_add_UScalar_bv_spec s.counters.witnesses_created 1#u32
          simp only [U32.checked_add] at ho
          rw [ho] at hcreated
          exact LedgerMove.authorized _ (by rfl) (push_ok hv) rfl (by simpa using hcreated.2.1)
            rfl rfl rfl
    · ledger_frame h
  · ledger_frame h

/-- A committed admission sets both admission counters to one more than before; the creation and
    protected-operation counters are unchanged. -/
theorem committed_admission_counts
    (self s' : Authority) (witness : Option permit.Witness) (request : permit.AdmissionRequest)
    (current : CurrentFacts) (execution : permit.Execution)
    (h : admission.Authority.admit self witness request current = Result.ok (.Committed execution, s')) :
    s'.counters.witnesses_created = self.counters.witnesses_created ∧
      s'.counters.attempts_admitted.val = self.counters.attempts_admitted.val + 1 ∧
      s'.counters.witness_consumptions = s'.counters.attempts_admitted ∧
      s'.counters.protected_operations = self.counters.protected_operations := by
  obtain ⟨staged, available, admitted, -, -, -, -, -, -, hcommit, hcreated, hprotected, hadmitted⟩ :=
    committed_admission_path self s' witness request current execution h
  rw [commit_sets_admission_counters _ _ _ _ _ _ hcommit]
  exact ⟨hcreated, hadmitted, rfl, hprotected⟩

/-- Admission consumes one live witness, registers one unstarted attempt and counts both, or —
    on denial and preflight — changes neither table nor the ledger counters. -/
theorem admission_moves_ledger (self s' : Authority) (witness : Option permit.Witness)
    (request : permit.AdmissionRequest) (current : CurrentFacts) (outcome : permit.Admission)
    (h : admission.Authority.admit self witness request current = Result.ok (outcome, s')) :
    LedgerMove self s' := by
  by_cases hcommitted : ∃ execution, outcome = .Committed execution
  · obtain ⟨execution, rfl⟩ := hcommitted
    obtain ⟨retained, hget, hlive, hwitnesses⟩ :=
      committed_admission_consumes_one_live_witness self s' witness request current execution h
    obtain ⟨hattempts, -, -⟩ :=
      committed_admission_registers_one_unstarted_attempt self s' witness request current execution h
    obtain ⟨hcreated, hadmitted, hconsumed, hprotected⟩ :=
      committed_admission_counts self s' witness request current execution h
    exact LedgerMove.admitted _ retained _ hget hlive hwitnesses (by rfl) hattempts hcreated hadmitted
      hconsumed hprotected
  · obtain ⟨hw, ha, -, hkept⟩ := refused_admission_preserves_ledger self s' witness request current
      outcome h (fun execution heq => hcommitted ⟨execution, heq⟩)
    exact LedgerMove.unchanged hw ha hkept

/-- A start marks one unstarted attempt started and counts one protected operation, or changes
    nothing. -/
theorem start_moves_ledger (s s' : Authority) (execution : permit.Execution)
    (outcome : core.result.Result permit.Started report.BoundaryError)
    (h : admission.Authority.start_execution s execution = Result.ok (outcome, s')) :
    LedgerMove s s' := by
  cases outcome with
  | Ok started =>
    obtain ⟨record, hr, hfresh, hattempts, hprotected, hwitnesses, -, -, -, -, hcreated, hconsumed,
      hadmitted⟩ := start_execution_marks_one_unstarted_attempt s s' execution started h
    exact LedgerMove.started _ record ((attempt_record_ok _ _ _).1 hr).2.1 hfresh hattempts hwitnesses
      hcreated hconsumed hadmitted hprotected
  | Err error =>
    rw [start_execution_refusal_returns_input s s' execution error h]
    exact LedgerMove.unchanged rfl rfl ⟨rfl, rfl, rfl, rfl⟩

/-- An observation rewrites at most the observed attempt record, keeping its key, plan and start
    flag, and counts only approved observations. -/
theorem observe_moves_ledger (s s' : Authority) (description : report.ObservationDescription)
    (outcome : core.result.Result report.Observation report.BoundaryError)
    (h : observation.Authority.observe_from_trusted_host s description = Result.ok (outcome, s')) :
    LedgerMove s s' := by
  unfold observation.Authority.observe_from_trusted_host at h
  obtain ⟨r, -, h⟩ := bind_eq_ok h
  split at h
  · obtain ⟨⟨record, back⟩, hmut, h⟩ := bind_eq_ok h
    simp only [uncurry_apply_pair] at h
    obtain ⟨hget, hback⟩ := index_mut_ok hmut
    observed_leaf h hget hback
  · simp only [Result.ok.injEq, Prod.mk.injEq] at h
    obtain ⟨-, rfl⟩ := h
    exact LedgerMove.unchanged rfl rfl ⟨rfl, rfl, rfl, rfl⟩

/-- Retiring a witness rewrites only a live witness, to `Retired`; a consumed or retired witness
    is refused without change. -/
theorem retire_moves_ledger (s s' : Authority) (witness : permit.Witness)
    (outcome : core.result.Result Unit permit.Denial)
    (h : observation.Authority.retire_witness s witness = Result.ok (outcome, s')) :
    LedgerMove s s' := by
  unfold observation.Authority.retire_witness at h
  obtain ⟨r, hr, h⟩ := bind_eq_ok h
  split at h
  · rename_i record
    split at h
    · rename_i hlive
      obtain ⟨⟨wr, back⟩, hmut, h⟩ := bind_eq_ok h
      simp only [uncurry_apply_pair, Result.ok.injEq, Prod.mk.injEq] at h
      obtain ⟨-, rfl⟩ := h
      obtain ⟨hget, hback⟩ := index_mut_ok hmut
      have hfound := (witness_record_found _ _ _ hr).2
      rw [hfound, Option.some.injEq] at hget
      subst hget
      exact LedgerMove.retired _ _ hfound hlive (hback _) rfl ⟨rfl, rfl, rfl, rfl⟩
    all_goals ledger_frame h
  · ledger_frame h

theorem retireIfLive_eq_self (record : WitnessRecord) (h : record.state ≠ permit.WitnessState.Live) :
    retireIfLive record = record := by
  simp [retireIfLive, h]

/-- Retiring every live witness rewrites exactly the live records and changes neither the attempt
    table nor the counters. The generated loop is proved to terminate. -/
theorem retire_live_witnesses_effect (s s' : Authority)
    (h : observation.Authority.retire_live_witnesses s = Result.ok s') :
    s'.witnesses.val = s.witnesses.val.map retireIfLive ∧ s'.attempts = s.attempts ∧
      s'.counters = s.counters := by
  have hspec : observation.Authority.retire_live_witnesses_loop s 0#usize ⦃ r =>
      r.2.1.val = s.witnesses.val.map retireIfLive ∧ r.2.2.1 = s.attempts ∧
        r.2.2.2.2.2.1 = s.counters ⦄ := by
    unfold observation.Authority.retire_live_witnesses_loop
    apply loop.spec_decr_nat (fun st : Authority × Usize => st.1.witnesses.val.length - st.2.val)
      (fun st => st.1.attempts = s.attempts ∧ st.1.counters = s.counters ∧
        st.1.witnesses.val.length = s.witnesses.val.length ∧
        ∀ j, st.1.witnesses.val[j]? =
          (s.witnesses.val[j]?).map (fun record => if j < st.2.val then retireIfLive record else record))
    · rintro ⟨st, index⟩ ⟨ha, hc, hlen, hw⟩
      dsimp only at ha hc hlen hw ⊢
      unfold observation.Authority.retire_live_witnesses_loop.body
      by_cases hlt : index.val < st.witnesses.val.length
      · have hbound := alloc.vec.Vec.len_ineq st.witnesses
        obtain ⟨index1, hadd, hval⟩ := WP.spec_imp_exists
          (UScalar.add_spec (x := index) (y := 1#usize) (by scalar_tac))
        have hnext : index1.val = index.val + 1 := by simpa using hval
        have hlt' : index < alloc.vec.Vec.len st.witnesses := by scalar_tac
        have hget : alloc.vec.Vec.index (core.slice.index.SliceIndexUsizeSlice WitnessRecord)
            st.witnesses index = Result.ok (st.witnesses.val[index.val]) := by
          simp [alloc.vec.Vec.index_slice_index, alloc.vec.Vec.index_usize, hlt]
          rfl
        have hmut : alloc.vec.Vec.index_mut (core.slice.index.SliceIndexUsizeSlice WitnessRecord)
            st.witnesses index =
              Result.ok (st.witnesses.val[index.val], alloc.vec.Vec.set st.witnesses index) := by
          simp [alloc.vec.Vec.index_mut_slice_index, alloc.vec.Vec.index_mut_usize,
            alloc.vec.Vec.index_usize, hlt]
          rfl
        -- The record under the cursor is still the original one.
        have hcur : s.witnesses.val[index.val]? = some st.witnesses.val[index.val] := by
          have h1 := hw index.val
          rw [List.getElem?_eq_getElem hlt] at h1
          cases hs : s.witnesses.val[index.val]? with
          | none => rw [hs] at h1; simp at h1
          | some r => rw [hs] at h1; simp at h1; exact congrArg some h1.symm
        -- Writing the retired form of the cursor record advances the invariant by one slot.
        have hstep : ∀ x : WitnessRecord, retireIfLive st.witnesses.val[index.val] = x →
            ∀ j, (st.witnesses.val.set index.val x)[j]? =
              (s.witnesses.val[j]?).map
                (fun record => if j < index1.val then retireIfLive record else record) := by
          intro x hx j
          by_cases hj : j = index.val
          · subst hj
            rw [List.getElem?_set_self hlt, hcur]
            simp [hnext, hx]
          · rw [List.getElem?_set_ne (Ne.symm hj), hw j]
            have hiff : j < index1.val ↔ j < index.val := by omega
            cases s.witnesses.val[j]? with
            | none => rfl
            | some r => simp only [Option.map_some, hiff]
        simp only [hlt', if_true, hget, bind_tc_ok]
        split
        · rename_i hlive
          simp only [hmut, hadd, bind_tc_ok, uncurry_apply_pair, WP.spec_ok]
          refine ⟨⟨ha, hc, ?_, fun j => ?_⟩, ?_⟩
          · simp [alloc.vec.Vec.set_val_eq, hlen]
          · simp only [alloc.vec.Vec.set_val_eq]
            exact hstep _ (by simp [retireIfLive, hlive]) j
          · simp only [alloc.vec.Vec.set_val_eq, List.length_set]
            scalar_tac
        · rename_i hconsumed
          simp only [hadd, bind_tc_ok, WP.spec_ok]
          refine ⟨⟨ha, hc, hlen, fun j => ?_⟩, by scalar_tac⟩
          have h1 := hstep st.witnesses.val[index.val]
            (retireIfLive_eq_self _ (by simp [hconsumed])) j
          rwa [List.set_getElem_self] at h1
        · rename_i hretired
          simp only [hadd, bind_tc_ok, WP.spec_ok]
          refine ⟨⟨ha, hc, hlen, fun j => ?_⟩, by scalar_tac⟩
          have h1 := hstep st.witnesses.val[index.val]
            (retireIfLive_eq_self _ (by simp [hretired])) j
          rwa [List.set_getElem_self] at h1
      · have hge : ¬ index < alloc.vec.Vec.len st.witnesses := by scalar_tac
        simp only [hge, if_false, WP.spec_ok]
        refine ⟨?_, ha, hc⟩
        apply List.ext_getElem?
        intro j
        rw [hw j, List.getElem?_map]
        cases hs : s.witnesses.val[j]? with
        | none => rfl
        | some r =>
          have hj : j < s.witnesses.val.length := (List.getElem?_eq_some_iff.1 hs).1
          have hbefore : j < index.val := by omega
          simp [hbefore]
    · refine ⟨rfl, rfl, rfl, fun j => ?_⟩
      cases s.witnesses.val[j]? <;> simp
  obtain ⟨r, hr, hpost⟩ := WP.spec_imp_exists hspec
  obtain ⟨p, v, v1, v2, v3, c, io, o⟩ := r
  unfold observation.Authority.retire_live_witnesses at h
  rw [hr] at h
  simp only [bind_tc_ok, uncurry_apply_pair, Result.ok.injEq] at h
  subst h
  exact hpost

theorem cancel_moves_ledger (s s' : Authority)
    (outcome : core.result.Result report.InvocationOutcome report.BoundaryError)
    (h : observation.Authority.cancel_invocation s = Result.ok (outcome, s')) :
    LedgerMove s s' := by
  unfold observation.Authority.cancel_invocation at h
  split at h
  · obtain ⟨self1, hretire, h⟩ := bind_eq_ok h
    simp only [Result.ok.injEq, Prod.mk.injEq] at h
    obtain ⟨-, rfl⟩ := h
    obtain ⟨hw, ha, hc⟩ := retire_live_witnesses_effect _ self1 hretire
    exact LedgerMove.retiredAll hw ha (by rw [hc]; exact ⟨rfl, rfl, rfl, rfl⟩)
  all_goals (simp only [Result.ok.injEq, Prod.mk.injEq] at h; obtain ⟨-, rfl⟩ := h
             exact LedgerMove.unchanged rfl rfl ⟨rfl, rfl, rfl, rfl⟩)

theorem fail_moves_ledger (s s' : Authority)
    (outcome : core.result.Result report.InvocationOutcome report.BoundaryError)
    (h : observation.Authority.fail_invocation s = Result.ok (outcome, s')) :
    LedgerMove s s' := by
  unfold observation.Authority.fail_invocation at h
  split at h
  · obtain ⟨self1, hretire, h⟩ := bind_eq_ok h
    simp only [Result.ok.injEq, Prod.mk.injEq] at h
    obtain ⟨-, rfl⟩ := h
    obtain ⟨hw, ha, hc⟩ := retire_live_witnesses_effect _ self1 hretire
    exact LedgerMove.retiredAll hw ha (by rw [hc]; exact ⟨rfl, rfl, rfl, rfl⟩)
  all_goals (simp only [Result.ok.injEq, Prod.mk.injEq] at h; obtain ⟨-, rfl⟩ := h
             exact LedgerMove.unchanged rfl rfl ⟨rfl, rfl, rfl, rfl⟩)

theorem deliver_moves_ledger (s s' : Authority) (attempt : permit.AttemptKey)
    (observed : report.Observation) (outcome : core.result.Result report.Receipt report.BoundaryError)
    (h : observation.Authority.deliver_success s attempt observed = Result.ok (outcome, s')) :
    LedgerMove s s' := by
  unfold observation.Authority.deliver_success at h
  obtain ⟨r, -, h⟩ := bind_eq_ok h
  split at h
  · obtain ⟨b, -, h⟩ := bind_eq_ok h
    split at h
    · simp only [Result.ok.injEq, Prod.mk.injEq] at h
      obtain ⟨-, rfl⟩ := h
      exact LedgerMove.unchanged rfl rfl ⟨rfl, rfl, rfl, rfl⟩
    · obtain ⟨b1, -, h⟩ := bind_eq_ok h
      split at h
      · simp only [Result.ok.injEq, Prod.mk.injEq] at h
        obtain ⟨-, rfl⟩ := h
        exact LedgerMove.unchanged rfl rfl ⟨rfl, rfl, rfl, rfl⟩
      · obtain ⟨i, -, h⟩ := bind_eq_ok h
        obtain ⟨self1, hretire, h⟩ := bind_eq_ok h
        obtain ⟨r1, -, h⟩ := bind_eq_ok h
        simp only [Result.ok.injEq, Prod.mk.injEq] at h
        obtain ⟨-, rfl⟩ := h
        obtain ⟨hw, ha, hc⟩ := retire_live_witnesses_effect _ self1 hretire
        exact LedgerMove.retiredAll hw ha (by rw [hc]; exact ⟨rfl, rfl, rfl, rfl⟩)
  · simp only [Result.ok.injEq, Prod.mk.injEq] at h
    obtain ⟨-, rfl⟩ := h
    exact LedgerMove.unchanged rfl rfl ⟨rfl, rfl, rfl, rfl⟩

/-- Every public transition is one of the ledger moves. -/
theorem step_moves_ledger (s s' : Authority) (hstep : Step s s') : LedgerMove s s' := by
  cases hstep with
  | authorize plan current outcome h => exact authorize_moves_ledger _ _ _ _ _ h
  | advance checkpoint outcome h => exact advance_moves_ledger _ _ _ _ h
  | admission witness request current outcome h => exact admission_moves_ledger _ _ _ _ _ _ h
  | start execution outcome h => exact start_moves_ledger _ _ _ _ h
  | observe description outcome h => exact observe_moves_ledger _ _ _ _ h
  | retire witness outcome h => exact retire_moves_ledger _ _ _ _ h
  | cancel outcome h => exact cancel_moves_ledger _ _ _ h
  | fail outcome h => exact fail_moves_ledger _ _ _ h
  | deliver attempt observed outcome h => exact deliver_moves_ledger _ _ _ _ _ h

/-! ### Ledger invariant -/

theorem countP_set_keeps {α : Type} (p : α → Bool) (l : List α) (i : Nat) (old new : α)
    (hget : l[i]? = some old) (hp : p new = p old) : (l.set i new).countP p = l.countP p := by
  obtain ⟨hi, hold⟩ := List.getElem?_eq_some_iff.1 hget
  rw [List.countP_set hi, hold, hp]
  by_cases hpold : p old = true
  · have hmem : old ∈ l := by rw [← hold]; exact List.getElem_mem hi
    have : 0 < l.countP p := List.countP_pos_iff.2 ⟨old, hmem, hpold⟩
    simp [hpold]
    omega
  · simp [hpold]

theorem countP_set_adds {α : Type} (p : α → Bool) (l : List α) (i : Nat) (old new : α)
    (hget : l[i]? = some old) (hold : p old = false) (hnew : p new = true) :
    (l.set i new).countP p = l.countP p + 1 := by
  obtain ⟨hi, hgetold⟩ := List.getElem?_eq_some_iff.1 hget
  rw [List.countP_set hi, hgetold, hold, hnew]
  simp

theorem countP_map_retire (l : List WitnessRecord) :
    (l.map retireIfLive).countP isConsumed = l.countP isConsumed := by
  rw [List.countP_map]
  apply List.countP_congr
  intro record _
  by_cases hlive : record.state = permit.WitnessState.Live
  · simp [retireIfLive, isConsumed, hlive]
  · simp [retireIfLive, hlive]

/-- The published counters and the two tables agree: consumed witnesses and registered attempts
    are equal in number, protected operations count started attempts, and the creation and
    admission counters count their tables. -/
def LedgerConsistent (s : Authority) : Prop :=
  s.witnesses.val.countP isConsumed = s.attempts.val.length ∧
    s.counters.protected_operations.val = s.attempts.val.countP (fun record => record.started) ∧
    s.counters.witnesses_created.val = s.witnesses.val.length ∧
    s.counters.attempts_admitted.val = s.attempts.val.length ∧
    s.counters.witness_consumptions = s.counters.attempts_admitted

theorem ledger_move_preserves_consistency (s s' : Authority) (hmove : LedgerMove s s')
    (hinv : LedgerConsistent s) : LedgerConsistent s' := by
  obtain ⟨hconsumed, hprotected, hcreated, hadmitted, hconsumptions⟩ := hinv
  cases hmove with
  | unchanged hw ha hk =>
    obtain ⟨kc, kw, ka, kp⟩ := hk
    refine ⟨?_, ?_, ?_, ?_, ?_⟩
    · rw [hw, ha]; exact hconsumed
    · rw [kp, ha]; exact hprotected
    · rw [kc, hw]; exact hcreated
    · rw [ka, ha]; exact hadmitted
    · rw [kw, ka]; exact hconsumptions
  | authorized record hlive hw ha hc hcons hadm hprot =>
    refine ⟨?_, ?_, ?_, ?_, ?_⟩
    · rw [hw, ha, List.countP_append, hconsumed]; simp [isConsumed, hlive]
    · rw [hprot, ha]; exact hprotected
    · rw [hc, hcreated, hw]; simp
    · rw [hadm, ha]; exact hadmitted
    · rw [hcons, hadm]; exact hconsumptions
  | admitted slot retained record hget hlive hw hfresh ha hc hadm hcons hprot =>
    refine ⟨?_, ?_, ?_, ?_, ?_⟩
    · rw [hw, ha, countP_set_adds isConsumed _ slot retained _ hget (by simp [isConsumed, hlive])
        (by simp [isConsumed]), hconsumed]
      simp
    · rw [hprot, ha, List.countP_append, ← hprotected]; simp [hfresh]
    · rw [hc, hcreated, hw, List.length_set]
    · rw [hadm, hadmitted, ha]; simp
    · exact hcons
  | started slot record hget hfresh ha hw hc hcons hadm hprot =>
    refine ⟨?_, ?_, ?_, ?_, ?_⟩
    · rw [hw, ha, List.length_set]; exact hconsumed
    · rw [hprot, ha, countP_set_adds (fun record => record.started) _ slot record _ hget hfresh rfl,
        hprotected]
    · rw [hc, hw]; exact hcreated
    · rw [hadm, ha, List.length_set]; exact hadmitted
    · rw [hcons, hadm]; exact hconsumptions
  | observed slot record record' hget hsame ha hw hk =>
    obtain ⟨kc, kw, ka, kp⟩ := hk
    have hlen : s'.attempts.val.length = s.attempts.val.length := by rw [ha, List.length_set]
    refine ⟨?_, ?_, ?_, ?_, ?_⟩
    · rw [hw, hlen]; exact hconsumed
    · rw [kp, ha, countP_set_keeps (fun record => record.started) _ slot record record' hget
        hsame.2.2]
      exact hprotected
    · rw [kc, hw]; exact hcreated
    · rw [ka, hlen]; exact hadmitted
    · rw [kw, ka]; exact hconsumptions
  | retired slot retained hget hlive hw ha hk =>
    obtain ⟨kc, kw, ka, kp⟩ := hk
    have hlen : s'.witnesses.val.length = s.witnesses.val.length := by rw [hw, List.length_set]
    refine ⟨?_, ?_, ?_, ?_, ?_⟩
    · rw [hw, ha, countP_set_keeps isConsumed _ slot retained _ hget (by simp [isConsumed, hlive])]
      exact hconsumed
    · rw [kp, ha]; exact hprotected
    · rw [kc, hlen]; exact hcreated
    · rw [ka, ha]; exact hadmitted
    · rw [kw, ka]; exact hconsumptions
  | retiredAll hw ha hk =>
    obtain ⟨kc, kw, ka, kp⟩ := hk
    refine ⟨?_, ?_, ?_, ?_, ?_⟩
    · rw [hw, ha, countP_map_retire]; exact hconsumed
    · rw [kp, ha]; exact hprotected
    · rw [kc, hw, List.length_map]; exact hcreated
    · rw [ka, ha]; exact hadmitted
    · rw [kw, ka]; exact hconsumptions

/-- Initialization: a new authority has empty tables and zero counters. -/
theorem new_starts_ledger_consistent (profile : Profile) (s : Authority)
    (h : facts.Authority.new profile = Result.ok (.Ok s)) : LedgerConsistent s := by
  unfold facts.Authority.new at h
  split_ifs at h <;> try simp at h
  obtain ⟨r, -, h⟩ := bind_eq_ok h
  split at h
  · simp only [Result.ok.injEq, core.result.Result.Ok.injEq] at h
    subst h
    simp [LedgerConsistent, alloc.vec.Vec.with_capacity]
  · simp at h

/-- Preservation: every public transition keeps the counters and tables in agreement. -/
theorem step_preserves_ledger_consistent (s s' : Authority) (hstep : Step s s')
    (hinv : LedgerConsistent s) : LedgerConsistent s' :=
  ledger_move_preserves_consistency s s' (step_moves_ledger s s' hstep) hinv

/-- Over every finite serialized prefix of public transitions from `Authority.new`: each consumed
    witness is matched by one registered attempt in count, protected operations count exactly the
    started attempts, and the creation and admission counters count their tables. -/
theorem reachable_ledger_consistent (s : Authority) (h : Reachable s) : LedgerConsistent s := by
  induction h with
  | init profile s hnew => exact new_starts_ledger_consistent profile s hnew
  | step s s' _ hstep ih => exact step_preserves_ledger_consistent s s' hstep ih

/-! ### Traces: settled witnesses and started attempts stay settled -/

theorem ledger_move_preserves_settled_witnesses (s s' : Authority) (hmove : LedgerMove s s')
    (i : Nat) (record : WitnessRecord) (hget : s.witnesses.val[i]? = some record)
    (hsettled : record.state ≠ permit.WitnessState.Live) : s'.witnesses.val[i]? = some record := by
  have hi : i < s.witnesses.val.length := (List.getElem?_eq_some_iff.1 hget).1
  cases hmove with
  | unchanged hw _ _ => rw [hw]; exact hget
  | authorized _ _ hw _ _ _ _ _ => rw [hw, List.getElem?_append_left hi]; exact hget
  | admitted slot retained _ hslot hlive hw _ _ _ _ _ _ =>
    have hne : slot ≠ i := by
      rintro rfl
      rw [hslot, Option.some.injEq] at hget
      exact hsettled (hget ▸ hlive)
    rw [hw, List.getElem?_set_ne hne]; exact hget
  | started _ _ _ _ _ hw _ _ _ _ => rw [hw]; exact hget
  | observed _ _ _ _ _ _ hw _ => rw [hw]; exact hget
  | retired slot retained hslot hlive hw _ _ =>
    have hne : slot ≠ i := by
      rintro rfl
      rw [hslot, Option.some.injEq] at hget
      exact hsettled (hget ▸ hlive)
    rw [hw, List.getElem?_set_ne hne]; exact hget
  | retiredAll hw _ _ => rw [hw, List.getElem?_map, hget]; simp [retireIfLive, hsettled]

theorem ledger_move_keeps_started_attempts (s s' : Authority) (hmove : LedgerMove s s')
    (i : Nat) (record : AttemptRecord) (hget : s.attempts.val[i]? = some record)
    (hstarted : record.started = true) :
    ∃ record', s'.attempts.val[i]? = some record' ∧ record'.started = true ∧
      record'.key = record.key ∧ record'.plan = record.plan := by
  have hi : i < s.attempts.val.length := (List.getElem?_eq_some_iff.1 hget).1
  cases hmove with
  | unchanged _ ha _ => exact ⟨record, by rw [ha]; exact hget, hstarted, rfl, rfl⟩
  | authorized _ _ _ ha _ _ _ _ => exact ⟨record, by rw [ha]; exact hget, hstarted, rfl, rfl⟩
  | admitted _ _ _ _ _ _ _ ha _ _ _ _ =>
    exact ⟨record, by rw [ha, List.getElem?_append_left hi]; exact hget, hstarted, rfl, rfl⟩
  | started slot fresh hslot hfresh ha _ _ _ _ _ =>
    have hne : slot ≠ i := by
      rintro rfl
      rw [hslot, Option.some.injEq] at hget
      rw [hget] at hfresh
      simp [hfresh] at hstarted
    exact ⟨record, by rw [ha, List.getElem?_set_ne hne]; exact hget, hstarted, rfl, rfl⟩
  | observed slot old new hslot hsame ha _ _ =>
    by_cases hne : slot = i
    · subst hne
      rw [hslot, Option.some.injEq] at hget
      subst hget
      exact ⟨new, by rw [ha, List.getElem?_set_self hi], by rw [hsame.2.2]; exact hstarted,
        hsame.1, hsame.2.1⟩
    · exact ⟨record, by rw [ha, List.getElem?_set_ne hne]; exact hget, hstarted, rfl, rfl⟩
  | retired _ _ _ _ _ ha _ => exact ⟨record, by rw [ha]; exact hget, hstarted, rfl, rfl⟩
  | retiredAll _ ha _ => exact ⟨record, by rw [ha]; exact hget, hstarted, rfl, rfl⟩

/-- Once a witness leaves `Live`, no finite sequence of public transitions rewrites its record. -/
theorem settled_witness_is_never_rewritten (s s' : Authority) (h : Relation.ReflTransGen Step s s')
    (i : Nat) (record : WitnessRecord) (hget : s.witnesses.val[i]? = some record)
    (hsettled : record.state ≠ permit.WitnessState.Live) : s'.witnesses.val[i]? = some record := by
  induction h with
  | refl => exact hget
  | tail _ hstep ih =>
    exact ledger_move_preserves_settled_witnesses _ _ (step_moves_ledger _ _ hstep) i record ih
      hsettled

/-- Denial, preflight, retirement and every other transition cannot resurrect consumed authority:
    a consumed witness stays consumed under every finite sequence of public transitions. -/
theorem consumed_witness_never_returns_to_live (s s' : Authority)
    (h : Relation.ReflTransGen Step s s') (i : Nat) (record : WitnessRecord)
    (hget : s.witnesses.val[i]? = some record)
    (hconsumed : record.state = permit.WitnessState.Consumed) :
    ∀ record', s'.witnesses.val[i]? = some record' →
      record'.state = permit.WitnessState.Consumed := by
  intro record' hget'
  rw [settled_witness_is_never_rewritten s s' h i record hget (by simp [hconsumed]),
    Option.some.injEq] at hget'
  rw [← hget']
  exact hconsumed

/-- A started attempt stays started, with its key and plan, under every finite sequence of public
    transitions. -/
theorem started_attempt_stays_started (s s' : Authority) (h : Relation.ReflTransGen Step s s')
    (i : Nat) (record : AttemptRecord) (hget : s.attempts.val[i]? = some record)
    (hstarted : record.started = true) :
    ∃ record', s'.attempts.val[i]? = some record' ∧ record'.started = true ∧
      record'.key = record.key ∧ record'.plan = record.plan := by
  induction h with
  | refl => exact ⟨record, hget, hstarted, rfl, rfl⟩
  | tail _ hstep ih =>
    obtain ⟨mid, hmid, hmidstarted, hkey, hplan⟩ := ih
    obtain ⟨last, hlast, hlaststarted, hkey', hplan'⟩ :=
      ledger_move_keeps_started_attempts _ _ (step_moves_ledger _ _ hstep) i mid hmid hmidstarted
    exact ⟨last, hlast, hlaststarted, hkey'.trans hkey, hplan'.trans hplan⟩

/-- Attempts start at most once over every trace: once an attempt has started, no finite sequence
    of public transitions reaches a state in which its execution permit starts it again. -/
theorem started_attempt_never_starts_again (s s' s'' : Authority)
    (h : Relation.ReflTransGen Step s s') (execution : permit.Execution) (record : AttemptRecord)
    (hget : s.attempts.val[execution.attempt.slot.val]? = some record)
    (hstarted : record.started = true) (started : permit.Started) :
    admission.Authority.start_execution s' execution ≠ Result.ok (.Ok started, s'') := by
  intro hstart
  obtain ⟨fresh, hr, hfresh, -⟩ :=
    start_execution_marks_one_unstarted_attempt s' s'' execution started hstart
  obtain ⟨kept, hkept, hkeptstarted, -, -⟩ := started_attempt_stays_started s s' h _ record hget hstarted
  rw [((attempt_record_ok _ _ _).1 hr).2.1, Option.some.injEq] at hkept
  rw [← hkept, hfresh] at hkeptstarted
  exact Bool.false_ne_true hkeptstarted

end M5Authority
