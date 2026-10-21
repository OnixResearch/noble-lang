import NobleContracts.NamedSubject

/-!
  Conditional normal-return linking model for one same-artifact NamedV1 call.

  The executable-facing part is deliberately a model, not a theorem about
  `noble-wasm`, the Wasm loader, or arbitrary host contexts.  It records the
  operand/continuation ABI, exact embedded definition identity, and the
  exclusive-engine/no-protected-state-mutation premises needed to connect a
  finite runtime path to the pinned NamedV1 normal-return semantics.
-/

namespace NobleFoundation.LinkBoundary

/-! ## The source-bound two-result pilot -/

def sourceBytes (source : String) : List UInt8 := source.toUTF8.toList

/-- The exact source spelling used by the finite pilot fixture. -/
def calleeSource : String :=
  "module LinkCallee@1 [ export split def split [ - 7 ] ]\n"

/-- The exact root spelling used by the finite pilot fixture. Its immutable
import snapshot is bound separately below. -/
def callerSource : String :=
  "c.split -\n"

def importSource : String := "import LinkCallee@1 as c\n"

def calleeDefinitionSource : String := "def split [ - 7 ]"
def callerDefinitionSource : String := "c.split -\n"
def wrongCalleeDefinitionSource : String := "def split [ - 8 ]"

def calleeProvenance : NobleContracts.NamedV1.Provenance :=
  { moduleName := "LinkCallee"
    moduleVersion := 1
    moduleSource := sourceBytes calleeSource
    definitionSlot := 0
    definitionName := "split"
    definitionSource := sourceBytes calleeDefinitionSource }

def callerProvenance : NobleContracts.NamedV1.Provenance :=
  { moduleName := "LinkCaller"
    moduleVersion := 1
    moduleSource := sourceBytes callerSource
    definitionSlot := 0
    definitionName := "run"
    definitionSource := sourceBytes callerDefinitionSource }

/-- Public-order source body: `x y - 7` returns `[x-y, 7]`. -/
def splitBody : List NobleContracts.NamedV1.Op :=
  [.word 5, .lit (.i64 (7 : BitVec 64))]

/-- `Use.slot` is the accepted specialization/environment lookup slot.  It
is not the source definition ordinal (`Provenance.definitionSlot`) and is not
the generated Wasm table index or environment pointer. -/
def splitUse : NobleContracts.NamedV1.Use :=
  { slot := 0
    occurrence := 0
    caller := callerProvenance
    origin := calleeProvenance
    input := [.i64, .i64]
    output := [.i64, .i64] }

def splitEntry : NobleContracts.NamedV1.Entry :=
  { slot := 0
    origin := calleeProvenance
    input := [.i64, .i64]
    output := [.i64, .i64]
    body := splitBody
    dependencies := []
    rank := 0 }

def pilotEnvironment : NobleContracts.NamedV1.Environment := ⟨[splitEntry]⟩

def callerRoot : List NobleContracts.NamedV1.Op := [.call splitUse, .word 5]

def callerDependencies : List Nat := [0]

theorem split_entry_lookup :
    pilotEnvironment.lookup splitUse.slot = some splitEntry := by
  simp [pilotEnvironment, splitUse, splitEntry, NobleContracts.NamedV1.Environment.lookup]

theorem split_entry_matches_use : splitEntry.matches splitUse := by
  simp [splitEntry, splitUse, NobleContracts.NamedV1.Entry.matches]

theorem split_entry_typed :
    NobleContracts.NamedV1.CodeTyped pilotEnvironment splitEntry.body [.i64, .i64] [.i64, .i64] := by
  apply NobleContracts.NamedV1.CodeTyped.cons
  · exact NobleContracts.NamedV1.OpTyped.sub (s := [])
  · apply NobleContracts.NamedV1.CodeTyped.cons
    · exact NobleContracts.NamedV1.OpTyped.lit NobleContracts.NamedV1.HasType.i64
    · exact NobleContracts.NamedV1.CodeTyped.nil

theorem pilot_root_typed :
    NobleContracts.NamedV1.CodeTyped pilotEnvironment callerRoot [.i64, .i64] [.i64] := by
  apply NobleContracts.NamedV1.CodeTyped.cons
  · exact NobleContracts.NamedV1.OpTyped.call (s := []) split_entry_lookup split_entry_matches_use
      split_entry_typed
  · exact NobleContracts.NamedV1.CodeTyped.cons (NobleContracts.NamedV1.OpTyped.sub (s := [])) NobleContracts.NamedV1.CodeTyped.nil

theorem pilot_closure :
    NobleContracts.NamedV1.ExactClosure pilotEnvironment callerProvenance callerRoot callerDependencies 1 := by
  simp [NobleContracts.NamedV1.ExactClosure, pilotEnvironment, splitEntry, splitUse, callerRoot,
    callerDependencies, splitBody, NobleContracts.NamedV1.usesCode, NobleContracts.NamedV1.usesOp,
    NobleContracts.NamedV1.usesValue, NobleContracts.NamedV1.Environment.lookup,
    NobleContracts.NamedV1.Entry.matches]

/-- A named subject whose environment row and root are the fixture above.
This is a model fixture; construction does not authenticate source acceptance. -/
def pilotSubject : NobleContracts.NamedV1.Subject :=
  { provenance := callerProvenance
    env := pilotEnvironment
    root := callerRoot
    dependencies := callerDependencies
    rank := 1
    closure := pilot_closure
    checkedEntries := by
      intro entry hentry
      have heq : entry = splitEntry := by
        simpa [pilotEnvironment] using hentry
      subst entry
      exact split_entry_typed
    input := [.i64, .i64]
    output := [.i64]
    typed := pilot_root_typed }

private theorem split_body_run_unique
    (tail : NobleContracts.NamedV1.Stack) (x y : BitVec 64) (after : NobleContracts.NamedV1.Stack)
    (h : NobleContracts.NamedV1.Run pilotEnvironment splitBody
      (.i64 y :: .i64 x :: tail) after) :
    after = .i64 7 :: .i64 (x - y) :: tail := by
  unfold splitBody at h
  cases h with
  | cons hsub hrest =>
    cases hsub with
    | sub =>
      cases hrest with
      | cons hlit hnil =>
        cases hlit with
        | lit =>
          cases hnil
          rfl

private theorem caller_root_run_unique
    (tail : NobleContracts.NamedV1.Stack) (x y : BitVec 64) (after : NobleContracts.NamedV1.Stack)
    (h : NobleContracts.NamedV1.Run pilotEnvironment callerRoot
      (.i64 y :: .i64 x :: tail) after) :
    after = .i64 (x - y - 7) :: tail := by
  unfold callerRoot at h
  cases h with
  | cons hcall hrest =>
    cases hcall with
    | call hlookup _ hcallee =>
      simp [pilotEnvironment, splitUse, splitEntry,
        NobleContracts.NamedV1.Environment.lookup] at hlookup
      subst_vars
      have hcalleeOutput := split_body_run_unique tail x y _ hcallee
      obtain rfl := hcalleeOutput
      cases hrest with
      | cons hsub hnil =>
        cases hsub with
        | sub =>
          cases hnil
          rfl

/-- The concrete named call returns its two results in source order, for any
preserved stack tail. The subtraction is BitVec64 wrapping subtraction. -/
theorem scalar_call_entry_return (tail : NobleContracts.NamedV1.Stack) (x y : BitVec 64) :
    NobleContracts.NamedV1.Exec pilotEnvironment [.call splitUse]
      (tail ++ [.i64 x, .i64 y])
      (tail ++ [.i64 (x - y), .i64 7]) := by
  have hcallee : NobleContracts.NamedV1.Run pilotEnvironment splitBody
      (.i64 y :: .i64 x :: tail.reverse)
      (.i64 7 :: .i64 (x - y) :: tail.reverse) := by
    exact .cons .sub (.cons .lit .nil)
  have hcall : NobleContracts.NamedV1.Run pilotEnvironment [.call splitUse]
      (.i64 y :: .i64 x :: tail.reverse)
      (.i64 7 :: .i64 (x - y) :: tail.reverse) := by
    exact .cons (.call split_entry_lookup split_entry_matches_use hcallee) .nil
  simpa [NobleContracts.NamedV1.Exec, List.reverse_append] using hcall

/-- The complete `c.split -` caller is a NamedV1 normal-return theorem, not a
signature-only output assertion. It preserves every tail value and keeps the
callee's ordered results before applying wrapping subtraction. -/
theorem caller_after_named_call (tail : NobleContracts.NamedV1.Stack) (x y : BitVec 64)
    (after : NobleContracts.NamedV1.Stack)
    (h : NobleContracts.NamedV1.Exec pilotEnvironment callerRoot
      (tail ++ [.i64 x, .i64 y]) after) :
    after = tail ++ [.i64 (x - y - 7)] := by
  have hrun : NobleContracts.NamedV1.Run pilotEnvironment callerRoot
      (.i64 y :: .i64 x :: tail.reverse) after.reverse := by
    simpa [NobleContracts.NamedV1.Exec, List.reverse_append] using h
  have hout := caller_root_run_unique tail.reverse x y after.reverse hrun
  have hreverse := congrArg List.reverse hout
  simpa [List.reverse_append] using hreverse

/-- Constructive normal-return witness for the full `c.split -` body. -/
theorem pilot_caller_executes (tail : NobleContracts.NamedV1.Stack) (x y : BitVec 64) :
    NobleContracts.NamedV1.Exec pilotEnvironment callerRoot
      (tail ++ [.i64 x, .i64 y])
      (tail ++ [.i64 (x - y - 7)]) := by
  have hcallee : NobleContracts.NamedV1.Run pilotEnvironment splitBody
      (.i64 y :: .i64 x :: tail.reverse)
      (.i64 7 :: .i64 (x - y) :: tail.reverse) := by
    exact .cons .sub (.cons .lit .nil)
  have hrun : NobleContracts.NamedV1.Run pilotEnvironment callerRoot
      (.i64 y :: .i64 x :: tail.reverse)
      (.i64 (x - y - 7) :: tail.reverse) := by
    unfold callerRoot
    exact .cons (.call split_entry_lookup split_entry_matches_use hcallee)
      (.cons .sub .nil)
  simpa [NobleContracts.NamedV1.Exec, List.reverse_append] using hrun

theorem pilot_bool_prefix_executes (boolPrefix : Bool) (x y : BitVec 64) :
    NobleContracts.NamedV1.Exec pilotEnvironment callerRoot
      ([.bool boolPrefix, .i64 x, .i64 y])
      ([.bool boolPrefix, .i64 (x - y - 7)]) := by
  simpa using pilot_caller_executes ([.bool boolPrefix]) x y

theorem operand_order_distinguished :
    ((3 : BitVec 64) - (8 : BitVec 64)) ≠
      ((8 : BitVec 64) - (3 : BitVec 64)) := by
  decide

theorem result_order_distinguished :
    ([.i64 ((3 : BitVec 64) - 8), .i64 7] : NobleContracts.NamedV1.Stack) ≠
      [.i64 7, .i64 ((3 : BitVec 64) - 8)] := by
  intro h
  have hhead := congrArg (fun values : NobleContracts.NamedV1.Stack => values.head?) h
  simp at hhead

/-! ## Horizontal NamedV1 composition -/

private theorem run_append
    {env : NobleContracts.NamedV1.Environment} {left right : List NobleContracts.NamedV1.Op}
    {before after : NobleContracts.NamedV1.Stack} :
    NobleContracts.NamedV1.Run env (left ++ right) before after ↔
      ∃ middle, NobleContracts.NamedV1.Run env left before middle ∧ NobleContracts.NamedV1.Run env right middle after := by
  induction left generalizing before after with
  | nil =>
    simp only [List.nil_append]
    constructor
    · intro h
      exact ⟨before, .nil, h⟩
    · rintro ⟨middle, hleft, hright⟩
      cases hleft
      simpa using hright
  | cons op rest ih =>
    simp only [List.cons_append]
    constructor
    · intro h
      cases h with
      | cons hstep hrest =>
        obtain ⟨middle, hleft, hright⟩ := ih.mp hrest
        exact ⟨middle, .cons hstep hleft, hright⟩
    · rintro ⟨middle, hleft, hright⟩
      cases hleft with
      | cons hstep hrest =>
        exact .cons hstep (ih.mpr ⟨middle, hrest, hright⟩)

/-- Horizontal composition uses three independently established normal-return
segments. The call boundary resolves the exact selected entry and its full
ordered interface; no whole-caller output is assumed. -/
theorem horizontal_compose
    {env : NobleContracts.NamedV1.Environment} {pre post : List NobleContracts.NamedV1.Op}
    {use : NobleContracts.NamedV1.Use} {entry : NobleContracts.NamedV1.Entry}
    {before middle afterCall final : NobleContracts.NamedV1.Stack}
    (hlookup : env.lookup use.slot = some entry)
    (hmatch : entry.matches use)
    (hpre : NobleContracts.NamedV1.Run env pre before middle)
    (hcallee : NobleContracts.NamedV1.Run env entry.body middle afterCall)
    (hpost : NobleContracts.NamedV1.Run env post afterCall final) :
    NobleContracts.NamedV1.Run env (pre ++ [.call use] ++ post) before final := by
  have hcall : NobleContracts.NamedV1.Run env [.call use] middle afterCall :=
    .cons (.call hlookup hmatch hcallee) .nil
  have hpreCall : NobleContracts.NamedV1.Run env (pre ++ [.call use]) before afterCall :=
    run_append.mpr ⟨middle, hpre, hcall⟩
  exact run_append.mpr ⟨afterCall, hpreCall, hpost⟩

/-! ## Runtime ABI and shared engine state -/

def i64Tag : UInt32 := 1
def boolTag : UInt32 := 2
def operandCellBytes : Nat := 16
def continuationBase : Nat := 4096
def continuationCellBytes : Nat := 8

/-- One operand-memory cell in the actual field order tag:i32, padding:i32,
payload:i64. The padding is retained but semantically ignored. -/
structure OperandCell where
  tag : UInt32
  padding : UInt32
  payload : BitVec 64
  deriving DecidableEq, Repr

def i64Cell (value : BitVec 64) : OperandCell :=
  { tag := i64Tag, padding := 0, payload := value }

def boolCell (value : Bool) : OperandCell :=
  { tag := boolTag, padding := 0, payload := if value then 1 else 0 }

/-- `$sp` is a count of 16-byte operand cells. -/
def operandAddress (index : Nat) : Nat := operandCellBytes * index

/-- `$cp` is a count of 8-byte continuation cells beginning at byte 4096. -/
def continuationAddress (index : Nat) : Nat :=
  continuationBase + continuationCellBytes * index

theorem operand_slot_address (index : Nat) : operandAddress index = 16 * index := rfl

theorem continuation_slot_address (index : Nat) :
    continuationAddress index = 4096 + 8 * index := rfl

structure ContinuationSlot where
  index : UInt32
  environment : UInt32
  deriving DecidableEq, Repr

/-- A generated Wasm row is bound to one artifact, root/table index, runtime
environment pointer, full NamedV1 provenance, specialization slot, interface,
embedded bytes and modeled body. `definitionSlot` is the source ordinal;
`Use.slot` is a different lookup key. -/
structure ProgramBinding where
  artifactBytes : List UInt8
  rootIndex : UInt32
  environment : UInt32
  provenance : NobleContracts.NamedV1.Provenance
  /-- Lexical source definition ordinal (`Provenance.definitionSlot`). -/
  definitionSlot : Nat
  /-- Accepted per-use environment/entry specialization slot (`Use.slot`). -/
  specializationSlot : Nat
  /-- Exact import-resolution snapshot bytes, empty only for no imports. -/
  dependencySnapshotBytes : List UInt8
  /-- Exact direct dependency slots and rank for this root or selected entry. -/
  dependencySlots : List Nat
  closureRank : Nat
  input : List NobleContracts.Ty
  output : List NobleContracts.Ty
  embeddedBodyBytes : List UInt8
  body : List NobleContracts.NamedV1.Op

def bindingContinuation (binding : ProgramBinding) : ContinuationSlot :=
  ⟨binding.rootIndex, binding.environment⟩

def sameIdentityAndSignature (expected actual : ProgramBinding) : Prop :=
  expected.artifactBytes = actual.artifactBytes ∧
  expected.rootIndex = actual.rootIndex ∧
  expected.environment = actual.environment ∧
  expected.provenance = actual.provenance ∧
  expected.definitionSlot = actual.definitionSlot ∧
  expected.specializationSlot = actual.specializationSlot ∧
  expected.dependencySnapshotBytes = actual.dependencySnapshotBytes ∧
  expected.dependencySlots = actual.dependencySlots ∧
  expected.closureRank = actual.closureRank ∧
  expected.input = actual.input ∧
  expected.output = actual.output

def SameSignature (expected actual : ProgramBinding) : Prop :=
  expected.input = actual.input ∧ expected.output = actual.output

/-- Exact source/dependency/ABI identity plus the exact checked embedded body.
Signature equality alone is intentionally weaker. -/
def exactEmbeddedDefinition (expected actual : ProgramBinding) : Prop :=
  sameIdentityAndSignature expected actual ∧
  expected.embeddedBodyBytes = actual.embeddedBodyBytes ∧
  expected.body = actual.body

def EntryMatchesBinding (entry : NobleContracts.NamedV1.Entry) (binding : ProgramBinding) : Prop :=
  entry.slot = binding.specializationSlot ∧
  entry.origin = binding.provenance ∧
  entry.input = binding.input ∧
  entry.output = binding.output ∧
  entry.body = binding.body ∧
  entry.dependencies = binding.dependencySlots ∧
  entry.rank = binding.closureRank

def pilotCalleeBinding (artifact : List UInt8) (root environment : UInt32) :
    ProgramBinding :=
  { artifactBytes := artifact
    rootIndex := root
    environment := environment
    provenance := calleeProvenance
    definitionSlot := 0
    specializationSlot := 0
    dependencySnapshotBytes := []
    dependencySlots := []
    closureRank := 0
    input := [.i64, .i64]
    output := [.i64, .i64]
    embeddedBodyBytes := sourceBytes calleeDefinitionSource
    body := splitBody }

def pilotCallerBinding (artifact : List UInt8) (root environment : UInt32) :
    ProgramBinding :=
  { artifactBytes := artifact
    rootIndex := root
    environment := environment
    provenance := callerProvenance
    definitionSlot := 0
    specializationSlot := 0
    dependencySnapshotBytes := sourceBytes importSource
    dependencySlots := callerDependencies
    closureRank := 1
    input := [.i64, .i64]
    output := [.i64]
    embeddedBodyBytes := sourceBytes callerDefinitionSource
    body := callerRoot }

def wrongBodyBinding (artifact : List UInt8) (root environment : UInt32) :
    ProgramBinding :=
  { pilotCalleeBinding artifact root environment with
    embeddedBodyBytes := sourceBytes wrongCalleeDefinitionSource
    body := [.word 5, .lit (.i64 (8 : BitVec 64))] }

def changedVersionBinding (artifact : List UInt8) (root environment : UInt32) :
    ProgramBinding :=
  { pilotCalleeBinding artifact root environment with
    provenance :=
      { calleeProvenance with
        moduleVersion := 2
        moduleSource := sourceBytes
          "module LinkCallee@2 [ export split def split [ - 7 ] ]\n" } }

def changedImportSnapshotBinding (artifact : List UInt8)
    (root environment : UInt32) : ProgramBinding :=
  { pilotCallerBinding artifact root environment with
    dependencySnapshotBytes := sourceBytes importSource ++ [1] }

def postCheckSubstitutedBinding (artifact : List UInt8) (root environment : UInt32) :
    ProgramBinding :=
  { pilotCalleeBinding (artifact ++ [1]) root environment with
    embeddedBodyBytes := sourceBytes "embedded callee bytes changed after check"
    body := [.word 5, .lit (.i64 (8 : BitVec 64))] }

def tableUnambiguous (table : List ProgramBinding) : Prop :=
  ∀ ⦃left right⦄, left ∈ table → right ∈ table →
    bindingContinuation left = bindingContinuation right → left = right

theorem same_identity_required
    {table : List ProgramBinding} {expected actual : ProgramBinding}
    (hunique : tableUnambiguous table)
    (hexpected : expected ∈ table) (hactual : actual ∈ table)
    (hkey : bindingContinuation expected = bindingContinuation actual) :
    exactEmbeddedDefinition expected actual := by
  have hrow := hunique hexpected hactual hkey
  subst actual
  exact ⟨⟨rfl, rfl, rfl, rfl, rfl, rfl, rfl, rfl, rfl, rfl, rfl⟩, rfl, rfl⟩

/-- A matched exact source identity carries the pinned dependency version. -/
theorem dependency_version_identity_exact
    (expected actual : ProgramBinding)
    (h : sameIdentityAndSignature expected actual) :
    expected.provenance.moduleVersion = actual.provenance.moduleVersion := by
  rcases h with ⟨_, _, _, hprovenance, _, _, _, _, _, _, _⟩
  exact congrArg NobleContracts.NamedV1.Provenance.moduleVersion hprovenance

theorem dependency_snapshot_identity_exact
    (expected actual : ProgramBinding)
    (h : sameIdentityAndSignature expected actual) :
    expected.dependencySnapshotBytes = actual.dependencySnapshotBytes := by
  rcases h with ⟨_, _, _, _, _, _, hdependencies, _, _, _, _⟩
  exact hdependencies

/-- Changing the imported module version cannot reuse the checked callee
binding, even while the visible definition body and signature stay the same. -/
theorem dependency_version_change_refused
    (artifact : List UInt8) (root environment : UInt32) :
    ¬ exactEmbeddedDefinition (pilotCalleeBinding artifact root environment)
      (changedVersionBinding artifact root environment) := by
  intro h
  have hversion := dependency_version_identity_exact
    (pilotCalleeBinding artifact root environment)
    (changedVersionBinding artifact root environment) h.1
  change 1 = 2 at hversion
  exact (by decide : (1 : Nat) ≠ 2) hversion

theorem dependency_snapshot_change_refused
    (artifact : List UInt8) (root environment : UInt32) :
    ¬ sameIdentityAndSignature (pilotCallerBinding artifact root environment)
      (changedImportSnapshotBinding artifact root environment) := by
  intro h
  have hdependencies := dependency_snapshot_identity_exact
    (pilotCallerBinding artifact root environment)
    (changedImportSnapshotBinding artifact root environment) h
  have hlength := congrArg List.length hdependencies
  simp [pilotCallerBinding, changedImportSnapshotBinding] at hlength

/-- A same-signature replacement body is rejected by the exact embedded
definition binding. -/
theorem same_signature_wrong_body_refused
    (artifact : List UInt8) (root environment : UInt32) :
    sameIdentityAndSignature (pilotCalleeBinding artifact root environment)
      (wrongBodyBinding artifact root environment) ∧
    ¬ exactEmbeddedDefinition (pilotCalleeBinding artifact root environment)
      (wrongBodyBinding artifact root environment) := by
  constructor
  · simp [sameIdentityAndSignature, pilotCalleeBinding, wrongBodyBinding,
      calleeProvenance]
  · intro h
    have hbody := h.2.2
    simp [pilotCalleeBinding, wrongBodyBinding, splitBody] at hbody

/-- A byte substitution after checking is refused even though the selected
root, definition name, source version and interface were originally checked. -/
theorem embedded_callee_substitution_refused
    (artifact : List UInt8) (root environment : UInt32) :
    SameSignature (pilotCalleeBinding artifact root environment)
      (postCheckSubstitutedBinding artifact root environment) ∧
    ¬ exactEmbeddedDefinition (pilotCalleeBinding artifact root environment)
      (postCheckSubstitutedBinding artifact root environment) := by
  constructor
  · simp [SameSignature, pilotCalleeBinding, postCheckSubstitutedBinding]
  · intro h
    have hbody := h.2.2
    simp [pilotCalleeBinding, postCheckSubstitutedBinding, splitBody] at hbody

/-- Same compiled artifact is an explicit premise for the supported internal
dispatch path; this does not model independently loaded module ABIs. -/
def SameCompiledArtifact (caller callee : ProgramBinding) : Prop :=
  caller.artifactBytes = callee.artifactBytes

theorem pilot_bindings_share_artifact
    (artifact : List UInt8) (callerRoot callerEnvironment calleeRoot calleeEnvironment : UInt32) :
    SameCompiledArtifact
      (pilotCallerBinding artifact callerRoot callerEnvironment)
      (pilotCalleeBinding artifact calleeRoot calleeEnvironment) := rfl

theorem pilot_entry_selected_binding
    (artifact : List UInt8) (root environment : UInt32) :
    EntryMatchesBinding splitEntry (pilotCalleeBinding artifact root environment) := by
  simp [EntryMatchesBinding, splitEntry, pilotCalleeBinding, calleeProvenance]

theorem pilot_caller_root_binding
    (artifact : List UInt8) (root environment : UInt32) :
    (pilotCallerBinding artifact root environment).body = callerRoot ∧
    (pilotCallerBinding artifact root environment).provenance = callerProvenance ∧
    (pilotCallerBinding artifact root environment).dependencySnapshotBytes =
      sourceBytes importSource := by
  simp [pilotCallerBinding]

theorem pilot_caller_binding_closure
    (artifact : List UInt8) (root environment : UInt32) :
    NobleContracts.NamedV1.ExactClosure pilotEnvironment
      (pilotCallerBinding artifact root environment).provenance
      (pilotCallerBinding artifact root environment).body
      (pilotCallerBinding artifact root environment).dependencySlots
      (pilotCallerBinding artifact root environment).closureRank := by
  exact pilot_closure

theorem wrong_return_target_refused :
    (⟨17, 3⟩ : ContinuationSlot) ≠ ⟨18, 3⟩ := by
  decide

structure EngineAuthority where
  engineOwner : Nat
  callerOwner : Nat
  calleeOwner : Nat
  importedMemoryOwner : Nat
  importedTableOwner : Nat
  importedGlobalsOwner : Nat

/-- The caller, callee, private imported memory, table and globals are all
owned by this engine for the modeled interval. -/
def ExclusiveEngineOwnership (authority : EngineAuthority) : Prop :=
  authority.callerOwner = authority.engineOwner ∧
  authority.calleeOwner = authority.engineOwner ∧
  authority.importedMemoryOwner = authority.engineOwner ∧
  authority.importedTableOwner = authority.engineOwner ∧
  authority.importedGlobalsOwner = authority.engineOwner

structure EngineState where
  /-- Internal top-first view; physical memory order is `operands.reverse`. -/
  operands : List OperandCell
  sp : Nat
  /-- Dispatch order is LIFO at the head; physical memory order is reverse. -/
  continuations : List ContinuationSlot
  cp : Nat
  table : List ProgramBinding

def EngineCountersConsistent (state : EngineState) : Prop :=
  state.sp = state.operands.length ∧ state.cp = state.continuations.length

def enqueueOne (state : EngineState) (continuation : ContinuationSlot) : EngineState :=
  { state with
    continuations := continuation :: state.continuations
    cp := state.cp + 1 }

/-- The caller continuation is enqueued first, then the embedded callee. -/
def scheduleNamedCall (state : EngineState)
    (caller callee : ContinuationSlot) : EngineState :=
  enqueueOne (enqueueOne state caller) callee

def dispatchNext (state : EngineState) : Option (ContinuationSlot × EngineState) :=
  match state.continuations with
  | [] => none
  | continuation :: rest =>
      some (continuation,
        { state with continuations := rest, cp := state.cp - 1 })

def afterCalleeDispatch (state : EngineState)
    (caller : ContinuationSlot) : EngineState :=
  { state with
    continuations := caller :: state.continuations
    cp := state.cp + 1 }

def afterCallerDispatch (state : EngineState)
    (pending : List ContinuationSlot) : EngineState :=
  { state with continuations := pending, cp := state.cp - 1 }

theorem return_target_exact
    (state : EngineState) (caller callee : ContinuationSlot) :
    dispatchNext (scheduleNamedCall state caller callee) =
        some (callee, afterCalleeDispatch state caller) ∧
    dispatchNext (afterCalleeDispatch state caller) = some (caller, state) := by
  constructor <;> simp [dispatchNext, scheduleNamedCall, enqueueOne,
    afterCalleeDispatch]

def decodeCell (cell : OperandCell) : Option NobleContracts.NamedV1.Value :=
  if cell.tag == i64Tag then some (.i64 cell.payload)
  else if cell.tag == boolTag then
    if cell.payload == 0 then some (.bool false)
    else if cell.payload == 1 then some (.bool true)
    else none
  else none

def decodeCells : List OperandCell → Option NobleContracts.NamedV1.Stack
  | [] => some []
  | cell :: rest => do
      let value ← decodeCell cell
      let values ← decodeCells rest
      pure (value :: values)

/-- Relation from the internal top-first slot view to NamedV1 values for the
I64 operands and the preserved Bool/I64 prefix used by the executable pilot.
Other tags/heap references are outside this pure-scalar boundary. -/
def runtimeRepresents (state : EngineState) (namedInternal : NobleContracts.NamedV1.Stack) : Prop :=
  state.sp = state.operands.length ∧
  decodeCells state.operands = some namedInternal

theorem bool_cell_decodes (value : Bool) :
    decodeCell (boolCell value) = some (.bool value) := by
  cases value <;> simp [decodeCell, boolCell, boolTag, i64Tag]

theorem i64_cell_decodes (value : BitVec 64) :
    decodeCell (i64Cell value) = some (.i64 value) := by
  simp [decodeCell, i64Cell, i64Tag]

theorem bool_prefix_typed (value : Bool) :
    NobleContracts.NamedV1.HasType pilotEnvironment (.bool value) .bool := NobleContracts.NamedV1.HasType.bool

theorem pilot_bool_input_values_typed (boolPrefix : Bool) (x y : BitVec 64) :
    NobleContracts.NamedV1.HasType pilotEnvironment (.bool boolPrefix) .bool ∧
    NobleContracts.NamedV1.HasType pilotEnvironment (.i64 x) .i64 ∧
    NobleContracts.NamedV1.HasType pilotEnvironment (.i64 y) .i64 :=
  ⟨NobleContracts.NamedV1.HasType.bool, NobleContracts.NamedV1.HasType.i64, NobleContracts.NamedV1.HasType.i64⟩

/-! ## Scalar helper model and component simulation premises -/

inductive ScalarInstruction where
  | pushI64 : BitVec 64 → ScalarInstruction
  | sub
  deriving DecidableEq, Repr

/-- Successful scalar-only runtime transitions. The fixed suffix is the
protected bottom-first operand prefix, represented here at the end of the
internal top-first list. Subtraction pops top/right then next/left and computes
left-right; a pushed result uses the cleared padding word. Failure, quota and
termination behavior are outside this relation. -/
inductive RuntimeStep (protectedTail : List OperandCell) :
    ScalarInstruction → EngineState → EngineState → Prop where
  | push {state active value padding}
      (hstack : state.operands = active ++ protectedTail)
      (hsp : state.sp = state.operands.length) :
      RuntimeStep protectedTail (.pushI64 value) state
        { state with
          operands := { tag := i64Tag, padding := padding, payload := value } :: state.operands
          sp := state.sp + 1 }
  | sub {state right left active}
      (hstack : state.operands = right :: left :: (active ++ protectedTail))
      (hsp : state.sp = state.operands.length)
      (hright : right.tag = i64Tag) (hleft : left.tag = i64Tag) :
      RuntimeStep protectedTail .sub state
        { state with
          operands := i64Cell (left.payload - right.payload) :: (active ++ protectedTail)
          sp := state.sp - 1 }

inductive RuntimeRun (protectedTail : List OperandCell) :
    List ScalarInstruction → EngineState → EngineState → Prop where
  | nil {state} (hstack : ∃ active, state.operands = active ++ protectedTail) :
      RuntimeRun protectedTail [] state state
  | cons {instruction code before middle after} :
      RuntimeStep protectedTail instruction before middle →
      RuntimeRun protectedTail code middle after →
      RuntimeRun protectedTail (instruction :: code) before after

private theorem runtime_step_control_preserved
    {protectedTail : List OperandCell} {instruction : ScalarInstruction}
    {before after : EngineState}
    (h : RuntimeStep protectedTail instruction before after) :
    after.continuations = before.continuations ∧
    after.cp = before.cp ∧ after.table = before.table := by
  cases h <;> simp_all

theorem runtime_run_preserves_control
    {protectedTail : List OperandCell} {code : List ScalarInstruction}
    {before after : EngineState}
    (h : RuntimeRun protectedTail code before after) :
    after.continuations = before.continuations ∧
    after.cp = before.cp ∧ after.table = before.table := by
  induction h with
  | nil _ => exact ⟨rfl, rfl, rfl⟩
  | cons hstep _ ih =>
      obtain ⟨hcont, hcp, htable⟩ := runtime_step_control_preserved hstep
      exact ⟨ih.1.trans hcont, ih.2.1.trans hcp, ih.2.2.trans htable⟩

theorem enqueueOne_preserves_counters (state : EngineState)
    (continuation : ContinuationSlot)
    (hwf : EngineCountersConsistent state) :
    EngineCountersConsistent (enqueueOne state continuation) := by
  rcases hwf with ⟨hsp, hcp⟩
  simp [EngineCountersConsistent, enqueueOne, hsp, hcp]

theorem return_target_exact_after_run
    {protectedTail : List OperandCell} {code : List ScalarInstruction}
    {before after : EngineState} (h : RuntimeRun protectedTail code before after)
    (caller : ContinuationSlot) (pending : List ContinuationSlot)
    (hqueued : before.continuations = caller :: pending)
    (hcp : before.cp = pending.length + 1) :
    dispatchNext after =
      some (caller, afterCallerDispatch after pending) := by
  obtain ⟨hcont, hcount, _⟩ := runtime_run_preserves_control h
  have hfinalQueue : after.continuations = caller :: pending := hcont.trans hqueued
  have hfinalCount : after.cp = pending.length + 1 := hcount.trans hcp
  simp [dispatchNext, afterCallerDispatch, hfinalQueue, hfinalCount]

theorem runtime_step_preserves_sp_count
    {protectedTail : List OperandCell} {instruction : ScalarInstruction}
    {before after : EngineState}
    (h : RuntimeStep protectedTail instruction before after) :
    after.sp = after.operands.length := by
  cases h with
  | push hstack hbefore =>
      simp [hstack, hbefore, List.length_append]
  | sub hstack hbefore _ _ =>
      simp [hstack, hbefore, List.length_append] <;> omega

theorem runtime_run_preserves_sp_count
    {protectedTail : List OperandCell} {code : List ScalarInstruction}
    {before after : EngineState}
    (h : RuntimeRun protectedTail code before after)
    (hsp : before.sp = before.operands.length) :
    after.sp = after.operands.length := by
  revert hsp
  induction h with
  | nil _ => intro hsp; exact hsp
  | cons hstep _ ih =>
      intro hsp
      exact ih (runtime_step_preserves_sp_count hstep)

theorem runtime_run_preserves_counters
    {protectedTail : List OperandCell} {code : List ScalarInstruction}
    {before after : EngineState}
    (h : RuntimeRun protectedTail code before after)
    (hwf : EngineCountersConsistent before) :
    EngineCountersConsistent after := by
  rcases hwf with ⟨hsp, hcp⟩
  obtain ⟨hcontinuations, hcount, _⟩ := runtime_run_preserves_control h
  exact ⟨runtime_run_preserves_sp_count h hsp,
    hcount.trans (hcp.trans (congrArg List.length hcontinuations).symm)⟩

theorem runtime_run_preserves_protected_suffix
    {protectedTail : List OperandCell} {code : List ScalarInstruction}
    {before after : EngineState}
    (h : RuntimeRun protectedTail code before after) :
    ∃ active, after.operands = active ++ protectedTail := by
  induction h with
  | nil hstack => exact hstack
  | cons _ _ ih => exact ih

private def scalarInstruction : NobleContracts.NamedV1.Op → Option ScalarInstruction
  | .lit (.i64 value) => some (.pushI64 value)
  | .word 5 => some .sub
  | _ => none

def lowerScalarCode : List NobleContracts.NamedV1.Op → Option (List ScalarInstruction)
  | [] => some []
  | op :: rest => do
      let instruction ← scalarInstruction op
      let loweredRest ← lowerScalarCode rest
      pure (instruction :: loweredRest)

def pilotCalleeRuntimeCode : List ScalarInstruction :=
  [.sub, .pushI64 (7 : BitVec 64)]

def pilotCallerPostRuntimeCode : List ScalarInstruction := [.sub]

def pilotRuntimeState (operands : List OperandCell)
    (continuations : List ContinuationSlot) (table : List ProgramBinding) :
    EngineState :=
  { operands := operands
    sp := operands.length
    continuations := continuations
    cp := continuations.length
    table := table }

def pilotRuntimeEntryState (boolPrefix : Bool) (x y : BitVec 64)
    (continuations : List ContinuationSlot) (table : List ProgramBinding) :
    EngineState :=
  pilotRuntimeState [i64Cell y, i64Cell x, boolCell boolPrefix] continuations table

def pilotRuntimeCalleeState (boolPrefix : Bool) (x y : BitVec 64)
    (continuations : List ContinuationSlot) (table : List ProgramBinding) :
    EngineState :=
  pilotRuntimeState [i64Cell 7, i64Cell (x - y), boolCell boolPrefix]
    continuations table

def pilotRuntimeFinalState (boolPrefix : Bool) (x y : BitVec 64)
    (continuations : List ContinuationSlot) (table : List ProgramBinding) :
    EngineState :=
  pilotRuntimeState [i64Cell (x - y - 7), boolCell boolPrefix]
    continuations table

theorem pilot_runtime_entry_boundary (boolPrefix : Bool) (x y : BitVec 64)
    (continuations : List ContinuationSlot) (table : List ProgramBinding) :
    runtimeRepresents (pilotRuntimeEntryState boolPrefix x y continuations table)
      [.i64 y, .i64 x, .bool boolPrefix] := by
  cases boolPrefix <;> simp [runtimeRepresents, pilotRuntimeEntryState,
    pilotRuntimeState, decodeCells, decodeCell, i64Cell, boolCell, i64Tag, boolTag]

theorem pilot_runtime_callee_boundary (boolPrefix : Bool) (x y : BitVec 64)
    (continuations : List ContinuationSlot) (table : List ProgramBinding) :
    runtimeRepresents (pilotRuntimeCalleeState boolPrefix x y continuations table)
      [.i64 7, .i64 (x - y), .bool boolPrefix] := by
  cases boolPrefix <;> simp [runtimeRepresents, pilotRuntimeCalleeState,
    pilotRuntimeState, decodeCells, decodeCell, i64Cell, boolCell, i64Tag, boolTag]

theorem pilot_runtime_final_boundary (boolPrefix : Bool) (x y : BitVec 64)
    (continuations : List ContinuationSlot) (table : List ProgramBinding) :
    runtimeRepresents (pilotRuntimeFinalState boolPrefix x y continuations table)
      [.i64 (x - y - 7), .bool boolPrefix] := by
  cases boolPrefix <;> simp [runtimeRepresents, pilotRuntimeFinalState,
    pilotRuntimeState, decodeCells, decodeCell, i64Cell, boolCell, i64Tag, boolTag]

theorem pilot_runtime_callee_run (boolPrefix : Bool) (x y : BitVec 64)
    (continuations : List ContinuationSlot) (table : List ProgramBinding) :
    RuntimeRun [boolCell boolPrefix] pilotCalleeRuntimeCode
      (pilotRuntimeEntryState boolPrefix x y continuations table)
      (pilotRuntimeCalleeState boolPrefix x y continuations table) := by
  unfold pilotCalleeRuntimeCode
  apply RuntimeRun.cons
  · refine RuntimeStep.sub (right := i64Cell y) (left := i64Cell x) (active := []) ?_ ?_ ?_ ?_
    · simp [pilotRuntimeEntryState, pilotRuntimeState]
    · simp [pilotRuntimeEntryState, pilotRuntimeState]
    · simp [i64Cell, i64Tag]
    · simp [i64Cell, i64Tag]
  · apply RuntimeRun.cons
    · refine RuntimeStep.push (active := [i64Cell (x - y)]) (value := 7)
        (padding := 0) ?_ ?_
      · simp [i64Cell, boolCell]
      · simp [pilotRuntimeEntryState, pilotRuntimeState, i64Cell, boolCell]
    · apply RuntimeRun.nil
      refine ⟨[i64Cell 7, i64Cell (x - y)], ?_⟩
      simp [i64Cell, boolCell]

theorem pilot_runtime_caller_post_run (boolPrefix : Bool) (x y : BitVec 64)
    (continuations : List ContinuationSlot) (table : List ProgramBinding) :
    RuntimeRun [boolCell boolPrefix] pilotCallerPostRuntimeCode
      (pilotRuntimeCalleeState boolPrefix x y continuations table)
      (pilotRuntimeFinalState boolPrefix x y continuations table) := by
  unfold pilotCallerPostRuntimeCode
  apply RuntimeRun.cons
  · refine RuntimeStep.sub (right := i64Cell 7) (left := i64Cell (x - y))
      (active := []) ?_ ?_ ?_ ?_
    · simp [pilotRuntimeCalleeState, pilotRuntimeState, i64Cell, boolCell]
    · simp [pilotRuntimeCalleeState, pilotRuntimeState, i64Cell, boolCell]
    · simp [i64Cell, i64Tag]
    · simp [i64Cell, i64Tag]
  · apply RuntimeRun.nil
    refine ⟨[i64Cell (x - y - 7)], ?_⟩
    simp [i64Cell, boolCell]

theorem pilot_runtime_two_results_and_caller_law (boolPrefix : Bool) (x y : BitVec 64)
    (continuations : List ContinuationSlot) (table : List ProgramBinding) :
    RuntimeRun [boolCell boolPrefix] pilotCalleeRuntimeCode
      (pilotRuntimeEntryState boolPrefix x y continuations table)
      (pilotRuntimeCalleeState boolPrefix x y continuations table) ∧
    RuntimeRun [boolCell boolPrefix] pilotCallerPostRuntimeCode
      (pilotRuntimeCalleeState boolPrefix x y continuations table)
      (pilotRuntimeFinalState boolPrefix x y continuations table) ∧
    runtimeRepresents (pilotRuntimeFinalState boolPrefix x y continuations table)
      [.i64 (x - y - 7), .bool boolPrefix] := by
  exact ⟨pilot_runtime_callee_run boolPrefix x y continuations table,
    pilot_runtime_caller_post_run boolPrefix x y continuations table,
    pilot_runtime_final_boundary boolPrefix x y continuations table⟩

theorem pilot_callee_lowering :
    lowerScalarCode splitBody = some pilotCalleeRuntimeCode := by
  simp [lowerScalarCode, scalarInstruction, splitBody, pilotCalleeRuntimeCode]

theorem pilot_caller_post_lowering :
    lowerScalarCode [.word 5] = some pilotCallerPostRuntimeCode := by
  rfl

/-- Conditional horizontal link theorem. Source `pre`, the resolved callee
body, and source `post` each have their own NamedV1 and scalar-runtime segment
proofs and boundary representations. The call protocol enqueues caller then
callee, dispatches the exact embedded callee key, and after its normal return
dispatches the exact caller continuation. The theorem is conditional on the
same-artifact table binding, imported-state exclusive ownership, and segment
mapping premises; it is not a lowering or loader theorem. -/
theorem horizontal_runtime_link
    {env : NobleContracts.NamedV1.Environment} {pre post : List NobleContracts.NamedV1.Op}
    {use : NobleContracts.NamedV1.Use} {entry : NobleContracts.NamedV1.Entry}
    {sourceBefore sourceMiddle sourceAfterCall sourceFinal : NobleContracts.NamedV1.Stack}
    {protectedTail : List OperandCell}
    {preCode calleeCode postCode : List ScalarInstruction}
    {initial afterPre afterCallee final : EngineState}
    {expectedCaller actualCaller expectedCallee actualCallee : ProgramBinding}
    (authority : EngineAuthority)
    (pending : List ContinuationSlot)
    (howner : ExclusiveEngineOwnership authority)
    (hartifact : SameCompiledArtifact expectedCaller expectedCallee)
    (hlookup : env.lookup use.slot = some entry)
    (hmatch : entry.matches use)
    (hpreNamed : NobleContracts.NamedV1.Run env pre sourceBefore sourceMiddle)
    (hcalleeNamed : NobleContracts.NamedV1.Run env entry.body sourceMiddle sourceAfterCall)
    (hpostNamed : NobleContracts.NamedV1.Run env post sourceAfterCall sourceFinal)
    (hcallerClosure : NobleContracts.NamedV1.ExactClosure env expectedCaller.provenance
      expectedCaller.body expectedCaller.dependencySlots expectedCaller.closureRank)
    (hcallerBody : expectedCaller.body = pre ++ [.call use] ++ post)
    (hcallerSource : expectedCaller.provenance = use.caller)
    (hcalleeEntry : EntryMatchesBinding entry expectedCallee)
    (hcallerInput : expectedCaller.input = [.i64, .i64])
    (hcallerOutput : expectedCaller.output = [.i64])
    (hcallerMap : lowerScalarCode pre = some preCode)
    (hcalleeMap : lowerScalarCode entry.body = some calleeCode)
    (hpostMap : lowerScalarCode post = some postCode)
    (hunique : tableUnambiguous initial.table)
    (hexpectedCaller : expectedCaller ∈ initial.table)
    (hactualCaller : actualCaller ∈ initial.table)
    (hcallerKey : bindingContinuation expectedCaller = bindingContinuation actualCaller)
    (hexpectedCallee : expectedCallee ∈ initial.table)
    (hactualCallee : actualCallee ∈ initial.table)
    (hcalleeKey : bindingContinuation expectedCallee = bindingContinuation actualCallee)
    (hinitialQueue : initial.continuations = pending)
    (hinitialCount : initial.cp = pending.length)
    (hinitialBoundary : runtimeRepresents initial sourceBefore)
    (hpreBoundary : runtimeRepresents afterPre sourceMiddle)
    (hcalleeBoundary : runtimeRepresents afterCallee sourceAfterCall)
    (hfinalBoundary : runtimeRepresents final sourceFinal)
    (hpreRuntime : RuntimeRun protectedTail preCode initial afterPre)
    (hcalleeRuntime : RuntimeRun protectedTail calleeCode
      (afterCalleeDispatch afterPre (bindingContinuation expectedCaller)) afterCallee)
    (hpostRuntime : RuntimeRun protectedTail postCode
      (afterCallerDispatch afterCallee pending) final) :
    NobleContracts.NamedV1.Run env (pre ++ [.call use] ++ post) sourceBefore sourceFinal ∧
    exactEmbeddedDefinition expectedCaller actualCaller ∧
    exactEmbeddedDefinition expectedCallee actualCallee ∧
    NobleContracts.NamedV1.ExactClosure env expectedCaller.provenance expectedCaller.body
      expectedCaller.dependencySlots expectedCaller.closureRank ∧
    runtimeRepresents initial sourceBefore ∧
    runtimeRepresents afterPre sourceMiddle ∧
    runtimeRepresents afterCallee sourceAfterCall ∧
    runtimeRepresents final sourceFinal ∧
    dispatchNext (scheduleNamedCall afterPre
      (bindingContinuation expectedCaller) (bindingContinuation expectedCallee)) =
        some (bindingContinuation expectedCallee,
          afterCalleeDispatch afterPre (bindingContinuation expectedCaller)) ∧
    dispatchNext afterCallee =
      some (bindingContinuation expectedCaller,
        afterCallerDispatch afterCallee pending) ∧
    final.continuations = initial.continuations ∧
    final.cp = initial.cp ∧ final.table = initial.table ∧
    EngineCountersConsistent final ∧
    (∃ active, final.operands = active ++ protectedTail) ∧
    ExclusiveEngineOwnership authority ∧
    SameCompiledArtifact expectedCaller expectedCallee ∧
    SameCompiledArtifact actualCaller actualCallee ∧
    expectedCaller.provenance = use.caller ∧
    expectedCaller.input = [.i64, .i64] ∧
    expectedCaller.output = [.i64] ∧
    expectedCaller.body = pre ++ [.call use] ++ post ∧
    EntryMatchesBinding entry expectedCallee ∧
    lowerScalarCode pre = some preCode ∧
    lowerScalarCode entry.body = some calleeCode ∧
    lowerScalarCode post = some postCode := by
  have hsource := horizontal_compose hlookup hmatch hpreNamed hcalleeNamed hpostNamed
  have hcallerExact := same_identity_required hunique hexpectedCaller hactualCaller hcallerKey
  have hcalleeExact := same_identity_required hunique hexpectedCallee hactualCallee hcalleeKey
  have hactualArtifact : SameCompiledArtifact actualCaller actualCallee := by
    unfold SameCompiledArtifact
    exact hcallerExact.1.1.symm.trans (hartifact.trans hcalleeExact.1.1)
  have hpreControl := runtime_run_preserves_control hpreRuntime
  have hcalleeControl := runtime_run_preserves_control hcalleeRuntime
  have hpostControl := runtime_run_preserves_control hpostRuntime
  have hpreQueue : afterPre.continuations = pending :=
    hpreControl.1.trans hinitialQueue
  have hpreCount : afterPre.cp = initial.cp := hpreControl.2.1
  have hdispatches := return_target_exact afterPre
    (bindingContinuation expectedCaller) (bindingContinuation expectedCallee)
  have hcalleeQueue : afterCallee.continuations =
      bindingContinuation expectedCaller :: pending := by
    calc
      afterCallee.continuations =
          (afterCalleeDispatch afterPre (bindingContinuation expectedCaller)).continuations :=
        hcalleeControl.1
      _ = bindingContinuation expectedCaller :: pending := by
        simp [afterCalleeDispatch, hpreQueue]
  have hcalleeCount : afterCallee.cp = pending.length + 1 := by
    calc
      afterCallee.cp =
          (afterCalleeDispatch afterPre (bindingContinuation expectedCaller)).cp :=
        hcalleeControl.2.1
      _ = afterPre.cp + 1 := rfl
      _ = initial.cp + 1 := by rw [hpreCount]
      _ = pending.length + 1 := by rw [hinitialCount]
  have hcalleeDispatchQueue :
      (afterCalleeDispatch afterPre (bindingContinuation expectedCaller)).continuations =
        bindingContinuation expectedCaller :: pending := by
    simp [afterCalleeDispatch, hpreQueue]
  have hcalleeDispatchCount :
      (afterCalleeDispatch afterPre (bindingContinuation expectedCaller)).cp =
        pending.length + 1 := by
    simp [afterCalleeDispatch, hpreCount, hinitialCount]
  have hreturn := return_target_exact_after_run hcalleeRuntime
    (bindingContinuation expectedCaller) pending hcalleeDispatchQueue hcalleeDispatchCount
  have hfinalQueue : final.continuations = initial.continuations := by
    calc
      final.continuations = (afterCallerDispatch afterCallee pending).continuations :=
        hpostControl.1
      _ = pending := rfl
      _ = initial.continuations := hinitialQueue.symm
  have hfinalCount : final.cp = initial.cp := by
    calc
      final.cp = (afterCallerDispatch afterCallee pending).cp := hpostControl.2.1
      _ = afterCallee.cp - 1 := rfl
      _ = pending.length := by rw [hcalleeCount]; omega
      _ = initial.cp := hinitialCount.symm
  have hfinalTable : final.table = initial.table := by
    calc
      final.table = (afterCallerDispatch afterCallee pending).table := hpostControl.2.2
      _ = afterCallee.table := rfl
      _ = (afterCalleeDispatch afterPre (bindingContinuation expectedCaller)).table :=
        hcalleeControl.2.2
      _ = afterPre.table := rfl
      _ = initial.table := hpreControl.2.2
  have hfinalCounters : EngineCountersConsistent final := by
    refine ⟨hfinalBoundary.1, ?_⟩
    calc
      final.cp = initial.cp := hfinalCount
      _ = pending.length := hinitialCount
      _ = final.continuations.length := by
        rw [hfinalQueue, hinitialQueue]
  exact ⟨hsource, hcallerExact, hcalleeExact, hcallerClosure,
    hinitialBoundary, hpreBoundary,
    hcalleeBoundary, hfinalBoundary, hdispatches.1, hreturn, hfinalQueue,
    hfinalCount, hfinalTable, hfinalCounters,
    runtime_run_preserves_protected_suffix hpostRuntime,
    howner, hartifact, hactualArtifact, hcallerSource, hcallerInput, hcallerOutput, hcallerBody,
    hcalleeEntry, hcallerMap, hcalleeMap, hpostMap⟩

/-! ## Layered simulation composition -/

inductive Steps {State : Type} (step : State → State → Prop) : State → State → Prop where
  | refl (state) : Steps step state state
  | cons {first middle last} :
      step first middle → Steps step middle last → Steps step first last

def ForwardSimulation {Source Target : Type}
    (sourceStep : Source → Source → Prop)
    (targetStep : Target → Target → Prop)
    (related : Source → Target → Prop) : Prop :=
  ∀ ⦃source source' target⦄, related source target →
    sourceStep source source' →
    ∃ target', Steps targetStep target target' ∧ related source' target'

private theorem steps_trans
    {State : Type} {step : State → State → Prop} {a b c : State}
    (hab : Steps step a b) (hbc : Steps step b c) : Steps step a c := by
  induction hab with
  | refl => exact hbc
  | cons hstep _ ih => exact .cons hstep (ih hbc)

private theorem forward_simulation_steps
    {Source Target : Type}
    {sourceStep : Source → Source → Prop}
    {targetStep : Target → Target → Prop}
    {related : Source → Target → Prop}
    (simulation : ForwardSimulation sourceStep targetStep related)
    {source source' target}
    (hrelated : related source target)
    (hsteps : Steps sourceStep source source') :
    ∃ target', Steps targetStep target target' ∧ related source' target' := by
  induction hsteps generalizing target with
  | refl => exact ⟨target, .refl target, hrelated⟩
  | cons hstep hrest ih =>
      obtain ⟨middle, hfirst, hmid⟩ := simulation hrelated hstep
      obtain ⟨last, hlast, hfinal⟩ := ih hmid
      exact ⟨last, steps_trans hfirst hlast, hfinal⟩

/-- Vertical composition composes two local forward simulations by lifting
each finite intermediate-model segment. It assumes no whole-program output
claim and is independent of the horizontal named-call rule. -/
theorem vertical_simulation_compose
    {A B C : Type}
    {stepA : A → A → Prop} {stepB : B → B → Prop}
    {stepC : C → C → Prop}
    {relatedAB : A → B → Prop} {relatedBC : B → C → Prop}
    (simulationAB : ForwardSimulation stepA stepB relatedAB)
    (simulationBC : ForwardSimulation stepB stepC relatedBC) :
    ForwardSimulation stepA stepC
      (fun a c => ∃ b, relatedAB a b ∧ relatedBC b c) := by
  intro a a' c hrelated hstep
  obtain ⟨b, hab, hbc⟩ := hrelated
  obtain ⟨b', hstepsB, hab'⟩ := simulationAB hab hstep
  obtain ⟨c', hstepsC, hbc'⟩ :=
    forward_simulation_steps simulationBC hbc hstepsB
  exact ⟨c', hstepsC, ⟨b', hab', hbc'⟩⟩

end NobleFoundation.LinkBoundary
