import NobleFoundation.NetI64Plans

/-! Finite Rust compiler-to-Lean byte correspondence. These literal bytes come only
from rewrite::compile, then pass rewrite::check. The corresponding Lean fixture,
parser, rule and physical executor are independent obligations, not a universal
Rust compiler correctness proof. Regenerate with:
node verification/net-plans/regenerate-lean-corpus.mjs
-/
-- contract=ngraph-net-i64/3; plan=ngraph-net-plan/1
-- rules=noble-net-i64-rules/3; representation=noble-net-arena/1
-- checker=noble-net-plan-checker/1; executor=noble-net-plan-executor/1
-- source crates/noble-ngraph/src/net/rewrite.rs test-net-digest=dbda431a711b626e76b4951a7d22375fcd9a4274d1f19e401641768e617681c3
-- source crates/noble-ngraph/src/net.rs test-net-digest=fd4090460eb9c8d0828f983323b5b523a7774b6af69727630012dc2f6857a68b
-- source crates/noble-ngraph/src/net/plan/corpus.rs test-net-digest=06fad4f5918cb804b2a2923029e8138581f3a7ee7db445528b82cd713d6c59f1
-- source proofs/foundation/NobleFoundation/NetI64Plans.lean test-net-digest=49beda9c3a213719041a30d761a28678b51814fdaf2b12ab2dc557c7e1917d9b
-- length-framed emitted byte stream + revisions test-net-digest=9690658793bcea456e55e6db6e60b2c9d34d8d0fa705cdefe64182743ab572db
-- test-net is a test-only identity scheme, not a cryptographic signature.

namespace NobleFoundation.NetI64PlanCorpus

open NobleFoundation.NetI64
open NobleFoundation.NetI64Plans

-- op2ForwardReuse: emitted_bytes=104; rules+checker+bytes test-net-digest=3cf3c7f7b77ab7fab07c55eebfa20b8d229e52b9b488f8869a3adf49501b40a5
def op2ForwardReuseBytes : List UInt8 :=
  [78, 80, 76, 78, 1, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0] ++
  [0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1] ++
  [0, 2, 0, 0, 0, 0, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0] ++
  [2, 1, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 5, 27, 0, 0] ++
  [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 255, 2, 0, 0, 0] ++
  [0, 1, 1, 2, 0, 0, 0, 0, 1, 1, 0, 0, 0, 0, 0, 1] ++
  [0, 0, 0, 0, 0, 0, 0, 199]

theorem op2ForwardReuse_decoded_fields_and_symbols :
  (BytePlan.decode op2ForwardReuseBytes).isSome = true ∧
  (compileRule (.op2 .add) (.num (BitVec.ofNat 64 27))).isSome = true ∧
  ((BytePlan.decode op2ForwardReuseBytes).map (fun image =>
    (image.reuse, image.opcode.toNat, image.pair,
     image.action.1.toNat, image.action.2.toNat,
     image.boundaries.length, image.results.length, image.writes.length,
     image.externalWrites.length, image.arenaReservation,
     image.scratchReservation, image.worklistReservation,
     image.publications.length,
     image.results.map (fun result => result.2.2.1)))) =
  (compileRule (.op2 .add) (.num (BitVec.ofNat 64 27))).map (fun selected =>
    (true, 1, (0, 0, 1, 0),
     1, 0,
     2, 1, 2, 2,
     0, 256, 0, 0, selected.results.map Prod.snd)) := by
  constructor
  · set_option maxRecDepth 3000 in decide
  constructor
  · set_option maxRecDepth 3000 in decide
  · set_option maxRecDepth 3000 in rfl

-- The model fixture was declared independently of the Rust writer.
theorem op2ForwardReuse_matches_model : op2ForwardReuseBytes = op2OutputFixture.emit := by
  decide

theorem op2ForwardReuse_physical :
  op2PhysicalStart.executeCheckedImage op2ForwardReuseBytes op2PhysicalRedex =
    some op2PhysicalAfter := by
  rw [op2ForwardReuse_matches_model]
  exact PhysicalState.op2_output_execution_exact

-- op2ForwardAllocate: emitted_bytes=104; rules+checker+bytes test-net-digest=3cf4c7f7b77af6fab07c55ee1ea20b8d229e52b9b488f8869a3adf49501b5fe3
def op2ForwardAllocateBytes : List UInt8 :=
  [78, 80, 76, 78, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0] ++
  [0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0] ++
  [0, 2, 0, 0, 0, 0, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0] ++
  [2, 1, 1, 0, 0, 0, 1, 1, 2, 0, 0, 0, 5, 27, 0, 0] ++
  [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 255, 2, 0, 0, 0] ++
  [0, 1, 1, 2, 0, 0, 0, 0, 1, 1, 1, 0, 0, 0, 0, 1] ++
  [0, 0, 0, 0, 0, 0, 0, 199]

theorem op2ForwardAllocate_decoded_fields_and_symbols :
  (BytePlan.decode op2ForwardAllocateBytes).isSome = true ∧
  (compileRule (.op2 .add) (.num (BitVec.ofNat 64 27))).isSome = true ∧
  ((BytePlan.decode op2ForwardAllocateBytes).map (fun image =>
    (image.reuse, image.opcode.toNat, image.pair,
     image.action.1.toNat, image.action.2.toNat,
     image.boundaries.length, image.results.length, image.writes.length,
     image.externalWrites.length, image.arenaReservation,
     image.scratchReservation, image.worklistReservation,
     image.publications.length,
     image.results.map (fun result => result.2.2.1)))) =
  (compileRule (.op2 .add) (.num (BitVec.ofNat 64 27))).map (fun selected =>
    (false, 1, (0, 0, 1, 0),
     0, 0,
     2, 1, 2, 2,
     1, 256, 0, 0, selected.results.map Prod.snd)) := by
  constructor
  · set_option maxRecDepth 3000 in decide
  constructor
  · set_option maxRecDepth 3000 in decide
  · set_option maxRecDepth 3000 in rfl

-- op2ReverseReuse: emitted_bytes=104; rules+checker+bytes test-net-digest=3cf3c7f7b77af6bbb03d55eedea20b8d229e52d8b488f8869a3adf49501b40c4
def op2ReverseReuseBytes : List UInt8 :=
  [78, 80, 76, 78, 1, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0] ++
  [0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0] ++
  [1, 2, 1, 0, 0, 0, 1, 1, 0, 0, 0, 0, 1, 0, 0, 0] ++
  [2, 1, 1, 0, 0, 0, 1, 0, 1, 0, 0, 0, 5, 27, 0, 0] ++
  [0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 255, 2, 0, 0, 0] ++
  [0, 1, 1, 2, 0, 0, 0, 0, 1, 1, 0, 0, 0, 0, 0, 1] ++
  [0, 0, 0, 0, 0, 0, 0, 199]

theorem op2ReverseReuse_decoded_fields_and_symbols :
  (BytePlan.decode op2ReverseReuseBytes).isSome = true ∧
  (compileRule (.num (BitVec.ofNat 64 27)) (.op2 .add)).isSome = true ∧
  ((BytePlan.decode op2ReverseReuseBytes).map (fun image =>
    (image.reuse, image.opcode.toNat, image.pair,
     image.action.1.toNat, image.action.2.toNat,
     image.boundaries.length, image.results.length, image.writes.length,
     image.externalWrites.length, image.arenaReservation,
     image.scratchReservation, image.worklistReservation,
     image.publications.length,
     image.results.map (fun result => result.2.2.1)))) =
  (compileRule (.num (BitVec.ofNat 64 27)) (.op2 .add)).map (fun selected =>
    (true, 1, (0, 0, 1, 0),
     0, 1,
     2, 1, 2, 2,
     0, 256, 0, 0, selected.results.map Prod.snd)) := by
  constructor
  · set_option maxRecDepth 3000 in decide
  constructor
  · set_option maxRecDepth 3000 in decide
  · set_option maxRecDepth 3000 in rfl

-- op2ReverseAllocate: emitted_bytes=104; rules+checker+bytes test-net-digest=3cf4c7f7b77af6fab03d55ee3da20b8d229e52d8b488f8869a3adf49501b5fe3
def op2ReverseAllocateBytes : List UInt8 :=
  [78, 80, 76, 78, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0] ++
  [0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0] ++
  [0, 2, 1, 0, 0, 0, 1, 1, 0, 0, 0, 0, 1, 0, 0, 0] ++
  [2, 1, 1, 0, 0, 0, 1, 1, 2, 0, 0, 0, 5, 27, 0, 0] ++
  [0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 255, 2, 0, 0, 0] ++
  [0, 1, 1, 2, 0, 0, 0, 0, 1, 1, 1, 0, 0, 0, 0, 1] ++
  [0, 0, 0, 0, 0, 0, 0, 199]

theorem op2ReverseAllocate_decoded_fields_and_symbols :
  (BytePlan.decode op2ReverseAllocateBytes).isSome = true ∧
  (compileRule (.num (BitVec.ofNat 64 27)) (.op2 .add)).isSome = true ∧
  ((BytePlan.decode op2ReverseAllocateBytes).map (fun image =>
    (image.reuse, image.opcode.toNat, image.pair,
     image.action.1.toNat, image.action.2.toNat,
     image.boundaries.length, image.results.length, image.writes.length,
     image.externalWrites.length, image.arenaReservation,
     image.scratchReservation, image.worklistReservation,
     image.publications.length,
     image.results.map (fun result => result.2.2.1)))) =
  (compileRule (.num (BitVec.ofNat 64 27)) (.op2 .add)).map (fun selected =>
    (false, 1, (0, 0, 1, 0),
     0, 0,
     2, 1, 2, 2,
     1, 256, 0, 0, selected.results.map Prod.snd)) := by
  constructor
  · set_option maxRecDepth 3000 in decide
  constructor
  · set_option maxRecDepth 3000 in decide
  · set_option maxRecDepth 3000 in rfl

-- op1ForwardReuse: emitted_bytes=88; rules+checker+bytes test-net-digest=64cd9b87d2eb50ac382e55bded8d0a8d229d53d6b488f9487a1a232a31dde8da
def op1ForwardReuseBytes : List UInt8 :=
  [78, 80, 76, 78, 1, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0] ++
  [0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1] ++
  [0, 1, 0, 0, 0, 0, 1, 1, 0, 0, 0, 0, 1, 0, 0, 0] ++
  [0, 0, 0, 250, 255, 255, 255, 255, 255, 255, 255, 0, 0, 0, 0, 0] ++
  [255, 255, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 1] ++
  [0, 0, 0, 0, 0, 0, 0, 199]

theorem op1ForwardReuse_decoded_fields_and_symbols :
  (BytePlan.decode op1ForwardReuseBytes).isSome = true ∧
  (compileRule (.op1 .sub (BitVec.ofNat 64 7)) (.num (BitVec.ofNat 64 13))).isSome = true ∧
  ((BytePlan.decode op1ForwardReuseBytes).map (fun image =>
    (image.reuse, image.opcode.toNat, image.pair,
     image.action.1.toNat, image.action.2.toNat,
     image.boundaries.length, image.results.length, image.writes.length,
     image.externalWrites.length, image.arenaReservation,
     image.scratchReservation, image.worklistReservation,
     image.publications.length,
     image.results.map (fun result => result.2.2.1)))) =
  (compileRule (.op1 .sub (BitVec.ofNat 64 7)) (.num (BitVec.ofNat 64 13))).map (fun selected =>
    (true, 2, (0, 0, 1, 0),
     1, 0,
     1, 1, 1, 1,
     0, 256, 0, 0, selected.results.map Prod.snd)) := by
  constructor
  · set_option maxRecDepth 3000 in decide
  constructor
  · set_option maxRecDepth 3000 in decide
  · set_option maxRecDepth 3000 in rfl

-- The model fixture was declared independently of the Rust writer.
theorem op1ForwardReuse_matches_model : op1ForwardReuseBytes = op1OutputFixture.emit := by
  decide

theorem op1ForwardReuse_physical :
  op1PhysicalStart.executeCheckedImage op1ForwardReuseBytes op1PhysicalRedex =
    some op1PhysicalAfter := by
  rw [op1ForwardReuse_matches_model]
  exact PhysicalState.op1_wrapping_execution_exact

-- op1ForwardAllocate: emitted_bytes=88; rules+checker+bytes test-net-digest=64cd9b87d2eb31ac382e55bdae8d0a8d229e53d6d3c6f9487a1a232a31dde8da
def op1ForwardAllocateBytes : List UInt8 :=
  [78, 80, 76, 78, 1, 1, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0] ++
  [0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0] ++
  [0, 1, 0, 0, 0, 0, 1, 1, 0, 0, 0, 0, 1, 1, 2, 0] ++
  [0, 0, 0, 250, 255, 255, 255, 255, 255, 255, 255, 0, 0, 0, 0, 0] ++
  [255, 255, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 0, 1] ++
  [0, 0, 0, 0, 0, 0, 0, 199]

theorem op1ForwardAllocate_decoded_fields_and_symbols :
  (BytePlan.decode op1ForwardAllocateBytes).isSome = true ∧
  (compileRule (.op1 .sub (BitVec.ofNat 64 7)) (.num (BitVec.ofNat 64 13))).isSome = true ∧
  ((BytePlan.decode op1ForwardAllocateBytes).map (fun image =>
    (image.reuse, image.opcode.toNat, image.pair,
     image.action.1.toNat, image.action.2.toNat,
     image.boundaries.length, image.results.length, image.writes.length,
     image.externalWrites.length, image.arenaReservation,
     image.scratchReservation, image.worklistReservation,
     image.publications.length,
     image.results.map (fun result => result.2.2.1)))) =
  (compileRule (.op1 .sub (BitVec.ofNat 64 7)) (.num (BitVec.ofNat 64 13))).map (fun selected =>
    (false, 2, (0, 0, 1, 0),
     0, 0,
     1, 1, 1, 1,
     1, 256, 0, 0, selected.results.map Prod.snd)) := by
  constructor
  · set_option maxRecDepth 3000 in decide
  constructor
  · set_option maxRecDepth 3000 in decide
  · set_option maxRecDepth 3000 in rfl

-- op1ReverseReuse: emitted_bytes=88; rules+checker+bytes test-net-digest=64cd9c87d2eb31cb384d55bded8d0a8d229d53d6b4a7f9487a1a232a31dde8da
def op1ReverseReuseBytes : List UInt8 :=
  [78, 80, 76, 78, 1, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0] ++
  [0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0] ++
  [1, 1, 1, 0, 0, 0, 1, 1, 0, 0, 0, 0, 1, 0, 1, 0] ++
  [0, 0, 0, 250, 255, 255, 255, 255, 255, 255, 255, 1, 0, 0, 0, 0] ++
  [255, 255, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 1] ++
  [0, 0, 0, 0, 0, 0, 0, 199]

theorem op1ReverseReuse_decoded_fields_and_symbols :
  (BytePlan.decode op1ReverseReuseBytes).isSome = true ∧
  (compileRule (.num (BitVec.ofNat 64 13)) (.op1 .sub (BitVec.ofNat 64 7))).isSome = true ∧
  ((BytePlan.decode op1ReverseReuseBytes).map (fun image =>
    (image.reuse, image.opcode.toNat, image.pair,
     image.action.1.toNat, image.action.2.toNat,
     image.boundaries.length, image.results.length, image.writes.length,
     image.externalWrites.length, image.arenaReservation,
     image.scratchReservation, image.worklistReservation,
     image.publications.length,
     image.results.map (fun result => result.2.2.1)))) =
  (compileRule (.num (BitVec.ofNat 64 13)) (.op1 .sub (BitVec.ofNat 64 7))).map (fun selected =>
    (true, 2, (0, 0, 1, 0),
     0, 1,
     1, 1, 1, 1,
     0, 256, 0, 0, selected.results.map Prod.snd)) := by
  constructor
  · set_option maxRecDepth 3000 in decide
  constructor
  · set_option maxRecDepth 3000 in decide
  · set_option maxRecDepth 3000 in rfl

-- op1ReverseAllocate: emitted_bytes=88; rules+checker+bytes test-net-digest=64cd9c87d2eb31ac384d55bdae8d0a8d229e53d6d3c6f9487a1a232a31dde8da
def op1ReverseAllocateBytes : List UInt8 :=
  [78, 80, 76, 78, 1, 1, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0] ++
  [0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0] ++
  [0, 1, 1, 0, 0, 0, 1, 1, 0, 0, 0, 0, 1, 1, 2, 0] ++
  [0, 0, 0, 250, 255, 255, 255, 255, 255, 255, 255, 1, 0, 0, 0, 0] ++
  [255, 255, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 0, 1] ++
  [0, 0, 0, 0, 0, 0, 0, 199]

theorem op1ReverseAllocate_decoded_fields_and_symbols :
  (BytePlan.decode op1ReverseAllocateBytes).isSome = true ∧
  (compileRule (.num (BitVec.ofNat 64 13)) (.op1 .sub (BitVec.ofNat 64 7))).isSome = true ∧
  ((BytePlan.decode op1ReverseAllocateBytes).map (fun image =>
    (image.reuse, image.opcode.toNat, image.pair,
     image.action.1.toNat, image.action.2.toNat,
     image.boundaries.length, image.results.length, image.writes.length,
     image.externalWrites.length, image.arenaReservation,
     image.scratchReservation, image.worklistReservation,
     image.publications.length,
     image.results.map (fun result => result.2.2.1)))) =
  (compileRule (.num (BitVec.ofNat 64 13)) (.op1 .sub (BitVec.ofNat 64 7))).map (fun selected =>
    (false, 2, (0, 0, 1, 0),
     0, 0,
     1, 1, 1, 1,
     1, 256, 0, 0, selected.results.map Prod.snd)) := by
  constructor
  · set_option maxRecDepth 3000 in decide
  constructor
  · set_option maxRecDepth 3000 in decide
  · set_option maxRecDepth 3000 in rfl

-- dupForwardReuse: emitted_bytes=125; rules+checker+bytes test-net-digest=258d1bee887bb5bab27c55eec06045242ef5003dff8e1e519f16ddf3cc23defb
def dupForwardReuseBytes : List UInt8 :=
  [78, 80, 76, 78, 1, 0, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0] ++
  [0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1] ++
  [1, 2, 0, 0, 0, 0, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0] ++
  [2, 1, 1, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 42, 0, 0] ++
  [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 255, 255, 0, 1, 0, 0] ++
  [0, 0, 42, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 1, 255] ++
  [255, 2, 0, 0, 0, 1, 0, 1, 2, 0, 0, 0, 1, 0, 1, 0] ++
  [0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 199]

theorem dupForwardReuse_decoded_fields_and_symbols :
  (BytePlan.decode dupForwardReuseBytes).isSome = true ∧
  (compileRule (.dup) (.num (BitVec.ofNat 64 42))).isSome = true ∧
  ((BytePlan.decode dupForwardReuseBytes).map (fun image =>
    (image.reuse, image.opcode.toNat, image.pair,
     image.action.1.toNat, image.action.2.toNat,
     image.boundaries.length, image.results.length, image.writes.length,
     image.externalWrites.length, image.arenaReservation,
     image.scratchReservation, image.worklistReservation,
     image.publications.length,
     image.results.map (fun result => result.2.2.1)))) =
  (compileRule (.dup) (.num (BitVec.ofNat 64 42))).map (fun selected =>
    (true, 3, (0, 0, 1, 0),
     1, 1,
     2, 2, 2, 2,
     0, 256, 0, 0, selected.results.map Prod.snd)) := by
  constructor
  · set_option maxRecDepth 3000 in decide
  constructor
  · set_option maxRecDepth 3000 in decide
  · set_option maxRecDepth 3000 in rfl

-- The model fixture was declared independently of the Rust writer.
theorem dupForwardReuse_matches_model : dupForwardReuseBytes = dupOutputFixture.emit := by
  decide

theorem dupForwardReuse_physical :
  dupPhysicalStart.executeCheckedImage dupForwardReuseBytes dupPhysicalRedex =
    some dupPhysicalAfter := by
  rw [dupForwardReuse_matches_model]
  exact PhysicalState.dup_execution_exact

-- dupForwardAllocate: emitted_bytes=125; rules+checker+bytes test-net-digest=258d1bee887bf4f9b27c55ee1f6045242ef5005c3d8e20519f16ddf3cc239f7d
def dupForwardAllocateBytes : List UInt8 :=
  [78, 80, 76, 78, 1, 1, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0] ++
  [0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0] ++
  [0, 2, 0, 0, 0, 0, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0] ++
  [2, 1, 1, 0, 0, 0, 2, 1, 2, 0, 0, 0, 0, 42, 0, 0] ++
  [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 255, 255, 1, 3, 0, 0] ++
  [0, 0, 42, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 1, 255] ++
  [255, 2, 0, 0, 0, 1, 0, 1, 2, 0, 0, 0, 1, 0, 1, 2] ++
  [0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 199]

theorem dupForwardAllocate_decoded_fields_and_symbols :
  (BytePlan.decode dupForwardAllocateBytes).isSome = true ∧
  (compileRule (.dup) (.num (BitVec.ofNat 64 42))).isSome = true ∧
  ((BytePlan.decode dupForwardAllocateBytes).map (fun image =>
    (image.reuse, image.opcode.toNat, image.pair,
     image.action.1.toNat, image.action.2.toNat,
     image.boundaries.length, image.results.length, image.writes.length,
     image.externalWrites.length, image.arenaReservation,
     image.scratchReservation, image.worklistReservation,
     image.publications.length,
     image.results.map (fun result => result.2.2.1)))) =
  (compileRule (.dup) (.num (BitVec.ofNat 64 42))).map (fun selected =>
    (false, 3, (0, 0, 1, 0),
     0, 0,
     2, 2, 2, 2,
     2, 256, 0, 0, selected.results.map Prod.snd)) := by
  constructor
  · set_option maxRecDepth 3000 in decide
  constructor
  · set_option maxRecDepth 3000 in decide
  · set_option maxRecDepth 3000 in rfl

-- dupReverseReuse: emitted_bytes=125; rules+checker+bytes test-net-digest=256e1bee887bb5bab23d55eedf6045242ef500fee08e1e519f16ddf3cc23debc
def dupReverseReuseBytes : List UInt8 :=
  [78, 80, 76, 78, 1, 0, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0] ++
  [0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1] ++
  [1, 2, 1, 0, 0, 0, 1, 1, 0, 0, 0, 0, 1, 0, 0, 0] ++
  [2, 1, 1, 0, 0, 0, 2, 0, 1, 0, 0, 0, 0, 42, 0, 0] ++
  [0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 255, 255, 0, 0, 0, 0] ++
  [0, 0, 42, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 255] ++
  [255, 2, 0, 0, 0, 1, 0, 1, 2, 0, 0, 0, 1, 0, 1, 0] ++
  [0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 199]

theorem dupReverseReuse_decoded_fields_and_symbols :
  (BytePlan.decode dupReverseReuseBytes).isSome = true ∧
  (compileRule (.num (BitVec.ofNat 64 42)) (.dup)).isSome = true ∧
  ((BytePlan.decode dupReverseReuseBytes).map (fun image =>
    (image.reuse, image.opcode.toNat, image.pair,
     image.action.1.toNat, image.action.2.toNat,
     image.boundaries.length, image.results.length, image.writes.length,
     image.externalWrites.length, image.arenaReservation,
     image.scratchReservation, image.worklistReservation,
     image.publications.length,
     image.results.map (fun result => result.2.2.1)))) =
  (compileRule (.num (BitVec.ofNat 64 42)) (.dup)).map (fun selected =>
    (true, 3, (0, 0, 1, 0),
     1, 1,
     2, 2, 2, 2,
     0, 256, 0, 0, selected.results.map Prod.snd)) := by
  constructor
  · set_option maxRecDepth 3000 in decide
  constructor
  · set_option maxRecDepth 3000 in decide
  · set_option maxRecDepth 3000 in rfl

-- dupReverseAllocate: emitted_bytes=125; rules+checker+bytes test-net-digest=256e1bee887bf4f9b23d55ee3e6045242ef5001d3d8e20519f16ddf3cc239f7d
def dupReverseAllocateBytes : List UInt8 :=
  [78, 80, 76, 78, 1, 1, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0] ++
  [0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0] ++
  [0, 2, 1, 0, 0, 0, 1, 1, 0, 0, 0, 0, 1, 0, 0, 0] ++
  [2, 1, 1, 0, 0, 0, 2, 1, 2, 0, 0, 0, 0, 42, 0, 0] ++
  [0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 255, 255, 1, 3, 0, 0] ++
  [0, 0, 42, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 255] ++
  [255, 2, 0, 0, 0, 1, 0, 1, 2, 0, 0, 0, 1, 0, 1, 2] ++
  [0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 199]

theorem dupReverseAllocate_decoded_fields_and_symbols :
  (BytePlan.decode dupReverseAllocateBytes).isSome = true ∧
  (compileRule (.num (BitVec.ofNat 64 42)) (.dup)).isSome = true ∧
  ((BytePlan.decode dupReverseAllocateBytes).map (fun image =>
    (image.reuse, image.opcode.toNat, image.pair,
     image.action.1.toNat, image.action.2.toNat,
     image.boundaries.length, image.results.length, image.writes.length,
     image.externalWrites.length, image.arenaReservation,
     image.scratchReservation, image.worklistReservation,
     image.publications.length,
     image.results.map (fun result => result.2.2.1)))) =
  (compileRule (.num (BitVec.ofNat 64 42)) (.dup)).map (fun selected =>
    (false, 3, (0, 0, 1, 0),
     0, 0,
     2, 2, 2, 2,
     2, 256, 0, 0, selected.results.map Prod.snd)) := by
  constructor
  · set_option maxRecDepth 3000 in decide
  constructor
  · set_option maxRecDepth 3000 in decide
  · set_option maxRecDepth 3000 in rfl

-- eraForwardReuse: emitted_bytes=51; rules+checker+bytes test-net-digest=fb9347fc725a759aae5c550effde83c2aef59fdc602ddeaf5ea6dd29d05c80c4
def eraForwardReuseBytes : List UInt8 :=
  [78, 80, 76, 78, 1, 0, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0] ++
  [0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0] ++
  [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0] ++
  [0, 0, 199]

theorem eraForwardReuse_decoded_fields_and_symbols :
  (BytePlan.decode eraForwardReuseBytes).isSome = true ∧
  (compileRule (.era) (.num (BitVec.ofNat 64 42))).isSome = true ∧
  ((BytePlan.decode eraForwardReuseBytes).map (fun image =>
    (image.reuse, image.opcode.toNat, image.pair,
     image.action.1.toNat, image.action.2.toNat,
     image.boundaries.length, image.results.length, image.writes.length,
     image.externalWrites.length, image.arenaReservation,
     image.scratchReservation, image.worklistReservation,
     image.publications.length,
     image.results.map (fun result => result.2.2.1)))) =
  (compileRule (.era) (.num (BitVec.ofNat 64 42))).map (fun selected =>
    (true, 4, (0, 0, 1, 0),
     0, 0,
     0, 0, 0, 0,
     0, 256, 0, 0, selected.results.map Prod.snd)) := by
  constructor
  · set_option maxRecDepth 3000 in decide
  constructor
  · set_option maxRecDepth 3000 in decide
  · set_option maxRecDepth 3000 in rfl

-- The model fixture was declared independently of the Rust writer.
theorem eraForwardReuse_matches_model : eraForwardReuseBytes = eraPlanBytes := by
  decide

theorem eraForwardReuse_physical :
  eraPhysicalStart.executeCheckedImage eraForwardReuseBytes eraPhysicalRedex =
    some eraPhysicalAfter := by
  rw [eraForwardReuse_matches_model]
  exact PhysicalState.era_execution_exact

-- eraForwardAllocate: emitted_bytes=51; rules+checker+bytes test-net-digest=fb9347fc725a759aae5c550e1ede83c2aef59fdc602ddeaf5ea6dd29d05c80c4
def eraForwardAllocateBytes : List UInt8 :=
  [78, 80, 76, 78, 1, 1, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0] ++
  [0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0] ++
  [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0] ++
  [0, 0, 199]

theorem eraForwardAllocate_decoded_fields_and_symbols :
  (BytePlan.decode eraForwardAllocateBytes).isSome = true ∧
  (compileRule (.era) (.num (BitVec.ofNat 64 42))).isSome = true ∧
  ((BytePlan.decode eraForwardAllocateBytes).map (fun image =>
    (image.reuse, image.opcode.toNat, image.pair,
     image.action.1.toNat, image.action.2.toNat,
     image.boundaries.length, image.results.length, image.writes.length,
     image.externalWrites.length, image.arenaReservation,
     image.scratchReservation, image.worklistReservation,
     image.publications.length,
     image.results.map (fun result => result.2.2.1)))) =
  (compileRule (.era) (.num (BitVec.ofNat 64 42))).map (fun selected =>
    (false, 4, (0, 0, 1, 0),
     0, 0,
     0, 0, 0, 0,
     0, 256, 0, 0, selected.results.map Prod.snd)) := by
  constructor
  · set_option maxRecDepth 3000 in decide
  constructor
  · set_option maxRecDepth 3000 in decide
  · set_option maxRecDepth 3000 in rfl

-- eraReverseReuse: emitted_bytes=51; rules+checker+bytes test-net-digest=fb9347fc725a759aae5c550effde83c2aef59fdc602ddeaf5ea6dd29d05c80c4
def eraReverseReuseBytes : List UInt8 :=
  [78, 80, 76, 78, 1, 0, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0] ++
  [0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0] ++
  [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0] ++
  [0, 0, 199]

theorem eraReverseReuse_decoded_fields_and_symbols :
  (BytePlan.decode eraReverseReuseBytes).isSome = true ∧
  (compileRule (.num (BitVec.ofNat 64 42)) (.era)).isSome = true ∧
  ((BytePlan.decode eraReverseReuseBytes).map (fun image =>
    (image.reuse, image.opcode.toNat, image.pair,
     image.action.1.toNat, image.action.2.toNat,
     image.boundaries.length, image.results.length, image.writes.length,
     image.externalWrites.length, image.arenaReservation,
     image.scratchReservation, image.worklistReservation,
     image.publications.length,
     image.results.map (fun result => result.2.2.1)))) =
  (compileRule (.num (BitVec.ofNat 64 42)) (.era)).map (fun selected =>
    (true, 4, (0, 0, 1, 0),
     0, 0,
     0, 0, 0, 0,
     0, 256, 0, 0, selected.results.map Prod.snd)) := by
  constructor
  · set_option maxRecDepth 3000 in decide
  constructor
  · set_option maxRecDepth 3000 in decide
  · set_option maxRecDepth 3000 in rfl

-- eraReverseAllocate: emitted_bytes=51; rules+checker+bytes test-net-digest=fb9347fc725a759aae5c550e1ede83c2aef59fdc602ddeaf5ea6dd29d05c80c4
def eraReverseAllocateBytes : List UInt8 :=
  [78, 80, 76, 78, 1, 1, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0] ++
  [0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0] ++
  [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0] ++
  [0, 0, 199]

theorem eraReverseAllocate_decoded_fields_and_symbols :
  (BytePlan.decode eraReverseAllocateBytes).isSome = true ∧
  (compileRule (.num (BitVec.ofNat 64 42)) (.era)).isSome = true ∧
  ((BytePlan.decode eraReverseAllocateBytes).map (fun image =>
    (image.reuse, image.opcode.toNat, image.pair,
     image.action.1.toNat, image.action.2.toNat,
     image.boundaries.length, image.results.length, image.writes.length,
     image.externalWrites.length, image.arenaReservation,
     image.scratchReservation, image.worklistReservation,
     image.publications.length,
     image.results.map (fun result => result.2.2.1)))) =
  (compileRule (.num (BitVec.ofNat 64 42)) (.era)).map (fun selected =>
    (false, 4, (0, 0, 1, 0),
     0, 0,
     0, 0, 0, 0,
     0, 256, 0, 0, selected.results.map Prod.snd)) := by
  constructor
  · set_option maxRecDepth 3000 in decide
  constructor
  · set_option maxRecDepth 3000 in decide
  · set_option maxRecDepth 3000 in rfl

end NobleFoundation.NetI64PlanCorpus
