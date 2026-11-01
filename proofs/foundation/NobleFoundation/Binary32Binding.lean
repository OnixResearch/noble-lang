/-
  The certificate binding layer for the binary32 certificate checker (issue #44,
  NGRAPH-ERR-04).

  The design (`.cairn/changes/numerical-error-certificate`) separates *producing* a certificate from
  *applying* it: a certificate is evidence about one graph snapshot, one declared input-domain set,
  one executed node order, one contraction policy and one analyzer model revision. An accepted
  certificate says nothing about any other context. This module states and checks that boundary.

  What is modelled, and what is deliberately not:

  - A `Context` carries six *identities*: snapshot, graph revision, domain-set digest, executed-order
    digest, the FMA-contraction flag and the model revision. The checker compares them for equality
    and refuses on any difference. How each identity is computed, and whether it really names the
    graph it claims to, is outside this model and is not claimed here. A digest that is computed
    wrongly still compares equal to itself.
  - `checkBound` is `check` behind that equality gate. `checkBound_only_certifies_matching` states
    the premise explicitly: acceptance implies the two contexts are equal *and* the unbounded checker
    accepts. `checkBound_sound` lifts `check_sound` through the gate, so the bound statement is the
    unbounded one under an equal context, never more.
  - A mismatch is its own outcome, `notApplicable`: it is not a failed check (`unknown`), not an
    outside-fragment graph (`unsupported`) and not a budget outcome (`exhausted`). Reporting it as
    `unknown` would claim the bound was checked and failed; reporting it as `certified` would claim
    the bound applies.
  - Controls are decided by the kernel, each against the same accepted fixture certificate: a changed
    snapshot, revision, domain digest, executed order, contraction policy and stale model are each
    refused, while the matching context certifies. A binding layer that refused everything would
    pass the six refusals and fail the matching-context witness, so both are stated.

  This remains a statement about the exact reference model. No analyzer, Rust, Wasm or host
  correspondence is claimed, and an encoded `noble-ngraph` graph is still not bound.
-/
import NobleFoundation.Binary32Checker

namespace NobleFoundation.Binary32.Checker

/-- The context a certificate was produced for. Every field is an identity produced outside this
model: the model compares identities and claims nothing about how they are computed. -/
structure Context where
  /-- Identity of the graph snapshot the certificate was produced for. -/
  snapshot : Nat
  /-- The graph's revision counter. -/
  revision : Nat
  /-- Digest of the declared input-domain set. -/
  domainDigest : Nat
  /-- Digest of the node order the certificate was checked against. -/
  orderDigest : Nat
  /-- Whether the analyzer was permitted to contract `mul` and `add` into one operation. -/
  contraction : Bool
  /-- The analyzer model revision that proposed the bounds. -/
  model : Nat
deriving DecidableEq, Repr

/-- **The bound checker.** A certificate is certified only when the context it declares equals the
context being executed, and the unbounded checker accepts it. -/
def checkBound (budget : Budget) (graph : Graph) (declared actual : Context)
    (certificate : Certificate) : Outcome :=
  if declared = actual then check budget graph certificate else .notApplicable

/-- **Acceptance implies an equal context.** This is the layer's whole premise: nothing about two
different contexts, however similar, is transferred. -/
theorem checkBound_only_certifies_matching (budget : Budget) (graph : Graph) (declared actual : Context)
    (certificate : Certificate) (certified : checkBound budget graph declared actual certificate = .certified) :
    declared = actual ∧ check budget graph certificate = .certified := by
  unfold checkBound at certified
  split at certified
  · rename_i same
    exact ⟨same, certified⟩
  · simp at certified

/-- **The bounded statement is the unbounded one.** An accepted certificate encloses every node
under the declared context; the gate adds the equal-context premise and nothing else. -/
theorem checkBound_sound (budget : Budget) (graph : Graph) (declared actual : Context)
    (certificate : Certificate) (certified : checkBound budget graph declared actual certificate = .certified)
    (machine : List Bits) (real : List ℚ) (applies : Applies certificate.domains machine real) :
    AllEnclose certificate.bounds (machineValues machine graph) (realValues real graph) := by
  have ⟨_, unbounded⟩ := checkBound_only_certifies_matching budget graph declared actual certificate certified
  exact check_sound budget graph certificate unbounded machine real applies

/-- When the contexts are equal the gate is transparent, so a caller that has established equality
gets exactly the unbounded checker's answer. -/
theorem checkBound_agrees_when_equal (budget : Budget) (graph : Graph) (context : Context)
    (certificate : Certificate) : checkBound budget graph context context certificate =
      check budget graph certificate := by
  unfold checkBound
  simp

/-! ## Fixtures and controls

The fixture certificate is the checker's accepted cancellation certificate. Its context is stated
here, not derived from the graph: identities come from the producer, and this lane does not pretend
to compute them. -/

def certificateContext : Context :=
  { snapshot := 0x5EED, revision := 1, domainDigest := 0xD0A1, orderDigest := 0x0DDE,
    contraction := false, model := 7 }

set_option maxRecDepth 100000 in
/-- **The matching context certifies the fixture.** Without this witness the six refusals below
would be satisfied by a gate that refuses everything. -/
theorem matching_context_certifies :
    checkBound fixtureBudget cancellationGraph certificateContext certificateContext
      cancellationCertificate = .certified := by
  decide +kernel

set_option maxRecDepth 100000 in
/-- **A certificate from a different snapshot is not applicable.** -/
theorem changed_snapshot_is_not_applicable :
    checkBound fixtureBudget cancellationGraph certificateContext
      { certificateContext with snapshot := 0x5EF0 } cancellationCertificate = .notApplicable := by
  decide +kernel

set_option maxRecDepth 100000 in
/-- **A certificate from a different graph revision is not applicable.** -/
theorem changed_revision_is_not_applicable :
    checkBound fixtureBudget cancellationGraph certificateContext
      { certificateContext with revision := 2 } cancellationCertificate = .notApplicable := by
  decide +kernel

set_option maxRecDepth 100000 in
/-- **A certificate over a different declared domain set is not applicable**, even though the
certificate's own domains still pass the unbounded checker. -/
theorem changed_domain_is_not_applicable :
    checkBound fixtureBudget cancellationGraph certificateContext
      { certificateContext with domainDigest := 0xD0A2 } cancellationCertificate = .notApplicable := by
  decide +kernel

set_option maxRecDepth 100000 in
/-- **A certificate checked against a different executed node order is not applicable.** The
fragment itself is defined on stored order, so an order difference is a context difference. -/
theorem reordered_execution_is_not_applicable :
    checkBound fixtureBudget cancellationGraph certificateContext
      { certificateContext with orderDigest := 0x0DDF } cancellationCertificate = .notApplicable := by
  decide +kernel

set_option maxRecDepth 100000 in
/-- **A certificate produced with FMA contraction permitted is not applicable without it**, and
the reverse. Contraction changes values, so the policy is part of the context. -/
theorem fma_contraction_is_not_applicable :
    checkBound fixtureBudget cancellationGraph certificateContext
        { certificateContext with contraction := true } cancellationCertificate = .notApplicable ∧
      checkBound fixtureBudget cancellationGraph { certificateContext with contraction := true }
        certificateContext cancellationCertificate = .notApplicable := by
  decide +kernel

set_option maxRecDepth 100000 in
/-- **A stale model revision is not applicable**: the analyzer that proposed the bounds is part of
the context, so a certificate from an older model does not silently apply. -/
theorem stale_model_is_not_applicable :
    checkBound fixtureBudget cancellationGraph certificateContext
      { certificateContext with model := 6 } cancellationCertificate = .notApplicable := by
  decide +kernel

set_option maxRecDepth 100000 in
/-- **A mismatch never reports a failed check.** The refusal class stays distinct from `unknown`,
`unsupported` and `exhausted` for a case whose unbounded certificate would certify. -/
theorem mismatch_is_not_an_unknown_outcome :
    checkBound fixtureBudget cancellationGraph certificateContext
        { certificateContext with model := 6 } cancellationCertificate ≠ .unknown 0 ∧
      checkBound fixtureBudget cancellationGraph certificateContext
        { certificateContext with model := 6 } cancellationCertificate ≠ .unsupported 0 ∧
      checkBound fixtureBudget cancellationGraph certificateContext
        { certificateContext with model := 6 } cancellationCertificate ≠ .exhausted := by
  decide +kernel

end NobleFoundation.Binary32.Checker
