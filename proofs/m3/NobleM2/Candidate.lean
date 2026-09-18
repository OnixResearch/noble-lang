/-
Reference model: candidate, request, and outcome data (M2 fragment v0).

A candidate is untrusted input: only the `accepted` outcome carries a checked
result. The declarative judgment is stated over these forms in
`NobleM2.Judgment`.
-/

import NobleM2.Types
import NobleM2.Words

namespace NobleM2

/-- A literal's payload; text content is irrelevant to typing. -/
inductive Lit where
  | i64 : Int → Lit
  | bool : Bool → Lit
  | text : Lit
  | unit : Lit
  deriving Repr, DecidableEq, Inhabited

/-- The three fragment node forms; every node carries its instantiation witness. -/
inductive Node where
  /-- A literal with its construction-stack instantiation. -/
  | literal : Lit → Inst → Node
  /-- A resolved invocation with a fresh instantiation. -/
  | invocation : Nat → Inst → Node
  /-- A quotation literal over a finite body of node references. -/
  | quotation : List Nat → Inst → Node
  deriving Repr, DecidableEq, Inhabited

/-- The untrusted candidate: a finite node arena plus the entry body. -/
structure Candidate where
  format : Nat
  revision : Nat
  nodes : List Node
  body : List Nat
  deriving Repr, Inhabited

/-- The independently supplied expected interface and allowed effect bound. -/
structure Expected where
  stackIn : TyList
  stackOut : TyList
  allowedEffects : EffSet
  deriving Repr, Inhabited

/-- Declared finite limits. -/
structure Limits where
  bytes : Nat
  nodes : Nat
  depth : Nat
  typeSize : Nat
  stackHeight : Nat
  work : Nat
  diagnostics : Nat
  deriving Repr, Inhabited

/-- One acceptance request. -/
structure Request where
  inputBytes : Nat
  expected : Expected
  limits : Limits
  deriving Repr, Inhabited

/-- A derived node interface. -/
structure Interface where
  stackIn : TyList
  stackOut : TyList
  effects : EffSet
  deriving Repr, Inhabited

/-- One node's retained derivation. -/
structure Derivation where
  node : Nat
  interface : Interface
  deriving Repr, Inhabited

/-- The accepted result: the checked interface plus every node derivation. -/
structure Checked where
  interface : Interface
  derivations : List Derivation
  deriving Repr, Inhabited

/-- Which declared limit was exceeded. -/
inductive LimitKind where
  | bytes : LimitKind
  | nodes : LimitKind
  | depth : LimitKind
  | typeSize : LimitKind
  | stackHeight : LimitKind
  | work : LimitKind
  | diagnostics : LimitKind
  deriving Repr, Inhabited

/-- Out-of-fragment or foreign input. -/
inductive UnsupportedKind where
  | formatRevision : UnsupportedKind
  | nodeForm : UnsupportedKind
  | schemeForm : UnsupportedKind
  | recursiveDependency : Nat → UnsupportedKind
  | recursiveSchema : Nat → UnsupportedKind
  deriving Repr, Inhabited

/-- The violated constraint recorded by a rejection. -/
inductive Constraint where
  | stackJoin : Constraint
  | stackOrder : Constraint
  | effectInclusion : EffId → Constraint
  | eligibility : Ty → Constraint
  | unknownEffect : EffId → Constraint
  | instantiationKind : Constraint
  | instantiationArity : Constraint
  | malformedReference : Nat → Constraint
  | unknownDefinition : Nat → Constraint
  | cyclicWitness : Constraint
  deriving Repr, Inhabited

/-- One rejection's diagnostic. -/
structure Diagnostic where
  node : Option Nat
  definition : Option Nat
  expected : List Ty
  actual : List Ty
  constraint : Constraint
  provenanceAvailable : Bool
  truncated : Bool
  deriving Repr, Inhabited

/-- The five-way outcome domain. Only acceptance carries a checked program. -/
inductive Outcome where
  | accepted : Checked → Outcome
  | invalid : Diagnostic → Outcome
  | unsupported : UnsupportedKind → Outcome
  | exhausted : LimitKind → Outcome
  | internalFailure : Outcome
  deriving Repr, Inhabited

end NobleM2
