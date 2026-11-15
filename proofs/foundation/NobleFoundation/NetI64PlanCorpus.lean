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
-- source crates/noble-ngraph/src/net/rewrite.rs test-net-digest=2bdc30ff2aa3118e7c3dcb8bff53aa751133392b7a2df9c1a3d9671cfb96dd62
-- source crates/noble-ngraph/src/net.rs test-net-digest=c4e959a52256e503d129f78d8d14a64eae4170e2d5b9aa555b0ca42831d6c4c1
-- source crates/noble-ngraph/src/net/plan/corpus.rs test-net-digest=a0da2bed6e19aa8560d0a532a0220038da2a154b691aa71dbf2a0498ab039770
-- source proofs/foundation/NobleFoundation/NetI64Plans.lean + NetI64PlanLowering.lean test-net-digest=24b71371964527e383b93c9de1eb1dcdd2ede2747ff4e544b991e6c2a81afa75
-- length-framed emitted byte stream + revisions test-net-digest=cb66e2b1601e964ef5f830d94880b89588f39d9ea1b3b063e92050070dbb44fc
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

-- op2MaxForwardReuse: emitted_bytes=104; rules+checker+bytes test-net-digest=3cf3c7f7535b98db915d36cfbfa20b8d229e52b9b488f8869a3adf49501b40a5
def op2MaxForwardReuseBytes : List UInt8 :=
  [78, 80, 76, 78, 1, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0] ++
  [0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1] ++
  [0, 2, 0, 0, 0, 0, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0] ++
  [2, 1, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 5, 255, 255, 255] ++
  [255, 255, 255, 255, 255, 0, 0, 0, 0, 0, 1, 255, 2, 0, 0, 0] ++
  [0, 1, 1, 2, 0, 0, 0, 0, 1, 1, 0, 0, 0, 0, 0, 1] ++
  [0, 0, 0, 0, 0, 0, 0, 199]

theorem op2MaxForwardReuse_decoded_fields_and_symbols :
  (BytePlan.decode op2MaxForwardReuseBytes).isSome = true ∧
  (compileRule (.op2 .add) (.num (BitVec.ofNat 64 18446744073709551615))).isSome = true ∧
  ((BytePlan.decode op2MaxForwardReuseBytes).map (fun image =>
    (image.reuse, image.opcode.toNat, image.pair,
     image.action.1.toNat, image.action.2.toNat,
     image.boundaries.length, image.results.length, image.writes.length,
     image.externalWrites.length, image.arenaReservation,
     image.scratchReservation, image.worklistReservation,
     image.publications.length,
     image.results.map (fun result => result.2.2.1)))) =
  (compileRule (.op2 .add) (.num (BitVec.ofNat 64 18446744073709551615))).map (fun selected =>
    (true, 1, (0, 0, 1, 0),
     1, 0,
     2, 1, 2, 2,
     0, 256, 0, 0, selected.results.map Prod.snd)) := by
  constructor
  · set_option maxRecDepth 3000 in decide
  constructor
  · set_option maxRecDepth 3000 in decide
  · set_option maxRecDepth 3000 in rfl

-- op2MaxForwardAllocate: emitted_bytes=104; rules+checker+bytes test-net-digest=3cf4c7f7535bd7db915d36cf1ea20b8d229e52b9b488f8869a3adf49501b5fe3
def op2MaxForwardAllocateBytes : List UInt8 :=
  [78, 80, 76, 78, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0] ++
  [0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0] ++
  [0, 2, 0, 0, 0, 0, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0] ++
  [2, 1, 1, 0, 0, 0, 1, 1, 2, 0, 0, 0, 5, 255, 255, 255] ++
  [255, 255, 255, 255, 255, 0, 0, 0, 0, 0, 1, 255, 2, 0, 0, 0] ++
  [0, 1, 1, 2, 0, 0, 0, 0, 1, 1, 1, 0, 0, 0, 0, 1] ++
  [0, 0, 0, 0, 0, 0, 0, 199]

theorem op2MaxForwardAllocate_decoded_fields_and_symbols :
  (BytePlan.decode op2MaxForwardAllocateBytes).isSome = true ∧
  (compileRule (.op2 .add) (.num (BitVec.ofNat 64 18446744073709551615))).isSome = true ∧
  ((BytePlan.decode op2MaxForwardAllocateBytes).map (fun image =>
    (image.reuse, image.opcode.toNat, image.pair,
     image.action.1.toNat, image.action.2.toNat,
     image.boundaries.length, image.results.length, image.writes.length,
     image.externalWrites.length, image.arenaReservation,
     image.scratchReservation, image.worklistReservation,
     image.publications.length,
     image.results.map (fun result => result.2.2.1)))) =
  (compileRule (.op2 .add) (.num (BitVec.ofNat 64 18446744073709551615))).map (fun selected =>
    (false, 1, (0, 0, 1, 0),
     0, 0,
     2, 1, 2, 2,
     1, 256, 0, 0, selected.results.map Prod.snd)) := by
  constructor
  · set_option maxRecDepth 3000 in decide
  constructor
  · set_option maxRecDepth 3000 in decide
  · set_option maxRecDepth 3000 in rfl

-- op2MaxReverseReuse: emitted_bytes=104; rules+checker+bytes test-net-digest=3cf3c7f7535bd79c911e36cfdea20b8d229e52d8b488f8869a3adf49501b40c4
def op2MaxReverseReuseBytes : List UInt8 :=
  [78, 80, 76, 78, 1, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0] ++
  [0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0] ++
  [1, 2, 1, 0, 0, 0, 1, 1, 0, 0, 0, 0, 1, 0, 0, 0] ++
  [2, 1, 1, 0, 0, 0, 1, 0, 1, 0, 0, 0, 5, 255, 255, 255] ++
  [255, 255, 255, 255, 255, 1, 0, 0, 0, 0, 1, 255, 2, 0, 0, 0] ++
  [0, 1, 1, 2, 0, 0, 0, 0, 1, 1, 0, 0, 0, 0, 0, 1] ++
  [0, 0, 0, 0, 0, 0, 0, 199]

theorem op2MaxReverseReuse_decoded_fields_and_symbols :
  (BytePlan.decode op2MaxReverseReuseBytes).isSome = true ∧
  (compileRule (.num (BitVec.ofNat 64 18446744073709551615)) (.op2 .add)).isSome = true ∧
  ((BytePlan.decode op2MaxReverseReuseBytes).map (fun image =>
    (image.reuse, image.opcode.toNat, image.pair,
     image.action.1.toNat, image.action.2.toNat,
     image.boundaries.length, image.results.length, image.writes.length,
     image.externalWrites.length, image.arenaReservation,
     image.scratchReservation, image.worklistReservation,
     image.publications.length,
     image.results.map (fun result => result.2.2.1)))) =
  (compileRule (.num (BitVec.ofNat 64 18446744073709551615)) (.op2 .add)).map (fun selected =>
    (true, 1, (0, 0, 1, 0),
     0, 1,
     2, 1, 2, 2,
     0, 256, 0, 0, selected.results.map Prod.snd)) := by
  constructor
  · set_option maxRecDepth 3000 in decide
  constructor
  · set_option maxRecDepth 3000 in decide
  · set_option maxRecDepth 3000 in rfl

-- op2MaxReverseAllocate: emitted_bytes=104; rules+checker+bytes test-net-digest=3cf4c7f7535bd7db911e36cf3da20b8d229e52d8b488f8869a3adf49501b5fe3
def op2MaxReverseAllocateBytes : List UInt8 :=
  [78, 80, 76, 78, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0] ++
  [0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0] ++
  [0, 2, 1, 0, 0, 0, 1, 1, 0, 0, 0, 0, 1, 0, 0, 0] ++
  [2, 1, 1, 0, 0, 0, 1, 1, 2, 0, 0, 0, 5, 255, 255, 255] ++
  [255, 255, 255, 255, 255, 1, 0, 0, 0, 0, 1, 255, 2, 0, 0, 0] ++
  [0, 1, 1, 2, 0, 0, 0, 0, 1, 1, 1, 0, 0, 0, 0, 1] ++
  [0, 0, 0, 0, 0, 0, 0, 199]

theorem op2MaxReverseAllocate_decoded_fields_and_symbols :
  (BytePlan.decode op2MaxReverseAllocateBytes).isSome = true ∧
  (compileRule (.num (BitVec.ofNat 64 18446744073709551615)) (.op2 .add)).isSome = true ∧
  ((BytePlan.decode op2MaxReverseAllocateBytes).map (fun image =>
    (image.reuse, image.opcode.toNat, image.pair,
     image.action.1.toNat, image.action.2.toNat,
     image.boundaries.length, image.results.length, image.writes.length,
     image.externalWrites.length, image.arenaReservation,
     image.scratchReservation, image.worklistReservation,
     image.publications.length,
     image.results.map (fun result => result.2.2.1)))) =
  (compileRule (.num (BitVec.ofNat 64 18446744073709551615)) (.op2 .add)).map (fun selected =>
    (false, 1, (0, 0, 1, 0),
     0, 0,
     2, 1, 2, 2,
     1, 256, 0, 0, selected.results.map Prod.snd)) := by
  constructor
  · set_option maxRecDepth 3000 in decide
  constructor
  · set_option maxRecDepth 3000 in decide
  · set_option maxRecDepth 3000 in rfl

-- op1ZeroForwardReuse: emitted_bytes=88; rules+checker+bytes test-net-digest=65ce9b87d2eb50ac382e55bded8d0a8d229d53d6b488f9487a1ab64950fc07db
def op1ZeroForwardReuseBytes : List UInt8 :=
  [78, 80, 76, 78, 1, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0] ++
  [0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1] ++
  [0, 1, 0, 0, 0, 0, 1, 1, 0, 0, 0, 0, 1, 0, 0, 0] ++
  [0, 0, 0, 7, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0] ++
  [255, 255, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 1] ++
  [0, 0, 0, 0, 0, 0, 0, 199]

theorem op1ZeroForwardReuse_decoded_fields_and_symbols :
  (BytePlan.decode op1ZeroForwardReuseBytes).isSome = true ∧
  (compileRule (.op1 .sub (BitVec.ofNat 64 7)) (.num (BitVec.ofNat 64 0))).isSome = true ∧
  ((BytePlan.decode op1ZeroForwardReuseBytes).map (fun image =>
    (image.reuse, image.opcode.toNat, image.pair,
     image.action.1.toNat, image.action.2.toNat,
     image.boundaries.length, image.results.length, image.writes.length,
     image.externalWrites.length, image.arenaReservation,
     image.scratchReservation, image.worklistReservation,
     image.publications.length,
     image.results.map (fun result => result.2.2.1)))) =
  (compileRule (.op1 .sub (BitVec.ofNat 64 7)) (.num (BitVec.ofNat 64 0))).map (fun selected =>
    (true, 2, (0, 0, 1, 0),
     1, 0,
     1, 1, 1, 1,
     0, 256, 0, 0, selected.results.map Prod.snd)) := by
  constructor
  · set_option maxRecDepth 3000 in decide
  constructor
  · set_option maxRecDepth 3000 in decide
  · set_option maxRecDepth 3000 in rfl

-- op1ZeroForwardAllocate: emitted_bytes=88; rules+checker+bytes test-net-digest=65ce9b87d2eb31ac382e55bdae8d0a8d229e53d6d3c6f9487a1ab64950fc07db
def op1ZeroForwardAllocateBytes : List UInt8 :=
  [78, 80, 76, 78, 1, 1, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0] ++
  [0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0] ++
  [0, 1, 0, 0, 0, 0, 1, 1, 0, 0, 0, 0, 1, 1, 2, 0] ++
  [0, 0, 0, 7, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0] ++
  [255, 255, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 0, 1] ++
  [0, 0, 0, 0, 0, 0, 0, 199]

theorem op1ZeroForwardAllocate_decoded_fields_and_symbols :
  (BytePlan.decode op1ZeroForwardAllocateBytes).isSome = true ∧
  (compileRule (.op1 .sub (BitVec.ofNat 64 7)) (.num (BitVec.ofNat 64 0))).isSome = true ∧
  ((BytePlan.decode op1ZeroForwardAllocateBytes).map (fun image =>
    (image.reuse, image.opcode.toNat, image.pair,
     image.action.1.toNat, image.action.2.toNat,
     image.boundaries.length, image.results.length, image.writes.length,
     image.externalWrites.length, image.arenaReservation,
     image.scratchReservation, image.worklistReservation,
     image.publications.length,
     image.results.map (fun result => result.2.2.1)))) =
  (compileRule (.op1 .sub (BitVec.ofNat 64 7)) (.num (BitVec.ofNat 64 0))).map (fun selected =>
    (false, 2, (0, 0, 1, 0),
     0, 0,
     1, 1, 1, 1,
     1, 256, 0, 0, selected.results.map Prod.snd)) := by
  constructor
  · set_option maxRecDepth 3000 in decide
  constructor
  · set_option maxRecDepth 3000 in decide
  · set_option maxRecDepth 3000 in rfl

-- op1ZeroReverseReuse: emitted_bytes=88; rules+checker+bytes test-net-digest=65ce9c87d2eb31cb384d55bded8d0a8d229d53d6b4a7f9487a1ab64950fc07db
def op1ZeroReverseReuseBytes : List UInt8 :=
  [78, 80, 76, 78, 1, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0] ++
  [0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0] ++
  [1, 1, 1, 0, 0, 0, 1, 1, 0, 0, 0, 0, 1, 0, 1, 0] ++
  [0, 0, 0, 7, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0] ++
  [255, 255, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 1] ++
  [0, 0, 0, 0, 0, 0, 0, 199]

theorem op1ZeroReverseReuse_decoded_fields_and_symbols :
  (BytePlan.decode op1ZeroReverseReuseBytes).isSome = true ∧
  (compileRule (.num (BitVec.ofNat 64 0)) (.op1 .sub (BitVec.ofNat 64 7))).isSome = true ∧
  ((BytePlan.decode op1ZeroReverseReuseBytes).map (fun image =>
    (image.reuse, image.opcode.toNat, image.pair,
     image.action.1.toNat, image.action.2.toNat,
     image.boundaries.length, image.results.length, image.writes.length,
     image.externalWrites.length, image.arenaReservation,
     image.scratchReservation, image.worklistReservation,
     image.publications.length,
     image.results.map (fun result => result.2.2.1)))) =
  (compileRule (.num (BitVec.ofNat 64 0)) (.op1 .sub (BitVec.ofNat 64 7))).map (fun selected =>
    (true, 2, (0, 0, 1, 0),
     0, 1,
     1, 1, 1, 1,
     0, 256, 0, 0, selected.results.map Prod.snd)) := by
  constructor
  · set_option maxRecDepth 3000 in decide
  constructor
  · set_option maxRecDepth 3000 in decide
  · set_option maxRecDepth 3000 in rfl

-- op1ZeroReverseAllocate: emitted_bytes=88; rules+checker+bytes test-net-digest=65ce9c87d2eb31ac384d55bdae8d0a8d229e53d6d3c6f9487a1ab64950fc07db
def op1ZeroReverseAllocateBytes : List UInt8 :=
  [78, 80, 76, 78, 1, 1, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0] ++
  [0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0] ++
  [0, 1, 1, 0, 0, 0, 1, 1, 0, 0, 0, 0, 1, 1, 2, 0] ++
  [0, 0, 0, 7, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0] ++
  [255, 255, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 0, 1] ++
  [0, 0, 0, 0, 0, 0, 0, 199]

theorem op1ZeroReverseAllocate_decoded_fields_and_symbols :
  (BytePlan.decode op1ZeroReverseAllocateBytes).isSome = true ∧
  (compileRule (.num (BitVec.ofNat 64 0)) (.op1 .sub (BitVec.ofNat 64 7))).isSome = true ∧
  ((BytePlan.decode op1ZeroReverseAllocateBytes).map (fun image =>
    (image.reuse, image.opcode.toNat, image.pair,
     image.action.1.toNat, image.action.2.toNat,
     image.boundaries.length, image.results.length, image.writes.length,
     image.externalWrites.length, image.arenaReservation,
     image.scratchReservation, image.worklistReservation,
     image.publications.length,
     image.results.map (fun result => result.2.2.1)))) =
  (compileRule (.num (BitVec.ofNat 64 0)) (.op1 .sub (BitVec.ofNat 64 7))).map (fun selected =>
    (false, 2, (0, 0, 1, 0),
     0, 0,
     1, 1, 1, 1,
     1, 256, 0, 0, selected.results.map Prod.snd)) := by
  constructor
  · set_option maxRecDepth 3000 in decide
  constructor
  · set_option maxRecDepth 3000 in decide
  · set_option maxRecDepth 3000 in rfl

-- dupZeroForwardReuse: emitted_bytes=125; rules+checker+bytes test-net-digest=258d1bee727bb5bab27c55eec06045242ef5003dff8e1e519f00ddf3cc23defb
def dupZeroForwardReuseBytes : List UInt8 :=
  [78, 80, 76, 78, 1, 0, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0] ++
  [0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1] ++
  [1, 2, 0, 0, 0, 0, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0] ++
  [2, 1, 1, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0] ++
  [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 255, 255, 0, 1, 0, 0] ++
  [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 1, 255] ++
  [255, 2, 0, 0, 0, 1, 0, 1, 2, 0, 0, 0, 1, 0, 1, 0] ++
  [0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 199]

theorem dupZeroForwardReuse_decoded_fields_and_symbols :
  (BytePlan.decode dupZeroForwardReuseBytes).isSome = true ∧
  (compileRule (.dup) (.num (BitVec.ofNat 64 0))).isSome = true ∧
  ((BytePlan.decode dupZeroForwardReuseBytes).map (fun image =>
    (image.reuse, image.opcode.toNat, image.pair,
     image.action.1.toNat, image.action.2.toNat,
     image.boundaries.length, image.results.length, image.writes.length,
     image.externalWrites.length, image.arenaReservation,
     image.scratchReservation, image.worklistReservation,
     image.publications.length,
     image.results.map (fun result => result.2.2.1)))) =
  (compileRule (.dup) (.num (BitVec.ofNat 64 0))).map (fun selected =>
    (true, 3, (0, 0, 1, 0),
     1, 1,
     2, 2, 2, 2,
     0, 256, 0, 0, selected.results.map Prod.snd)) := by
  constructor
  · set_option maxRecDepth 3000 in decide
  constructor
  · set_option maxRecDepth 3000 in decide
  · set_option maxRecDepth 3000 in rfl

-- dupZeroForwardAllocate: emitted_bytes=125; rules+checker+bytes test-net-digest=258d1bee727bf4f9b27c55ee1f6045242ef5005c3d8e20519f00ddf3cc239f7d
def dupZeroForwardAllocateBytes : List UInt8 :=
  [78, 80, 76, 78, 1, 1, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0] ++
  [0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0] ++
  [0, 2, 0, 0, 0, 0, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0] ++
  [2, 1, 1, 0, 0, 0, 2, 1, 2, 0, 0, 0, 0, 0, 0, 0] ++
  [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 255, 255, 1, 3, 0, 0] ++
  [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 1, 255] ++
  [255, 2, 0, 0, 0, 1, 0, 1, 2, 0, 0, 0, 1, 0, 1, 2] ++
  [0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 199]

theorem dupZeroForwardAllocate_decoded_fields_and_symbols :
  (BytePlan.decode dupZeroForwardAllocateBytes).isSome = true ∧
  (compileRule (.dup) (.num (BitVec.ofNat 64 0))).isSome = true ∧
  ((BytePlan.decode dupZeroForwardAllocateBytes).map (fun image =>
    (image.reuse, image.opcode.toNat, image.pair,
     image.action.1.toNat, image.action.2.toNat,
     image.boundaries.length, image.results.length, image.writes.length,
     image.externalWrites.length, image.arenaReservation,
     image.scratchReservation, image.worklistReservation,
     image.publications.length,
     image.results.map (fun result => result.2.2.1)))) =
  (compileRule (.dup) (.num (BitVec.ofNat 64 0))).map (fun selected =>
    (false, 3, (0, 0, 1, 0),
     0, 0,
     2, 2, 2, 2,
     2, 256, 0, 0, selected.results.map Prod.snd)) := by
  constructor
  · set_option maxRecDepth 3000 in decide
  constructor
  · set_option maxRecDepth 3000 in decide
  · set_option maxRecDepth 3000 in rfl

-- dupZeroReverseReuse: emitted_bytes=125; rules+checker+bytes test-net-digest=256e1bee727bb5bab23d55eedf6045242ef500fee08e1e519f00ddf3cc23debc
def dupZeroReverseReuseBytes : List UInt8 :=
  [78, 80, 76, 78, 1, 0, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0] ++
  [0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1] ++
  [1, 2, 1, 0, 0, 0, 1, 1, 0, 0, 0, 0, 1, 0, 0, 0] ++
  [2, 1, 1, 0, 0, 0, 2, 0, 1, 0, 0, 0, 0, 0, 0, 0] ++
  [0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 255, 255, 0, 0, 0, 0] ++
  [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 255] ++
  [255, 2, 0, 0, 0, 1, 0, 1, 2, 0, 0, 0, 1, 0, 1, 0] ++
  [0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 199]

theorem dupZeroReverseReuse_decoded_fields_and_symbols :
  (BytePlan.decode dupZeroReverseReuseBytes).isSome = true ∧
  (compileRule (.num (BitVec.ofNat 64 0)) (.dup)).isSome = true ∧
  ((BytePlan.decode dupZeroReverseReuseBytes).map (fun image =>
    (image.reuse, image.opcode.toNat, image.pair,
     image.action.1.toNat, image.action.2.toNat,
     image.boundaries.length, image.results.length, image.writes.length,
     image.externalWrites.length, image.arenaReservation,
     image.scratchReservation, image.worklistReservation,
     image.publications.length,
     image.results.map (fun result => result.2.2.1)))) =
  (compileRule (.num (BitVec.ofNat 64 0)) (.dup)).map (fun selected =>
    (true, 3, (0, 0, 1, 0),
     1, 1,
     2, 2, 2, 2,
     0, 256, 0, 0, selected.results.map Prod.snd)) := by
  constructor
  · set_option maxRecDepth 3000 in decide
  constructor
  · set_option maxRecDepth 3000 in decide
  · set_option maxRecDepth 3000 in rfl

-- dupZeroReverseAllocate: emitted_bytes=125; rules+checker+bytes test-net-digest=256e1bee727bf4f9b23d55ee3e6045242ef5001d3d8e20519f00ddf3cc239f7d
def dupZeroReverseAllocateBytes : List UInt8 :=
  [78, 80, 76, 78, 1, 1, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0] ++
  [0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0] ++
  [0, 2, 1, 0, 0, 0, 1, 1, 0, 0, 0, 0, 1, 0, 0, 0] ++
  [2, 1, 1, 0, 0, 0, 2, 1, 2, 0, 0, 0, 0, 0, 0, 0] ++
  [0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 255, 255, 1, 3, 0, 0] ++
  [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 255] ++
  [255, 2, 0, 0, 0, 1, 0, 1, 2, 0, 0, 0, 1, 0, 1, 2] ++
  [0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 199]

theorem dupZeroReverseAllocate_decoded_fields_and_symbols :
  (BytePlan.decode dupZeroReverseAllocateBytes).isSome = true ∧
  (compileRule (.num (BitVec.ofNat 64 0)) (.dup)).isSome = true ∧
  ((BytePlan.decode dupZeroReverseAllocateBytes).map (fun image =>
    (image.reuse, image.opcode.toNat, image.pair,
     image.action.1.toNat, image.action.2.toNat,
     image.boundaries.length, image.results.length, image.writes.length,
     image.externalWrites.length, image.arenaReservation,
     image.scratchReservation, image.worklistReservation,
     image.publications.length,
     image.results.map (fun result => result.2.2.1)))) =
  (compileRule (.num (BitVec.ofNat 64 0)) (.dup)).map (fun selected =>
    (false, 3, (0, 0, 1, 0),
     0, 0,
     2, 2, 2, 2,
     2, 256, 0, 0, selected.results.map Prod.snd)) := by
  constructor
  · set_option maxRecDepth 3000 in decide
  constructor
  · set_option maxRecDepth 3000 in decide
  · set_option maxRecDepth 3000 in rfl

-- eraZeroForwardReuse: emitted_bytes=51; rules+checker+bytes test-net-digest=fb9347fc725a759aae5c550effde83c2aef59fdc602ddeaf5ea6dd29d05c80c4
def eraZeroForwardReuseBytes : List UInt8 :=
  [78, 80, 76, 78, 1, 0, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0] ++
  [0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0] ++
  [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0] ++
  [0, 0, 199]

theorem eraZeroForwardReuse_decoded_fields_and_symbols :
  (BytePlan.decode eraZeroForwardReuseBytes).isSome = true ∧
  (compileRule (.era) (.num (BitVec.ofNat 64 0))).isSome = true ∧
  ((BytePlan.decode eraZeroForwardReuseBytes).map (fun image =>
    (image.reuse, image.opcode.toNat, image.pair,
     image.action.1.toNat, image.action.2.toNat,
     image.boundaries.length, image.results.length, image.writes.length,
     image.externalWrites.length, image.arenaReservation,
     image.scratchReservation, image.worklistReservation,
     image.publications.length,
     image.results.map (fun result => result.2.2.1)))) =
  (compileRule (.era) (.num (BitVec.ofNat 64 0))).map (fun selected =>
    (true, 4, (0, 0, 1, 0),
     0, 0,
     0, 0, 0, 0,
     0, 256, 0, 0, selected.results.map Prod.snd)) := by
  constructor
  · set_option maxRecDepth 3000 in decide
  constructor
  · set_option maxRecDepth 3000 in decide
  · set_option maxRecDepth 3000 in rfl

-- eraZeroForwardAllocate: emitted_bytes=51; rules+checker+bytes test-net-digest=fb9347fc725a759aae5c550e1ede83c2aef59fdc602ddeaf5ea6dd29d05c80c4
def eraZeroForwardAllocateBytes : List UInt8 :=
  [78, 80, 76, 78, 1, 1, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0] ++
  [0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0] ++
  [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0] ++
  [0, 0, 199]

theorem eraZeroForwardAllocate_decoded_fields_and_symbols :
  (BytePlan.decode eraZeroForwardAllocateBytes).isSome = true ∧
  (compileRule (.era) (.num (BitVec.ofNat 64 0))).isSome = true ∧
  ((BytePlan.decode eraZeroForwardAllocateBytes).map (fun image =>
    (image.reuse, image.opcode.toNat, image.pair,
     image.action.1.toNat, image.action.2.toNat,
     image.boundaries.length, image.results.length, image.writes.length,
     image.externalWrites.length, image.arenaReservation,
     image.scratchReservation, image.worklistReservation,
     image.publications.length,
     image.results.map (fun result => result.2.2.1)))) =
  (compileRule (.era) (.num (BitVec.ofNat 64 0))).map (fun selected =>
    (false, 4, (0, 0, 1, 0),
     0, 0,
     0, 0, 0, 0,
     0, 256, 0, 0, selected.results.map Prod.snd)) := by
  constructor
  · set_option maxRecDepth 3000 in decide
  constructor
  · set_option maxRecDepth 3000 in decide
  · set_option maxRecDepth 3000 in rfl

-- eraZeroReverseReuse: emitted_bytes=51; rules+checker+bytes test-net-digest=fb9347fc725a759aae5c550effde83c2aef59fdc602ddeaf5ea6dd29d05c80c4
def eraZeroReverseReuseBytes : List UInt8 :=
  [78, 80, 76, 78, 1, 0, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0] ++
  [0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0] ++
  [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0] ++
  [0, 0, 199]

theorem eraZeroReverseReuse_decoded_fields_and_symbols :
  (BytePlan.decode eraZeroReverseReuseBytes).isSome = true ∧
  (compileRule (.num (BitVec.ofNat 64 0)) (.era)).isSome = true ∧
  ((BytePlan.decode eraZeroReverseReuseBytes).map (fun image =>
    (image.reuse, image.opcode.toNat, image.pair,
     image.action.1.toNat, image.action.2.toNat,
     image.boundaries.length, image.results.length, image.writes.length,
     image.externalWrites.length, image.arenaReservation,
     image.scratchReservation, image.worklistReservation,
     image.publications.length,
     image.results.map (fun result => result.2.2.1)))) =
  (compileRule (.num (BitVec.ofNat 64 0)) (.era)).map (fun selected =>
    (true, 4, (0, 0, 1, 0),
     0, 0,
     0, 0, 0, 0,
     0, 256, 0, 0, selected.results.map Prod.snd)) := by
  constructor
  · set_option maxRecDepth 3000 in decide
  constructor
  · set_option maxRecDepth 3000 in decide
  · set_option maxRecDepth 3000 in rfl

-- eraZeroReverseAllocate: emitted_bytes=51; rules+checker+bytes test-net-digest=fb9347fc725a759aae5c550e1ede83c2aef59fdc602ddeaf5ea6dd29d05c80c4
def eraZeroReverseAllocateBytes : List UInt8 :=
  [78, 80, 76, 78, 1, 1, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0] ++
  [0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0] ++
  [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0] ++
  [0, 0, 199]

theorem eraZeroReverseAllocate_decoded_fields_and_symbols :
  (BytePlan.decode eraZeroReverseAllocateBytes).isSome = true ∧
  (compileRule (.num (BitVec.ofNat 64 0)) (.era)).isSome = true ∧
  ((BytePlan.decode eraZeroReverseAllocateBytes).map (fun image =>
    (image.reuse, image.opcode.toNat, image.pair,
     image.action.1.toNat, image.action.2.toNat,
     image.boundaries.length, image.results.length, image.writes.length,
     image.externalWrites.length, image.arenaReservation,
     image.scratchReservation, image.worklistReservation,
     image.publications.length,
     image.results.map (fun result => result.2.2.1)))) =
  (compileRule (.num (BitVec.ofNat 64 0)) (.era)).map (fun selected =>
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
