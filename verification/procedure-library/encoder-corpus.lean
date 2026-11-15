import Lean
import NobleFoundation.ProcedureLibraryWire

open Lean
open NobleFoundation.ProcedureLibraryWire

private def fixtureCase (name : String) (fixture : Fixture) : Lean.Json :=
  Lean.Json.mkObj [
    ("name", toJson name),
    ("chunk_id", toJson fixture.chunkId),
    ("source_revision", toJson fixture.sourceRevision),
    ("subject_bytes", toJson (encodeFiniteSubject fixture))
  ]

def main : IO Unit := do
  let corpus := Lean.Json.mkObj [
    ("schema", toJson ("noble-procedure-library-lean-byte-corpus/1" : String)),
    ("cases", toJson #[
      fixtureCase "arithCommuted/Synthesis" commutedFixture,
      fixtureCase "arithSubtract/Chunk" subtractFixture
    ])
  ]
  IO.println corpus.compress
