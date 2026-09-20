import Init.System.IO

/- The normal module writer replaces files, which cannot target file bind mounts.
   Compile on bounded tmpfs, then copy only four bounded data files into fixed
   writable slots. This entire process remains an untrusted producer when its
   input is user source; no produced binary object reaches the proof consumer. -/
private def artifactLimit : Nat := 33554432

private def transfer (source destination : System.FilePath) : IO Unit := do
  if !(← source.pathExists) then return
  let metadata ← source.symlinkMetadata
  unless metadata.type == .file && metadata.byteSize.toNat ≤ artifactLimit do
    throw (IO.userError "MC1_ARTIFACT_LIMIT: invalid or oversized module artifact")
  let input ← IO.FS.Handle.mk source .read
  let output ← IO.FS.Handle.mk destination .write
  let mut copied := 0
  while true do
    let chunk ← input.read (min 65536 (artifactLimit + 1 - copied)).toUSize
    if chunk.isEmpty then break
    copied := copied + chunk.size
    if copied > artifactLimit then
      throw (IO.userError "MC1_ARTIFACT_LIMIT: module artifact grew beyond limit")
    output.write chunk

def main (args : List String) : IO UInt32 := do
  try
    let [source, root, destination] := args
      | throw (IO.userError "MC1_COMPILER_ARGUMENTS: expected source, root, output")
    let some executable ← IO.getEnv "NOBLE_LEAN_EXECUTABLE"
      | throw (IO.userError "MC1_COMPILER_TOOL: consumer did not select Lean")
    let temporary : System.FilePath := "/tmp/NobleOutput.olean"
    let child ← IO.Process.spawn {
      cmd := executable
      args := #["-R", root, "-j", "2", "-o", temporary.toString, source]
      stdin := .null
    }
    let status ← child.wait
    if status != 0 then return status
    let destination : System.FilePath := destination
    transfer temporary destination
    transfer (temporary.withExtension "ir") (destination.withExtension "ir")
    transfer (temporary.toString ++ ".server") (destination.toString ++ ".server")
    transfer (temporary.toString ++ ".private") (destination.toString ++ ".private")
    return (0 : UInt32)
  catch error =>
    IO.eprintln error
    return (1 : UInt32)
