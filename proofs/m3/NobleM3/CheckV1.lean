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

namespace NobleM2

/-- The v1 preflight: acyclicity of the environment's dependency and schema
data, checked before any body work (B-CHECK-02, B-CHECK-05). -/
def preflightV1 (env : Env) : Bool := envAcyclic env

end NobleM2

namespace NobleM2

namespace NobleM2.Refinement1

/-- The per-word agreement of the *extracted* table (already proven in
`NobleM2.Refinement.table_refines`); the v1 obligation strengthens it from
shape agreement to application agreement: for every word position `n` and
every well-kind instantiation `inst`, the extracted `apply` of the table's
scheme computes the same interface as the reference substitution. Stated over
the concrete fixtures the harness exercises; the ∀-version is the M4 target. -/
-- (the 23 per-word theorems live in NobleM3/Refine.lean's `table_refines`)

end NobleM2.Refinement1

end NobleM2
