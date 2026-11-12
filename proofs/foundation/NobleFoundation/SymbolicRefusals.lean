/-
Query verdicts and the hazards that must not certify (issue #56, acceptance item 5).

`verification/symbolic/solver.mjs` reports one status per query: `certified` only
from a bound kernel certificate; `refuted` only for a solver model that replays on
the original graphs; `inconclusive` for a stale or altered binding, a malformed
witness, an UNKNOWN answer, a solver error or a timeout; and `unsupported` for a
query the lane refuses to run inside its declared node, slot and formula budget.
This file states those rules as a decision model over observations a caller
supplies:

* a stale or altered binding, a malformed certificate or witness, and a corrupted
  encoding are `inconclusive`;
* an exhausted budget is `unsupported`, and an unfinished query is `inconclusive`,
  which is a different status from `unsupported` and is not certification;
* `refuted` requires a witness that replays on the original graphs;
* only a bound, well-formed, intact, in-budget, finished query whose bound
  certificate established agreement is `certified`.

Scope: `QueryOutcome` is supplied by the caller and `classify` is defined here.
Nothing yet connects this model to `verification/symbolic/solver.mjs`: no lane
constructs a `QueryOutcome`, so these theorems are about the decision rules, not
about the lane's observed results. `refuted` is a graph-model counterexample whose
witness replayed; it is not a Wasm theorem.
-/
namespace NobleFoundation.SymbolicSolver

/-- The status a bounded query lane may report. -/
inductive Verdict where
  /-- A bound kernel certificate established agreement. -/
  | certified
  /-- A solver model replayed as a counterexample on the original graphs. -/
  | refuted
  /-- The query produced no result this method may use. -/
  | inconclusive
  /-- The query was refused before it ran: an exhausted budget or a fragment
  violation. -/
  | unsupported
deriving DecidableEq, Repr

/-- What a caller observed about one query. Each field mirrors something
`verification/symbolic/solver.mjs` can observe. -/
structure QueryOutcome where
  /-- The query is bound to the expected encoder, revision, profile and pair bytes. -/
  bound : Bool
  /-- The certificate or witness parsed and its fields are consistent. -/
  certificateWellFormed : Bool
  /-- The graph-to-formula encoding round-trips and its fixed points hold. -/
  encodingIntact : Bool
  /-- The query stayed inside the lane's declared node, slot and formula budget. -/
  withinBudget : Bool
  /-- The search reached an answer rather than an UNKNOWN, an error or a timeout. -/
  finished : Bool
  /-- A bound kernel certificate established agreement. -/
  foundAgreement : Bool
  /-- The search returned a model. -/
  foundWitness : Bool
  /-- The returned model replays on the original graphs. -/
  witnessReplays : Bool

/-- Classify a query. A binding, certificate, encoding or completion failure is
inconclusive, an exhausted budget is unsupported, agreement is certified only from a
bound certificate, and a counterexample is refuted only when its witness replays. -/
def classify (outcome : QueryOutcome) : Verdict :=
  if !outcome.bound then .inconclusive
  else if !outcome.certificateWellFormed then .inconclusive
  else if !outcome.encodingIntact then .inconclusive
  else if !outcome.withinBudget then .unsupported
  else if !outcome.finished then .inconclusive
  else if outcome.foundAgreement then .certified
  else if outcome.foundWitness && outcome.witnessReplays then .refuted
  else .inconclusive

/-- A stale or altered binding is inconclusive. -/
theorem stale_revision_inconclusive (outcome : QueryOutcome) (hbound : outcome.bound = false) :
    classify outcome = .inconclusive := by
  simp [classify, hbound]

/-- A malformed certificate or witness is inconclusive. -/
theorem malformed_certificate_inconclusive (outcome : QueryOutcome)
    (hbound : outcome.bound = true) (hcert : outcome.certificateWellFormed = false) :
    classify outcome = .inconclusive := by
  simp [classify, hbound, hcert]

/-- A corrupted encoding is inconclusive. -/
theorem corrupted_encoding_inconclusive (outcome : QueryOutcome)
    (hbound : outcome.bound = true) (hcert : outcome.certificateWellFormed = true)
    (hencoding : outcome.encodingIntact = false) : classify outcome = .inconclusive := by
  simp [classify, hbound, hcert, hencoding]

/-- An exhausted budget is unsupported: the lane refused to run the query. -/
theorem exhausted_budget_unsupported (outcome : QueryOutcome)
    (hbound : outcome.bound = true) (hcert : outcome.certificateWellFormed = true)
    (hencoding : outcome.encodingIntact = true) (hbudget : outcome.withinBudget = false) :
    classify outcome = .unsupported := by
  simp [classify, hbound, hcert, hencoding, hbudget]

/-- An unfinished query is inconclusive: an UNKNOWN, an error and a timeout share
one status, and none of them is refusal or certification. -/
theorem unfinished_query_inconclusive (outcome : QueryOutcome)
    (hbound : outcome.bound = true) (hcert : outcome.certificateWellFormed = true)
    (hencoding : outcome.encodingIntact = true) (hbudget : outcome.withinBudget = true)
    (hfinished : outcome.finished = false) : classify outcome = .inconclusive := by
  simp [classify, hbound, hcert, hencoding, hbudget, hfinished]

/-- A forced UNKNOWN is never certification. -/
theorem unfinished_query_not_certified (outcome : QueryOutcome)
    (hbound : outcome.bound = true) (hcert : outcome.certificateWellFormed = true)
    (hencoding : outcome.encodingIntact = true) (hbudget : outcome.withinBudget = true)
    (hfinished : outcome.finished = false) : classify outcome ≠ .certified := by
  rw [unfinished_query_inconclusive outcome hbound hcert hencoding hbudget hfinished]
  simp

/-- A model that does not replay on the original graphs is never refuted, so an
altered or malformed witness cannot certify a counterexample. -/
theorem unreplayed_witness_not_refuted (outcome : QueryOutcome)
    (hbound : outcome.bound = true) (hcert : outcome.certificateWellFormed = true)
    (hencoding : outcome.encodingIntact = true) (hbudget : outcome.withinBudget = true)
    (hfinished : outcome.finished = true) (hagreement : outcome.foundAgreement = false)
    (hwitness : outcome.foundWitness = true) (hreplay : outcome.witnessReplays = false) :
    classify outcome = .inconclusive := by
  simp [classify, hbound, hcert, hencoding, hbudget, hfinished, hagreement, hwitness, hreplay]

/-- A finished query that found no witness is inconclusive: an external UNSAT
answer without an accepted proof is not agreement. -/
theorem no_witness_inconclusive (outcome : QueryOutcome)
    (hbound : outcome.bound = true) (hcert : outcome.certificateWellFormed = true)
    (hencoding : outcome.encodingIntact = true) (hbudget : outcome.withinBudget = true)
    (hfinished : outcome.finished = true) (hagreement : outcome.foundAgreement = false)
    (hwitness : outcome.foundWitness = false) : classify outcome = .inconclusive := by
  simp [classify, hbound, hcert, hencoding, hbudget, hfinished, hagreement, hwitness]

/-- A counterexample status requires a replayed witness: `refuted` implies both that
a model was found and that it replayed on the original graphs. -/
theorem refuted_requires_replay (outcome : QueryOutcome)
    (hrefuted : classify outcome = .refuted) :
    outcome.foundWitness = true ∧ outcome.witnessReplays = true := by
  unfold classify at hrefuted
  split at hrefuted
  · simp at hrefuted
  · split at hrefuted
    · simp at hrefuted
    · split at hrefuted
      · simp at hrefuted
      · split at hrefuted
        · simp at hrefuted
        · split at hrefuted
          · simp at hrefuted
          · split at hrefuted
            · simp at hrefuted
            · split at hrefuted
              · next hfound =>
                  simp_all
              · simp at hrefuted

/-- The positive control: a clean, finished query whose bound certificate established
agreement is certified. -/
theorem clean_query_certifies (outcome : QueryOutcome)
    (hbound : outcome.bound = true) (hcert : outcome.certificateWellFormed = true)
    (hencoding : outcome.encodingIntact = true) (hbudget : outcome.withinBudget = true)
    (hfinished : outcome.finished = true) (hagreement : outcome.foundAgreement = true) :
    classify outcome = .certified := by
  simp [classify, hbound, hcert, hencoding, hbudget, hfinished, hagreement]

/-! ### Fixtures

Concrete observations for the distinctions the lane draws. -/

/-- The clean query is certified. -/
theorem clean_query_fixture :
    classify ⟨true, true, true, true, true, true, false, false⟩ = .certified := by
  decide

/-- An unfinished query that already found agreement is inconclusive, not certified:
the UNKNOWN status is the query's completion, not its result. -/
theorem unfinished_query_fixture :
    classify ⟨true, true, true, true, false, true, false, false⟩ = .inconclusive := by
  decide

/-- A finished query with no agreement and no witness is inconclusive, not refuted: an
external UNSAT answer is not a counterexample. -/
theorem no_witness_fixture :
    classify ⟨true, true, true, true, true, false, false, false⟩ = .inconclusive := by
  decide

/-- A witness that does not replay is inconclusive, not refuted. -/
theorem unreplayed_witness_fixture :
    classify ⟨true, true, true, true, true, false, true, false⟩ = .inconclusive := by
  decide

/-- A witness that replays is refuted. -/
theorem refuted_witness_fixture :
    classify ⟨true, true, true, true, true, false, true, true⟩ = .refuted := by
  decide

/-- An exhausted budget is unsupported, which is a different status from the
inconclusive one an unfinished query takes. -/
theorem exhausted_budget_fixture :
    classify ⟨true, true, true, false, false, false, false, false⟩ = .unsupported := by
  decide

end NobleFoundation.SymbolicSolver
