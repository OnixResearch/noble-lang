/-
  Issue #62 statement extractor for the independent-replay binding lane.

  Prints the canonical statement of every root named in `NOBLE_REPLAY_ROOTS` (space separated),
  read from Noble's compiled foundation environment through Lean's own `Expr`, never from a
  lean4export file. `binding.mjs` renders the same canonical form from an export's records and
  requires the two to agree with the committed `statements.json`.

  Canonical form: binder names and `mdata` are dropped (alpha-equivalence), binder kinds are kept,
  names are dotted and unescaped, string literals are UTF-8 hex, and the statement is
  `(stmt [level params] type)`. Run with `lake env lean` from `proofs/foundation`.
-/
import NobleFoundation
open Lean

namespace NobleReplayStatements

def nameText (n : Name) : String := n.toString (escape := false)

partial def level : Level → String
  | .zero => "0"
  | .succ l => s!"(s {level l})"
  | .max a b => s!"(max {level a} {level b})"
  | .imax a b => s!"(imax {level a} {level b})"
  | .param n => s!"(u {nameText n})"
  | .mvar _ => "(mvar)"

def binder : BinderInfo → String
  | .default => "d"
  | .implicit => "i"
  | .strictImplicit => "s"
  | .instImplicit => "c"

def hexDigit (n : Nat) : Char := "0123456789abcdef".toList.getD n '0'

def hex (s : String) : String :=
  String.join (s.toUTF8.toList.map fun b => String.ofList [hexDigit (b.toNat / 16), hexDigit (b.toNat % 16)])

partial def expr : Expr → String
  | .bvar i => s!"#{i}"
  | .sort l => s!"(sort {level l})"
  | .const n us => "(c " ++ nameText n ++ String.join (us.map fun u => " " ++ level u) ++ ")"
  | .app f a => s!"(@ {expr f} {expr a})"
  | .lam _ t b bi => s!"(fun {binder bi} {expr t} {expr b})"
  | .forallE _ t b bi => s!"(pi {binder bi} {expr t} {expr b})"
  | .letE _ t v b _ => s!"(let {expr t} {expr v} {expr b})"
  | .lit (.natVal n) => s!"(nat {n})"
  | .lit (.strVal s) => s!"(str {hex s})"
  | .mdata _ e => expr e
  | .proj s i e => s!"(proj {nameText s} {i} {expr e})"
  | .fvar _ => "(fvar)"
  | .mvar _ => "(mvar)"

def statement (info : ConstantInfo) : String :=
  "(stmt [" ++ " ".intercalate (info.levelParams.map nameText) ++ "] " ++ expr info.type ++ ")"

end NobleReplayStatements

open NobleReplayStatements in
#eval show CoreM Unit from do
  let roots := ((← IO.getEnv "NOBLE_REPLAY_ROOTS").getD "").splitOn " " |>.filter (· ≠ "")
  if roots.isEmpty then throwError "NOBLE_REPLAY_ROOTS names no root"
  for root in roots do
    let info ← getConstInfo root.toName
    IO.println s!"STATEMENT\t{root}\t{statement info}"
