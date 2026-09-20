import NobleContractImpl.Funs
import NobleContracts.Model

open Aeneas Aeneas.Std Result WP

set_option maxHeartbeats 1000000
set_option maxRecDepth 2048
set_option Aeneas.customDoElab false

namespace NobleContractImpl.Projection

open noble_contracts

/-- Every reference survives the actual Rust copy loop, in order. This is a
universal result over bounded slices, not an equation on selected examples. -/
@[step]
theorem lower_body_spec (body : Slice U32) :
    wire.lower_body body ⦃ result => result.val = body.val ⦄ := by
  unfold wire.lower_body wire.lower_body_loop
  dsimp only
  apply loop.spec_decr_nat
    (fun (state : alloc.vec.Vec U32 × Usize) => body.val.length - state.2.val)
    (fun (state : alloc.vec.Vec U32 × Usize) => state.1.val = body.val.take state.2.val ∧
      state.2.val ≤ body.val.length)
    (fun (result : alloc.vec.Vec U32) => result.val = body.val)
  · rintro ⟨out, index⟩ ⟨hprefix, hbound⟩
    simp only [wire.lower_body_loop.body]
    split
    · rename_i hlt
      have hindex : index.val < body.val.length := by scalar_tac
      have hsize : out.val.length = index.val := by
        rw [hprefix, List.length_take, Nat.min_eq_left hbound]
      have hcapacity : out.val.length < Usize.max := by
        have := body.property
        omega
      step as ⟨element, helement⟩
      step as ⟨next, hnext⟩
      step as ⟨nextIndex, hnextIndex⟩
      have hnew : nextIndex.val = index.val + 1 := by scalar_tac
      refine ⟨?_, ?_, ?_⟩
      · rw [hnew, hnext, helement, hprefix]
        exact (List.take_succ_eq_append_getElem hindex).symm
      · omega
      · omega
    · rename_i hstop
      simp only [spec_ok]
      have heq : index.val = body.val.length := by scalar_tac
      simpa [heq] using hprefix
  · simp [alloc.vec.Vec.with_capacity, alloc.vec.Vec.new]

/-- Interpret projected nodes using the reviewed normal-return `Op` datatype.
The lookup is arbitrary: quotation edges may not be reordered or substituted. -/
def meaning (lookup : U32 → NobleContracts.Op) : wire.SemanticNode → NobleContracts.Op
  | .I64 value => .lit (.i64 value.bv)
  | .Boolean value => .lit (.bool value)
  | .UnitValue => .lit .unit
  | .Word value => .word value.val
  | .Quotation body => .block (body.val.map lookup)

/-- The source graph's meaning, independent of the executable projection. -/
def sourceMeaning (lookup : U32 → NobleContracts.Op) :
    noble_contracts.noble_kernel.untrusted.Node → Option NobleContracts.Op
  | .Literal (.I64Lit value) _ => some (.lit (.i64 value.bv))
  | .Literal (.BoolLit value) _ => some (.lit (.bool value))
  | .Literal .UnitLit _ => some (.lit .unit)
  | .Literal .TextLit _ => none
  | .Invocation word _ => if word < 22#u32 then some (.word word.val) else none
  | .Quotation body _ => some (.block (body.val.map lookup))

/-- Successful projection preserves literal bits, primitive identity, and the
entire ordered quotation body for every node and every reference environment.
Typing witnesses cannot alter that executable meaning. -/
theorem lower_node_refines
    (lookup : U32 → NobleContracts.Op)
    (node : noble_contracts.noble_kernel.untrusted.Node) :
    wire.lower_node node ⦃ result =>
      match result with
      | .Ok projected => sourceMeaning lookup node = some (meaning lookup projected)
      | .Err _ => sourceMeaning lookup node = none ⦄ := by
  cases node with
  | Literal value inst => cases value <;> simp [wire.lower_node, sourceMeaning, meaning]
  | Invocation word inst =>
    simp only [wire.lower_node]
    split <;> simp_all [sourceMeaning, meaning]
  | Quotation body inst =>
    simp only [wire.lower_node]
    step as ⟨projected, hprojected⟩
    simp [sourceMeaning, meaning, hprojected, alloc.vec.Vec.deref]

/-- In particular, no host/unknown primitive obtains an executable projection. -/
theorem rejects_nonpure_word (word : U32) (inst : noble_contracts.noble_kernel.words.Inst)
    (h : ¬ word < 22#u32) :
    wire.lower_node (.Invocation word inst) =
      ok (.Err wire.ProjectionError.HostWord) := by
  simp [wire.lower_node, h]

end NobleContractImpl.Projection
