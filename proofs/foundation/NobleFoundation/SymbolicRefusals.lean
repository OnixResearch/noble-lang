/-
Query verdicts and the hazards that must not certify (issue #56, acceptance item 5).

A bounded query lane reports one of four verdicts. The hazard theorems below say
that each condition the acceptance criterion names - a stale revision, a malformed
certificate, a corrupted encoding, an exhausted budget - produces `refused` rather
than `certified`, and that an unfinished query produces `unknown`, which is a
different status from refusal and also not certification.

`refused` means this method did not establish a result. `unknown` means the query
did not finish. Neither asserts agreement or disagreement.
-/
namespace NobleFoundation.SymbolicSolver

/-- The status a bounded query lane may report. -/
inductive Verdict where
  /-- A checked certificate was produced. -/
  | certified
  /-- A kernel-checked witness refutes the pair. -/
  | counterexample
  /-- The query did not finish within its budget. -/
  | unknown
  /-- A precondition failed; no result is claimed. -/
  | refused
deriving DecidableEq, Repr

/-- What a query lane holds when it reports. Every field is a condition the acceptance criterion
names, in the form the lane can actually observe. -/
structure QueryOutcome where
  /-- The semantic revision the query was built against is the current one. -/
  revisionCurrent : Bool
  /-- The certificate parsed and its fields are consistent. -/
  certificateWellFormed : Bool
  /-- The graph-to-formula encoding round-trips. -/
  encodingIntact : Bool
  /-- The search stayed inside its declared budget. -/
  withinBudget : Bool
  /-- The search reached a conclusion rather than timing out. -/
  finished : Bool
  /-- The search established agreement. -/
  foundAgreement : Bool

/-- Classify a query. Hazards refuse; an unfinished query is unknown; only a clean, finished query
that established agreement is certified. -/
def classify (o : QueryOutcome) : Verdict :=
  if !o.revisionCurrent then .refused
  else if !o.certificateWellFormed then .refused
  else if !o.encodingIntact then .refused
  else if !o.withinBudget then .refused
  else if !o.finished then .unknown
  else if o.foundAgreement then .certified
  else .counterexample

/-- A stale semantic revision refuses. -/
theorem stale_revision_refused (o : QueryOutcome) (h : o.revisionCurrent = false) :
    classify o = .refused := by
  unfold classify
  rw [h]
  rfl

/-- A malformed certificate refuses. -/
theorem malformed_certificate_refused (o : QueryOutcome) (hrev : o.revisionCurrent = true)
    (h : o.certificateWellFormed = false) : classify o = .refused := by
  unfold classify
  rw [hrev, h]
  rfl

/-- A corrupted encoding refuses. -/
theorem corrupted_encoding_refused (o : QueryOutcome) (hrev : o.revisionCurrent = true)
    (hcert : o.certificateWellFormed = true) (h : o.encodingIntact = false) :
    classify o = .refused := by
  unfold classify
  rw [hrev, hcert, h]
  rfl

/-- An exhausted budget refuses: the search did not stay inside what it declared. -/
theorem exhausted_budget_refused (o : QueryOutcome) (hrev : o.revisionCurrent = true)
    (hcert : o.certificateWellFormed = true) (henc : o.encodingIntact = true)
    (h : o.withinBudget = false) : classify o = .refused := by
  unfold classify
  rw [hrev, hcert, henc, h]
  rfl

/-- An unfinished query is unknown, not refused and not certified. -/
theorem timeout_is_unknown (o : QueryOutcome) (hrev : o.revisionCurrent = true)
    (hcert : o.certificateWellFormed = true) (henc : o.encodingIntact = true)
    (hbudget : o.withinBudget = true) (h : o.finished = false) :
    classify o = .unknown := by
  unfold classify
  rw [hrev, hcert, henc, hbudget, h]
  rfl

/-- UNKNOWN is never certification. -/
theorem unknown_is_not_certified (o : QueryOutcome) : classify o ≠ .certified ∨ o.foundAgreement = true := by
  unfold classify
  split
  · exact Or.inl (by simp)
  · split
    · exact Or.inl (by simp)
    · split
      · exact Or.inl (by simp)
      · split
        · exact Or.inl (by simp)
        · split
          · exact Or.inl (by simp)
          · split
            · exact Or.inr ‹o.foundAgreement = true›
            · exact Or.inl (by simp)

/-- The positive control: a clean, finished query that established agreement is certified. -/
theorem clean_query_certifies (o : QueryOutcome) (hrev : o.revisionCurrent = true)
    (hcert : o.certificateWellFormed = true) (henc : o.encodingIntact = true)
    (hbudget : o.withinBudget = true) (hfin : o.finished = true)
    (hagree : o.foundAgreement = true) : classify o = .certified := by
  unfold classify
  rw [hrev, hcert, henc, hbudget, hfin, hagree]
  rfl

end NobleFoundation.SymbolicSolver
