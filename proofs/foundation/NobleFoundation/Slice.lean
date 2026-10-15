import NobleFoundation.Sequence
import NobleFoundation.Helper

/-!
# End-to-end slice specification (issue #15, Phase 3)

The subject is the closed, inert quotation root

```text
[ quote [ 1 + ] compose run ]
```

Its body quotes the I64 on top of the stack, composes the resulting program
with `[ 1 + ]`, and runs the composition. This is arithmetic plus
composition, in the `MC2-pure-quotation@1` shape that #10's PAA-1 selects:
one closed quotation root, no definitions, no resources.

The chain from this specification to the loaded bytes, with every unproved
edge, is in `proofs/foundation/README.md` ("Phase 3"). These theorems are the
model end of that chain. `verification/foundation/slice/` holds the MC1
contract and the self-contained MC1 proof that `noble build --require-proof`
rechecks against the exact source.
-/

namespace NobleFoundation

open NobleContracts

/-- The body of the subject quotation (MC1 word numbers). -/
def sliceBody : List Op :=
  [.word 8, .block [.lit (i64v 1), .word 4], .word 9, .word 10]

/-- The whole subject source: one literal block. -/
def sliceRoot : List Op := [.block sliceBody]

/-- Bit-vector form: for any prefix and any I64 `n`, the body has exactly one
normal result, `n + 1` (wrapping), with the prefix untouched. -/
theorem slice_body_exec (pre : Stack) (n : BitVec 64) (t : Stack) :
    Exec sliceBody (pre ++ [.i64 n]) t ↔ t = pre ++ [.i64 (n + 1)] := by
  simp [Exec, sliceBody, i64v, List.reverse_append]

/-- Signed-integer form against the comparator oracle. -/
theorem slice_body_signed (pre : Stack) (x : ℤ) (t : Stack) :
    Exec sliceBody (pre ++ [i64v x]) t ↔ t = pre ++ [i64v (MathlibI64.wrap (x + 1))] := by
  rw [i64v, slice_body_exec, i64v, ofInt_wrap, BitVec.ofInt_add]
  rfl

/-- The subject root is inert: it pushes the program and runs nothing. -/
theorem slice_root_inert (s t : Stack) : Exec sliceRoot s t ↔ t = s ++ [.program sliceBody] := by
  simp [sliceRoot]

/-- Driver `x [ quote [ 1 + ] compose run ] run`: construct, then invoke. -/
theorem slice_driver (pre : Stack) (x : ℤ) (t : Stack) :
    Exec (sliceRoot ++ [.word 10]) (pre ++ [i64v x]) t ↔
      t = pre ++ [i64v (MathlibI64.wrap (x + 1))] := by
  rw [exec_append_iff]
  constructor
  · rintro ⟨m, hm, hr⟩
    rw [(slice_root_inert _ m).mp hm, exec_run_iff] at hr
    exact (slice_body_signed pre x t).mp hr
  · intro ht
    refine ⟨_, (slice_root_inert _ _).mpr rfl, ?_⟩
    rw [exec_run_iff]
    exact (slice_body_signed pre x t).mpr ht

/-- Typing with an arbitrary untouched base: the body maps `base ++ [I64]` to
`base ++ [I64]`. The inner `quote` is typed at the program interface
`base → base ++ [I64]`, and `compose` meets `[ 1 + ]` at `base ++ [I64]`. -/
theorem slice_body_typed (base : List Ty) :
    CodeTyped sliceBody (base ++ [.i64]) (base ++ [.i64]) := by
  unfold sliceBody
  refine .cons (.quote (t := base)) ?_
  refine .cons (.block (i := base ++ [.i64]) (o := base ++ [.i64])
    (.cons (.lit .i64) (by
      simp only [List.append_assoc, List.cons_append, List.nil_append]
      exact .cons .add .nil))) ?_
  simp only [List.append_assoc, List.cons_append, List.nil_append]
  exact .cons .compose (.cons .run .nil)

/-- The MC1 exported claim, in the exact form MC2 contracts use. -/
theorem slice_exported_claim :
    exportedClaim sliceBody [.i64] [.i64] [] (fun _ _ _ => True)
      (fun before after _ => ∃ n, before = [.i64 n] ∧ after = [.i64 (n + 1)]) :=
  exportedClaim_unaryI64 (fun tail n final h => (slice_body_exec tail n final).mp h)

/-- Concrete witnesses used by the executed slice. -/
theorem slice_examples (pre t : Stack) :
    (Exec (sliceRoot ++ [.word 10]) (pre ++ [i64v 41]) t ↔ t = pre ++ [i64v 42]) ∧
    (Exec (sliceRoot ++ [.word 10]) (pre ++ [i64v 9223372036854775807]) t ↔
      t = pre ++ [i64v (-9223372036854775808)]) := by
  constructor
  · have := slice_driver pre 41 t
    rwa [show MathlibI64.wrap (41 + 1) = 42 by decide] at this
  · have := slice_driver pre 9223372036854775807 t
    rwa [show MathlibI64.wrap (9223372036854775807 + 1) = -9223372036854775808 by decide] at this

end NobleFoundation
