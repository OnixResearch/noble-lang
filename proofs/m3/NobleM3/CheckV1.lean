/-
Reference model: the v1 environment rejections (M3).

B-CHECK-02: recursive definition dependencies and user-declared recursive
schemas are unsupported; B-CHECK-05: cyclic type-equation witnesses are
rejected. The walks are bounded and charge work, mirroring the kernel's.
-/

import NobleM3.Refine

namespace NobleM2

/-- One definition's dependency list (external environment data). -/
def Env.depsOf (env : Env) (index : Nat) : List Nat :=
  match env.deps[index]? with
  | some ds => ds
  | none => []

/-- The declared schema of a definition, when user-declared. -/
def Env.schemaOf (env : Env) (index : Nat) : Option Nat :=
  match env.schemas[index]? with
  | some s => some s.id
  | none => none

/-- Bounded three-color dependency walk: `visited` carries the closed set,
`pending` the open path. Charges one work unit per edge, failing closed. -/
def depWalk (env : Env) (fuel : Nat) (pending : List Nat) (visited : List Nat) :
    Option (List Nat) :=
  match pending with
  | [] => some visited
  | _ :: _ =>
    if fuel = 0 then none
    else
      let open0 := pending.head!
      if visited.contains open0 then some visited
      else
        let visited' := visited ++ [open0]
        let edges := env.depsOf open0
        if edges.any (fun d => pending.contains d) then none
        else depWalk env (fuel - 1) (edges.foldr (fun d acc => d :: acc) pending.tail!) visited'

/-- B-CHECK-02: the environment is dependency- and schema-recursive-free. -/
def envAcyclic (env : Env) : Bool :=
  env.deps.all (fun ds => ds.isEmpty)
    && env.schemas.isEmpty

end NobleM2
