import NobleContracts.Obligation

namespace NobleContracts

/-- Reference representation of `[ 1 + ]`; application evidence additionally
binds this representation to the frontend's actual accepted/exported subject. -/
def increment : List Op := [.lit (.i64 1), .word 4]

/-- The actual construction and invocation of two composed increment programs. -/
def twoIncrements : List Op :=
  [.block increment, .block increment, .word 9, .word 10]

/-- The runtime builder `quote [ + ] compose`: its input is a captured value. -/
def captureBuilder : List Op := [.word 8, .block [.word 4], .word 9]

/-- The code value constructed by `captureBuilder` for one arbitrary capture. -/
def capturedAdd (capture : BitVec 64) : List Op :=
  [.lit (.i64 capture), .word 4]

/-- Universal wrapping I64 correctness, including both overflow boundaries and
an arbitrary whole stack prefix.  The reverse direction also proves this
particular finite program has the stated normal execution. -/
@[simp] theorem increment_exec_iff (n : BitVec 64) (tail result : Stack) :
    Exec increment (tail ++ [.i64 n]) result ↔
      result = tail ++ [.i64 (n + 1)] := by
  simp [increment, Exec]

theorem increment_normal (n : BitVec 64) (tail : Stack) :
    Exec increment (tail ++ [.i64 n]) (tail ++ [.i64 (n + 1)]) :=
  (increment_exec_iff n tail _).mpr rfl

/-- A library-level partial-correctness contract retaining the original input. -/
theorem increment_pc :
    PC increment
      (fun before => ∃ tail n, before = tail ++ [.i64 n])
      (fun before after => ∃ tail n,
        before = tail ++ [.i64 n] ∧ after = tail ++ [.i64 (n + 1)]) := by
  intro before after hi he
  obtain ⟨tail, n, rfl⟩ := hi
  exact ⟨tail, n, rfl, (increment_exec_iff n tail after).mp he⟩

theorem increment_typed : CodeTyped increment [.i64] [.i64] := by
  exact .cons (OpTyped.lit (s := [.i64]) HasType.i64)
    (.cons (OpTyped.add (s := [])) .nil)

/-- Sequential composition, with a genuinely shared intermediate state. -/
theorem increment_sequence_exec_iff (n : BitVec 64) (tail result : Stack) :
    Exec (increment ++ increment) (tail ++ [.i64 n]) result ↔
      result = tail ++ [.i64 ((n + 1) + 1)] := by
  simp [increment, Exec]

/-- The runtime `compose` program obeys the same universal law. -/
@[simp] theorem twoIncrements_exec_iff (n : BitVec 64) (tail result : Stack) :
    Exec twoIncrements (tail ++ [.i64 n]) result ↔
      result = tail ++ [.i64 (n + 2)] := by
  have arithmetic : (n + 1) + 1 = n + 2 := by
    rw [BitVec.add_assoc]
    rfl
  simp [twoIncrements, increment, Exec]
  change (result = tail ++ [.i64 ((n + 1) + 1)]) ↔ result = tail ++ [.i64 (n + 2)]
  rw [arithmetic]

theorem twoIncrements_normal (n : BitVec 64) (tail : Stack) :
    Exec twoIncrements (tail ++ [.i64 n]) (tail ++ [.i64 (n + 2)]) :=
  (twoIncrements_exec_iff n tail _).mpr rfl

theorem twoIncrements_typed : CodeTyped twoIncrements [.i64] [.i64] := by
  exact .cons (OpTyped.block (s := [.i64]) increment_typed)
    (.cons (OpTyped.block (s := [.i64, .program [.i64] [.i64]]) increment_typed)
      (.cons (OpTyped.compose (s := [.i64]) (i := [.i64]) (m := [.i64]) (o := [.i64]))
        (.cons (OpTyped.run (s := [.i64]) (t := [.i64])) .nil)))

/-- This theorem proves the builder executes to the captured code, instead of
assuming that the program returned by the builder has the desired behavior. -/
@[simp] theorem captureBuilder_exec_iff (capture : BitVec 64) (tail result : Stack) :
    Exec captureBuilder (tail ++ [.i64 capture]) result ↔
      result = tail ++ [.program (capturedAdd capture)] := by
  simp [captureBuilder, capturedAdd, Exec]

/-- Every captured code value implements addition for every later argument.
The order is `later + capture`, exactly the stack order of literal then `+`. -/
@[simp] theorem capturedAdd_exec_iff (capture later : BitVec 64) (tail result : Stack) :
    Exec (capturedAdd capture) (tail ++ [.i64 later]) result ↔
      result = tail ++ [.i64 (later + capture)] := by
  simp [capturedAdd, Exec]

theorem capturedAdd_typed (capture : BitVec 64) :
    CodeTyped (capturedAdd capture) [.i64] [.i64] := by
  exact .cons (OpTyped.lit (s := [.i64]) HasType.i64)
    (.cons (OpTyped.add (s := [])) .nil)

theorem captureBuilder_typed :
    CodeTyped captureBuilder [.i64] [.program [.i64] [.i64]] := by
  exact .cons (OpTyped.quote (s := []) (a := .i64) (t := [.i64]))
    (.cons (OpTyped.block (s := [.program [.i64] [.i64, .i64]])
      (.cons (OpTyped.add (s := [])) .nil))
      (.cons (OpTyped.compose (s := []) (i := [.i64])
        (m := [.i64, .i64]) (o := [.i64])) .nil))

/-- Full runtime-generated family theorem: the capture is universally bound
before the builder runs, and the later argument and tail are universally bound
afterwards.  This is not a theorem about a literal-only collection of programs. -/
theorem captureBuilder_family (capture : BitVec 64) (tail built : Stack)
    (construction : Exec captureBuilder (tail ++ [.i64 capture]) built) :
    ∃ generated,
      built = tail ++ [.program generated] ∧
      CodeTyped generated [.i64] [.i64] ∧
      ∀ (later : BitVec 64) (laterTail result : Stack),
        Exec generated (laterTail ++ [.i64 later]) result ↔
          result = laterTail ++ [.i64 (later + capture)] := by
  exact ⟨capturedAdd capture,
    (captureBuilder_exec_iff capture tail built).mp construction,
    capturedAdd_typed capture, capturedAdd_exec_iff capture⟩

/-- Normal construction and normal invocation both exist for this finite
family.  This is stronger than the partial-correctness meaning of `Maps`. -/
theorem captureBuilder_family_normal (capture later : BitVec 64)
    (constructionTail invocationTail : Stack) :
    Exec captureBuilder (constructionTail ++ [.i64 capture])
      (constructionTail ++ [.program (capturedAdd capture)]) ∧
    Exec (capturedAdd capture) (invocationTail ++ [.i64 later])
      (invocationTail ++ [.i64 (later + capture)]) := by
  exact ⟨(captureBuilder_exec_iff capture constructionTail _).mpr rfl,
    (capturedAdd_exec_iff capture later invocationTail _).mpr rfl⟩

theorem capturedAdd_maps (capture later : BitVec 64) :
    Maps (.program (capturedAdd capture)) [.i64 later] [.i64 (later + capture)] := by
  intro result he
  exact (capturedAdd_exec_iff capture later [] result).mp he

/-- `dip` does not expose its saved value to the body, and restores it on top. -/
theorem dip_increment_exec_iff (n : BitVec 64) (saved : Value) (tail result : Stack) :
    Exec [.word 3] (tail ++ [.i64 n, saved, .program increment]) result ↔
      result = tail ++ [.i64 (n + 1), saved] := by
  simp [increment, Exec, List.append_assoc]

/-- A structural example preserving the same arbitrary stack tail. -/
theorem pair_unpair_exec_iff (a b : Value) (tail result : Stack) :
    Exec [.word 13, .word 14] (tail ++ [a, b]) result ↔ result = tail ++ [a, b] := by
  simp [Exec, List.append_assoc]

/-- An actual two-branch program, with the false branch equal to the identity. -/
theorem if_increment_exec_iff (n : BitVec 64) (condition : Bool) (tail result : Stack) :
    Exec [.block increment, .block [], .word 18] (tail ++ [.i64 n, .bool condition]) result ↔
      result = tail ++ [.i64 (if condition then n + 1 else n)] := by
  cases condition <;> simp [increment, Exec]

/-- The reusable, typed exported boundary for the increment example. -/
theorem increment_exported :
    exportedClaim increment [.i64] [.i64] []
      (fun _ _ _ => True)
      (fun before after _ => ∃ n, before = [.i64 n] ∧ after = [.i64 (n + 1)]) := by
  apply exportedClaim_unaryI64
  intro tail n final he
  exact (increment_exec_iff n tail final).mp he

theorem twoIncrements_exported :
    exportedClaim twoIncrements [.i64] [.i64] []
      (fun _ _ _ => True)
      (fun before after _ => ∃ n, before = [.i64 n] ∧ after = [.i64 (n + 2)]) := by
  apply exportedClaim_unaryI64
  intro tail n final he
  exact (twoIncrements_exec_iff n tail final).mp he

/-- The builder theorem also establishes the exact typed, tail-preserving
exported claim and a universally quantified semantic `maps` postcondition. -/
theorem captureBuilder_exported :
    exportedClaim captureBuilder [.i64] [.program [.i64] [.i64]] []
      (fun _ _ _ => True)
      (fun before after _ => ∃ capture generated,
        before = [.i64 capture] ∧ after = [.program generated] ∧
        ∀ later : BitVec 64,
          Maps (.program generated) [.i64 later] [.i64 (later + capture)]) := by
  intro tail before params hi hp hpre final he
  obtain ⟨capture, rfl⟩ := (stackTyped_i64_iff before).mp hi
  refine ⟨[.program (capturedAdd capture)],
    (captureBuilder_exec_iff capture tail final).mp he, ?_,
    capture, capturedAdd capture, rfl, rfl, capturedAdd_maps capture⟩
  exact StackTyped.cons (HasType.program (capturedAdd_typed capture)) StackTyped.nil

end NobleContracts
