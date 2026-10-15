import NobleFoundation.Instr
import NobleContracts.Composition

/-!
# Lifting the I64 bridge through finite sequences and quote/compose/run

`AOp` is the selected straight-line fragment: accepted I64 literal pushes and
the three wrapping operations.  `AOp.ref` is an *independent* reference stack
machine over mathematical integers whose only arithmetic is the comparator
oracle's `MathlibI64.wrap`, and whose `push` refuses out-of-range literals as
the frontend does (`Literal.lean`).  `lowerAll` maps the fragment onto MC1
`Op`s.  The theorems below show that MC1 execution of the lowered code is
exactly the reference machine, for every preserved stack prefix.

`PExpr` adds the program-construction fragment: a quoted literal (`quote`),
a literal block, and `compose`; `run` (word 10) executes the constructed value.
Construction is proved pure — it pushes exactly one program value and never
executes its body — and running it is exactly executing the composed body.
-/

namespace NobleFoundation

open NobleContracts

inductive AOp where
  | push (n : ℤ)
  | add
  | sub
  | mul
  deriving DecidableEq, Repr

def AOp.lower : AOp → Op
  | .push n => .lit (i64v n)
  | .add => .word 4
  | .sub => .word 5
  | .mul => .word 6

def lowerAll (ops : List AOp) : List Op := ops.map AOp.lower

/-- Reference semantics, top-first, deeper operand `x`, top operand `y`. -/
def AOp.ref : AOp → List ℤ → Option (List ℤ)
  | .push n, s => if I64Range n then some (n :: s) else none
  | .add, y :: x :: s => some (MathlibI64.wrap (x + y) :: s)
  | .sub, y :: x :: s => some (MathlibI64.wrap (x - y) :: s)
  | .mul, y :: x :: s => some (MathlibI64.wrap (x * y) :: s)
  | _, _ => none

def refRun : List AOp → List ℤ → Option (List ℤ)
  | [], s => some s
  | op :: ops, s => (op.ref s).bind (refRun ops)

/-- Static stack depth (what MC1 typing sees; literal range is not part of it). -/
def AOp.depth : AOp → ℕ → Option ℕ
  | .push _, k => some (k + 1)
  | _, k + 2 => some (k + 1)
  | _, _ => none

def depthRun : List AOp → ℕ → Option ℕ
  | [], k => some k
  | op :: ops, k => (op.depth k).bind (depthRun ops)

/-- Public, bottom-first encoding of a top-first integer stack. -/
def enc (zs : List ℤ) : Stack := (zs.map i64v).reverse

theorem refRun_append (a b : List AOp) (zs : List ℤ) :
    refRun (a ++ b) zs = (refRun a zs).bind (refRun b) := by
  induction a generalizing zs with
  | nil => simp [refRun]
  | cons op ops ih =>
      simp only [List.cons_append, refRun]
      cases op.ref zs <;> simp [ih]

theorem depthRun_append (a b : List AOp) (k : ℕ) :
    depthRun (a ++ b) k = (depthRun a k).bind (depthRun b) := by
  induction a generalizing k with
  | nil => simp [depthRun]
  | cons op ops ih =>
      simp only [List.cons_append, depthRun]
      cases op.depth k <;> simp [ih]

private theorem ofInt_wrap_add (x y : ℤ) :
    BitVec.ofInt 64 x + BitVec.ofInt 64 y = BitVec.ofInt 64 (MathlibI64.wrap (x + y)) := by
  rw [ofInt_wrap, BitVec.ofInt_add]
private theorem ofInt_wrap_sub (x y : ℤ) :
    BitVec.ofInt 64 x - BitVec.ofInt 64 y = BitVec.ofInt 64 (MathlibI64.wrap (x - y)) := by
  rw [ofInt_wrap, ofInt_sub]
private theorem ofInt_wrap_mul (x y : ℤ) :
    BitVec.ofInt 64 x * BitVec.ofInt 64 y = BitVec.ofInt 64 (MathlibI64.wrap (x * y)) := by
  rw [ofInt_wrap, BitVec.ofInt_mul]

/-- Internal-orientation simulation: the lowered code runs exactly like the
reference machine above an arbitrary untouched remainder `r`. -/
theorem run_lowerAll {ops : List AOp} {zs zs' : List ℤ} (h : refRun ops zs = some zs')
    (r t : Stack) : Run (lowerAll ops) (zs.map i64v ++ r) t ↔ t = zs'.map i64v ++ r := by
  induction ops generalizing zs with
  | nil =>
      simp only [refRun, Option.some.injEq] at h
      subst h; simp [lowerAll]
  | cons op ops ih =>
      simp only [refRun] at h
      cases hop : op.ref zs with
      | none => rw [hop] at h; cases h
      | some m =>
          rw [hop, Option.bind_some] at h
          have hstep : ∀ u, Step op.lower (zs.map i64v ++ r) u ↔ u = m.map i64v ++ r := by
            intro u
            cases op with
            | push n =>
                simp only [AOp.ref] at hop
                split_ifs at hop; cases hop
                simp [AOp.lower]
            | add =>
                match zs, hop with
                | y :: x :: s, hop =>
                    simp only [AOp.ref, Option.some.injEq] at hop; subst hop
                    simp [AOp.lower, i64v, Step.add_iff, ofInt_wrap_add]
            | sub =>
                match zs, hop with
                | y :: x :: s, hop =>
                    simp only [AOp.ref, Option.some.injEq] at hop; subst hop
                    simp [AOp.lower, i64v, Step.sub_iff, ofInt_wrap_sub]
            | mul =>
                match zs, hop with
                | y :: x :: s, hop =>
                    simp only [AOp.ref, Option.some.injEq] at hop; subst hop
                    simp [AOp.lower, i64v, Step.mul_iff, ofInt_wrap_mul]
          simp only [lowerAll, List.map_cons, Run.cons_iff]
          constructor
          · rintro ⟨u, hu, hrun⟩
            rw [(hstep u).mp hu] at hrun
            exact (ih h).mp hrun
          · intro ht
            exact ⟨_, (hstep _).mpr rfl, (ih h).mpr ht⟩

/-- **Public-orientation lifting through finite sequences**: for every
preserved prefix `pre`, MC1 execution of the lowered sequence on
`pre ++ enc zs` has exactly one normal result, `pre ++ enc zs'`, where `zs'`
is the reference machine's result. -/
theorem exec_lowerAll {ops : List AOp} {zs zs' : List ℤ} (h : refRun ops zs = some zs')
    (pre t : Stack) : Exec (lowerAll ops) (pre ++ enc zs) t ↔ t = pre ++ enc zs' := by
  unfold Exec enc
  rw [List.reverse_append, List.reverse_reverse, run_lowerAll h]
  constructor
  · intro ht; rw [← List.reverse_reverse t, ht]; simp
  · rintro rfl; simp

/-- The reference machine keeps every stack entry in the signed range. -/
theorem refRun_range {ops : List AOp} {zs zs' : List ℤ} (h : refRun ops zs = some zs')
    (hz : ∀ z ∈ zs, I64Range z) : ∀ z ∈ zs', I64Range z := by
  induction ops generalizing zs with
  | nil => simp only [refRun, Option.some.injEq] at h; subst h; exact hz
  | cons op ops ih =>
      simp only [refRun] at h
      cases hop : op.ref zs with
      | none => rw [hop] at h; cases h
      | some m =>
          rw [hop, Option.bind_some] at h
          apply ih h
          have hw : ∀ z, I64Range (MathlibI64.wrap z) := fun z => by
            rw [wrap_eq_toInt_ofInt]; exact toInt_range _
          cases op with
          | push n =>
              simp only [AOp.ref] at hop; split_ifs at hop with hn; cases hop
              intro z hzm; simp at hzm; rcases hzm with rfl | hzm
              · exact hn
              · exact hz z hzm
          | add | sub | mul =>
              match zs, hop with
              | y :: x :: s, hop =>
                  simp only [AOp.ref, Option.some.injEq] at hop; subst hop
                  intro z hzm; simp at hzm; rcases hzm with rfl | hzm
                  · exact hw _
                  · exact hz z (by simp [hzm])

/-- Reference success implies the static depth check succeeds. -/
theorem depthRun_of_refRun {ops : List AOp} {zs zs' : List ℤ} (h : refRun ops zs = some zs') :
    depthRun ops zs.length = some zs'.length := by
  induction ops generalizing zs with
  | nil => simp only [refRun, Option.some.injEq] at h; subst h; rfl
  | cons op ops ih =>
      simp only [refRun] at h
      cases hop : op.ref zs with
      | none => rw [hop] at h; cases h
      | some m =>
          rw [hop, Option.bind_some] at h
          simp only [depthRun]
          cases op with
          | push n =>
              simp only [AOp.ref] at hop; split_ifs at hop; cases hop
              simpa [AOp.depth] using ih h
          | add | sub | mul =>
              match zs, hop with
              | y :: x :: s, hop =>
                  simp only [AOp.ref, Option.some.injEq] at hop; subst hop
                  simpa [AOp.depth] using ih h

private theorem rep_succ (base : List Ty) (k : ℕ) :
    base ++ List.replicate (k + 1) Ty.i64 = (base ++ List.replicate k Ty.i64) ++ [.i64] := by
  rw [List.replicate_succ', List.append_assoc]

private theorem rep_succ_succ (base : List Ty) (k : ℕ) :
    base ++ List.replicate (k + 2) Ty.i64 = (base ++ List.replicate k Ty.i64) ++ [.i64, .i64] := by
  rw [List.replicate_succ', List.replicate_succ', List.append_assoc, List.append_assoc]; rfl

/-- **Typing premise**: the lowered sequence is `CodeTyped` from
`base ++ k × I64` to `base ++ k' × I64` whenever the static depth check passes;
`base` is an arbitrary typed prefix that is never consumed. -/
theorem codeTyped_lowerAll {ops : List AOp} {k k' : ℕ} (h : depthRun ops k = some k')
    (base : List Ty) :
    CodeTyped (lowerAll ops) (base ++ List.replicate k .i64) (base ++ List.replicate k' .i64) := by
  induction ops generalizing k with
  | nil => simp only [depthRun, Option.some.injEq] at h; subst h; exact .nil
  | cons op ops ih =>
      simp only [depthRun] at h
      cases hop : op.depth k with
      | none => rw [hop] at h; cases h
      | some m =>
          rw [hop, Option.bind_some] at h
          refine .cons ?_ (ih h)
          cases op with
          | push n =>
              simp only [AOp.depth, Option.some.injEq] at hop; subst hop
              rw [rep_succ]; exact .lit .i64
          | add | sub | mul =>
              match k, hop with
              | j + 2, hop =>
                  simp only [AOp.depth, Option.some.injEq] at hop; subst hop
                  rw [rep_succ_succ, rep_succ]
                  first | exact .add | exact .sub | exact .mul

/-! ## Program construction fragment: quote / block / compose, then run -/

inductive PExpr where
  /-- a literal block `[ ops ]` -/
  | arith (ops : List AOp)
  /-- `n quote` -/
  | quoted (n : ℤ)
  /-- `p q compose` -/
  | compose (p q : PExpr)

/-- Code that constructs the program value. -/
def PExpr.code : PExpr → List Op
  | .arith ops => [.block (lowerAll ops)]
  | .quoted n => [.lit (i64v n), .word 8]
  | .compose p q => p.code ++ q.code ++ [.word 9]

/-- The exact body (recipe) of the constructed program. -/
def PExpr.body : PExpr → List Op
  | .arith ops => lowerAll ops
  | .quoted n => [.lit (i64v n)]
  | .compose p q => p.body ++ q.body

/-- The arithmetic sequence the body denotes. -/
def PExpr.flat : PExpr → List AOp
  | .arith ops => ops
  | .quoted n => [.push n]
  | .compose p q => p.flat ++ q.flat

theorem PExpr.body_eq (P : PExpr) : P.body = lowerAll P.flat := by
  induction P with
  | arith ops => rfl
  | quoted n => rfl
  | compose p q ihp ihq => simp [PExpr.body, PExpr.flat, ihp, ihq, lowerAll]

/-- **Construction is pure**: building the program pushes exactly one program
value with body `P.body` on top of any stack, and executes nothing. -/
theorem PExpr.exec_code (P : PExpr) (s t : Stack) :
    Exec P.code s t ↔ t = s ++ [.program P.body] := by
  induction P generalizing s t with
  | arith ops => simp [PExpr.code, PExpr.body]
  | quoted n =>
      simp only [PExpr.code, PExpr.body]
      rw [show [Op.lit (i64v n), Op.word 8] = [Op.lit (i64v n)] ++ [Op.word 8] from rfl,
        exec_append_iff]
      simp
  | compose p q ihp ihq =>
      simp only [PExpr.code, PExpr.body, exec_append_iff, ihp, ihq]
      constructor
      · rintro ⟨m, ⟨u, rfl, rfl⟩, h⟩
        simp at h; exact h
      · rintro rfl
        exact ⟨s ++ [.program p.body, .program q.body], ⟨_, rfl, by simp⟩,
          by simp⟩

/-- Constructing then running is executing the composed body. -/
theorem PExpr.exec_code_run (P : PExpr) (s t : Stack) :
    Exec (P.code ++ [.word 10]) s t ↔ Exec P.body s t := by
  rw [exec_append_iff]
  constructor
  · rintro ⟨m, hm, hr⟩
    rw [(P.exec_code s m).mp hm] at hr
    exact exec_run_iff.mp hr
  · intro h
    exact ⟨_, (P.exec_code s _).mpr rfl, exec_run_iff.mpr h⟩

/-- Reflection observes exactly the composed recipe (no normalization). -/
theorem PExpr.exec_code_reflect (P : PExpr) (s t : Stack) :
    Exec (P.code ++ [.word 11]) s t ↔ t = s ++ [.«syntax» P.body] := by
  rw [exec_append_iff]
  constructor
  · rintro ⟨m, hm, hr⟩
    rw [(P.exec_code s m).mp hm] at hr
    exact exec_reflect_iff.mp hr
  · rintro rfl
    exact ⟨_, (P.exec_code s _).mpr rfl, exec_reflect_iff.mpr rfl⟩

/-- **End-to-end fragment theorem**: build `P` by quote/block/compose, run it,
and the result is the reference machine's result on the flattened sequence,
above an arbitrary preserved prefix. -/
theorem PExpr.exec_run_ref (P : PExpr) {zs zs' : List ℤ} (h : refRun P.flat zs = some zs')
    (pre t : Stack) :
    Exec (P.code ++ [.word 10]) (pre ++ enc zs) t ↔ t = pre ++ enc zs' := by
  rw [P.exec_code_run, P.body_eq]
  exact exec_lowerAll h pre t

/-- Typing of the construction code: placed on any stack type `S`, it pushes a
program typed from `base ++ k × I64` to `base ++ k' × I64`. -/
theorem PExpr.codeTyped (P : PExpr) (S base : List Ty) {k k' : ℕ}
    (h : depthRun P.flat k = some k') :
    CodeTyped P.code S
      (S ++ [.program (base ++ List.replicate k .i64) (base ++ List.replicate k' .i64)]) := by
  induction P generalizing S k k' with
  | arith ops => exact .cons (.block (codeTyped_lowerAll h base)) .nil
  | quoted n =>
      simp only [PExpr.flat, depthRun, AOp.depth, Option.bind_some,
        Option.some.injEq] at h
      subst h
      rw [rep_succ]
      exact .cons (.lit .i64) (.cons .quote .nil)
  | compose p q ihp ihq =>
      simp only [PExpr.flat] at h
      rw [depthRun_append] at h
      cases hp : depthRun p.flat k with
      | none => rw [hp] at h; cases h
      | some m =>
          rw [hp, Option.bind_some] at h
          have cp := ihp S hp
          have cq := ihq (S ++ [.program (base ++ List.replicate k .i64)
            (base ++ List.replicate m .i64)]) h
          simp only [List.append_assoc, List.cons_append, List.nil_append] at cq
          exact (cp.append cq).append (.cons .compose .nil)

/-- Typing of construct-then-run, with the exact premise that the program's
input interface is the whole current stack type. -/
theorem PExpr.codeTyped_run (P : PExpr) (base : List Ty) {k k' : ℕ}
    (h : depthRun P.flat k = some k') :
    CodeTyped (P.code ++ [.word 10]) (base ++ List.replicate k .i64)
      (base ++ List.replicate k' .i64) :=
  (P.codeTyped _ base h).append (.cons .run .nil)

/-- Nested quotation: quoting a program value and running twice executes it. -/
theorem exec_quote_program_run_run (p : List Op) (s t : Stack) :
    Exec [.block p, .word 8, .word 10, .word 10] s t ↔ Exec p s t := by
  rw [show [Op.block p, .word 8, .word 10, .word 10] =
      [.block p] ++ [.word 8, .word 10] ++ [.word 10] from rfl,
    exec_append_iff]
  constructor
  · rintro ⟨m, hm, hr⟩
    obtain ⟨u, hu, hq⟩ := exec_append_iff.mp hm
    rw [exec_block_iff.mp hu, exec_quote_run_iff] at hq
    subst hq
    exact exec_run_iff.mp hr
  · intro h
    exact ⟨s ++ [.program p], exec_append_iff.mpr ⟨_, exec_block_iff.mpr rfl,
      exec_quote_run_iff.mpr rfl⟩, exec_run_iff.mpr h⟩

/-- Inputs pushed by literal operations, then a constructed program is run:
the whole run equals the reference machine from the empty operand stack, above
any preserved prefix. This is the per-case statement the differential
comparator instantiates. -/
theorem PExpr.exec_inputs_run (P : PExpr) (ins : List AOp) {zs' : List ℤ}
    (h : refRun (ins ++ P.flat) [] = some zs') (pre t : Stack) :
    Exec (lowerAll ins ++ (P.code ++ [.word 10])) pre t ↔ t = pre ++ enc zs' := by
  rw [refRun_append] at h
  cases hm : refRun ins [] with
  | none => rw [hm] at h; cases h
  | some m =>
      rw [hm, Option.bind_some] at h
      rw [exec_append_iff]
      have hins := exec_lowerAll hm pre
      simp only [enc, List.map_nil, List.reverse_nil, List.append_nil] at hins
      constructor
      · rintro ⟨u, hu, hr⟩
        rw [(hins u).mp hu] at hr
        exact (P.exec_run_ref h pre t).mp hr
      · intro ht
        exact ⟨_, (hins _).mpr rfl, (P.exec_run_ref h pre t).mpr ht⟩

/-- Straight-line form of the same statement (no program construction). -/
theorem exec_lowerAll_from_empty {ops : List AOp} {zs' : List ℤ} (h : refRun ops [] = some zs')
    (pre t : Stack) : Exec (lowerAll ops) pre t ↔ t = pre ++ enc zs' := by
  have := exec_lowerAll h pre t
  simpa [enc] using this

/-- Nested quotation form: `ins [ ops ] quote run run`. -/
theorem exec_inputs_nested (ins ops : List AOp) {zs' : List ℤ}
    (h : refRun (ins ++ ops) [] = some zs') (pre t : Stack) :
    Exec (lowerAll ins ++ [.block (lowerAll ops), .word 8, .word 10, .word 10]) pre t ↔
      t = pre ++ enc zs' := by
  have h' : refRun (ins ++ (PExpr.arith ops).flat) [] = some zs' := h
  rw [← (PExpr.arith ops).exec_inputs_run ins h' pre t, exec_append_iff, exec_append_iff]
  constructor
  · rintro ⟨u, hu, hr⟩
    exact ⟨u, hu, (PExpr.arith ops).exec_code_run u t |>.mpr
      ((exec_quote_program_run_run _ u t).mp hr)⟩
  · rintro ⟨u, hu, hr⟩
    exact ⟨u, hu, (exec_quote_program_run_run _ u t).mpr
      (((PExpr.arith ops).exec_code_run u t).mp hr)⟩

end NobleFoundation
