import Lean

/- This module runs only in the untrusted producer sandbox, after MC1Proof has
   been compiled unchanged and imported by a separate export script. Its JSON
   is evidence data, never an acceptance decision. -/

open Lean Elab Command

namespace NobleProducer

private def maxInputBytes : Nat := 8388608
private def maxDeclarations : Nat := 4096
private def maxNodes : Nat := 262144
private def maxDepth : Nat := 512
private def maxNameComponents : Nat := 256
private def maxUInt : Nat := 4294967295
private def natLiteralLimit : Nat := 10 ^ 4096

private structure ExportState where
  nodes : Nat := 0
  bytes : Nat := 0
  dependencies : NameSet := {}

private abbrev ExportM := StateT ExportState (Except String)

private def reject (code message : String) : ExportM α :=
  throw s!"{code}: {message}"

private def reserveBytes (amount : Nat) : ExportM Unit := do
  let total := (← get).bytes + amount
  if total > maxInputBytes then
    reject "MC1_INPUT_LIMIT" "proof JSON exceeds 8388608 bytes"
  modify fun s => { s with bytes := total }

private def visitNode (depth : Nat) : ExportM Unit := do
  if depth > maxDepth then
    reject "MC1_DEPTH_LIMIT" "proof expression or universe exceeds depth 512"
  let count := (← get).nodes + 1
  if count > maxNodes then
    reject "MC1_NODE_LIMIT" "proof exceeds 262144 expression and universe nodes"
  modify fun s => { s with nodes := count }

/- Count exactly the UTF-8 representation used by Lean.Json.compress, including
   escaping, before constructing the final string. The raw-size check bounds
   the work needed to scan any individual producer-provided string. -/
private def reserveString (value : String) : ExportM Unit := do
  if value.utf8ByteSize > maxInputBytes then
    reject "MC1_INPUT_LIMIT" "proof string exceeds the JSON byte bound"
  let size := value.foldl (fun size c => size +
    if c == '"' || c == '\\' || c == '\n' || c == '\x0d' then 2
    else if c.toNat < 32 then 6
    else c.utf8Size) 2
  reserveBytes size

private def jsonString (value : String) : ExportM Json := do
  reserveString value
  return .str value

private def jsonUInt (value : Nat) : ExportM Json := do
  if value > maxUInt then
    reject "MC1_UINT_LIMIT" "name component or expression index exceeds uint32"
  let mut remaining := value / 10
  let mut digits := 1
  while remaining != 0 do
    digits := digits + 1
    remaining := remaining / 10
  reserveBytes digits
  return .num (JsonNumber.fromNat value)

private def jsonBool (value : Bool) : ExportM Json := do
  reserveBytes (if value then 4 else 5)
  return .bool value

private def jsonArray (values : Array Json) : ExportM Json := do
  reserveBytes (2 + (values.size - 1))
  return .arr values

private def jsonObject (fields : List (String × Json)) : ExportM Json := do
  reserveBytes (2 + (fields.length - 1))
  for (key, _) in fields do
    reserveString key
    reserveBytes 1
  return Json.mkObj fields

private def encodeName (value : Name) : ExportM Json := do
  let mut current := value
  let mut components : Array Json := #[]
  while true do
    match current with
    | .anonymous => break
    | .str parent part =>
      if components.size >= maxNameComponents then
        reject "MC1_NAME_LIMIT" "name exceeds 256 components"
      components := components.push (← jsonString part)
      current := parent
    | .num parent part =>
      if components.size >= maxNameComponents then
        reject "MC1_NAME_LIMIT" "name exceeds 256 components"
      components := components.push (← jsonUInt part)
      current := parent
  jsonArray components.reverse

private def encodeBinderInfo (value : BinderInfo) : ExportM Json :=
  jsonString <| match value with
    | .default => "default"
    | .implicit => "implicit"
    | .strictImplicit => "strictImplicit"
    | .instImplicit => "instImplicit"

private partial def encodeLevel (depth : Nat) (value : Level) : ExportM Json := do
  visitNode depth
  match value with
  | .zero => jsonArray #[← jsonString "zero"]
  | .succ level =>
    jsonArray #[← jsonString "succ", ← encodeLevel (depth + 1) level]
  | .max left right =>
    jsonArray #[← jsonString "max", ← encodeLevel (depth + 1) left,
      ← encodeLevel (depth + 1) right]
  | .imax left right =>
    jsonArray #[← jsonString "imax", ← encodeLevel (depth + 1) left,
      ← encodeLevel (depth + 1) right]
  | .param name => jsonArray #[← jsonString "param", ← encodeName name]
  | .mvar _ => reject "MC1_UNSUPPORTED_EXPR" "universe metavariable in proof closure"

private def noteDependency (name : Name) : ExportM Unit :=
  modify fun s => { s with dependencies := s.dependencies.insert name }

private partial def encodeExpr (depth : Nat) (value : Expr) : ExportM Json := do
  -- Metadata is not logical data. Strip it without changing the transported
  -- expression's node count or depth, and never traverse its arbitrary payload.
  let mut value := value
  while let .mdata _ body := value do
    value := body
  visitNode depth
  match value with
  | .bvar index => jsonArray #[← jsonString "bvar", ← jsonUInt index]
  | .sort level =>
    jsonArray #[← jsonString "sort", ← encodeLevel (depth + 1) level]
  | .const name levels =>
    noteDependency name
    let mut encodedLevels : Array Json := #[]
    for level in levels do
      encodedLevels := encodedLevels.push (← encodeLevel (depth + 1) level)
    jsonArray #[← jsonString "const", ← encodeName name, ← jsonArray encodedLevels]
  | .app fn arg =>
    jsonArray #[← jsonString "app", ← encodeExpr (depth + 1) fn,
      ← encodeExpr (depth + 1) arg]
  | .lam name type body binderInfo =>
    jsonArray #[← jsonString "lam", ← encodeName name,
      ← encodeExpr (depth + 1) type, ← encodeExpr (depth + 1) body,
      ← encodeBinderInfo binderInfo]
  | .forallE name type body binderInfo =>
    jsonArray #[← jsonString "forall", ← encodeName name,
      ← encodeExpr (depth + 1) type, ← encodeExpr (depth + 1) body,
      ← encodeBinderInfo binderInfo]
  | .letE name type rhs body nondep =>
    jsonArray #[← jsonString "let", ← encodeName name,
      ← encodeExpr (depth + 1) type, ← encodeExpr (depth + 1) rhs,
      ← encodeExpr (depth + 1) body, ← jsonBool nondep]
  | .lit (.natVal number) =>
    if number >= natLiteralLimit then
      reject "MC1_NAT_LITERAL_LIMIT" "natural literal exceeds 4096 decimal digits"
    jsonArray #[← jsonString "nat", ← jsonString (toString number)]
  | .lit (.strVal string) => jsonArray #[← jsonString "str", ← jsonString string]
  | .proj typeName index struct =>
    -- Expr.foldConsts omits this name; it is nevertheless a kernel dependency.
    noteDependency typeName
    jsonArray #[← jsonString "proj", ← encodeName typeName, ← jsonUInt index,
      ← encodeExpr (depth + 1) struct]
  | .fvar _ => reject "MC1_UNSUPPORTED_EXPR" "free variable in proof closure"
  | .mvar _ => reject "MC1_UNSUPPORTED_EXPR" "metavariable in proof closure"
  | .mdata _ _ => reject "MC1_UNSUPPORTED_EXPR" "unstripped proof metadata"

private def encodeDeclaration (name : Name) (info : ConstantInfo) : ExportM Json := do
  unless info.name == name do
    reject "MC1_DECLARATION_NAME_MISMATCH" "constant map key differs from declaration name"
  if info.isUnsafe || info.isPartial then
    reject "MC1_UNSAFE_DEPENDENCY" s!"{name}"
  let (kind, value) ← match info with
    | .thmInfo declaration => pure ("theorem", declaration.value)
    | .defnInfo declaration => pure ("definition", declaration.value)
    | .opaqueInfo declaration => pure ("opaque", declaration.value)
    | .axiomInfo _ => reject "MC1_AXIOM_REJECTED" s!"producer-local axiom {name}"
    | .inductInfo _ => reject "MC1_UNSUPPORTED_DECLARATION" s!"producer-local inductive {name}"
    | .ctorInfo _ => reject "MC1_UNSUPPORTED_DECLARATION" s!"producer-local constructor {name}"
    | .recInfo _ => reject "MC1_UNSUPPORTED_DECLARATION" s!"producer-local recursor {name}"
    | .quotInfo _ => reject "MC1_UNSUPPORTED_DECLARATION" s!"producer-local quotient {name}"
  let mut levels : Array Json := #[]
  for level in info.levelParams do
    levels := levels.push (← encodeName level)
  jsonObject [
    ("kind", ← jsonString kind),
    ("name", ← encodeName name),
    ("levels", ← jsonArray levels),
    ("type", ← encodeExpr 0 info.type),
    ("value", ← encodeExpr 0 value)]

private def encodeProof (supplied trusted : Kernel.Environment) (root : Name) : ExportM Json := do
  if (trusted.find? root).isSome then
    reject "MC1_SUBJECT_MISMATCH" "application theorem occurs in the trusted library"
  let some (.thmInfo rootInfo) := supplied.find? root
    | reject "MC1_THEOREM_MISSING" s!"expected theorem {root}"
  unless rootInfo.levelParams.isEmpty do
    reject "MC1_TYPE_MISMATCH" "the selected theorem must have no universe parameters"
  let mut declarations : Array Json := #[]
  -- Explicit DFS avoids recursion on declaration-chain length. The boolean is
  -- an exit marker; active names detect cycles, finished names avoid duplicates.
  let mut pending : Array (Name × Bool) := #[(root, false)]
  let mut active : NameSet := {}
  let mut finished : NameSet := {}
  while !pending.isEmpty do
    let (name, exiting) := pending.back!
    pending := pending.pop
    if exiting then
      active := active.erase name
      finished := finished.insert name
    else if finished.contains name then
      continue
    else
      if active.contains name then
        reject "MC1_DEPENDENCY_CYCLE" s!"cyclic producer declaration {name}"
      if declarations.size >= maxDeclarations then
        reject "MC1_DECLARATION_LIMIT" "proof closure exceeds 4096 producer declarations"
      let some info := supplied.find? name
        | reject "MC1_DEPENDENCY_MISSING" s!"{name}"
      active := active.insert name
      modify fun s => { s with dependencies := {} }
      declarations := declarations.push (← encodeDeclaration name info)
      pending := pending.push (name, true)
      for dependency in (← get).dependencies do
        -- Only this independent environment determines imported-name exclusion.
        -- Its declarations (and their closure) will come from the consumer's own
        -- trusted build, not from any producer versions bearing the same names.
        unless (trusted.find? dependency).isSome || finished.contains dependency do
          pending := pending.push (dependency, false)
  jsonObject [
    ("format", ← jsonString "noble-mc1-proof/v1"),
    ("root", ← encodeName root),
    ("declarations", ← jsonArray declarations)]

elab "#mc1_export " root:ident : command => do
  let supplied ← getEnv
  -- The export script is not a `module` file: private imported bodies must be
  -- available. Use the actual checked constant map, not elaborator extensions,
  -- source declaration lists, mutual-block metadata, or pretty-printed terms.
  let trusted ← liftIO <| importModules #[{ module := `MC1Obligation }] {}
    (trustLevel := 0) (loadExts := false) (level := .private)
  let encoded := (encodeProof supplied.toKernelEnv trusted.toKernelEnv root.getId).run' {}
  let json ← match encoded with
    | .ok json => pure json
    | .error message => throwErrorAt root "{message}"
  let payload := json.compress
  if payload.utf8ByteSize > maxInputBytes then
    throwErrorAt root "MC1_INPUT_LIMIT: proof JSON exceeds 8388608 bytes"
  liftIO <| IO.FS.writeFile "/out/MC1Proof.json" payload

end NobleProducer
