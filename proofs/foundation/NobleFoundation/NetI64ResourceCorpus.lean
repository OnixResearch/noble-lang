import NobleFoundation.NetI64Resource
import NobleFoundation.NetI64Corpus

/-!
Finite byte fixture emitted from the admitted Rust net and independently accepted resource
certificate. The Rust freshness test binds the literals to exact bytes and BLAKE3 digests.
The kernel checks the certificate field offsets, the translated net, and its cost/agent claims.
BLAKE3, the Rust binary decoder, and universal Rust/Lean correspondence are NOT proved here.
-/

namespace NobleFoundation.NetI64ResourceCorpus

open NobleFoundation.NetI64
open NobleFoundation.NetI64Resource

private def compareId : List Nat → List Nat → Ordering
  | [], [] => .eq
  | [], _ :: _ => .lt
  | _ :: _, [] => .gt
  | left :: leftTail, right :: rightTail =>
      match compare left right with
      | .eq => compareId leftTail rightTail
      | order => order

private def comparePort (left right : INet.Port) : Ordering :=
  match compareId left.1 right.1 with
  | .eq => compare left.2 right.2
  | order => order

private def compareWire (left right : INet.Port × INet.Port) : Ordering :=
  match comparePort left.1 right.1 with
  | .eq => comparePort left.2 right.2
  | order => order

private def insertSorted {α : Type} (less : α → α → Bool) (value : α) : List α → List α
  | [] => [value]
  | head :: rest =>
      if less value head then value :: head :: rest
      else head :: insertSorted less value rest

private def sortSorted {α : Type} (less : α → α → Bool) : List α → List α
  | [] => []
  | head :: rest => insertSorted less head (sortSorted less rest)

private def canonicalNet (net : INet.XNet NetSym) : INet.XNet NetSym :=
  { agents := sortSorted (fun left right => compareId left.1 right.1 == .lt) net.agents
    free := sortSorted (fun left right => comparePort left right == .lt) net.free
    wires := sortSorted (fun left right => compareWire left right == .lt) net.wires }

private def littleEndian (bytes : List Nat) : Nat :=
  bytes.reverse.foldl (fun value byte => value * 256 + byte) 0

-- Exact accepted Rust bytes: net=039a016ec671138bf71f45736ffc02f418e7a77b77864f3c6e0133309f8c6aa3, certificate=bb6b7df7bb5119093069fda84f830f851c037fbe657ad8cf48b5d91386789404.
def program : Program := { inputArity := 2, nodes := [.input 0, .input 1], outputs := [0, 0, 0, 0] }
def inputs : List (BitVec 64) := [BitVec.ofNat 64 18446744073709551599, BitVec.ofNat 64 22]
def rustNet : INet.XNet NetSym := {
    agents := [([0, 0, 0], .num (BitVec.ofNat 64 18446744073709551599)),
    ([0, 0, 1], .num (BitVec.ofNat 64 22)),
    ([0, 2, 0, 0], .dup),
    ([0, 2, 0, 1], .dup),
    ([0, 2, 0, 2], .dup),
    ([0, 3, 1], .era)],
    free := [([9, 9, 0], 0), ([9, 9, 1], 0), ([9, 9, 2], 0), ([9, 9, 3], 0)],
    wires := [(([0, 0, 0], 0), ([0, 2, 0, 0], 0)),
    (([0, 0, 1], 0), ([0, 3, 1], 0)),
    (([0, 2, 0, 0], 0), ([0, 0, 0], 0)),
    (([0, 2, 0, 0], 1), ([9, 9, 0], 0)),
    (([0, 2, 0, 0], 2), ([0, 2, 0, 1], 0)),
    (([0, 2, 0, 1], 0), ([0, 2, 0, 0], 2)),
    (([0, 2, 0, 1], 1), ([9, 9, 1], 0)),
    (([0, 2, 0, 1], 2), ([0, 2, 0, 2], 0)),
    (([0, 2, 0, 2], 0), ([0, 2, 0, 1], 2)),
    (([0, 2, 0, 2], 1), ([9, 9, 2], 0)),
    (([0, 2, 0, 2], 2), ([9, 9, 3], 0)),
    (([0, 3, 1], 0), ([0, 0, 1], 0)),
    (([9, 9, 0], 0), ([0, 2, 0, 0], 1)),
    (([9, 9, 1], 0), ([0, 2, 0, 1], 1)),
    (([9, 9, 2], 0), ([0, 2, 0, 2], 1)),
    (([9, 9, 3], 0), ([0, 2, 0, 2], 2))]
  }
def rustOutputs : List NobleFoundation.PackedSchedule.Val := [.i64 (BitVec.ofNat 64 18446744073709551599), .i64 (BitVec.ofNat 64 18446744073709551599), .i64 (BitVec.ofNat 64 18446744073709551599), .i64 (BitVec.ofNat 64 18446744073709551599)]
def certificateBytes : List Nat := [78, 71, 82, 83, 67, 49, 0, 0, 21, 0, 110, 103, 114, 97, 112, 104, 45, 110, 101, 116, 45, 114, 101, 115, 111, 117, 114, 99, 101, 47, 49, 29, 0, 110, 103, 114, 97, 112, 104, 45, 110, 101, 116, 45, 114, 101, 115, 111, 117, 114, 99, 101, 45, 99, 104, 101, 99, 107, 101, 114, 47, 48, 32, 0, 78, 111, 98, 108, 101, 70, 111, 117, 110, 100, 97, 116, 105, 111, 110, 46, 78, 101, 116, 73, 54, 52, 82, 101, 115, 111, 117, 114, 99, 101, 47, 48, 26, 0, 110, 103, 114, 97, 112, 104, 45, 110, 101, 116, 45, 114, 101, 115, 111, 117, 114, 99, 101, 45, 99, 111, 115, 116, 47, 48, 8, 58, 176, 118, 7, 243, 99, 129, 191, 176, 223, 241, 131, 46, 236, 150, 248, 135, 208, 179, 99, 5, 113, 182, 89, 167, 56, 210, 34, 198, 101, 189, 42, 248, 47, 74, 19, 141, 228, 206, 54, 251, 15, 156, 134, 238, 32, 102, 124, 22, 102, 65, 150, 107, 155, 229, 137, 220, 28, 214, 246, 190, 26, 19, 198, 9, 176, 242, 200, 191, 176, 150, 183, 216, 120, 20, 53, 185, 220, 16, 82, 202, 1, 90, 69, 134, 59, 9, 148, 48, 34, 59, 135, 244, 24, 15, 4, 0, 0, 0, 0, 0, 0, 0, 6, 0, 0, 0, 0, 0, 0, 0, 6, 98, 108, 97, 107, 101, 51]
def netBytes : List Nat := [78, 71, 82, 78, 69, 84, 51, 0, 220, 233, 76, 131, 68, 34, 53, 163, 183, 2, 227, 163, 176, 178, 177, 83, 163, 151, 158, 125, 104, 118, 233, 29, 244, 87, 31, 103, 111, 231, 193, 215, 155, 84, 93, 86, 145, 86, 64, 101, 213, 183, 225, 67, 124, 101, 35, 143, 158, 140, 146, 234, 7, 118, 138, 77, 14, 86, 108, 101, 27, 10, 90, 43, 16, 0, 110, 103, 114, 97, 112, 104, 45, 110, 101, 116, 45, 105, 54, 52, 47, 51, 25, 0, 110, 103, 114, 97, 112, 104, 45, 110, 101, 116, 45, 105, 54, 52, 45, 108, 111, 119, 101, 114, 105, 110, 103, 47, 48, 2, 0, 0, 0, 239, 255, 255, 255, 255, 255, 255, 255, 22, 0, 0, 0, 0, 0, 0, 0, 4, 0, 0, 0, 2, 0, 0, 0, 1, 0, 3, 0, 0, 0, 1, 0, 4, 0, 0, 0, 1, 0, 4, 0, 0, 0, 2, 0, 6, 0, 0, 0, 0, 0, 0, 0, 0, 0, 239, 255, 255, 255, 255, 255, 255, 255, 1, 2, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 22, 0, 0, 0, 0, 0, 0, 0, 1, 5, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 3, 1, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0, 1, 3, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 3, 1, 2, 0, 0, 0, 2, 2, 1, 0, 0, 0, 1, 4, 0, 0, 0, 0, 1, 0, 0, 0, 0, 2, 0, 0, 0, 3, 1, 3, 0, 0, 0, 2, 2, 2, 0, 0, 0, 2, 3, 0, 0, 0, 2, 1, 0, 0, 0, 4, 1, 1, 0, 0, 0, 0, 0, 0]
def netBlake3 : List Nat := [3, 154, 1, 110, 198, 113, 19, 139, 247, 31, 69, 115, 111, 252, 2, 244, 24, 231, 167, 123, 119, 134, 79, 60, 110, 1, 51, 48, 159, 140, 106, 163]
def certificateBlake3 : List Nat := [187, 107, 125, 247, 187, 81, 25, 9, 48, 105, 253, 168, 79, 131, 15, 133, 28, 3, 127, 190, 101, 122, 216, 207, 72, 181, 217, 19, 134, 120, 148, 4]
def graphId : List Nat := [220, 233, 76, 131, 68, 34, 53, 163, 183, 2, 227, 163, 176, 178, 177, 83, 163, 151, 158, 125, 104, 118, 233, 29, 244, 87, 31, 103, 111, 231, 193, 215]
def buildKey : List Nat := [155, 84, 93, 86, 145, 86, 64, 101, 213, 183, 225, 67, 124, 101, 35, 143, 158, 140, 146, 234, 7, 118, 138, 77, 14, 86, 108, 101, 27, 10, 90, 43]
def certificateNetDigest : List Nat := [8, 58, 176, 118, 7, 243, 99, 129, 191, 176, 223, 241, 131, 46, 236, 150, 248, 135, 208, 179, 99, 5, 113, 182, 89, 167, 56, 210, 34, 198, 101, 189]
def certificateRuleDigest : List Nat := [42, 248, 47, 74, 19, 141, 228, 206, 54, 251, 15, 156, 134, 238, 32, 102, 124, 22, 102, 65, 150, 107, 155, 229, 137, 220, 28, 214, 246, 190, 26, 19]
def certificateCostDigest : List Nat := [198, 9, 176, 242, 200, 191, 176, 150, 183, 216, 120, 20, 53, 185, 220, 16, 82, 202, 1, 90, 69, 134, 59, 9, 148, 48, 34, 59, 135, 244, 24, 15]

/-- The magic and all four length-prefixed identity revisions are byte-exact. -/
theorem fixture_schema :
    certificateBytes.take 8 = [78, 71, 82, 83, 67, 49, 0, 0] ∧
    (certificateBytes.drop 8).take 2 = [21, 0] ∧
    (certificateBytes.drop 10).take 21 = [110, 103, 114, 97, 112, 104, 45, 110, 101, 116, 45, 114, 101, 115, 111, 117, 114, 99, 101, 47, 49] ∧
    (certificateBytes.drop 31).take 2 = [29, 0] ∧
    (certificateBytes.drop 33).take 29 = [110, 103, 114, 97, 112, 104, 45, 110, 101, 116, 45, 114, 101, 115, 111, 117, 114, 99, 101, 45, 99, 104, 101, 99, 107, 101, 114, 47, 48] ∧
    (certificateBytes.drop 62).take 2 = [32, 0] ∧
    (certificateBytes.drop 64).take 32 = [78, 111, 98, 108, 101, 70, 111, 117, 110, 100, 97, 116, 105, 111, 110, 46, 78, 101, 116, 73, 54, 52, 82, 101, 115, 111, 117, 114, 99, 101, 47, 48] ∧
    (certificateBytes.drop 96).take 2 = [26, 0] ∧
    (certificateBytes.drop 98).take 26 = [110, 103, 114, 97, 112, 104, 45, 110, 101, 116, 45, 114, 101, 115, 111, 117, 114, 99, 101, 45, 99, 111, 115, 116, 47, 48] := by decide

/-- Binary field positions are decoded from the literal, rather than postulated. -/
theorem fixture_byte_fields :
    netBytes.take 8 = [78, 71, 82, 78, 69, 84, 51, 0] ∧
    (netBytes.drop 8).take 32 = graphId ∧
    (netBytes.drop 40).take 32 = buildKey ∧
    (certificateBytes.drop 124).take 32 = certificateNetDigest ∧
    (certificateBytes.drop 156).take 32 = certificateRuleDigest ∧
    (certificateBytes.drop 188).take 32 = certificateCostDigest ∧
    littleEndian ((certificateBytes.drop 220).take 8) = 4 ∧
    littleEndian ((certificateBytes.drop 228).take 8) = 6 := by decide

/-- The independently accepted decoded net agrees with this source translation. -/
theorem fixture_translation :
    (Program.translateChecked program inputs).map canonicalNet = some (canonicalNet rustNet) := by decide

theorem fixture_resources :
    program.interactionCount = 4 ∧
    (program.translate inputs).agents.length = 6 ∧
    NobleFoundation.NetI64.checkCertificate program inputs (Certificate.forProgram program inputs) = true := by decide

/-- Byte-decoded claims agree with the kernel-checked translated program. -/
theorem fixture_byte_claims_match_translation :
    littleEndian ((certificateBytes.drop 220).take 8) = program.interactionCount ∧
    littleEndian ((certificateBytes.drop 228).take 8) = (program.translate inputs).agents.length := by decide

end NobleFoundation.NetI64ResourceCorpus
