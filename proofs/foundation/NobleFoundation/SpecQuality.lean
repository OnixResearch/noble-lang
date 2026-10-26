/-
Specification-quality obligations for retained contract relations (issue #65,
phase 1).

An implementation proof answers "does this program satisfy its contract".
It does not answer three separate questions about the contract itself:

* is the declared domain *inhabited* at all (`Admits`)?
* does every admissible input have *some* typed output satisfying the
  postcondition (`Realizable`)?
* when the owner asks for it, is a satisfying output *unique* under the
  declared observation relation (`Deterministic`)?

rust-djinn separates existence, unambiguity and implementation correctness in
this way. This file borrows the distinction, not a trusted translation.

Every claim here is a statement about this model. `Relation` abstracts the shape
of `NobleContracts.exportedClaim`: typed parameters and inputs, a precondition
over them, and a postcondition relating the input, the exact typed output and the
parameters. Nothing here relates the model to the Rust renderer or to the
retained contracts, so no correspondence claim is made. In particular:

* a *false* precondition makes a universal claim provable while saying nothing
  about the program (`realizable_of_uninhabited` is a vacuity statement);
* relational realizability says nothing about termination, traps, quotas or
  success (`increment` is total only because its postcondition is total);
* determinacy says nothing about intended meaning;
* a failed output search is never a realizability refutation, and no theorem here
  models solver proposals, timeouts or exhaustion.
-/
namespace NobleFoundation.SpecQuality

/-- One retained contract relation, abstracted to the exact statement shape of
`exportedClaim`: parameters, typed inputs, and a postcondition over the input,
the typed output and the parameters. -/
structure Relation where
  /-- Logical parameters. They are not executable inputs. -/
  Params : Type
  /-- The declared typed input stack. -/
  Input : Type
  /-- The declared typed output stack. -/
  Output : Type
  /-- The precondition over inputs and parameters. -/
  pre : Params → Input → Prop
  /-- The postcondition over inputs, outputs and parameters. -/
  post : Params → Input → Output → Prop

/-- Domain inhabitation: some well-typed input/parameter assignment satisfies
the precondition. -/
def Admits (r : Relation) : Prop := ∃ params input, r.pre params input

/-- Relational realizability: every admissible input/parameter assignment has
some typed output satisfying the postcondition. This is not a termination
claim. -/
def Realizable (r : Relation) : Prop :=
  ∀ params input, r.pre params input → ∃ out, r.post params input out

/-- Output determinacy: two satisfying outputs for the same inputs and
parameters are equal. This file uses exact equality; any observational quotient
would have to be selected, versioned and checked separately. -/
def Deterministic (r : Relation) : Prop :=
  ∀ params input out₁ out₂,
    r.pre params input → r.post params input out₁ → r.post params input out₂ →
      out₁ = out₂

/-- A universal realizability proposition is provable from a false precondition
alone. This is exactly the vacuity the reports must mark: the proof carries no
evidence about any program. -/
theorem realizable_of_uninhabited (r : Relation) (h : ¬ Admits r) :
    Realizable r := by
  intro params input hpre
  exact absurd ⟨params, input, hpre⟩ h

/-- A contradictory postcondition under an inhabited domain refutes relational
realizability. The witness of inhabitation is required: an empty domain would
make the statement vacuous instead. -/
theorem unrealizable_of_contradictory_post (r : Relation) (hin : Admits r)
    (hpost : ∀ params input out, r.pre params input → ¬ r.post params input out) :
    ¬ Realizable r := by
  obtain ⟨params, input, hpre⟩ := hin
  intro hreal
  obtain ⟨out, hout⟩ := hreal params input hpre
  exact hpost params input out hpre hout

/-- Two distinct satisfying outputs for one admissible input refute determinacy.
This is the ambiguity witness an independent producer must supply. -/
theorem not_deterministic_of_two_outputs (r : Relation) {params input out₁ out₂}
    (hpre : r.pre params input) (h₁ : r.post params input out₁)
    (h₂ : r.post params input out₂) (hne : out₁ ≠ out₂) :
    ¬ Deterministic r := by
  intro hdet
  exact hne (hdet params input out₁ out₂ hpre h₁ h₂)

/-- A unique satisfying output under the precondition entails determinacy on
that input. The premise is per input; this is not a claim that searching for a
second output failed. -/
theorem deterministic_of_unique_output (r : Relation)
    (hunique : ∀ params input out₁ out₂,
      r.pre params input → r.post params input out₁ → r.post params input out₂ →
        out₁ = out₂) :
    Deterministic r := hunique

/-! ### The exact wrapping increment

The first acceptance instance: a pure `I64` increment with a true precondition,
wrapping arithmetic (`BitVec 64` addition wraps), and an exact output
postcondition. Parameters are the unit type, so the relation is purely
input/output. -/

/-- Wrapping `I64` increment with a true precondition. -/
def increment : Relation where
  Params := Unit
  Input := BitVec 64
  Output := BitVec 64
  pre := fun _ _ => True
  post := fun _ n out => out = n + 1

/-- The increment's domain is inhabited. -/
theorem increment_inhabited : Admits increment := ⟨(), (0 : BitVec 64), trivial⟩

/-- The increment is relationally realizable: the wrapped successor exists for
every input. -/
theorem increment_realizable : Realizable increment := by
  intro _ (n : BitVec 64) _
  exact ⟨n + 1, rfl⟩

/-- The increment is deterministic. -/
theorem increment_deterministic : Deterministic increment := by
  intro _ (_ : BitVec 64) (out₁ : BitVec 64) (out₂ : BitVec 64) _ h₁ h₂
  rw [h₁, h₂]

/-! ### A contradictory postcondition

Admissible domain, impossible postcondition: non-realizability with a checked
inhabitation witness. -/

/-- True precondition, impossible postcondition. -/
def contradictory : Relation where
  Params := Unit
  Input := BitVec 64
  Output := BitVec 64
  pre := fun _ _ => True
  post := fun _ _ _ => False

/-- The contradictory relation's domain is inhabited, so its refutation is not
vacuous. -/
theorem contradictory_inhabited : Admits contradictory := ⟨(), (0 : BitVec 64), trivial⟩

/-- The contradictory relation is not relationally realizable. -/
theorem contradictory_not_realizable : ¬ Realizable contradictory :=
  unrealizable_of_contradictory_post contradictory contradictory_inhabited
    (fun _ _ _ _ h => h)

/-! ### An underconstrained `Bool` output

The postcondition accepts any `Bool`, so realization holds while determinacy
fails with two distinct checked outputs for one input. This separation is the
reason realizability and determinacy are separate obligations rather than one
"the specification is fine" bit. -/

/-- Any-`Bool` postcondition over a true precondition. -/
def underconstrainedBool : Relation where
  Params := Unit
  Input := BitVec 64
  Output := Bool
  pre := fun _ _ => True
  post := fun _ _ _ => True

/-- Two distinct satisfying outputs for one input. -/
theorem underconstrainedBool_two_outputs :
    underconstrainedBool.post () (0 : BitVec 64) true ∧
      underconstrainedBool.post () (0 : BitVec 64) false ∧
      true ≠ false := ⟨trivial, trivial, fun h => Bool.noConfusion h⟩

/-- The underconstrained relation is realizable. -/
theorem underconstrainedBool_realizable : Realizable underconstrainedBool := by
  intro _ (_ : BitVec 64) _
  exact ⟨true, trivial⟩

/-- The underconstrained relation is not deterministic. -/
theorem underconstrainedBool_not_deterministic :
    ¬ Deterministic underconstrainedBool :=
  not_deterministic_of_two_outputs underconstrainedBool (params := ()) (input := (0 : BitVec 64))
    (out₁ := true) (out₂ := false) (hpre := trivial) (h₁ := trivial) (h₂ := trivial)
    (fun h => Bool.noConfusion h)

/-! ### Determinacy does not imply realizability

A relation whose output type is empty satisfies determinacy vacuously — there
are no two outputs to compare — while an admissible input has no satisfying
output at all. A report MUST NOT read a determinacy pass as existence. -/

/-- Impossible postcondition with an empty output type. -/
def emptyOutput : Relation where
  Params := Unit
  Input := BitVec 64
  Output := Empty
  pre := fun _ _ => True
  post := fun _ _ _ => False

/-- Determinacy of the empty-output relation holds vacuously. -/
theorem emptyOutput_deterministic : Deterministic emptyOutput := by
  intro _ (_ : BitVec 64) out₁ _ _ _ _
  exact out₁.elim

/-- The empty-output relation is not realizable despite its determinacy. -/
theorem emptyOutput_not_realizable : ¬ Realizable emptyOutput := by
  intro hreal
  obtain ⟨out, _⟩ := hreal () (0 : BitVec 64) trivial
  exact out.elim

/-- Determinacy alone never witnesses existence: a deterministic relation may
still have no output for an admissible input. -/
theorem determinacy_does_not_imply_realizability :
    Deterministic emptyOutput ∧ ¬ Realizable emptyOutput :=
  ⟨emptyOutput_deterministic, emptyOutput_not_realizable⟩

end NobleFoundation.SpecQuality
