import Lean
import Lean.Replay

/- The only producer input to this process is bounded UTF-8 JSON. In particular,
   neither importModules nor the native/binary loader sees a producer artifact.
   Every transported declaration is reconstructed and checked by the kernel. -/

open Lean

namespace MC1Consumer

private def inputLimit : Nat := 8388608
private def declarationLimit : Nat := 4096
private def nodeLimit : Nat := 262144
private def depthLimit : Nat := 512
private def uintLimit : Nat := 4294967295

private def fail (code message : String) : IO α :=
  throw <| IO.userError s!"{code}: {message}"

private def readBounded (path : System.FilePath) : IO ByteArray :=
  IO.FS.withFile path .read fun handle => do
    let mut bytes := ByteArray.empty
    while true do
      let chunk ← handle.read (min 65536 (inputLimit + 1 - bytes.size)).toUSize
      if chunk.isEmpty then
        return bytes
      if bytes.size + chunk.size > inputLimit then
        fail "MC1_INPUT_LIMIT" "proof JSON exceeds 8388608 bytes"
      bytes := bytes ++ chunk
    return bytes

/- Bound parser recursion and numeric work *before* Json.parse. Lean's JSON
   parser computes positive decimal exponents eagerly, and its object map erases
   duplicate keys. All wire numbers are uints; the number of raw object fields
   must equal the exact field count of the decoded envelope and declarations. -/
private def preflight (bytes : ByteArray) : IO Nat := do
  let mut inString := false
  let mut escaped := false
  let mut depth := 0
  let mut fields := 0
  let mut digits := 0
  let mut number := 0
  for byte in bytes do
    let c := byte.toNat
    if inString then
      if escaped then
        escaped := false
      else if c == 92 then
        escaped := true
      else if c == 34 then
        inString := false
    else
      if 48 ≤ c && c ≤ 57 then
        digits := digits + 1
        number := 10 * number + c - 48
        if digits > 10 || number > uintLimit then
          fail "MC1_UINT_LIMIT" "JSON numbers must be uint32 decimal integers"
      else
        if digits > 0 then
          unless c == 32 || c == 9 || c == 10 || c == 13 ||
              c == 44 || c == 93 || c == 125 do
            fail "MC1_JSON_NUMBER" "non-integral or non-decimal JSON number"
        digits := 0
        number := 0
        if c == 45 || c == 43 || c == 46 then
          fail "MC1_JSON_NUMBER" "signed or fractional JSON numbers are not supported"
        if c == 34 then
          inString := true
        else if c == 91 || c == 123 then
          depth := depth + 1
          if depth > depthLimit + 8 then
            fail "MC1_DEPTH_LIMIT" "JSON nesting exceeds its bound"
        else if c == 93 || c == 125 then
          if depth == 0 then
            fail "MC1_JSON_INVALID" "unbalanced JSON delimiters"
          depth := depth - 1
        else if c == 58 then
          fields := fields + 1
  if inString || depth != 0 then
    fail "MC1_JSON_INVALID" "unterminated JSON string or container"
  return fields

private structure DecodeState where
  nodes : Nat := 0
  dependencies : NameSet := {}

private abbrev DecodeM := StateT DecodeState (Except String)

private def reject (code message : String) : DecodeM α :=
  throw s!"{code}: {message}"

private def node (depth : Nat) : DecodeM Unit := do
  if depth > depthLimit then
    reject "MC1_DEPTH_LIMIT" "expression or universe nesting exceeds 512"
  let state ← get
  if state.nodes ≥ nodeLimit then
    reject "MC1_NODE_LIMIT" "expression and universe inventory exceeds 262144"
  set { state with nodes := state.nodes + 1 }

private def array (value : Json) : DecodeM (Array Json) :=
  match value with
  | .arr values => pure values
  | _ => reject "MC1_WIRE_TYPE" "expected a JSON array"

private def string (value : Json) : DecodeM String :=
  match value with
  | .str text => pure text
  | _ => reject "MC1_WIRE_TYPE" "expected a JSON string"

private def boolean (value : Json) : DecodeM Bool :=
  match value with
  | .bool result => pure result
  | _ => reject "MC1_WIRE_TYPE" "expected a JSON boolean"

private def uint (value : Json) : DecodeM Nat := do
  match value with
  | .num number =>
    match number.mantissa with
    | .ofNat n =>
      if number.exponent != 0 || n > uintLimit then
        reject "MC1_UINT_LIMIT" "expected a uint32 integer"
      return n
    | _ => reject "MC1_WIRE_TYPE" "negative integers are not supported"
  | _ => reject "MC1_WIRE_TYPE" "expected a JSON integer"

private def record (value : Json) (keys : List String) : DecodeM Unit := do
  let .obj fields := value
    | reject "MC1_WIRE_TYPE" "expected a JSON object"
  unless fields.size == keys.length do
    reject "MC1_WIRE_FIELDS" "unexpected object field count"
  for key in keys do
    unless (fields.get? key).isSome do
      reject "MC1_WIRE_FIELDS" s!"missing object field {key}"

private def field (value : Json) (key : String) : DecodeM Json := do
  match value.getObjVal? key with
  | .ok result => return result
  | .error _ => reject "MC1_WIRE_FIELDS" s!"missing object field {key}"

private def arity (values : Array Json) (expected : Nat) : DecodeM Unit := do
  unless values.size == expected do
    reject "MC1_WIRE_ARITY" "unexpected tagged-array arity"

private def decodeName (value : Json) : DecodeM Name := do
  let components ← array value
  if components.size > 256 then
    reject "MC1_NAME_LIMIT" "name exceeds 256 components"
  let mut name := Name.anonymous
  for component in components do
    match component with
    | .str text => name := .str name text
    | .num _ => name := .num name (← uint component)
    | _ => reject "MC1_WIRE_TYPE" "name components must be strings or uint32 integers"
  return name

private def binderInfo (value : Json) : DecodeM BinderInfo := do
  match ← string value with
  | "default" => return .default
  | "implicit" => return .implicit
  | "strictImplicit" => return .strictImplicit
  | "instImplicit" => return .instImplicit
  | _ => reject "MC1_UNSUPPORTED_FORMAT" "unsupported binder annotation"

private partial def decodeLevel (value : Json) (params : NameSet) (depth : Nat) : DecodeM Level := do
  node depth
  let values ← array value
  if values.isEmpty then
    reject "MC1_WIRE_ARITY" "empty universe tag"
  match ← string values[0]! with
  | "zero" =>
    arity values 1
    return .zero
  | "succ" =>
    arity values 2
    return .succ (← decodeLevel values[1]! params (depth + 1))
  | "max" =>
    arity values 3
    return .max (← decodeLevel values[1]! params (depth + 1))
      (← decodeLevel values[2]! params (depth + 1))
  | "imax" =>
    arity values 3
    return .imax (← decodeLevel values[1]! params (depth + 1))
      (← decodeLevel values[2]! params (depth + 1))
  | "param" =>
    arity values 2
    let name ← decodeName values[1]!
    unless params.contains name do
      reject "MC1_FREE_UNIVERSE" "undeclared universe parameter"
    return .param name
  | _ => reject "MC1_UNSUPPORTED_FORMAT" "unsupported universe tag (metavariables forbidden)"

private def naturalLiteral (value : Json) : DecodeM Nat := do
  let text ← string value
  if text.isEmpty || text.utf8ByteSize > 4096 then
    reject "MC1_LITERAL_LIMIT" "natural literals require 1 to 4096 decimal digits"
  let mut result := 0
  for byte in text.toByteArray do
    let digit := byte.toNat
    unless 48 ≤ digit && digit ≤ 57 do
      reject "MC1_WIRE_TYPE" "natural literal is not an unsigned decimal string"
    result := 10 * result + digit - 48
  return result

private def dependency (name : Name) : DecodeM Unit :=
  modify fun state => { state with dependencies := state.dependencies.insert name }

private partial def decodeExpr (value : Json) (params : NameSet)
    (depth binders : Nat) : DecodeM Expr := do
  node depth
  let values ← array value
  if values.isEmpty then
    reject "MC1_WIRE_ARITY" "empty expression tag"
  match ← string values[0]! with
  | "bvar" =>
    arity values 2
    let index ← uint values[1]!
    if index ≥ binders then
      reject "MC1_FREE_VARIABLE" "unbound de Bruijn variable"
    return .bvar index
  | "sort" =>
    arity values 2
    return .sort (← decodeLevel values[1]! params (depth + 1))
  | "const" =>
    arity values 3
    let name ← decodeName values[1]!
    let levels ← array values[2]!
    let mut decoded := #[]
    for level in levels do
      decoded := decoded.push (← decodeLevel level params (depth + 1))
    dependency name
    return .const name decoded.toList
  | "app" =>
    arity values 3
    return .app (← decodeExpr values[1]! params (depth + 1) binders)
      (← decodeExpr values[2]! params (depth + 1) binders)
  | "lam" =>
    arity values 5
    return .lam (← decodeName values[1]!)
      (← decodeExpr values[2]! params (depth + 1) binders)
      (← decodeExpr values[3]! params (depth + 1) (binders + 1))
      (← binderInfo values[4]!)
  | "forall" =>
    arity values 5
    return .forallE (← decodeName values[1]!)
      (← decodeExpr values[2]! params (depth + 1) binders)
      (← decodeExpr values[3]! params (depth + 1) (binders + 1))
      (← binderInfo values[4]!)
  | "let" =>
    arity values 6
    return .letE (← decodeName values[1]!)
      (← decodeExpr values[2]! params (depth + 1) binders)
      (← decodeExpr values[3]! params (depth + 1) binders)
      (← decodeExpr values[4]! params (depth + 1) (binders + 1))
      (← boolean values[5]!)
  | "nat" =>
    arity values 2
    return .lit (.natVal (← naturalLiteral values[1]!))
  | "str" =>
    arity values 2
    return .lit (.strVal (← string values[1]!))
  | "proj" =>
    arity values 4
    let name ← decodeName values[1]!
    let index ← uint values[2]!
    let body ← decodeExpr values[3]! params (depth + 1) binders
    dependency name
    return .proj name index body
  | _ => reject "MC1_UNSUPPORTED_FORMAT" "unsupported expression tag (free/metavariables forbidden)"

