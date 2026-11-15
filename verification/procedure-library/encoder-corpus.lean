import Lean
import NobleFoundation.ProcedureLibraryWire

open Lean
open NobleFoundation.ProcedureLibraryWire

private def dependencyCase (dependency : WireLibraryDependency) : Lean.Json :=
  Lean.Json.mkObj [
    ("procedure_id", toJson dependency.procedureId),
    ("chunk_id", toJson dependency.chunkId),
    ("source_revision", toJson dependency.sourceRevision)
  ]

private def fixtureCase (name : String) (fixture : Fixture) (producer closure : Nat) : Lean.Json :=
  let modeled := withClosure closure (withProducer name producer fixture)
  Lean.Json.mkObj [
    ("name", toJson s!"{name}/{producer}/{closure}"),
    ("source_task", toJson name),
    ("producer_tag", toJson producer),
    ("closure_count", toJson closure),
    ("chunk_id", toJson modeled.chunkId),
    ("source_revision", toJson modeled.sourceRevision),
    ("library_dependencies", toJson (modeled.libraryDependencies.map dependencyCase)),
    ("subject_bytes", toJson (encodeFiniteSubject modeled))
  ]

def main : IO Unit := do
  let sources : List (String × Fixture) := [
    ("arithCommuted", commutedFixture),
    ("arithSubtract", subtractFixture),
    ("arithScaled", scaledFixture),
    ("arithProduct", productFixture)
  ]
  let cases := sources.flatMap (fun (name, fixture) =>
    ([0, 1, 2] : List Nat).flatMap (fun producer =>
      ([0, 1, 2] : List Nat).map (fixtureCase name fixture producer)))
  let corpus := Lean.Json.mkObj [
    ("schema", toJson ("noble-procedure-library-lean-byte-corpus/2" : String)),
    ("cases", toJson cases)
  ]
  IO.println corpus.compress
