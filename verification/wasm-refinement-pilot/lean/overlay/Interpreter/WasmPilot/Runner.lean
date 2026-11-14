import Interpreter.Wasm.Decoder.Wat
import Interpreter.Wasm.SmallStep

namespace Interpreter.WasmPilot

open Wasm Wasm.Decoder.Wat Wasm.SmallStep

private def importedGlobalValue (name : String) : Option Value :=
  match name with
  | "failure" | "quota_reason" | "phase" | "heap_cursor" | "heap_baseline"
  | "allocation_peak" | "operand_peak" | "continuation_peak" | "steps"
  | "quote_invocations" | "sp" | "cp" | "observed_program" | "recipe_count"
  | "reflection_steps" | "rp" | "generation" => some (.i32 0)
  | "allocation_limit" => some (.i32 196608)
  | "recipe_limit" | "depth_limit" => some (.i32 256)
  | "operand_limit" => some (.i32 2048)
  | "continuation_limit" => some (.i32 4096)
  | "step_limit" => some (.i32 100000)
  | "allocated_total" | "released_total" => some (.i64 0)
  | _ => none

private def setImportedGlobals : List (String × String) → Nat → List Value → Except String (List Value)
  | [], _, values => .ok values
  | (moduleName, fieldName) :: rest, index, values => do
    if moduleName != "noble" then throw s!"unexpected global import module `{moduleName}`"
    let some value := importedGlobalValue fieldName
      | throw s!"no reviewed ABI initial value for global `{fieldName}`"
    if index ≥ values.length then throw s!"global import index {index} is out of range"
    setImportedGlobals rest (index + 1) (Wasm.listSetAt values index value)

private def checkSubjectImports (module : Wasm.Module) : Except String Unit := do
  if !module.imports.isEmpty then throw "subject imports a function; the selected ABI has none"
  if module.importedMemories != [("noble", "memory")] then
    throw "subject memory imports do not match the one-memory Core-Bootstrap ABI"
  if module.importedTables != [("noble", "table")] then
    throw "subject table imports do not match the one-table Core-Bootstrap ABI"
  let expectedGlobals : List (String × ValueType) := [
    ("failure", .i32), ("quota_reason", .i32), ("phase", .i32), ("heap_cursor", .i32),
    ("heap_baseline", .i32), ("allocation_limit", .i32), ("recipe_limit", .i32),
    ("depth_limit", .i32), ("operand_limit", .i32), ("continuation_limit", .i32),
    ("step_limit", .i32), ("allocated_total", .i64), ("released_total", .i64),
    ("allocation_peak", .i32), ("operand_peak", .i32), ("continuation_peak", .i32),
    ("steps", .i32), ("quote_invocations", .i32), ("sp", .i32), ("cp", .i32),
    ("observed_program", .i32), ("recipe_count", .i32), ("reflection_steps", .i32),
    ("rp", .i32), ("generation", .i32),
  ]
  let actualGlobalNames := module.importedGlobals.map Prod.snd
  if actualGlobalNames.length != expectedGlobals.length ||
      !(expectedGlobals.all (fun (name, _) => actualGlobalNames.contains name)) then
    throw "subject global import names do not match the reviewed ABI"
  let globalDecls := module.globals.take expectedGlobals.length
  if globalDecls.length != expectedGlobals.length ||
      !(module.importedGlobals.zip globalDecls).all
        (fun ((_, name), decl) =>
          match expectedGlobals.find? (fun (expectedName, _) => expectedName == name) with
          | some (_, valueType) => decl.declaredType == some valueType && decl.isMut
          | none => false) then
    throw "subject imported global types/mutability do not match the reviewed ABI"
  if module.memory.isNone then throw "subject has no imported linear memory"
  if module.memory.get!.pagesMin.toNat != 16 || module.memory.get!.pagesMax != some 16 then
    throw "subject imported memory bounds differ from 16 fixed pages"
  if !module.extraMemories.isEmpty then throw "subject has an additional defined memory"
  if module.tables.length != 1 then throw "subject does not have exactly one imported table"
  let table := module.tables.head!
  if table.min != 16384 || table.max != some 16384 || table.elemType != .funcref then
    throw "subject table does not match 16384 fixed funcref entries"
  if module.startFunc.isSome then throw "subject unexpectedly has a start function"

private def initialAbiStore (module : Wasm.Module) : Except String (Store Unit) := do
  let store : Store Unit := module.initialStore
  let globals ← setImportedGlobals module.importedGlobals 0 store.globals.globals
  pure { store with globals := { store.globals with globals := globals } }

private def invoke (module : Wasm.Module) (store : Store Unit) (name : String)
    (args : List Value) : Except String (List Value × Store Unit) := do
  let some entry := module.findExport name
    | throw s!"missing export `{name}`"
  let instance_ : ModuleInstance Unit := { module, host := {} }
  let config ← (initConfig instance_ entry store args.reverse).mapError (·.message)
  let trace := runSteps 50000000 config
  match trace.result with
  | .success values finalStore => .ok (values, finalStore.wasm)
  | .trapped reason _ => .error s!"trap in `{name}`: {reason.message}"
  | .outOfFuel _ => .error s!"fuel exhaustion in `{name}`"
  | .internalError error _ => .error s!"interpreter internal error in `{name}`: {error.message}"

private def invokeStatus (module : Wasm.Module) (store : Store Unit) (name : String)
    (args : List Value) : Except String (Store Unit) := do
  let (results, finalStore) ← invoke module store name args
  match results with
  | [.i32 0] => .ok finalStore
  | [.i32 status] => throw s!"`{name}` returned status {status}"
  | _ => throw s!"`{name}` did not return one i32 status"

private def readResult (module : Wasm.Module) (store : Store Unit) (prefixBool : Option Bool) : Except String UInt64 := do
  let (lengthResults, store) ← invoke module store "stack_length" []
  let expectedLength := if prefixBool.isSome then 2 else 1
  match lengthResults with
  | [.i32 count] =>
    if count.toNat != expectedLength then throw s!"expected {expectedLength} result cells, found {count}"
  | _ => throw "stack_length did not return one i32"
  let store ← match prefixBool with
    | none => .ok store
    | some expected => do
      let (kindResults, store) ← invoke module store "stack_kind" [.i32 0]
      match kindResults with
      | [.i32 2] => pure ()
      | _ => throw "linking fixture's preserved stack prefix is not Bool"
      let (valueResults, store) ← invoke module store "stack_value" [.i32 0]
      match valueResults with
      | [.i64 value] =>
        let expectedValue : UInt64 := if expected then 1 else 0
        if value != expectedValue then throw "linking fixture's Bool prefix changed"
      | _ => throw "stack_value did not return one i64 for Bool prefix"
      pure store
  let resultIndex : UInt32 := if prefixBool.isSome then 1 else 0
  let (kindResults, store) ← invoke module store "stack_kind" [.i32 resultIndex]
  match kindResults with
  | [.i32 1] => pure ()
  | [.i32 kind] => throw s!"expected I64 result cell tag 1, found {kind}"
  | _ => throw "stack_kind did not return one i32"
  let (valueResults, _) ← invoke module store "stack_value" [.i32 resultIndex]
  match valueResults with
  | [.i64 value] => .ok value
  | _ => throw "stack_value did not return one i64"

private def runOperation (wat xText yText prefixText : String) : Except String UInt64 := do
  let module ← (decodeForVerification wat).mapError (toString ·)
  checkSubjectImports module
  let store ← initialAbiStore module
  let store ← invokeStatus module store "begin"
    [.i32 196608, .i32 256, .i32 256, .i32 2048, .i32 4096, .i32 100000]
  let prefixBool ← if prefixText.isEmpty then
      .ok none
    else if prefixText == "true" then
      .ok (some true)
    else if prefixText == "false" then
      .ok (some false)
    else
      .error "optional Bool input must be `true` or `false`"
  let store ← match prefixBool with
    | none => .ok store
    | some value => invokeStatus module store "push_bool" [.i32 (if value then 1 else 0)]
  let x ← (parseI64 xText).mapError (toString ·)
  let y ← (parseI64 yText).mapError (toString ·)
  let store ← invokeStatus module store "push_i64" [.i64 x]
  let store ← invokeStatus module store "push_i64" [.i64 y]
  let store ← invokeStatus module store "submit" []
  readResult module store prefixBool

def entryPoint (args : List String) : IO Unit := do
  match args with
  | [watPath, xText, yText] =>
    let wat ← IO.FS.readFile watPath
    match runOperation wat xText yText "" with
    | .ok result => IO.println s!"PILOT_RESULT {result.toNat}"
    | .error message => throw <| IO.userError message
  | [watPath, prefixText, xText, yText] =>
    let wat ← IO.FS.readFile watPath
    match runOperation wat xText yText prefixText with
    | .ok result => IO.println s!"PILOT_RESULT {result.toNat}"
    | .error message => throw <| IO.userError message
  | _ => throw <| IO.userError "usage: pilot-runner <WAT file> <signed I64 x> <signed I64 y> [Bool prefix]"

end Interpreter.WasmPilot

def main (args : List String) : IO Unit :=
  Interpreter.WasmPilot.entryPoint args
