private structure WireDecl where
  name : Name
  declaration : Declaration
  dependencies : NameSet
  deriving Inhabited

private def decodeDeclaration (value : Json) : DecodeM WireDecl := do
  record value ["kind", "name", "levels", "type", "value"]
  let kind ← string (← field value "kind")
  unless kind == "theorem" || kind == "definition" || kind == "opaque" do
    reject "MC1_UNSUPPORTED_FORMAT" "only safe theorem, definition, and opaque declarations are supported"
  let name ← decodeName (← field value "name")
  if name == .anonymous then
    reject "MC1_DECLARATION_NAME" "anonymous declarations are forbidden"
  let encodedParams ← array (← field value "levels")
  let mut params : NameSet := {}
  let mut parameterNames := #[]
  for encoded in encodedParams do
    let param ← decodeName encoded
    if param == .anonymous || params.contains param then
      reject "MC1_UNIVERSE_PARAMETERS" "anonymous or duplicate universe parameter"
    params := params.insert param
    parameterNames := parameterNames.push param
  modify fun state => { state with dependencies := {} }
  let type ← decodeExpr (← field value "type") params 0 0
  let body ← decodeExpr (← field value "value") params 0 0
  let levelParams := parameterNames.toList
  let declaration := match kind with
    | "theorem" => Declaration.thmDecl { name, levelParams, type, value := body }
    | "definition" => Declaration.defnDecl {
        name, levelParams, type, value := body, hints := .opaque, safety := .safe }
    | _ => Declaration.opaqueDecl { name, levelParams, type, value := body, isUnsafe := false }
  return { name, declaration, dependencies := (← get).dependencies }

private def decodeEnvelope (value : Json) (expectedRoot : Name) (rawFields : Nat) :
    DecodeM (Array WireDecl) := do
  record value ["format", "root", "declarations"]
  unless (← string (← field value "format")) == "noble-mc1-proof/v1" do
    reject "MC1_UNSUPPORTED_FORMAT" "expected noble-mc1-proof/v1"
  unless (← decodeName (← field value "root")) == expectedRoot do
    reject "MC1_SUBJECT_MISMATCH" "wire root does not match the selected theorem"
  let declarations ← array (← field value "declarations")
  if declarations.isEmpty || declarations.size > declarationLimit then
    reject "MC1_DECLARATION_LIMIT" "expected 1 to 4096 declarations"
  unless rawFields == 3 + 5 * declarations.size do
    reject "MC1_WIRE_FIELDS" "duplicate or unexpected JSON object fields"
  declarations.mapM decodeDeclaration

/- Kahn ordering checks completeness and cycles explicitly, without trusting a
   producer order, a producer status, or replay's handling of pending constants. -/
private def checkDeclarations (trusted fresh : Kernel.Environment)
    (declarations : Array WireDecl) (root : Name) : IO Kernel.Environment := do
  let mut indices : Std.HashMap Name Nat := {}
  for i in [:declarations.size] do
    let item := declarations[i]!
    if (trusted.find? item.name).isSome || indices.contains item.name then
      fail "MC1_DUPLICATE_DECLARATION" s!"duplicate or trusted name {item.name}"
    indices := indices.insert item.name i
  let some rootIndex := indices[root]?
    | fail "MC1_THEOREM_MISSING" "selected theorem is absent from the wire declarations"
  let .thmDecl selected := declarations[rootIndex]!.declaration
    | fail "MC1_THEOREM_MISSING" "selected declaration is not a theorem"
  unless selected.levelParams.isEmpty do
    fail "MC1_TYPE_MISMATCH" "the selected theorem must have no universe parameters"

  let mut reachable : NameSet := ({} : NameSet).insert root
  let mut pending := #[rootIndex]
  let mut cursor := 0
  while cursor < pending.size do
    let item := declarations[pending[cursor]!]!
    cursor := cursor + 1
    for dep in item.dependencies do
      if let some index := indices[dep]? then
        unless reachable.contains dep do
          reachable := reachable.insert dep
          pending := pending.push index
  unless pending.size == declarations.size do
    fail "MC1_UNREACHABLE_DECLARATION" "wire contains declarations outside the root dependency closure"

  let mut indegrees := Array.replicate declarations.size 0
  let mut dependents : Array (Array Nat) := Array.replicate declarations.size #[]
  let mut ready := #[]
  for i in [:declarations.size] do
    let mut count := 0
    for dep in declarations[i]!.dependencies do
      if let some index := indices[dep]? then
        count := count + 1
        dependents := dependents.set! index (dependents[index]!.push i)
      else
        let some info := fresh.find? dep
          | fail "MC1_DEPENDENCY_MISSING" s!"missing dependency {dep}"
        if info.isUnsafe || info.isPartial then
          fail "MC1_UNSAFE_DEPENDENCY" s!"{dep}"
    indegrees := indegrees.set! i count
    if count == 0 then
      ready := ready.push i
  let mut checked := fresh
  cursor := 0
  while cursor < ready.size do
    let index := ready[cursor]!
    let item := declarations[index]!
    cursor := cursor + 1
    match checked.addDeclCore 0 item.declaration (cancelTk? := none) with
    | .ok next => checked := next
    | .error _ => fail "MC1_KERNEL_REJECTED" s!"kernel rejected declaration {item.name}"
    for dependent in dependents[index]! do
      let remaining := indegrees[dependent]! - 1
      indegrees := indegrees.set! dependent remaining
      if remaining == 0 then
        ready := ready.push dependent
  unless cursor == declarations.size do
    fail "MC1_DEPENDENCY_CYCLE" "cyclic declaration dependencies"
  return checked

/- Inspect raw types and bodies, including opaque/theorem bodies, projection
   names, and recursor rules. Do not reduce terms before this inventory: even an
   erased argument or let value may carry a forbidden axiom. -/
private def rawDependencies (info : ConstantInfo) : IO NameSet := do
  let mut names : NameSet := {}
  let mut pending : Array Expr := #[info.type]
  if let some body := info.value? (allowOpaque := true) then
    pending := pending.push body
  match info with
  | .inductInfo value =>
    for name in value.all do names := names.insert name
    for name in value.ctors do names := names.insert name
  | .ctorInfo value => names := names.insert value.induct
  | .recInfo value =>
    for name in value.all do names := names.insert name
    for rule in value.rules do
      names := names.insert rule.ctor
      pending := pending.push rule.rhs
  | _ => pure ()
  let mut visited : ExprSet := {}
  while !pending.isEmpty do
    let expr := pending.back!
    pending := pending.pop
    unless visited.contains expr do
      visited := visited.insert expr
      match expr with
      | .const name _ => names := names.insert name
      | .proj name _ body =>
        names := names.insert name
        pending := pending.push body
      | .app fn arg => pending := (pending.push fn).push arg
      | .lam _ type body _ | .forallE _ type body _ =>
        pending := (pending.push type).push body
      | .letE _ type value body _ =>
        pending := ((pending.push type).push value).push body
      | .mdata _ body => pending := pending.push body
      | .fvar _ | .mvar _ =>
        fail "MC1_FREE_VARIABLE" "raw dependency contains a free or metavariable"
      | _ => pure ()
  return names

private def auditAxioms (env : Kernel.Environment) (root : Name) : IO (Array Name) := do
  let allowed : List Name := [`propext, `Classical.choice, `Quot.sound]
  let mut visited : NameSet := ({} : NameSet).insert root
  let mut pending : Array Name := #[root]
  let mut axioms : Array Name := #[]
  let mut index := 0
  while index < pending.size do
    if pending.size > 200000 then
      fail "MC1_DEPENDENCY_LIMIT" "transitive declaration inventory exceeded its bound"
    let name := pending[index]!
    index := index + 1
    let some info := env.find? name
      | fail "MC1_DEPENDENCY_MISSING" s!"{name}"
    if info.isUnsafe || info.isPartial then
      fail "MC1_UNSAFE_DEPENDENCY" s!"{name}"
    if let .axiomInfo _ := info then
      unless allowed.contains name do
        fail "MC1_AXIOM_REJECTED" s!"{name}"
      axioms := axioms.push name
    for dep in (← rawDependencies info) do
      unless visited.contains dep do
        visited := visited.insert dep
        pending := pending.push dep
  return axioms

private def check (refutation : Bool) (path : System.FilePath) : IO Unit := do
  let bytes ← readBounded path
  let rawFields ← preflight bytes
  let some text := String.fromUTF8? bytes
    | fail "MC1_JSON_INVALID" "proof JSON is not UTF-8"
  let value ← match Json.parse text with
    | .ok value => pure value
    | .error _ => fail "MC1_JSON_INVALID" "malformed JSON"
  let theoremName := if refutation then `MC1Proof.refutation else `MC1Proof.proof
  let declarations ← match (decodeEnvelope value theoremName rawFields).run {} with
    | .ok (declarations, _) => pure declarations
    | .error message => throw <| IO.userError message

  initSearchPath (← findSysroot)
  -- The search path is consumer-owned. No producer module is ever imported.
  let imported ← importModules #[{ module := `MC1Obligation }] {}
    (trustLevel := 0) (loadExts := false)
  let trusted := imported.toKernelEnv
  let fresh ← (← mkEmptyEnvironment).replay trusted.constants.map₁
  let checked ← checkDeclarations trusted fresh.toKernelEnv declarations theoremName
  let some (.thmInfo proof) := checked.find? theoremName
    | fail "MC1_THEOREM_MISSING" "kernel did not retain the selected theorem"
  let claim := mkConst `MC1Obligation.claim
  let expected := if refutation then mkApp (mkConst `Not) claim else claim
  match Kernel.isDefEq (.ofKernelEnv checked) {} proof.type expected with
  | .ok true => pure ()
  | .ok false => fail "MC1_TYPE_MISMATCH" "theorem is not the consumer's exact expected type"
  | .error _ => fail "MC1_TYPE_MISMATCH" "kernel type comparison failed"
  let axioms ← auditAxioms checked theoremName
  IO.println "NOBLE-MC1-ACCEPT"
  for ax in axioms do
    IO.println s!"axiom:{ax}"

end MC1Consumer

def main (args : List String) : IO UInt32 := do
  try
    match args with
    | ["proof", path] => MC1Consumer.check false path
    | ["refutation", path] => MC1Consumer.check true path
    | _ => throw <| IO.userError "MC1_CHECKER_USAGE: expected proof|refutation JSON_PATH"
    return 0
  catch error =>
    IO.eprintln s!"MC1_RECHECK_REJECTED: {error}"
    return 1
