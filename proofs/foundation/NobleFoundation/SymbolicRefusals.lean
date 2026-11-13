/-
Query verdicts and the hazards that must not certify (issue #56, acceptance item 5).

`verification/symbolic/solver.mjs` reports one status per query row. The rules are its code, not
this file's invention:

* a row the lane refuses before it runs — an exhausted node, slot or formula budget, or a graph
  outside the parsed fragment — is `unsupported`, and nothing else about it is examined
  (`solver.mjs`: `admit` and `encode` throw before `outcome` is reached);
* a row whose binding is stale or altered — another encoder, semantic revision or I64 profile,
  or other pair bytes — is `inconclusive` (`outcome`'s first check);
* a row with a bound kernel certificate is `certified` whatever the solver answered
  (`outcome`'s second check, and the claim status the run loop computes);
* a row whose solver model parsed and replays on the original graphs is `refuted`;
* every other row is `inconclusive`: UNSAT without an accepted proof, UNKNOWN, a timeout, a
  solver error, a model that did not parse, or a model that agrees.

Two things the lane observes are deliberately **not** status inputs. Its encoder checks
(concrete simulation and claim check) are recorded per row; a corrupted query whose model still
replays on the original graphs is `refuted`, because the replay is against the graphs and never
against the formula. And the lane's per-row `outcome` is computed with no certificate; the
certificate decides the aggregate claim status, which refuses the contradiction of a certificate
alongside a replaying counterexample.

This file states those rules as a decision model over an `Observation` the lane records per row.
`SymbolicRefusalsReplay` is generated from the retained receipt and has the kernel decide every
recorded row's status from its tuple; it also pinpoints which recorded row kills each seeded
mutation of `classify`. `SymbolicEncoder` is the separate encoder subject.
-/
namespace NobleFoundation.SymbolicSolver

/-- The status a bounded query lane may report. -/
inductive Verdict where
  /-- A bound kernel certificate established agreement. -/
  | certified
  /-- A solver model replayed as a counterexample on the original graphs. -/
  | refuted
  /-- The row produced no result this method may use. -/
  | inconclusive
  /-- The row was refused before it ran: an exhausted budget or a fragment violation. -/
  | unsupported
deriving DecidableEq, Repr

/-- The answer one solver process gave. `notRun` is a row that never reached a solver. -/
inductive Answer where
  /-- The solver reported a satisfying assignment. -/
  | sat
  /-- The solver reported unsatisfiable. -/
  | unsat
  /-- The solver reported UNKNOWN. -/
  | unknown
  /-- The solver was killed by the watchdog. -/
  | timeout
  /-- The solver failed: a nonzero exit, unparseable output or a spawn error. -/
  | error
  /-- The row was refused before any solver ran. -/
  | notRun
deriving DecidableEq, Repr

/-- What the solver or endpoint lane observed about one query or byte-binding control row. Every
field mirrors a fact the lane can see; none of them is a status. -/
structure Observation where
  /-- The row stayed inside the lane's node, slot and formula budget and inside the parsed
fragment, so it ran. A row refused before execution (including the endpoint lane's fixed module
argument boundary) has `withinBudget = false` and no other fact was examined. -/
  withinBudget : Bool
  /-- Every byte and revision binding is current: encoder version, semantic revision, I64
  profile and pair bytes. A source-backed endpoint control also binds its source bytes and loaded
  module digest. -/
  bound : Bool
  /-- The row's claim has a kernel certificate accepted from a clean, bound foundation receipt. -/
  certificate : Bool
  /-- The solver's model parsed with exactly the declared 64-bit slots. -/
  modelParsed : Bool
  /-- The parsed model replays as a counterexample on the original graphs. -/
  modelReplays : Bool
  /-- The solver's answer. -/
  answer : Answer
deriving DecidableEq, Repr

/-- Classify one query row exactly as `verification/symbolic/solver.mjs` derives its status:
refusal first, then the binding, then a bound certificate before any answer, then a replayed
model, and `inconclusive` for everything else. -/
def classify (observed : Observation) : Verdict :=
  if !observed.withinBudget then .unsupported
  else if !observed.bound then .inconclusive
  else if observed.certificate then .certified
  else if observed.answer = .sat then
    (if !observed.modelParsed then .inconclusive
     else if observed.modelReplays then .refuted else .inconclusive)
  else .inconclusive

/-- The status of one pair's claim, as the run loop aggregates its rows: a bound kernel
certificate decides, otherwise any refuted row refutes, otherwise the claim is inconclusive. A
claim cannot carry both a certificate and a refuting row: the lane fails on that contradiction
rather than reporting either status. -/
def claimStatus (certificate : Bool) (runs : List Verdict) : Verdict :=
  if certificate then .certified
  else if runs.contains .refuted then .refuted
  else .inconclusive

/-! ### The rules, one branch each -/

/-- A refused row is `unsupported`, whatever else it carries: refusal precedes every other check. -/
theorem refused_is_unsupported (observed : Observation)
    (hrefused : observed.withinBudget = false) : classify observed = .unsupported := by
  simp [classify, hrefused]

/-- A stale or altered binding is `inconclusive` whatever certificate or answer it carries. -/
theorem stale_binding_inconclusive (observed : Observation)
    (hbudget : observed.withinBudget = true) (hbound : observed.bound = false) :
    classify observed = .inconclusive := by
  simp [classify, hbudget, hbound]

/-- A bound kernel certificate certifies, whatever the solver answered and whichever model it
returned: the certificate's subject is the graphs, not the answer. -/
theorem certificate_certifies (observed : Observation)
    (hbudget : observed.withinBudget = true) (hbound : observed.bound = true)
    (hcertificate : observed.certificate = true) : classify observed = .certified := by
  simp [classify, hbudget, hbound, hcertificate]

/-- An answer other than `sat` is never a counterexample: without a certificate it stays
`inconclusive`, whether it is UNSAT, UNKNOWN, a timeout, an error or a row that never ran. -/
theorem unfinished_inconclusive (observed : Observation)
    (hbudget : observed.withinBudget = true) (hbound : observed.bound = true)
    (hcertificate : observed.certificate = false) (hanswer : observed.answer ≠ .sat) :
    classify observed = .inconclusive := by
  simp [classify, hbudget, hbound, hcertificate, hanswer]

/-- A model that did not parse is never a counterexample. -/
theorem rejected_model_inconclusive (observed : Observation)
    (hbudget : observed.withinBudget = true) (hbound : observed.bound = true)
    (hcertificate : observed.certificate = false) (hanswer : observed.answer = .sat)
    (hparsed : observed.modelParsed = false) : classify observed = .inconclusive := by
  simp [classify, hbudget, hbound, hcertificate, hanswer, hparsed]

/-- A model that parses but agrees on the original graphs is never a counterexample. -/
theorem agreeing_model_inconclusive (observed : Observation)
    (hbudget : observed.withinBudget = true) (hbound : observed.bound = true)
    (hcertificate : observed.certificate = false) (hanswer : observed.answer = .sat)
    (hparsed : observed.modelParsed = true) (hreplays : observed.modelReplays = false) :
    classify observed = .inconclusive := by
  simp [classify, hbudget, hbound, hcertificate, hanswer, hparsed, hreplays]

/-- A parsed model that replays on the original graphs is a counterexample. -/
theorem replaying_model_refutes (observed : Observation)
    (hbudget : observed.withinBudget = true) (hbound : observed.bound = true)
    (hcertificate : observed.certificate = false) (hanswer : observed.answer = .sat)
    (hparsed : observed.modelParsed = true) (hreplays : observed.modelReplays = true) :
    classify observed = .refuted := by
  simp [classify, hbudget, hbound, hcertificate, hanswer, hparsed, hreplays]

/-- A row that was not refused is never `unsupported`. -/
theorem not_refused_not_unsupported (observed : Observation)
    (hbudget : observed.withinBudget = true) : classify observed ≠ .unsupported := by
  cases hbound : observed.bound <;> cases hcertificate : observed.certificate <;>
    cases hanswer : observed.answer <;> cases hparsed : observed.modelParsed <;>
      cases hreplays : observed.modelReplays <;>
        simp [classify, hbudget, hbound, hcertificate, hanswer, hparsed, hreplays]

/-- `unsupported` is exactly a refused row. -/
theorem unsupported_iff_refused (observed : Observation) :
    classify observed = .unsupported ↔ observed.withinBudget = false := by
  by_cases hrefused : observed.withinBudget = false
  · exact ⟨fun _ => hrefused, fun _ => refused_is_unsupported observed hrefused⟩
  · have hbudget : observed.withinBudget = true := by
      cases h : observed.withinBudget <;> simp_all
    exact ⟨fun h => absurd h (not_refused_not_unsupported observed hbudget),
      fun h => absurd h hrefused⟩

/-- Without a certificate a row never certifies. -/
theorem certificate_absent_not_certified (observed : Observation)
    (hbudget : observed.withinBudget = true) (hbound : observed.bound = true)
    (hcertificate : observed.certificate = false) : classify observed ≠ .certified := by
  cases hanswer : observed.answer <;> cases hparsed : observed.modelParsed <;>
    cases hreplays : observed.modelReplays <;>
      simp [classify, hbudget, hbound, hcertificate, hanswer, hparsed, hreplays]

/-- `certified` requires a certificate: nothing else in a row can produce that status. -/
theorem certified_requires_certificate (observed : Observation)
    (hcertified : classify observed = .certified) : observed.certificate = true := by
  by_cases hcertificate : observed.certificate = true
  · exact hcertificate
  · have habsent : observed.certificate = false := by
      cases h : observed.certificate <;> simp_all
    have hbudget : observed.withinBudget = true := by
      by_cases hrefused : observed.withinBudget = false
      · rw [refused_is_unsupported observed hrefused] at hcertified
        exact absurd hcertified (by simp)
      · cases h : observed.withinBudget <;> simp_all
    have hbound : observed.bound = true := by
      by_cases hstale : observed.bound = false
      · rw [stale_binding_inconclusive observed hbudget hstale] at hcertified
        exact absurd hcertified (by simp)
      · cases h : observed.bound <;> simp_all
    exact absurd hcertified (certificate_absent_not_certified observed hbudget hbound habsent)

/-- `refuted` requires a row that ran, was bound, had no certificate, answered `sat`, and whose
model parsed and replayed. -/
theorem refuted_requires_replay (observed : Observation)
    (hrefuted : classify observed = .refuted) :
    observed.withinBudget = true ∧ observed.bound = true ∧ observed.certificate = false ∧
      observed.answer = .sat ∧ observed.modelParsed = true ∧ observed.modelReplays = true := by
  cases hbudget : observed.withinBudget <;> cases hbound : observed.bound <;>
    cases hcertificate : observed.certificate <;> cases hanswer : observed.answer <;>
      cases hparsed : observed.modelParsed <;> cases hreplays : observed.modelReplays <;>
        simp_all [classify]

/-! ### Claim aggregation -/

/-- A claim with a bound certificate is certified whatever its rows say. -/
theorem claim_certificate (runs : List Verdict) : claimStatus true runs = .certified := by
  simp [claimStatus]

/-- A claim with a refuted row is refuted. -/
theorem claim_refutes (runs : List Verdict) (hrefuted : runs.contains .refuted = true) :
    claimStatus false runs = .refuted := by
  unfold claimStatus
  rw [ite_eq_right (by simp)]
  rw [ite_eq_left hrefuted]

/-- A claim with neither a certificate nor a refuted row is inconclusive. -/
theorem claim_inconclusive (runs : List Verdict) (hrefuted : runs.contains .refuted = false) :
    claimStatus false runs = .inconclusive := by
  unfold claimStatus
  rw [ite_eq_right (by simp)]
  rw [ite_eq_right (by simpa using hrefuted)]

/-! ### Fixtures

One concrete observation per branch, so each rule is decided as well as stated. -/

/-- A refused row is unsupported. -/
theorem refused_fixture :
    classify ⟨false, false, false, false, false, .notRun⟩ = .unsupported := by decide

/-- A stale binding with a certificate is inconclusive: the binding is checked first. -/
theorem stale_binding_fixture :
    classify ⟨true, false, true, false, false, .unsat⟩ = .inconclusive := by decide

/-- A certificate with an UNKNOWN answer is certified: the certificate is checked before the
answer. -/
theorem certificate_with_unknown_fixture :
    classify ⟨true, true, true, false, false, .unknown⟩ = .certified := by decide

/-- A certificate with a sat answer and a replaying model is still certified at row level; the
claim-level refusal of that contradiction is the lane's assertion, not this row rule. -/
theorem certificate_with_replayed_model_fixture :
    classify ⟨true, true, true, true, true, .sat⟩ = .certified := by decide

/-- External UNSAT without a certificate is inconclusive. -/
theorem unsat_fixture :
    classify ⟨true, true, false, false, false, .unsat⟩ = .inconclusive := by decide

/-- A sat answer whose model did not parse is inconclusive. -/
theorem rejected_model_fixture :
    classify ⟨true, true, false, false, false, .sat⟩ = .inconclusive := by decide

/-- A sat answer whose model agrees is inconclusive. -/
theorem agreeing_model_fixture :
    classify ⟨true, true, false, true, false, .sat⟩ = .inconclusive := by decide

/-- A sat answer whose model replays is refuted. -/
theorem replayed_model_fixture :
    classify ⟨true, true, false, true, true, .sat⟩ = .refuted := by decide

/-- UNKNOWN is inconclusive, a different status from refusal. -/
theorem unknown_fixture :
    classify ⟨true, true, false, false, false, .unknown⟩ = .inconclusive := by decide

/-- A watchdog timeout is inconclusive. -/
theorem timeout_fixture :
    classify ⟨true, true, false, false, false, .timeout⟩ = .inconclusive := by decide

/-- A solver error is inconclusive. -/
theorem error_fixture :
    classify ⟨true, true, false, false, false, .error⟩ = .inconclusive := by decide

/-- A refused claim row with a certificate the caller supplied is still unsupported. -/
theorem refused_with_certificate_fixture :
    classify ⟨false, true, true, false, false, .notRun⟩ = .unsupported := by decide

/-- A certified claim is certified even when its only row is inconclusive. -/
theorem claim_certificate_fixture : claimStatus true [.inconclusive] = .certified := by decide

/-- A claim with a refuted row is refuted. -/
theorem claim_refuted_fixture : claimStatus false [.inconclusive, .refuted] = .refuted := by decide

/-- A claim with neither is inconclusive. -/
theorem claim_inconclusive_fixture : claimStatus false [.inconclusive] = .inconclusive := by decide

/-! ### Seeded mutations of `classify`

Each variant is one plausible single-clause mistake, and each `*_differs` theorem names a concrete
observation whose status it changes. The generated `SymbolicRefusalsReplay` then names the receipt
row that observes that shape, so a mutation is killed by a recorded result, not by inspection.
-/

/-- Mutation: the binding is checked before refusal, which is what the retired model did. -/
def classifyBindingFirst (observed : Observation) : Verdict :=
  if !observed.bound then .inconclusive
  else if !observed.withinBudget then .unsupported
  else if observed.certificate then .certified
  else if observed.answer = .sat then
    (if !observed.modelParsed then .inconclusive
     else if observed.modelReplays then .refuted else .inconclusive)
  else .inconclusive

/-- Mutation: the solver's answer decides before the certificate. -/
def classifyAnswerFirst (observed : Observation) : Verdict :=
  if !observed.withinBudget then .unsupported
  else if !observed.bound then .inconclusive
  else if observed.answer = .sat then
    (if !observed.modelParsed then .inconclusive
     else if observed.modelReplays then .refuted else .inconclusive)
  else .inconclusive

/-- Mutation: a corrupted encoding is treated as inconclusive, as the retired model did. Unlike
`classify` this variant takes the encoder verdict as an input. -/
def classifyEncodingChecked (observed : Observation) (encodingIntact : Bool) : Verdict :=
  if !observed.withinBudget then .unsupported
  else if !observed.bound then .inconclusive
  else if !encodingIntact then .inconclusive
  else if observed.certificate then .certified
  else if observed.answer = .sat then
    (if !observed.modelParsed then .inconclusive
     else if observed.modelReplays then .refuted else .inconclusive)
  else .inconclusive

/-- Mutation: a sat answer alone is reported as a counterexample. -/
def classifySatRefutes (observed : Observation) : Verdict :=
  if !observed.withinBudget then .unsupported
  else if !observed.bound then .inconclusive
  else if observed.certificate then .certified
  else if observed.answer = .sat then .refuted
  else .inconclusive

/-- Mutation: external UNSAT is accepted as agreement. -/
def classifyUnsatCertifies (observed : Observation) : Verdict :=
  if !observed.withinBudget then .unsupported
  else if !observed.bound then .inconclusive
  else if observed.certificate then .certified
  else if observed.answer = .unsat then .certified
  else if observed.answer = .sat then
    (if !observed.modelParsed then .inconclusive
     else if observed.modelReplays then .refuted else .inconclusive)
  else .inconclusive

/-- The retired model's order differs on a refused row whose binding is also stale. -/
theorem mutation_binding_first_differs :
    classifyBindingFirst ⟨false, false, false, false, false, .notRun⟩ ≠
      classify ⟨false, false, false, false, false, .notRun⟩ := by decide

/-- Checking the binding first would also hide a refusal behind a stale binding. -/
theorem mutation_binding_first_refusal :
    classifyBindingFirst ⟨false, false, false, false, false, .notRun⟩ = .inconclusive := by decide

/-- Certifying after the answer changes a certified pair whose model also replayed. -/
theorem mutation_answer_first_differs :
    classifyAnswerFirst ⟨true, true, true, true, true, .sat⟩ ≠
      classify ⟨true, true, true, true, true, .sat⟩ := by decide

/-- Feeding the encoder verdict into the status changes a corrupted query whose model replayed. -/
theorem mutation_encoding_checked_differs :
    classifyEncodingChecked ⟨true, true, false, true, true, .sat⟩ false ≠
      classify ⟨true, true, false, true, true, .sat⟩ := by decide

/-- Reporting a sat answer as a counterexample changes an unparsed model. -/
theorem mutation_sat_refutes_differs :
    classifySatRefutes ⟨true, true, false, false, false, .sat⟩ ≠
      classify ⟨true, true, false, false, false, .sat⟩ := by decide

/-- Accepting external UNSAT as agreement changes an inconclusive row. -/
theorem mutation_unsat_certifies_differs :
    classifyUnsatCertifies ⟨true, true, false, false, false, .unsat⟩ ≠
      classify ⟨true, true, false, false, false, .unsat⟩ := by decide

end NobleFoundation.SymbolicSolver
