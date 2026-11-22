import NobleContracts.NamedV3
import NobleContracts.Examples

/-!
T2, T4 and T5 restricted to lifted frames
(`.cairn/changes/reusable-contract-calls`, task "Close the amendment's follow-up vocabulary").

`liftMc1Claim` (T4) gives an MC1-proved callee's claim only at tails that are lifted MC1 stacks:
an arbitrary named tail may hold call-bearing Programs that have no MC1 preimage, and the MC1
claim is partial correctness over MC1 tails only, so it says nothing about a run on such a tail.
`callFrame` and `ChainStep` consume the unrestricted `exportedNamedClaim`. This module closes the
gap by restricting both sides to the same tails, as the amendment of 2026-11-20 allows:

* `exportedLiftedClaim` is a named subject's claim over lifted frames only: the frame, the input
  and the result are lifts of MC1 stacks.
* `liftedClaim_of_mc1` (T4, restricted) gives that claim to any subject whose root is the lift of
  MC1 profile code with an MC1 exported claim.
* `liftedCallFrame` (T2, restricted) consumes a callee's lifted claim at a lifted caller frame.
* `liftedChainExec` and `liftedChainClaim` (T5, restricted) run a chain of such calls and of
  caller code segments, and give the caller its own lifted claim, so the route composes.

Lifted chains also generalize T5 in the three ways the amendment lists: caller code segments of
MC1 profile code between calls (`LiftedSegment`, e.g. the `swap` of `[ swap d.sub2 ]`),
frame-carrying steps (each bridge splits off a carried bottom that stays below the step's result),
and postconditions over both the input and the result (`liftedChainClaim` takes one chain per
admitted input). The unrestricted bridge would need an argument that profile code never inspects
the tail below its inputs; it is not claimed here.

These are model theorems over `NamedV1`/`NobleContracts` semantics. They are outside the pinned
`MODEL` import closure, like `Refutation`.
-/

namespace NobleContracts.NamedV3

/-- A lifted frame above a lifted stack is the lift of their concatenation. -/
private theorem lift_append (a b : NobleContracts.Stack) :
    NamedV1.liftStack a ++ NamedV1.liftStack b = NamedV1.liftStack (a ++ b) := by
  simp [NamedV1.liftStack, List.map_append]

/-- A named subject's exported claim restricted to lifted frames: every normal run of its root on
a lifted frame above a lifted, typed input satisfying `pre` returns the same frame above a lifted,
typed result satisfying `post`. A lifted frame is an MC1 stack, so it holds no call-bearing
Program. -/
def exportedLiftedClaim (subject : NamedV1.Subject)
    (pre post : NobleContracts.Stack → NobleContracts.Stack → NobleContracts.Stack → Prop) :
    Prop :=
  ∀ (tail before : NobleContracts.Stack) (final : NamedV1.Stack),
    NobleContracts.StackTyped before subject.input → pre before [] [] →
    NamedV1.Exec subject.env subject.root
      (NamedV1.liftStack tail ++ NamedV1.liftStack before) final →
    ∃ after, final = NamedV1.liftStack tail ++ NamedV1.liftStack after ∧
      NobleContracts.StackTyped after subject.output ∧ post before after []

/-- T4 restricted to lifted frames: the MC1 exported claim of profile code is the lifted claim of
any subject whose root is that code's lift. This is `liftMc1Claim`'s argument, keeping the
result's MC1 typing that a chain's join needs. -/
theorem liftedClaim_of_mc1 {code : List NobleContracts.Op}
    {pre post : NobleContracts.Stack → NobleContracts.Stack → NobleContracts.Stack → Prop}
    (subject : NamedV1.Subject) (hroot : subject.root = NamedV1.liftCode code)
    (profile : ProfileCode code)
    (claim : NobleContracts.exportedClaim code subject.input subject.output [] pre post) :
    exportedLiftedClaim subject pre post := by
  intro tail before final hbefore hpre execution
  rw [hroot, lift_append] at execution
  obtain ⟨after, hfinal, hexec⟩ := exec_reflect profile execution
  obtain ⟨res, hres, htyped, hpost⟩ := claim tail before [] hbefore .nil hpre after hexec
  refine ⟨res, ?_, htyped, hpost⟩
  rw [hfinal, hres, lift_append]

/-- T2 restricted to lifted frames: one call occurrence of a callee with a lifted claim, run on a
lifted caller frame above lifted arguments, returns the frame above a lifted result. -/
theorem liftedCallFrame {caller : NamedV1.Environment} (subject : NamedV1.Subject)
    (entry : NamedV1.Entry) (use : NamedV1.Use)
    (e : EntryEmbedding caller subject entry use)
    (pre post : NobleContracts.Stack → NobleContracts.Stack → NobleContracts.Stack → Prop)
    (claim : exportedLiftedClaim subject pre post)
    (bodies : ∀ row ∈ subject.env.entries, Sequential row.body)
    (profile : Sequential subject.root) :
    ∀ (frame args : NobleContracts.Stack) (final : NamedV1.Stack),
      NobleContracts.StackTyped args subject.input →
      pre args [] [] →
      NamedV1.Exec caller [.call use]
        (NamedV1.liftStack frame ++ NamedV1.liftStack args) final →
      ∃ res, final = NamedV1.liftStack frame ++ NamedV1.liftStack res ∧
        NobleContracts.StackTyped res subject.output ∧ post args res [] := by
  intro frame args final hargs hpre execution
  have run : NamedV1.Run caller [.call use]
      (NamedV1.liftStack frame ++ NamedV1.liftStack args).reverse final.reverse := execution
  cases run with
  | cons step rest =>
      cases step with
      | call lookupCaller matchesCaller inner =>
          cases rest with
          | nil =>
              rw [e.lookup] at lookupCaller
              injection lookupCaller with sameEntry
              rw [← sameEntry, e.body] at inner
              have innerCallee : NamedV1.Run subject.env subject.root
                  (NamedV1.liftStack frame ++ NamedV1.liftStack args).reverse final.reverse :=
                (embed e.embedding (Subject.resolves subject) bodies profile
                  (callsResolve_of_resolves profile (Subject.root_resolves subject))).mpr inner
              exact claim frame args final hargs hpre innerCallee

/-- One call step of a lifted chain: a resolved occurrence of a callee with a lifted claim, plus
the caller's bridge into the callee's precondition and its join out of the postcondition, both
over MC1 stacks. The bridge splits the incoming stack into a carried bottom and the callee's
arguments; the carried values stay below the callee's result (a frame-carrying step). -/
structure LiftedChainStep (caller : NamedV1.Environment)
    (P Q : NobleContracts.Stack → Prop) where
  subject : NamedV1.Subject
  entry : NamedV1.Entry
  use : NamedV1.Use
  embedding : EntryEmbedding caller subject entry use
  bodies : ∀ row ∈ subject.env.entries, Sequential row.body
  profile : Sequential subject.root
  pre : NobleContracts.Stack → NobleContracts.Stack → NobleContracts.Stack → Prop
  post : NobleContracts.Stack → NobleContracts.Stack → NobleContracts.Stack → Prop
  claim : exportedLiftedClaim subject pre post
  /-- the caller's bridge: a carried bottom and the callee's typed arguments -/
  bridge : ∀ s, P s → ∃ carried args, s = carried ++ args ∧ pre args [] [] ∧
    NobleContracts.StackTyped args subject.input
  /-- the caller's join out of the callee postcondition, below the carried values -/
  join : ∀ carried args res, P (carried ++ args) → post args res [] →
    NobleContracts.StackTyped res subject.output → Q (carried ++ res)

/-- A segment of the caller's own code between calls: MC1 profile code (literals and builtins,
e.g. the `swap` of `[ swap d.sub2 ]`) with an MC1 exported claim and the same carried split. -/
structure LiftedSegment (P Q : NobleContracts.Stack → Prop) where
  code : List NobleContracts.Op
  profileCode : ProfileCode code
  inputTypes : List Ty
  outputTypes : List Ty
  pre : NobleContracts.Stack → NobleContracts.Stack → NobleContracts.Stack → Prop
  post : NobleContracts.Stack → NobleContracts.Stack → NobleContracts.Stack → Prop
  claim : NobleContracts.exportedClaim code inputTypes outputTypes [] pre post
  bridge : ∀ s, P s → ∃ carried args, s = carried ++ args ∧ pre args [] [] ∧
    NobleContracts.StackTyped args inputTypes
  join : ∀ carried args res, P (carried ++ args) → post args res [] →
    NobleContracts.StackTyped res outputTypes → Q (carried ++ res)

/-- A chain of lifted call steps and caller code segments over MC1 assertions. -/
inductive LiftedChain (caller : NamedV1.Environment) :
    (NobleContracts.Stack → Prop) → (NobleContracts.Stack → Prop) → Type where
  | nil (P) : LiftedChain caller P P
  | cons {P Q R} : LiftedChainStep caller P Q → LiftedChain caller Q R → LiftedChain caller P R
  | segment {P Q R} : LiftedSegment P Q → LiftedChain caller Q R → LiftedChain caller P R

/-- The caller code a lifted chain justifies: one call per call step and the lifted code of each
segment, in order. -/
def LiftedChain.code {caller : NamedV1.Environment} {P Q : NobleContracts.Stack → Prop} :
    LiftedChain caller P Q → List NamedV1.Op
  | .nil _ => []
  | .cons step rest => .call step.use :: rest.code
  | .segment seg rest => NamedV1.liftCode seg.code ++ rest.code

/-- T5 execution, restricted to lifted frames: a chain of call steps and MC1 code segments
transports the caller's MC1 assertion through the whole sequence at any lifted frame, and the
result is a lifted MC1 stack above the same frame. -/
theorem liftedChainExec {caller : NamedV1.Environment} :
    ∀ {P Q : NobleContracts.Stack → Prop}, (chain : LiftedChain caller P Q) →
      ∀ (tail args : NobleContracts.Stack) (final : NamedV1.Stack), P args →
        NamedV1.Exec caller chain.code (NamedV1.liftStack tail ++ NamedV1.liftStack args) final →
        ∃ res, final = NamedV1.liftStack tail ++ NamedV1.liftStack res ∧ Q res := by
  intro P Q chain
  induction chain with
  | nil P =>
      intro tail args final hP execution
      refine ⟨args, ?_, hP⟩
      have empty : final.reverse = (NamedV1.liftStack tail ++ NamedV1.liftStack args).reverse := by
        have h : NamedV1.Run caller [] (NamedV1.liftStack tail ++ NamedV1.liftStack args).reverse
            final.reverse := by
          simpa [NamedV1.Exec, LiftedChain.code] using execution
        simpa using (run_nil_iff.mp h)
      simpa using congrArg List.reverse empty
  | @cons P Q R step rest ih =>
      intro tail args final hP execution
      obtain ⟨carried, top, hsplit, hpre, htop⟩ := step.bridge args hP
      obtain ⟨mid, hcall, hrest⟩ :=
        (exec_append_iff (a := [NamedV1.Op.call step.use]) (b := LiftedChain.code rest)).mp
          (by simpa [LiftedChain.code] using execution)
      rw [hsplit, ← lift_append carried top, ← List.append_assoc, lift_append tail carried] at hcall
      obtain ⟨res, hmid, htyped, hpost⟩ :=
        liftedCallFrame step.subject step.entry step.use step.embedding step.pre step.post
          step.claim step.bodies step.profile (tail ++ carried) top mid htop hpre hcall
      rw [hsplit] at hP
      have hnext : NamedV1.liftStack tail ++ NamedV1.liftStack (carried ++ res) = mid := by
        rw [hmid]
        simp [NamedV1.liftStack, List.map_append]
      obtain ⟨res', hfinal, hQ⟩ :=
        ih tail (carried ++ res) final (step.join carried top res hP hpost htyped)
          (by rw [hnext]; exact hrest)
      exact ⟨res', hfinal, hQ⟩
  | @segment P Q R seg rest ih =>
      intro tail args final hP execution
      obtain ⟨carried, top, hsplit, hpre, htop⟩ := seg.bridge args hP
      obtain ⟨mid, hseg, hrest⟩ :=
        (exec_append_iff (a := NamedV1.liftCode seg.code) (b := LiftedChain.code rest)).mp
          (by simpa [LiftedChain.code] using execution)
      rw [lift_append tail args] at hseg
      obtain ⟨after, hmid, hexec⟩ := exec_reflect seg.profileCode hseg
      rw [hsplit, ← List.append_assoc] at hexec
      obtain ⟨res, hres, htyped, hpost⟩ :=
        seg.claim (tail ++ carried) top [] htop .nil hpre after hexec
      rw [hsplit] at hP
      have hnext : NamedV1.liftStack tail ++ NamedV1.liftStack (carried ++ res) = mid := by
        rw [hmid, hres]
        simp [NamedV1.liftStack, List.map_append]
      obtain ⟨res', hfinal, hQ⟩ :=
        ih tail (carried ++ res) final (seg.join carried top res hP hpost htyped)
          (by rw [hnext]; exact hrest)
      exact ⟨res', hfinal, hQ⟩

/-- T5 restricted to lifted frames, with a postcondition over both the input and the result: a
subject whose root is the code of a lifted chain from each admitted input `x` to `post x` has the
lifted claim relating every admitted input to its result. The chain may depend on the input; its
code may not. The caller's claim is again a lifted claim, so the caller can itself be a callee. -/
theorem liftedChainClaim (subject : NamedV1.Subject)
    (pre : NobleContracts.Stack → Prop)
    (post : NobleContracts.Stack → NobleContracts.Stack → Prop)
    (chains : ∀ input, LiftedChain subject.env (fun s => s = input ∧ pre input) (post input))
    (root : ∀ input, subject.root = (chains input).code)
    (typed : ∀ input res, pre input → post input res →
      NobleContracts.StackTyped res subject.output) :
    exportedLiftedClaim subject (fun before _ _ => pre before)
      (fun before after _ => post before after) := by
  intro tail before final _ hpre execution
  rw [root before] at execution
  obtain ⟨res, hfinal, hpost⟩ :=
    liftedChainExec (chains before) tail before final ⟨rfl, hpre⟩ execution
  exact ⟨res, hfinal, typed before res hpre hpost, hpost⟩

/-! ## Witnesses

Two caller segments with a postcondition over the input, and the PREUSE-S04 shape
`[ swap d.sub2 ]` as a caller's own lifted claim through a concrete caller table. These are
semantic fixtures, not accepted-source evidence: the provenance records and the table are
assembled by hand. -/

/-- `increment` as a caller segment: it consumes the top `I64` only and carries the value
below it. -/
def incrementSegment (below : NobleContracts.Value) (n : BitVec 64) :
    LiftedSegment (fun s => s = [below, .i64 n]) (fun s => s = [below, .i64 (n + 1)]) where
  code := NobleContracts.increment
  profileCode := by simp [NobleContracts.increment, ProfileCode, ScalarMC1]
  inputTypes := [.i64]
  outputTypes := [.i64]
  pre := fun _ _ _ => True
  post := fun before after _ => ∃ m, before = [.i64 m] ∧ after = [.i64 (m + 1)]
  claim := NobleContracts.increment_exported
  bridge := fun s hs => ⟨[below], [.i64 n], by simp [hs], trivial, .cons .i64 .nil⟩
  join := by
    rintro carried args res hP ⟨m, rfl, rfl⟩ _
    obtain ⟨hc, hm⟩ : carried = [below] ∧ m = n := by
      cases carried with
      | nil => simp at hP
      | cons c cs =>
          cases cs with
          | nil => simp_all
          | cons _ _ => simp at hP
    subst hc hm
    simp

/-- In any caller environment and at any lifted frame, running `increment` twice as caller code
returns the input plus two, and the value below it is carried through both segments. -/
theorem twoIncrementSegments (caller : NamedV1.Environment) (tail : NobleContracts.Stack)
    (below : NobleContracts.Value) (n : BitVec 64) (final : NamedV1.Stack)
    (h : NamedV1.Exec caller
      (NamedV1.liftCode NobleContracts.increment ++ (NamedV1.liftCode NobleContracts.increment ++ []))
      (NamedV1.liftStack tail ++ NamedV1.liftStack [below, .i64 n]) final) :
    final = NamedV1.liftStack tail ++ NamedV1.liftStack [below, .i64 (n + 2)] := by
  obtain ⟨res, hfinal, hres⟩ := liftedChainExec (caller := caller)
    (.segment (incrementSegment below n) (.segment (incrementSegment below (n + 1)) (.nil _)))
    tail [below, .i64 n] final rfl h
  have two : (1 : BitVec 64) + 1 = 2 := by decide
  rw [hfinal, hres, BitVec.add_assoc, two]

/-- `swap` on two `I64` values as an MC1 exported claim. -/
theorem swap_exported :
    NobleContracts.exportedClaim [.word 2] [.i64, .i64] [.i64, .i64] []
      (fun _ _ _ => True)
      (fun before after _ => ∃ x y, before = [.i64 x, .i64 y] ∧ after = [.i64 y, .i64 x]) := by
  intro tail before params hbefore _ _ final execution
  cases hbefore with
  | cons hx rest =>
      cases rest with
      | cons hy done =>
          cases done
          cases hx
          cases hy
          rw [NobleContracts.exec_swap_iff] at execution
          exact ⟨_, execution, .cons .i64 (.cons .i64 .nil), _, _, rfl, rfl⟩

/-- The lifted `increment` body is typed from any stack ending in `I64` to itself. -/
theorem incrementBody_typed (env : NamedV1.Environment) (s : List Ty) :
    NamedV1.CodeTyped env (NamedV1.liftCode NobleContracts.increment) (s ++ [.i64]) (s ++ [.i64]) := by
  simp only [NamedV1.liftCode, NobleContracts.increment, List.map, NamedV1.liftOp,
    NamedV1.liftValue]
  have add : NamedV1.OpTyped env (.word 4) ((s ++ [.i64]) ++ [.i64]) (s ++ [.i64]) := by
    rw [List.append_assoc]
    exact .add
  exact .cons (.lit .i64) (.cons add .nil)

/-- Provenance records for the witness table. -/
def witnessCallee : NamedV1.Provenance :=
  { moduleName := "d", moduleVersion := 1, moduleSource := [], definitionSlot := 0,
    definitionName := "inc", definitionSource := [] }

def witnessCallerProvenance : NamedV1.Provenance :=
  { moduleName := "caller", moduleVersion := 1, moduleSource := [], definitionSlot := 0,
    definitionName := "s04", definitionSource := [] }

/-- `increment` as a callee subject with an empty table: its root is the lift of MC1 code. -/
def incrementSubject : NamedV1.Subject where
  provenance := witnessCallee
  env := { entries := [] }
  root := NamedV1.liftCode NobleContracts.increment
  dependencies := []
  rank := 0
  closure := by
    simp [NamedV1.ExactClosure, NamedV1.liftCode, NobleContracts.increment, NamedV1.liftOp,
      NamedV1.liftValue, NamedV1.usesCode, NamedV1.usesOp, NamedV1.usesValue]
  checkedEntries := by simp
  input := [.i64]
  output := [.i64]
  typed := incrementBody_typed _ []

/-- T4 for the witness callee: its MC1 claim is its lifted claim. -/
theorem incrementSubject_lifted :
    exportedLiftedClaim incrementSubject (fun _ _ _ => True)
      (fun before after _ => ∃ m, before = [.i64 m] ∧ after = [.i64 (m + 1)]) :=
  liftedClaim_of_mc1 incrementSubject rfl
    (by simp [NobleContracts.increment, ProfileCode, ScalarMC1]) NobleContracts.increment_exported

/-- The caller's row for the callee: the accepted body and interface verbatim, at slot 7. -/
def incrementEntry : NamedV1.Entry :=
  { slot := 7, origin := witnessCallee, input := [.i64], output := [.i64],
    body := NamedV1.liftCode NobleContracts.increment, dependencies := [], rank := 0 }

def witnessCaller : NamedV1.Environment := { entries := [incrementEntry] }

def incrementUse : NamedV1.Use :=
  { slot := 7, occurrence := 0, caller := witnessCallerProvenance, origin := witnessCallee,
    input := [.i64], output := [.i64] }

/-- The call occurrence's embedding: an empty callee table, occurrence `k` at slot `k + 7`. -/
def incrementEmbedding :
    EntryEmbedding witnessCaller incrementSubject incrementEntry incrementUse where
  embedding :=
    { snapshot := by
        intro entry h
        simp [incrementSubject] at h
      rename := fun occurrence => occurrence + 7
      injective := fun a b h => by simpa using h
      fresh := fun _ => rfl }
  placement := rfl
  lookup := rfl
  entryMatches := ⟨rfl, rfl, rfl, rfl⟩
  body := rfl
  input := rfl
  output := rfl
  origin := rfl

/-- The caller `[ swap d.inc ]`: a `swap` segment, then one call of the witness callee. -/
def swapCallSubject : NamedV1.Subject where
  provenance := witnessCallerProvenance
  env := witnessCaller
  root := NamedV1.liftCode [.word 2] ++ [.call incrementUse]
  dependencies := [7]
  rank := 1
  closure := by
    simp [NamedV1.ExactClosure, witnessCaller, incrementEntry, incrementUse, NamedV1.liftCode,
      NobleContracts.increment, NamedV1.liftOp, NamedV1.liftValue, NamedV1.usesCode,
      NamedV1.usesOp, NamedV1.usesValue, NamedV1.Environment.lookup, NamedV1.Entry.matches]
  checkedEntries := by
    intro entry h
    simp only [witnessCaller, List.mem_singleton] at h
    subst h
    exact incrementBody_typed _ []
  input := [.i64, .i64]
  output := [.i64, .i64]
  typed := by
    simp only [NamedV1.liftCode, List.map, NamedV1.liftOp, List.cons_append, List.nil_append]
    exact .cons (.swap (s := []))
      (.cons (.call (entry := incrementEntry) (s := [.i64]) rfl ⟨rfl, rfl, rfl, rfl⟩
        (incrementBody_typed _ [.i64])) .nil)

/-- The assertions of the `[ swap d.inc ]` chain for one admitted input. -/
def PairInput (input : NobleContracts.Stack) : Prop := ∃ x y, input = [.i64 x, .i64 y]

/-- `swap` as the first segment for one admitted input (nothing carried). -/
def swapPairSegment (input : NobleContracts.Stack) :
    LiftedSegment (fun s => s = input ∧ PairInput input)
      (fun s => ∃ x y, input = [.i64 x, .i64 y] ∧ s = [.i64 y, .i64 x]) where
  code := [.word 2]
  profileCode := by simp [ProfileCode]
  inputTypes := [.i64, .i64]
  outputTypes := [.i64, .i64]
  pre := fun _ _ _ => True
  post := fun before after _ => ∃ x y, before = [.i64 x, .i64 y] ∧ after = [.i64 y, .i64 x]
  claim := swap_exported
  bridge := by
    rintro s ⟨rfl, x, y, rfl⟩
    exact ⟨[], [.i64 x, .i64 y], rfl, trivial, .cons .i64 (.cons .i64 .nil)⟩
  join := by
    rintro carried args res ⟨hs, x, y, hi⟩ ⟨a, b, rfl, rfl⟩ _
    rw [hi] at hs
    cases carried with
    | nil =>
        simp only [List.nil_append, List.cons.injEq, NobleContracts.Value.i64.injEq,
          and_true] at hs
        obtain ⟨rfl, rfl⟩ := hs
        exact ⟨a, b, hi, rfl⟩
    | cons c cs =>
        have lengths := congrArg List.length hs
        simp at lengths

/-- The call as the second step for one admitted input: it consumes the top `I64` and carries
the value below it. -/
def incrementPairStep (input : NobleContracts.Stack) :
    LiftedChainStep witnessCaller
      (fun s => ∃ x y, input = [.i64 x, .i64 y] ∧ s = [.i64 y, .i64 x])
      (fun s => ∃ x y, input = [.i64 x, .i64 y] ∧ s = [.i64 y, .i64 (x + 1)]) where
  subject := incrementSubject
  entry := incrementEntry
  use := incrementUse
  embedding := incrementEmbedding
  bodies := by
    intro row h
    simp [incrementSubject] at h
  profile := by
    simp [incrementSubject, Sequential, NamedV1.liftCode, NobleContracts.increment,
      NamedV1.liftOp, NamedV1.liftValue, Scalar]
  pre := fun _ _ _ => True
  post := fun before after _ => ∃ m, before = [.i64 m] ∧ after = [.i64 (m + 1)]
  claim := incrementSubject_lifted
  bridge := by
    rintro s ⟨x, y, _, rfl⟩
    exact ⟨[.i64 y], [.i64 x], rfl, trivial, .cons .i64 .nil⟩
  join := by
    rintro carried args res ⟨x, y, hi, hs⟩ ⟨m, rfl, rfl⟩ _
    refine ⟨x, y, hi, ?_⟩
    cases carried with
    | nil => simp at hs
    | cons c cs =>
        cases cs with
        | nil =>
            simp only [List.cons_append, List.nil_append, List.cons.injEq,
              NobleContracts.Value.i64.injEq, and_true] at hs
            obtain ⟨rfl, rfl⟩ := hs
            rfl
        | cons _ _ => simp at hs

/-- PREUSE-S04's shape `[ swap d.sub2 ]` as the caller's own lifted claim: on any lifted frame
the caller returns `[y, x + 1]` for every input `[x, y]` (bottom first). The value `y` that `swap`
moves down is carried below the callee's argument, and the postcondition relates the result to
both inputs. -/
theorem swapCall_lifted :
    exportedLiftedClaim swapCallSubject (fun before _ _ => PairInput before)
      (fun before after _ => ∃ x y, before = [.i64 x, .i64 y] ∧ after = [.i64 y, .i64 (x + 1)]) :=
  liftedChainClaim swapCallSubject PairInput
    (fun input s => ∃ x y, input = [.i64 x, .i64 y] ∧ s = [.i64 y, .i64 (x + 1)])
    (fun input => .segment (swapPairSegment input) (.cons (incrementPairStep input) (.nil _)))
    (fun _ => rfl)
    (by
      rintro input res _ ⟨x, y, _, rfl⟩
      exact .cons .i64 (.cons .i64 .nil))

end NobleContracts.NamedV3
