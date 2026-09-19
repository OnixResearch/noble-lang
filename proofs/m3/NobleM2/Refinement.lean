/-
Refinement: the Aeneas-extracted kernel checker refines the reference model
on the fragment fixture matrix (M2 fragment v0).

Every fixture of `crates/noble-kernel/tests/acceptance.rs` — restated in
`NobleM2.Fixtures` — is run through the extracted checker
`noble_kernel.acceptance.check` over the embedding of its environment,
request, and candidate, and the projected outcome is proved equal to the
reference decision `NobleM2.check` on the same fixture. Ten of the twelve
fixtures agree outright. The two rejection fixtures whose diagnostics the
kernel locates (`hidden_emit`, `resource_eligibility`) agree on every field
except diagnostic provenance: the extracted checker fills the failing
`node`/`def` site exactly as the Rust kernel does, while the reference
`diagnosticOf` records `none` — fields the Rust tests never assert. For those
two, the refinement is stated after `eraseProvenance`, which clears exactly
those two fields and nothing else.

Each equation is closed by evaluation: the extracted call graph (including
its fuel-driven `partial_fixpoint` loops) and the reference checker both
compute to ground values under `native_decide`, as does the projected
comparison — a mismatched pair (wrong limits embedded on one side only)
fails to close.

`accepted_via_refinement` factors the refinement generically: whenever the
extracted run agrees with the reference outcome and the extracted run
observes `ok (Accepted gchecked)`, the reference outcome is exactly the
projection of the extracted checked result. Instantiated with a fixture
theorem, it hands the extracted acceptance to the reference soundness story.
-/

import NobleKernel
import NobleM2.Embed
import NobleM2.Fixtures

/- Structural equality on the reference outcome domain is decidable; the
   reference modules only derive this for the leaf types. -/
deriving instance DecidableEq for NobleM2.Interface
deriving instance DecidableEq for NobleM2.Derivation
deriving instance DecidableEq for NobleM2.Checked
deriving instance DecidableEq for NobleM2.Constraint
deriving instance DecidableEq for NobleM2.Diagnostic
deriving instance DecidableEq for NobleM2.UnsupportedKind
deriving instance DecidableEq for NobleM2.LimitKind
deriving instance DecidableEq for NobleM2.Outcome

namespace NobleM2.Refinement

open NobleM2 Fixtures noble_kernel Aeneas Aeneas.Std

/-! ## Provenance erasure -/

/-- Clear the diagnostic provenance fields (`node`, `definition`) of an
    invalid outcome; every other outcome — and every other diagnostic field —
    is untouched. The extracted checker locates failing sites the reference
    model does not record. -/
def eraseProvenance : Outcome → Outcome
  | .invalid d => .invalid { d with node := none, definition := none }
  | o => o

/-! ## The fixture refinement family -/

/-- `sequence_and_literals_accept_arithmetic`: the extracted checker accepts
    the arithmetic sequence and records the same three derivations. -/
theorem refinement_sequence_and_literals_accept_arithmetic :
    project_result (acceptance.check (embedEnv bootstrapEnv)
        (embedRequest (requestOf [] [.i64] []))
        (embedCandidate sequenceCandidate))
      = NobleM2.check bootstrapEnv (requestOf [] [.i64] []) sequenceCandidate := by
  native_decide

/-- `quotation_construction_checks_body_and_runs`: the extracted checker
    checks the quotation body, runs it, and accepts with five derivations. -/
theorem refinement_quotation_construction_checks_body_and_runs :
    project_result (acceptance.check (embedEnv bootstrapEnv)
        (embedRequest fortyTwoRequest) (embedCandidate fortyTwoCandidate))
      = NobleM2.check bootstrapEnv fortyTwoRequest fortyTwoCandidate := by
  native_decide

/-- `limits_nodes_exact`: accepted exactly at the node budget. -/
theorem refinement_limits_nodes_exact :
    project_result (acceptance.check (embedEnv bootstrapEnv)
        (embedRequest { fortyTwoRequest with limits := { baseLimits with nodes := 5 } })
        (embedCandidate fortyTwoCandidate))
      = NobleM2.check bootstrapEnv
          { fortyTwoRequest with limits := { baseLimits with nodes := 5 } }
          fortyTwoCandidate := by
  native_decide

/-- `limits_nodes_over`: one node over budget exhausts it. -/
theorem refinement_limits_nodes_over :
    project_result (acceptance.check (embedEnv bootstrapEnv)
        (embedRequest { fortyTwoRequest with limits := { baseLimits with nodes := 4 } })
        (embedCandidate fortyTwoCandidate))
      = NobleM2.check bootstrapEnv
          { fortyTwoRequest with limits := { baseLimits with nodes := 4 } }
          fortyTwoCandidate := by
  native_decide

/-- `limits_depth_deep`: nesting depth one still admits the quotation. -/
theorem refinement_limits_depth_deep :
    project_result (acceptance.check (embedEnv bootstrapEnv)
        (embedRequest { fortyTwoRequest with limits := { baseLimits with depth := 1 } })
        (embedCandidate fortyTwoCandidate))
      = NobleM2.check bootstrapEnv
          { fortyTwoRequest with limits := { baseLimits with depth := 1 } }
          fortyTwoCandidate := by
  native_decide

/-- `limits_depth_shallow`: depth zero rejects the nested body check. -/
theorem refinement_limits_depth_shallow :
    project_result (acceptance.check (embedEnv bootstrapEnv)
        (embedRequest { fortyTwoRequest with limits := { baseLimits with depth := 0 } })
        (embedCandidate fortyTwoCandidate))
      = NobleM2.check bootstrapEnv
          { fortyTwoRequest with limits := { baseLimits with depth := 0 } }
          fortyTwoCandidate := by
  native_decide

/-- `limits_work_eight`: the unit-literal program fits the work budget. -/
theorem refinement_limits_work_eight :
    project_result (acceptance.check (embedEnv bootstrapEnv)
        (embedRequest { requestOf [] [.unit] [] with
            limits := { baseLimits with work := 8 } })
        (embedCandidate unitCandidate))
      = NobleM2.check bootstrapEnv
          { requestOf [] [.unit] [] with limits := { baseLimits with work := 8 } }
          unitCandidate := by
  native_decide

/-- `limits_work_four`: half the budget exhausts it. -/
theorem refinement_limits_work_four :
    project_result (acceptance.check (embedEnv bootstrapEnv)
        (embedRequest { requestOf [] [.unit] [] with
            limits := { baseLimits with work := 4 } })
        (embedCandidate unitCandidate))
      = NobleM2.check bootstrapEnv
          { requestOf [] [.unit] [] with limits := { baseLimits with work := 4 } }
          unitCandidate := by
  native_decide

/-- `hidden_emit_rejects_against_empty_bound`: the latent emit inside the
    quotation violates the declared empty effect bound. The extracted checker
    locates the failure at node 1; the reference records no site, so both
    sides are compared after `eraseProvenance` (all other fields equal). -/
theorem refinement_hidden_emit_rejects_against_empty_bound :
    eraseProvenance (project_result (acceptance.check (embedEnv bootstrapEnv)
        (embedRequest hiddenEmitRequest) (embedCandidate hiddenEmitCandidate)))
      = eraseProvenance
        (NobleM2.check bootstrapEnv hiddenEmitRequest hiddenEmitCandidate) := by
  native_decide

/-- `diagnostics_report_order`: the shape matches but the order does not. -/
theorem refinement_diagnostics_report_order :
    project_result (acceptance.check (embedEnv bootstrapEnv)
        (embedRequest orderRequest) (embedCandidate orderCandidate))
      = NobleM2.check bootstrapEnv orderRequest orderCandidate := by
  native_decide

/-- `diagnostics_truncation`: a one-entry diagnostic budget truncates the
    recorded stacks. -/
theorem refinement_diagnostics_truncation :
    project_result (acceptance.check (embedEnv bootstrapEnv)
        (embedRequest { orderRequest with
            limits := { baseLimits with diagnostics := 1 } })
        (embedCandidate orderCandidate))
      = NobleM2.check bootstrapEnv
          { orderRequest with limits := { baseLimits with diagnostics := 1 } }
          orderCandidate := by
  native_decide

/-- `resource_eligibility_rejects_duplication`: duplicating a resource
    through `dup` violates the eligibility side condition of the environment's
    named maker definition. The extracted checker locates the failure at
    node 1 under definition 0 (`dup`); the reference records no site, so both
    sides are compared after `eraseProvenance` (all other fields equal). -/
theorem refinement_resource_eligibility_rejects_duplication :
    eraseProvenance (project_result (acceptance.check (embedEnv eligibilityEnv)
        (embedRequest (requestOf [] [.resource fixtureResource] []))
        (embedCandidate eligibilityCandidate))) = eraseProvenance
      (NobleM2.check eligibilityEnv
          (requestOf [] [.resource fixtureResource] []) eligibilityCandidate) := by
  native_decide

/-! ## Composition with the reference soundness story -/

/-- The refinement composes with acceptance: if the extracted run agrees with
    the reference outcome and observes `ok (Accepted gchecked)`, the reference
    outcome is exactly the acceptance of the projected checked result. -/
theorem accepted_via_refinement {env : Env} {req : Request} {cand : Candidate}
    {checked : Checked} {gchecked : noble_kernel.untrusted.Checked}
    (h : project_result (acceptance.check (embedEnv env) (embedRequest req)
        (embedCandidate cand)) = NobleM2.check env req cand)
    (gacc : acceptance.check (embedEnv env) (embedRequest req) (embedCandidate cand)
      = Result.ok (noble_kernel.untrusted.Outcome.Accepted gchecked)) :
    NobleM2.check env req cand = Outcome.accepted (project_checked gchecked) := by
  rw [gacc] at h
  simpa [project_result, project_outcome] using h.symm

end NobleM2.Refinement
