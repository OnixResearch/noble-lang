/-!
Finite wire subprofile for issue #118's differential corpus. These definitions encode
only the two explicit no-capture/no-dependency, one-empty-instantiation I64
source fixtures below. They do not model arbitrary chunks, #56 certificates,
the experimental digest, or a universal Rust/Lean refinement theorem.
-/
namespace NobleFoundation.ProcedureLibraryWire

private def u16be (value : Nat) : List Nat :=
  [value / 256 % 256, value % 256]

private def u32be (value : Nat) : List Nat :=
  [value / 16777216 % 256, value / 65536 % 256,
   value / 256 % 256, value % 256]

private def u64be (value : Nat) : List Nat :=
  (List.range 8).map (fun index => value / (256 ^ (7 - index)) % 256)

private def i64be (value : Int) : List Nat :=
  u64be ((value % (2 ^ 64)).toNat)

private def ascii (value : String) : List Nat :=
  value.toUTF8.data.toList.map (fun byte => byte.toNat)

private def sizedAscii (value : String) : List Nat :=
  u32be value.utf8ByteSize ++ ascii value

private def nibble (character : Char) : Nat :=
  if character ≤ '9' then character.toNat - '0'.toNat
  else character.toNat - 'a'.toNat + 10

private def hexOctets : List Char → List Nat
  | high :: low :: rest => (16 * nibble high + nibble low) :: hexOctets rest
  | _ => []

private def hex (value : String) : List Nat := hexOctets value.toList

inductive WireNode where
  | i64 (value : Int)
  | operation (tag : Nat) (instantiation : Nat)
  deriving DecidableEq, Repr

private def nodeBytes : WireNode → List Nat
  | .i64 value => [0] ++ i64be value
  | .operation tag instantiation => [3, tag] ++ u16be instantiation

structure Fixture where
  chunkId : List Nat
  sourceRevision : List Nat
  nodes : List WireNode
  producerTag : Nat
  toolRevision : Nat
  taskId : Option (List Nat)
  grammarRevision : Option Nat
  deriving DecidableEq, Repr

private def chunkBytes (fixture : Fixture) : List Nat :=
  u32be 1 ++ u32be 0 ++ sizedAscii "pure-i64-bool/1" ++
  fixture.chunkId ++ fixture.sourceRevision ++
  u32be 2 ++ [0, 0] ++ u32be 1 ++ [0] ++
  u32be 0 ++ u32be 0 ++ u32be 0 ++
  u32be 1 ++ u32be 0 ++
  u32be fixture.nodes.length ++ fixture.nodes.flatMap nodeBytes ++
  u32be fixture.nodes.length ++ (List.range fixture.nodes.length).flatMap u32be

private def provenanceBytes (fixture : Fixture) : List Nat :=
  [fixture.producerTag] ++ u32be fixture.toolRevision ++
  (match fixture.taskId with
   | none => [0]
   | some bytes => [1] ++ bytes) ++
  (match fixture.grammarRevision with
   | none => [0]
   | some revision => [1] ++ u32be revision)

/-- Experimental immutable-subject bytes, restricted to the two finite fixtures. -/
def encodeFiniteSubject (fixture : Fixture) : List Nat :=
  sizedAscii "noble-procedure-record/1" ++ u32be 1 ++ [1] ++ u32be 0 ++
  chunkBytes fixture ++ provenanceBytes fixture ++ u32be 0

/-- Source id and revision are independently checked against the Rust fixture. -/
def commutedFixture : Fixture := {
  chunkId := hex "caa60f63d1b5a87dcbe7c47e280fe264"
  sourceRevision := hex "14a7ef63e8e0fba64a43cb0366e9fad64078719ca79628397b71a2075f9428ae"
  nodes := [.i64 7, .operation 3 0, .operation 3 0]
  producerTag := 1
  toolRevision := 2
  taskId := some (ascii "arithCommuted" ++ List.replicate 19 0)
  grammarRevision := some 0
}

/-- A second body and provenance branch of the same finite wire subprofile. -/
def subtractFixture : Fixture := {
  chunkId := hex "0f6040a45e12e5557d47a90ad2240bf7"
  sourceRevision := hex "68d5a576bac0b4baa870aa805949e23c4b96c47e905dc91a6a2b86c5ae36d09a"
  nodes := [.operation 4 0, .i64 7, .operation 3 0]
  producerTag := 0
  toolRevision := 1
  taskId := none
  grammarRevision := none
}

set_option maxRecDepth 4096 in
theorem commuted_fixture_wire_length :
    (encodeFiniteSubject commutedFixture).length = 227 := by decide

set_option maxRecDepth 4096 in
theorem subtract_fixture_wire_length :
    (encodeFiniteSubject subtractFixture).length = 191 := by decide

set_option maxRecDepth 4096 in
theorem commuted_fixture_encoder_prefix :
    (encodeFiniteSubject commutedFixture).take 4 = [0, 0, 0, 24] := by decide

end NobleFoundation.ProcedureLibraryWire
