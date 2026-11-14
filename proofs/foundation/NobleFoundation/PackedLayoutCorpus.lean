/-
Generated from accepted Rust CheckedGraph/PackedLayout pairs and refusal-checked single-field
copies by `cargo test -p noble-ngraph --test schedule_correspondence -- --ignored write_packed_layout_lean_file`.
The Lean model checks only its stated finite I64/Bool fragment; this is not a Rust refinement proof.
-/
import NobleFoundation.PackedLayoutEncoded

namespace NobleFoundation.PackedLayoutCorpus
open NobleFoundation.PackedSchedule
open NobleFoundation.PackedLayout
open NobleFoundation.PackedLayoutEncoded

def interleaved_real : Input := {
  nodes := 
  [.input 0,
   .input 1,
   .constant 0xFFFFFFFFFFFFFFFD,
   .add 0 1,
   .sub 1 0,
   .mul 3 2,
   .mul 4 2,
   .eq 5 6],
  layerCount := 4,
  failNodes := [],
  layerOf := [0, 0, 0, 1, 1, 2, 2, 3],
  tables := {
    revisionOk := true,
    regs := 9,
    registers := [some 0, some 1, some 2, some 3, some 4, some 5, some 6, some 7],
    consts := [(2, BitVec.ofNat 64 18446744073709551613)],
    slotRegs := [(0, 0), (1, 1)],
    layers := [(0, 0), (0, 2), (2, 4), (4, 5)],
    failSuffixMin := [NobleFoundation.PackedLayout.maxRegister, NobleFoundation.PackedLayout.maxRegister, NobleFoundation.PackedLayout.maxRegister, NobleFoundation.PackedLayout.maxRegister, NobleFoundation.PackedLayout.maxRegister]
  }
}

theorem interleaved_real_encoded_accepted :
    validateEncoded (encodeInput interleaved_real) = true := by
  rw [validateEncoded_agrees]
  decide

def two_constants_real : Input := {
  nodes := 
  [.input 0,
   .input 1,
   .constant 0x0000000000000003,
   .constant 0x0000000000000005,
   .add 0 1,
   .sub 1 0,
   .mul 4 2,
   .mul 5 3,
   .eq 6 7],
  layerCount := 4,
  failNodes := [],
  layerOf := [0, 0, 0, 0, 1, 1, 2, 2, 3],
  tables := {
    revisionOk := true,
    regs := 10,
    registers := [some 0, some 1, some 2, some 3, some 4, some 5, some 6, some 7, some 8],
    consts := [(2, BitVec.ofNat 64 3), (3, BitVec.ofNat 64 5)],
    slotRegs := [(0, 0), (1, 1)],
    layers := [(0, 0), (0, 2), (2, 4), (4, 5)],
    failSuffixMin := [NobleFoundation.PackedLayout.maxRegister, NobleFoundation.PackedLayout.maxRegister, NobleFoundation.PackedLayout.maxRegister, NobleFoundation.PackedLayout.maxRegister, NobleFoundation.PackedLayout.maxRegister]
  }
}

theorem two_constants_real_encoded_accepted :
    validateEncoded (encodeInput two_constants_real) = true := by
  rw [validateEncoded_agrees]
  decide

def two_constants_stale_revision : Input := {
  nodes := 
  [.input 0,
   .input 1,
   .constant 0x0000000000000003,
   .constant 0x0000000000000005,
   .add 0 1,
   .sub 1 0,
   .mul 4 2,
   .mul 5 3,
   .eq 6 7],
  layerCount := 4,
  failNodes := [],
  layerOf := [0, 0, 0, 0, 1, 1, 2, 2, 3],
  tables := {
    revisionOk := false,
    regs := 10,
    registers := [some 0, some 1, some 2, some 3, some 4, some 5, some 6, some 7, some 8],
    consts := [(2, BitVec.ofNat 64 3), (3, BitVec.ofNat 64 5)],
    slotRegs := [(0, 0), (1, 1)],
    layers := [(0, 0), (0, 2), (2, 4), (4, 5)],
    failSuffixMin := [NobleFoundation.PackedLayout.maxRegister, NobleFoundation.PackedLayout.maxRegister, NobleFoundation.PackedLayout.maxRegister, NobleFoundation.PackedLayout.maxRegister, NobleFoundation.PackedLayout.maxRegister]
  }
}

theorem two_constants_stale_revision_encoded_rejected :
    validateEncoded (encodeInput two_constants_stale_revision) = false := by
  rw [validateEncoded_agrees]
  decide

def two_constants_register_map_shape : Input := {
  nodes := 
  [.input 0,
   .input 1,
   .constant 0x0000000000000003,
   .constant 0x0000000000000005,
   .add 0 1,
   .sub 1 0,
   .mul 4 2,
   .mul 5 3,
   .eq 6 7],
  layerCount := 4,
  failNodes := [],
  layerOf := [0, 0, 0, 0, 1, 1, 2, 2, 3],
  tables := {
    revisionOk := true,
    regs := 10,
    registers := [some 0, some 1, some 2, some 3, some 4, some 5, some 6, some 7],
    consts := [(2, BitVec.ofNat 64 3), (3, BitVec.ofNat 64 5)],
    slotRegs := [(0, 0), (1, 1)],
    layers := [(0, 0), (0, 2), (2, 4), (4, 5)],
    failSuffixMin := [NobleFoundation.PackedLayout.maxRegister, NobleFoundation.PackedLayout.maxRegister, NobleFoundation.PackedLayout.maxRegister, NobleFoundation.PackedLayout.maxRegister, NobleFoundation.PackedLayout.maxRegister]
  }
}

theorem two_constants_register_map_shape_encoded_rejected :
    validateEncoded (encodeInput two_constants_register_map_shape) = false := by
  rw [validateEncoded_agrees]
  decide

def two_constants_register_out_of_range : Input := {
  nodes := 
  [.input 0,
   .input 1,
   .constant 0x0000000000000003,
   .constant 0x0000000000000005,
   .add 0 1,
   .sub 1 0,
   .mul 4 2,
   .mul 5 3,
   .eq 6 7],
  layerCount := 4,
  failNodes := [],
  layerOf := [0, 0, 0, 0, 1, 1, 2, 2, 3],
  tables := {
    revisionOk := true,
    regs := 10,
    registers := [some 10, some 1, some 2, some 3, some 4, some 5, some 6, some 7, some 8],
    consts := [(2, BitVec.ofNat 64 3), (3, BitVec.ofNat 64 5)],
    slotRegs := [(0, 0), (1, 1)],
    layers := [(0, 0), (0, 2), (2, 4), (4, 5)],
    failSuffixMin := [NobleFoundation.PackedLayout.maxRegister, NobleFoundation.PackedLayout.maxRegister, NobleFoundation.PackedLayout.maxRegister, NobleFoundation.PackedLayout.maxRegister, NobleFoundation.PackedLayout.maxRegister]
  }
}

theorem two_constants_register_out_of_range_encoded_rejected :
    validateEncoded (encodeInput two_constants_register_out_of_range) = false := by
  rw [validateEncoded_agrees]
  decide

def two_constants_aliased_registers : Input := {
  nodes := 
  [.input 0,
   .input 1,
   .constant 0x0000000000000003,
   .constant 0x0000000000000005,
   .add 0 1,
   .sub 1 0,
   .mul 4 2,
   .mul 5 3,
   .eq 6 7],
  layerCount := 4,
  failNodes := [],
  layerOf := [0, 0, 0, 0, 1, 1, 2, 2, 3],
  tables := {
    revisionOk := true,
    regs := 10,
    registers := [some 0, some 0, some 2, some 3, some 4, some 5, some 6, some 7, some 8],
    consts := [(2, BitVec.ofNat 64 3), (3, BitVec.ofNat 64 5)],
    slotRegs := [(0, 0), (1, 1)],
    layers := [(0, 0), (0, 2), (2, 4), (4, 5)],
    failSuffixMin := [NobleFoundation.PackedLayout.maxRegister, NobleFoundation.PackedLayout.maxRegister, NobleFoundation.PackedLayout.maxRegister, NobleFoundation.PackedLayout.maxRegister, NobleFoundation.PackedLayout.maxRegister]
  }
}

theorem two_constants_aliased_registers_encoded_rejected :
    validateEncoded (encodeInput two_constants_aliased_registers) = false := by
  rw [validateEncoded_agrees]
  decide

def two_constants_register_count : Input := {
  nodes := 
  [.input 0,
   .input 1,
   .constant 0x0000000000000003,
   .constant 0x0000000000000005,
   .add 0 1,
   .sub 1 0,
   .mul 4 2,
   .mul 5 3,
   .eq 6 7],
  layerCount := 4,
  failNodes := [],
  layerOf := [0, 0, 0, 0, 1, 1, 2, 2, 3],
  tables := {
    revisionOk := true,
    regs := 11,
    registers := [some 0, some 1, some 2, some 3, some 4, some 5, some 6, some 7, some 8],
    consts := [(2, BitVec.ofNat 64 3), (3, BitVec.ofNat 64 5)],
    slotRegs := [(0, 0), (1, 1)],
    layers := [(0, 0), (0, 2), (2, 4), (4, 5)],
    failSuffixMin := [NobleFoundation.PackedLayout.maxRegister, NobleFoundation.PackedLayout.maxRegister, NobleFoundation.PackedLayout.maxRegister, NobleFoundation.PackedLayout.maxRegister, NobleFoundation.PackedLayout.maxRegister]
  }
}

theorem two_constants_register_count_encoded_rejected :
    validateEncoded (encodeInput two_constants_register_count) = false := by
  rw [validateEncoded_agrees]
  decide

def two_constants_one_register_held : Input := {
  nodes := 
  [.input 0,
   .input 1,
   .constant 0x0000000000000003,
   .constant 0x0000000000000005,
   .add 0 1,
   .sub 1 0,
   .mul 4 2,
   .mul 5 3,
   .eq 6 7],
  layerCount := 4,
  failNodes := [],
  layerOf := [0, 0, 0, 0, 1, 1, 2, 2, 3],
  tables := {
    revisionOk := true,
    regs := 10,
    registers := [some 0, some 1, some 2, some 3, some 4, some 5, some 6, some 7, some 9],
    consts := [(2, BitVec.ofNat 64 3), (3, BitVec.ofNat 64 5)],
    slotRegs := [(0, 0), (1, 1)],
    layers := [(0, 0), (0, 2), (2, 4), (4, 5)],
    failSuffixMin := [NobleFoundation.PackedLayout.maxRegister, NobleFoundation.PackedLayout.maxRegister, NobleFoundation.PackedLayout.maxRegister, NobleFoundation.PackedLayout.maxRegister, NobleFoundation.PackedLayout.maxRegister]
  }
}

theorem two_constants_one_register_held_encoded_rejected :
    validateEncoded (encodeInput two_constants_one_register_held) = false := by
  rw [validateEncoded_agrees]
  decide

def two_constants_constant_entry : Input := {
  nodes := 
  [.input 0,
   .input 1,
   .constant 0x0000000000000003,
   .constant 0x0000000000000005,
   .add 0 1,
   .sub 1 0,
   .mul 4 2,
   .mul 5 3,
   .eq 6 7],
  layerCount := 4,
  failNodes := [],
  layerOf := [0, 0, 0, 0, 1, 1, 2, 2, 3],
  tables := {
    revisionOk := true,
    regs := 10,
    registers := [some 0, some 1, some 2, some 3, some 4, some 5, some 6, some 7, some 8],
    consts := [(2, BitVec.ofNat 64 4), (3, BitVec.ofNat 64 5)],
    slotRegs := [(0, 0), (1, 1)],
    layers := [(0, 0), (0, 2), (2, 4), (4, 5)],
    failSuffixMin := [NobleFoundation.PackedLayout.maxRegister, NobleFoundation.PackedLayout.maxRegister, NobleFoundation.PackedLayout.maxRegister, NobleFoundation.PackedLayout.maxRegister, NobleFoundation.PackedLayout.maxRegister]
  }
}

theorem two_constants_constant_entry_encoded_rejected :
    validateEncoded (encodeInput two_constants_constant_entry) = false := by
  rw [validateEncoded_agrees]
  decide

def two_constants_constant_count : Input := {
  nodes := 
  [.input 0,
   .input 1,
   .constant 0x0000000000000003,
   .constant 0x0000000000000005,
   .add 0 1,
   .sub 1 0,
   .mul 4 2,
   .mul 5 3,
   .eq 6 7],
  layerCount := 4,
  failNodes := [],
  layerOf := [0, 0, 0, 0, 1, 1, 2, 2, 3],
  tables := {
    revisionOk := true,
    regs := 10,
    registers := [some 0, some 1, some 2, some 3, some 4, some 5, some 6, some 7, some 8],
    consts := [(2, BitVec.ofNat 64 3)],
    slotRegs := [(0, 0), (1, 1)],
    layers := [(0, 0), (0, 2), (2, 4), (4, 5)],
    failSuffixMin := [NobleFoundation.PackedLayout.maxRegister, NobleFoundation.PackedLayout.maxRegister, NobleFoundation.PackedLayout.maxRegister, NobleFoundation.PackedLayout.maxRegister, NobleFoundation.PackedLayout.maxRegister]
  }
}

theorem two_constants_constant_count_encoded_rejected :
    validateEncoded (encodeInput two_constants_constant_count) = false := by
  rw [validateEncoded_agrees]
  decide

def two_constants_constant_duplicate : Input := {
  nodes := 
  [.input 0,
   .input 1,
   .constant 0x0000000000000003,
   .constant 0x0000000000000005,
   .add 0 1,
   .sub 1 0,
   .mul 4 2,
   .mul 5 3,
   .eq 6 7],
  layerCount := 4,
  failNodes := [],
  layerOf := [0, 0, 0, 0, 1, 1, 2, 2, 3],
  tables := {
    revisionOk := true,
    regs := 10,
    registers := [some 0, some 1, some 2, some 3, some 4, some 5, some 6, some 7, some 8],
    consts := [(2, BitVec.ofNat 64 3), (2, BitVec.ofNat 64 3)],
    slotRegs := [(0, 0), (1, 1)],
    layers := [(0, 0), (0, 2), (2, 4), (4, 5)],
    failSuffixMin := [NobleFoundation.PackedLayout.maxRegister, NobleFoundation.PackedLayout.maxRegister, NobleFoundation.PackedLayout.maxRegister, NobleFoundation.PackedLayout.maxRegister, NobleFoundation.PackedLayout.maxRegister]
  }
}

theorem two_constants_constant_duplicate_encoded_rejected :
    validateEncoded (encodeInput two_constants_constant_duplicate) = false := by
  rw [validateEncoded_agrees]
  decide

def two_constants_slot_entry : Input := {
  nodes := 
  [.input 0,
   .input 1,
   .constant 0x0000000000000003,
   .constant 0x0000000000000005,
   .add 0 1,
   .sub 1 0,
   .mul 4 2,
   .mul 5 3,
   .eq 6 7],
  layerCount := 4,
  failNodes := [],
  layerOf := [0, 0, 0, 0, 1, 1, 2, 2, 3],
  tables := {
    revisionOk := true,
    regs := 10,
    registers := [some 0, some 1, some 2, some 3, some 4, some 5, some 6, some 7, some 8],
    consts := [(2, BitVec.ofNat 64 3), (3, BitVec.ofNat 64 5)],
    slotRegs := [(0, 99), (1, 1)],
    layers := [(0, 0), (0, 2), (2, 4), (4, 5)],
    failSuffixMin := [NobleFoundation.PackedLayout.maxRegister, NobleFoundation.PackedLayout.maxRegister, NobleFoundation.PackedLayout.maxRegister, NobleFoundation.PackedLayout.maxRegister, NobleFoundation.PackedLayout.maxRegister]
  }
}

theorem two_constants_slot_entry_encoded_rejected :
    validateEncoded (encodeInput two_constants_slot_entry) = false := by
  rw [validateEncoded_agrees]
  decide

def two_constants_slot_count : Input := {
  nodes := 
  [.input 0,
   .input 1,
   .constant 0x0000000000000003,
   .constant 0x0000000000000005,
   .add 0 1,
   .sub 1 0,
   .mul 4 2,
   .mul 5 3,
   .eq 6 7],
  layerCount := 4,
  failNodes := [],
  layerOf := [0, 0, 0, 0, 1, 1, 2, 2, 3],
  tables := {
    revisionOk := true,
    regs := 10,
    registers := [some 0, some 1, some 2, some 3, some 4, some 5, some 6, some 7, some 8],
    consts := [(2, BitVec.ofNat 64 3), (3, BitVec.ofNat 64 5)],
    slotRegs := [(0, 0)],
    layers := [(0, 0), (0, 2), (2, 4), (4, 5)],
    failSuffixMin := [NobleFoundation.PackedLayout.maxRegister, NobleFoundation.PackedLayout.maxRegister, NobleFoundation.PackedLayout.maxRegister, NobleFoundation.PackedLayout.maxRegister, NobleFoundation.PackedLayout.maxRegister]
  }
}

theorem two_constants_slot_count_encoded_rejected :
    validateEncoded (encodeInput two_constants_slot_count) = false := by
  rw [validateEncoded_agrees]
  decide

def two_constants_slot_duplicate : Input := {
  nodes := 
  [.input 0,
   .input 1,
   .constant 0x0000000000000003,
   .constant 0x0000000000000005,
   .add 0 1,
   .sub 1 0,
   .mul 4 2,
   .mul 5 3,
   .eq 6 7],
  layerCount := 4,
  failNodes := [],
  layerOf := [0, 0, 0, 0, 1, 1, 2, 2, 3],
  tables := {
    revisionOk := true,
    regs := 10,
    registers := [some 0, some 1, some 2, some 3, some 4, some 5, some 6, some 7, some 8],
    consts := [(2, BitVec.ofNat 64 3), (3, BitVec.ofNat 64 5)],
    slotRegs := [(0, 0), (0, 0)],
    layers := [(0, 0), (0, 2), (2, 4), (4, 5)],
    failSuffixMin := [NobleFoundation.PackedLayout.maxRegister, NobleFoundation.PackedLayout.maxRegister, NobleFoundation.PackedLayout.maxRegister, NobleFoundation.PackedLayout.maxRegister, NobleFoundation.PackedLayout.maxRegister]
  }
}

theorem two_constants_slot_duplicate_encoded_rejected :
    validateEncoded (encodeInput two_constants_slot_duplicate) = false := by
  rw [validateEncoded_agrees]
  decide

def two_constants_layer_shape : Input := {
  nodes := 
  [.input 0,
   .input 1,
   .constant 0x0000000000000003,
   .constant 0x0000000000000005,
   .add 0 1,
   .sub 1 0,
   .mul 4 2,
   .mul 5 3,
   .eq 6 7],
  layerCount := 4,
  failNodes := [],
  layerOf := [0, 0, 0, 0, 1, 1, 2, 2, 3],
  tables := {
    revisionOk := true,
    regs := 10,
    registers := [some 0, some 1, some 2, some 3, some 4, some 5, some 6, some 7, some 8],
    consts := [(2, BitVec.ofNat 64 3), (3, BitVec.ofNat 64 5)],
    slotRegs := [(0, 0), (1, 1)],
    layers := [(0, 0), (0, 2), (2, 4), (4, 5), (5, 5)],
    failSuffixMin := [NobleFoundation.PackedLayout.maxRegister, NobleFoundation.PackedLayout.maxRegister, NobleFoundation.PackedLayout.maxRegister, NobleFoundation.PackedLayout.maxRegister, NobleFoundation.PackedLayout.maxRegister]
  }
}

theorem two_constants_layer_shape_encoded_rejected :
    validateEncoded (encodeInput two_constants_layer_shape) = false := by
  rw [validateEncoded_agrees]
  decide

def two_constants_layer_order : Input := {
  nodes := 
  [.input 0,
   .input 1,
   .constant 0x0000000000000003,
   .constant 0x0000000000000005,
   .add 0 1,
   .sub 1 0,
   .mul 4 2,
   .mul 5 3,
   .eq 6 7],
  layerCount := 4,
  failNodes := [],
  layerOf := [0, 0, 0, 0, 1, 1, 2, 2, 3],
  tables := {
    revisionOk := true,
    regs := 10,
    registers := [some 0, some 1, some 2, some 3, some 4, some 5, some 6, some 7, some 8],
    consts := [(2, BitVec.ofNat 64 3), (3, BitVec.ofNat 64 5)],
    slotRegs := [(0, 0), (1, 1)],
    layers := [(1, 0), (0, 2), (2, 4), (4, 5)],
    failSuffixMin := [NobleFoundation.PackedLayout.maxRegister, NobleFoundation.PackedLayout.maxRegister, NobleFoundation.PackedLayout.maxRegister, NobleFoundation.PackedLayout.maxRegister, NobleFoundation.PackedLayout.maxRegister]
  }
}

theorem two_constants_layer_order_encoded_rejected :
    validateEncoded (encodeInput two_constants_layer_order) = false := by
  rw [validateEncoded_agrees]
  decide

def two_constants_failure_suffix : Input := {
  nodes := 
  [.input 0,
   .input 1,
   .constant 0x0000000000000003,
   .constant 0x0000000000000005,
   .add 0 1,
   .sub 1 0,
   .mul 4 2,
   .mul 5 3,
   .eq 6 7],
  layerCount := 4,
  failNodes := [],
  layerOf := [0, 0, 0, 0, 1, 1, 2, 2, 3],
  tables := {
    revisionOk := true,
    regs := 10,
    registers := [some 0, some 1, some 2, some 3, some 4, some 5, some 6, some 7, some 8],
    consts := [(2, BitVec.ofNat 64 3), (3, BitVec.ofNat 64 5)],
    slotRegs := [(0, 0), (1, 1)],
    layers := [(0, 0), (0, 2), (2, 4), (4, 5)],
    failSuffixMin := [0, NobleFoundation.PackedLayout.maxRegister, NobleFoundation.PackedLayout.maxRegister, NobleFoundation.PackedLayout.maxRegister, NobleFoundation.PackedLayout.maxRegister]
  }
}

theorem two_constants_failure_suffix_encoded_rejected :
    validateEncoded (encodeInput two_constants_failure_suffix) = false := by
  rw [validateEncoded_agrees]
  decide

def two_constants_layer_coverage : Input := {
  nodes := 
  [.input 0,
   .input 1,
   .constant 0x0000000000000003,
   .constant 0x0000000000000005,
   .add 0 1,
   .sub 1 0,
   .mul 4 2,
   .mul 5 3,
   .eq 6 7],
  layerCount := 4,
  failNodes := [],
  layerOf := [0, 0, 0, 0, 1, 1, 2, 2, 3],
  tables := {
    revisionOk := true,
    regs := 10,
    registers := [some 0, some 1, some 2, some 3, some 4, some 5, some 6, some 7, some 8],
    consts := [(2, BitVec.ofNat 64 3), (3, BitVec.ofNat 64 5)],
    slotRegs := [(0, 0), (1, 1)],
    layers := [(0, 0), (0, 3), (3, 4), (4, 5)],
    failSuffixMin := [NobleFoundation.PackedLayout.maxRegister, NobleFoundation.PackedLayout.maxRegister, NobleFoundation.PackedLayout.maxRegister, NobleFoundation.PackedLayout.maxRegister, NobleFoundation.PackedLayout.maxRegister]
  }
}

theorem two_constants_layer_coverage_encoded_rejected :
    validateEncoded (encodeInput two_constants_layer_coverage) = false := by
  rw [validateEncoded_agrees]
  decide

/-! These controls demonstrate why the input encoding and size claim
must retain every field. The metadata extension below is model-only: its extra layer is not
derived by Rust `CheckedGraph` and is deliberately outside the Rust correspondence boundary. -/

def nodesOnlyEncoding (input : Input) : List Bool :=
  encodeList encodeNode input.nodes

theorem nodesOnlyEncoding_is_not_injective :
    ∃ first second, first ≠ second ∧ nodesOnlyEncoding first = nodesOnlyEncoding second := by
  refine ⟨two_constants_real, two_constants_stale_revision, ?_, ?_⟩
  · intro same
    have revisionSame := congrArg (fun input : Input => input.tables.revisionOk) same
    simp [two_constants_real, two_constants_stale_revision] at revisionSame
  · rfl

def finiteFixtureLookup (input : Input) : Bool :=
  [interleaved_real.layerCount, two_constants_real.layerCount].contains input.layerCount

def two_constants_with_trusted_layer_extension : Input :=
  { two_constants_real with
    layerCount := 5,
    tables := { two_constants_real.tables with
      layers := two_constants_real.tables.layers ++ [(5, 5)],
      failSuffixMin := two_constants_real.tables.failSuffixMin ++ [maxRegister] } }

theorem finiteFixtureLookup_misses_model_accepted_extension :
    acceptedInput two_constants_with_trusted_layer_extension = true ∧
      finiteFixtureLookup two_constants_with_trusted_layer_extension = false := by
  decide

theorem node_count_does_not_measure_encoded_input :
    two_constants_real.nodes.length = two_constants_with_trusted_layer_extension.nodes.length ∧
      encodedSize two_constants_real <
        encodedSize two_constants_with_trusted_layer_extension := by
  constructor
  · rfl
  · have hsize :
        encodedSize two_constants_with_trusted_layer_extension =
          encodedSize two_constants_real + 79 := by
      simp [encodedSize, encodeInput, encodeTables, encodeNatPair,
        encodeConstant, encodeRegister, encodeWord, encodeNode, encodeBool, natWidth,
        maxRegister,
        two_constants_with_trusted_layer_extension, two_constants_real]
    omega

end NobleFoundation.PackedLayoutCorpus
