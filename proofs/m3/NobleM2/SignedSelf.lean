/-
Reference model: signature-checked recursive definitions (K-CHECK-02, #99).

"Recursive definitions require an explicit signature and check recursive calls against that
signature without polymorphic recursion." The kernel states this as the signed check
`noble_kernel::acceptance::check_signed_structure` (`crates/noble-kernel/src/acceptance/preflight.rs`,
`nodes.rs`). Its owner is a named definition whose scheme is the declared signature, and the
owner witness instantiates that signature at the expected interface. The check:
- admits a `SignedSelf` node only for that owner;
- types each one exactly as an `Invocation` of the owner's own scheme (`nodes.rs`);
- requires every such site's witness to be, binding for binding, the owner witness.

A self call is therefore a monomorphic use of the declared signature inside the body that
defines it.

**Representation.** A signed candidate here is a reference-model candidate whose self sites are
marked by index. Each marked node is the owner invocation the kernel types it as, so the existing
`Derives` judgment and its `check_soundness` theorem apply to the body unchanged. `checkSigned`
runs the kernel's signed preflight and then the reference checker. `checkSigned_soundness` proves
that acceptance implies `SignedDerives`:
- the body derives the expected interface;
- that interface is the owner's declared signature at the owner witness;
- every self site invokes the owner at exactly that witness.

**What the model leaves out.**
- *Module ownership.* The kernel also requires the owner to belong to the calling module; the
  reference environment carries no module data.
- *The public checker's refusal.* The model represents a self site as an invocation, so it cannot
  state that the public checker refuses a `SignedSelf` node. The kernel's crate tests cover that
  refusal.
- *Ordinary self invocations.* A self edge through an ordinary invocation is the dependency
  audit's to refuse (B-CHECK-02), as in the kernel, which has no whole graph during this check.
  The model claims nothing about it.

This is a model theorem: no correspondence with the extracted kernel, whose retained extraction
predates `SignedSelf`.
-/

import NobleM2.CheckSoundness

namespace NobleM2

namespace SignedSelf

/-- A candidate with its self sites marked: the node indices the kernel would carry as `SignedSelf`. -/
structure SignedCandidate where
  cand : Candidate
  selfSites : List Nat
  deriving Repr, Inhabited

/-- Every marked self site is an invocation of the owner at exactly the owner witness. -/
def sitesAt (sc : SignedCandidate) (owner : Nat) (ownerInst : Inst) : Bool :=
  sc.selfSites.all fun site => decide (sc.cand.nodes[site]? = some (.invocation owner ownerInst))

/-- The kernel's signed preflight, then the reference checker.
- The owner must be a named definition with a scheme, its declared signature.
- That signature, at the owner witness, must be exactly the expected interface.
- Every marked self site must invoke the owner at the owner witness.

Each refusal is `unsupported nodeForm`, as in the kernel. -/
def checkSigned (env : Env) (request : Request) (sc : SignedCandidate) (owner : Nat)
    (ownerInst : Inst) : Outcome :=
  if env.kind owner = some .named then
    match env.scheme owner with
    | none => .unsupported .nodeForm
    | some scheme =>
      match applyScheme env request.limits scheme ownerInst with
      | .error _ => .unsupported .nodeForm
      | .ok (iface, _) =>
        if iface.stackIn = request.expected.stackIn ∧ iface.stackOut = request.expected.stackOut
            ∧ iface.effects = request.expected.allowedEffects then
          if sitesAt sc owner ownerInst = true then check env request sc.cand
          else .unsupported .nodeForm
        else .unsupported .nodeForm
  else .unsupported .nodeForm

/-- K-CHECK-02 at the model level:
- the owner is a named definition;
- its declared signature, at a resolution of the owner witness, is exactly the expected
  interface;
- every self site invokes the owner at that witness;
- the body derives the expected interface. -/
def SignedDerives (env : Env) (request : Request) (sc : SignedCandidate) (owner : Nat)
    (ownerInst : Inst) (effects : EffSet) : Prop :=
  env.kind owner = some .named ∧
    (∃ scheme resolved, env.scheme owner = some scheme ∧ ResolvesTo ownerInst resolved ∧
      scheme.instantiate resolved =
        some (request.expected.stackIn, request.expected.stackOut, request.expected.allowedEffects)) ∧
    (∀ site, site ∈ sc.selfSites → sc.cand.nodes[site]? = some (.invocation owner ownerInst)) ∧
    Derives env sc.cand sc.cand.body request.expected.stackIn request.expected.stackOut effects

/-- `sitesAt` holds exactly when every marked site invokes the owner at the owner witness. -/
theorem sitesAt_iff {sc : SignedCandidate} {owner : Nat} {ownerInst : Inst} :
    sitesAt sc owner ownerInst = true ↔
      ∀ site, site ∈ sc.selfSites → sc.cand.nodes[site]? = some (.invocation owner ownerInst) := by
  unfold sitesAt
  rw [List.all_eq_true]
  constructor
  · intro h site hsite
    exact of_decide_eq_true (h site hsite)
  · intro h site hsite
    exact decide_eq_true (h site hsite)

/-- **Soundness of the signed check.** Acceptance implies the reference checker's well-formedness
premise and K-CHECK-02's `SignedDerives`, at the checked bound. -/
theorem checkSigned_soundness {env : Env} {request : Request} {sc : SignedCandidate}
    {owner : Nat} {ownerInst : Inst} {checked : Checked}
    (h : checkSigned env request sc owner ownerInst = .accepted checked) :
    WellFormed request sc.cand checked ∧
      SignedDerives env request sc owner ownerInst checked.interface.effects := by
  unfold checkSigned at h
  by_cases hk : env.kind owner = some .named
  · simp only [hk, ↓reduceIte] at h
    cases hs : env.scheme owner with
    | none => simp only [hs] at h; cases h
    | some scheme =>
      simp only [hs] at h
      cases hap : applyScheme env request.limits scheme ownerInst with
      | error problem => simp only [hap] at h; cases h
      | ok pair =>
        obtain ⟨iface, resolved⟩ := pair
        simp only [hap] at h
        by_cases hi : iface.stackIn = request.expected.stackIn ∧
            iface.stackOut = request.expected.stackOut ∧
            iface.effects = request.expected.allowedEffects
        · simp only [hi, and_self, ↓reduceIte] at h
          by_cases hsites : sitesAt sc owner ownerInst = true
          · simp only [hsites, ↓reduceIte] at h
            obtain ⟨hwf, hderiv⟩ := check_soundness h
            refine ⟨hwf, hk, ⟨scheme, resolved, hs, applyScheme_ok_resolves hap, ?_⟩,
              sitesAt_iff.mp hsites, hderiv⟩
            rw [applyScheme_ok hap, hi.1, hi.2.1, hi.2.2]
          · simp only [hsites, Bool.false_eq_true, ↓reduceIte] at h
            cases h
        · simp only [hi, ↓reduceIte] at h
          cases h
  · simp only [hk, ↓reduceIte] at h
    cases h

/-- **No polymorphic recursion.** A self site whose witness is not the owner witness is refused,
even when the ordinary invocation rule would type it. -/
theorem checkSigned_refuses_polymorphic_recursion {env : Env} {request : Request}
    {sc : SignedCandidate} {owner : Nat} {ownerInst inst : Inst} {site : Nat}
    (hsite : site ∈ sc.selfSites) (hnode : sc.cand.nodes[site]? = some (.invocation owner inst))
    (hne : inst ≠ ownerInst) (checked : Checked) :
    checkSigned env request sc owner ownerInst ≠ .accepted checked := by
  intro h
  have hat := (checkSigned_soundness h).2.2.2.1 site hsite
  rw [hnode] at hat
  injection hat with hat
  injection hat with _ hinst
  exact hne hinst

/-- **Self means the owner.** A marked site that invokes any other definition is refused. Mutual
recursion is outside K-CHECK-02 until it is specified. -/
theorem checkSigned_refuses_a_foreign_self_site {env : Env} {request : Request}
    {sc : SignedCandidate} {owner other : Nat} {ownerInst inst : Inst} {site : Nat}
    (hsite : site ∈ sc.selfSites) (hnode : sc.cand.nodes[site]? = some (.invocation other inst))
    (hne : other ≠ owner) (checked : Checked) :
    checkSigned env request sc owner ownerInst ≠ .accepted checked := by
  intro h
  have hat := (checkSigned_soundness h).2.2.2.1 site hsite
  rw [hnode] at hat
  injection hat with hat
  injection hat with hdef _
  exact hne hdef

/-- **A signature is required.** An owner that is not a named definition, so it has no declared
signature to check its self calls against, is refused. -/
theorem checkSigned_requires_a_named_owner {env : Env} {request : Request}
    {sc : SignedCandidate} {owner : Nat} {ownerInst : Inst}
    (hk : env.kind owner ≠ some .named) (checked : Checked) :
    checkSigned env request sc owner ownerInst ≠ .accepted checked := by
  intro h
  exact hk (checkSigned_soundness h).2.1

/-! ## Fixtures

The bootstrap environment plus one named definition at index 23 whose declared signature is
`S -- S ! pure`: the `forever` of the kernel's crate tests. As there, the environment records no
self edge for it.

Each fixture is stated through a `Bool` classifier of the outcome, and a lemma turns it back into
the outcome it classifies. The refusals are decided by the kernel itself (`decide +kernel`).

An acceptance runs the whole reference checker, whose effect union `EffSet.union` is
well-founded recursion that the kernel cannot reduce. So, like the reference model's own
acceptance fixtures (`Regression.ce_accepted`, `Termination.accepted`), the three acceptances are
closed by evaluation (`native_decide`). Their axioms are reported per declaration; the soundness
theorem and its corollaries use none of them. -/

/-- Whether an outcome accepts with exactly this interface. -/
def acceptsWith (outcome : Outcome) (stackIn stackOut : TyList) (effects : EffSet) : Bool :=
  match outcome with
  | .accepted checked =>
      decide (checked.interface.stackIn = stackIn ∧ checked.interface.stackOut = stackOut ∧
        checked.interface.effects = effects)
  | .invalid _ | .unsupported _ | .exhausted _ | .internalFailure => false

/-- Whether an outcome is the `unsupported nodeForm` refusal. -/
def refusesNodeForm (outcome : Outcome) : Bool :=
  match outcome with
  | .unsupported .nodeForm => true
  | .accepted _ | .invalid _ | .unsupported _ | .exhausted _ | .internalFailure => false

theorem acceptsWith_spec {outcome : Outcome} {stackIn stackOut : TyList} {effects : EffSet}
    (h : acceptsWith outcome stackIn stackOut effects = true) :
    ∃ checked, outcome = .accepted checked ∧ checked.interface.stackIn = stackIn ∧
      checked.interface.stackOut = stackOut ∧ checked.interface.effects = effects := by
  cases outcome with
  | accepted checked => exact ⟨checked, rfl, of_decide_eq_true h⟩
  | invalid _ => cases h
  | unsupported _ => cases h
  | exhausted _ => cases h
  | internalFailure => cases h

theorem refusesNodeForm_spec {outcome : Outcome} (h : refusesNodeForm outcome = true) :
    outcome = .unsupported .nodeForm := by
  cases outcome with
  | unsupported kind => cases kind <;> first | rfl | cases h
  | accepted _ => cases h
  | invalid _ => cases h
  | exhausted _ => cases h
  | internalFailure => cases h

/-- `S -- S ! pure`. -/
def stackScheme : Scheme :=
  ⟨[Kind.stack], PartList.ofList [StackPart.stack 0], PartList.ofList [StackPart.stack 0],
    SlotList.nil⟩

/-- The bootstrap environment plus the signed owner at index 23. -/
def ownerEnv : Env :=
  { defs := bootstrapEnv.defs ++ [stackScheme]
    kinds := bootstrapEnv.kinds ++ [.named]
    effects := bootstrapEnv.effects
    deps := []
    schemas := [] }

def ownerIndex : Nat := 23

def fixtureLimits : Limits := ⟨65536, 256, 32, 64, 16, 10000, 64⟩

/-- The owner witness that binds `S` to the empty stack, and one that binds it to `[i64]`. -/
def emptyWitness : Inst := ⟨[.stack .nil]⟩
def intWitness : Inst := ⟨[.stack (.cons .i64 .nil)]⟩

/-- `def forever [ forever ]` at `[] -- []`: one self site at the owner witness. -/
def forever : SignedCandidate := ⟨⟨0, 0, [.invocation ownerIndex emptyWitness], [0]⟩, [0]⟩

def emptyRequest : Request := ⟨0, ⟨.nil, .nil, EffSet.empty⟩, fixtureLimits⟩

/-- The owner checked at `[i64] -- [i64]`, whose self site instantiates `S` to the empty stack
instead. The ordinary invocation rule types it, since `[] -- []` joins any stack, but the
instantiation differs from the owner witness. -/
def polymorphic : SignedCandidate := ⟨⟨0, 0, [.invocation ownerIndex emptyWitness], [0]⟩, [0]⟩

/-- The same owner with its self site at the owner witness. -/
def monomorphic : SignedCandidate := ⟨⟨0, 0, [.invocation ownerIndex intWitness], [0]⟩, [0]⟩

def intRequest : Request := ⟨0, ⟨.cons .i64 .nil, .cons .i64 .nil, EffSet.empty⟩, fixtureLimits⟩

/-- The signed check accepts `forever` at its declared interface `[] -- [] ! pure`. -/
theorem forever_accepted :
    acceptsWith (checkSigned ownerEnv emptyRequest forever ownerIndex emptyWitness)
      .nil .nil EffSet.empty = true := by
  native_decide

/-- So `forever` satisfies K-CHECK-02. -/
theorem forever_signed_derives :
    SignedDerives ownerEnv emptyRequest forever ownerIndex emptyWitness EffSet.empty := by
  obtain ⟨checked, haccepted, _, _, heffects⟩ := acceptsWith_spec forever_accepted
  have hsigned := (checkSigned_soundness haccepted).2
  rw [heffects] at hsigned
  exact hsigned

/-- The ordinary checker types the polymorphic self call at `[i64] -- [i64]`... -/
theorem polymorphic_ordinary_check_accepts :
    acceptsWith (check ownerEnv intRequest polymorphic.cand)
      (.cons .i64 .nil) (.cons .i64 .nil) EffSet.empty = true := by
  native_decide

/-- ...and the signed check refuses it. -/
theorem polymorphic_signed_refused :
    checkSigned ownerEnv intRequest polymorphic ownerIndex intWitness = .unsupported .nodeForm :=
  refusesNodeForm_spec (by decide +kernel)

/-- At the owner witness, the same body is accepted. -/
theorem monomorphic_accepted :
    acceptsWith (checkSigned ownerEnv intRequest monomorphic ownerIndex intWitness)
      (.cons .i64 .nil) (.cons .i64 .nil) EffSet.empty = true := by
  native_decide

/-- `dup` (index 0) is a bootstrap word, not a named definition, so it has no declared signature
and cannot own a self call. -/
theorem unsigned_owner_refused :
    checkSigned ownerEnv emptyRequest ⟨⟨0, 0, [.invocation 0 emptyWitness], [0]⟩, [0]⟩ 0
      emptyWitness = .unsupported .nodeForm :=
  refusesNodeForm_spec (by decide +kernel)

/-- A self site that invokes a different named definition (index 23, while the owner is a second
named definition at 24) is refused: mutual recursion is outside K-CHECK-02. -/
def twoOwnerEnv : Env :=
  { ownerEnv with defs := ownerEnv.defs ++ [stackScheme], kinds := ownerEnv.kinds ++ [.named] }

theorem foreign_self_site_refused :
    checkSigned twoOwnerEnv emptyRequest forever 24 emptyWitness = .unsupported .nodeForm :=
  refusesNodeForm_spec (by decide +kernel)

end SignedSelf

end NobleM2
